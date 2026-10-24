//! Apple Core Audio Format files, read the way libsndfile 1.2.2 reads them (`caf.c`, `alac.c`,
//! `pcm.c`), with symphonia's decoders for the samples.
//!
//! symphonia 0.6.1's CAF demuxer rejects 8-bit linear PCM, misreads µ-law and A-law, fails on
//! libsndfile's ALAC files (it reads the packet table's padding as the next chunk) and on files
//! whose audio has an odd byte count (24-bit mono with an odd frame count, which libsndfile pads
//! with a byte after the `data` chunk). No other maintained crate reads CAF (QG-13), so the
//! container is parsed here and only the packets go to symphonia's PCM, G.711 and ALAC
//! decoders, which convert to `f64` exactly as libsndfile does.
//!
//! libsndfile's rules, which the golden inputs and `tests/fixtures/formats` record:
//!
//! - **Chunks.** `desc` must follow the file header (else the file is not recognised as CAF at
//!   all, [`crate::container::sniff`]). The chunk walk stops at a chunk with a zero
//!   marker, a negative size, or a size beyond the file length, and when fewer than 8 bytes are
//!   left; a file without a `data` chunk is malformed. `info` and `pakt` chunks that do not fit
//!   in the file are malformed.
//! - **Data length.** A `data` chunk that claims more than 10 bytes beyond the file is cut to
//!   the file length minus 8; otherwise its size counts, and a chunk that ends at or past the end
//!   of the file reaches to the end of the file (`pcm_init`).
//! - **Rates** are the `desc` rate rounded to the nearest integer, ties to even (`lrint`).
//! - **ALAC frames** are 4096 per packet except the last, whose frames are those it decodes to;
//!   a last packet that the file does not hold in full counts as empty (`alac_reader_calc_frames`).
//! - **Strings** come from the `info` chunk; a key that occurs twice keeps its last value.
//! - **ALAC packets** are read into one buffer that keeps its bytes between packets
//!   (libsndfile's `byte_buffer`), and the decoder reads past a packet's end into what an
//!   earlier packet left there. libsndfile's own ALAC writer produces such packets: its 20- and
//!   32-bit uncompressed elements are longer than the packets it stores them in, and libsndfile
//!   reads them back that way.
//! - **ALAC channels** come out in the file's element order; symphonia's decoder reorders more
//!   than two channels to its speaker positions, which [`AlacQuirks`] undoes.
//! - **Uncompressed ALAC channel pairs above 16 bits.** libsndfile's ALAC decoder (its port of
//!   Apple's) reads the second channel of an uncompressed ("escape") channel pair element with
//!   20, 24 or 32 bits without shifting its upper 16 bits into place, so that channel decodes to
//!   `(upper >> (32 - bits)) | lower`. libsndfile writes such elements for noise-like audio
//!   (the golden `caf-alac24.caf`). [`AlacQuirks`] reproduces this for every such element up to
//!   the first compressed element of a packet, which is every element of a mono or stereo file;
//!   in a file with more channels, an escape pair after a compressed element keeps symphonia's
//!   correct decoding (no recorded file has one).

use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};

use symphonia::core::audio::Channels;
use symphonia::core::codecs::audio::AudioCodecParameters;
use symphonia::core::codecs::audio::well_known as codec;

use crate::error::Rejection;
use crate::format::Subtype;
use crate::inspect::METADATA_FIELDS;

/// libsndfile's largest channel count (`SF_MAX_CHANNELS`).
const MAX_CHANNELS: u32 = 1024;
/// The frames of one packet handed to the PCM and G.711 decoders.
const PCM_PACKET_FRAMES: u64 = 4096;
/// The longest ALAC packet libsndfile decodes (`ALAC_FRAME_LENGTH`).
const ALAC_FRAME_LENGTH: u32 = 4096;
/// libsndfile's ALAC packet buffer (`ALAC_MAX_CHANNEL_COUNT * ALAC_BYTE_BUFFER_SIZE`).
const ALAC_BYTE_BUFFER: usize = 8 * 0x2_0000;

/// The sample encoding of a CAF file (`decode_desc_chunk`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CafEncoding {
    /// Linear PCM, integer or float, `bytes` per sample.
    Pcm {
        /// libsndfile's subtype.
        subtype: Subtype,
        /// Bytes per sample.
        bytes: u32,
        /// Whether the samples are little-endian (format flag 2).
        little_endian: bool,
    },
    /// G.711 µ-law.
    Ulaw,
    /// G.711 A-law.
    Alaw,
    /// Apple Lossless with `bits` per sample (16, 20, 24 or 32).
    Alac {
        /// Bits per sample.
        bits: u32,
    },
}

/// What libsndfile reads from a CAF file's chunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CafHeader {
    /// Frames per second (`lrint` of the `desc` rate).
    pub(crate) sample_rate: u32,
    /// Channels per frame.
    pub(crate) channels: u32,
    /// The encoding.
    pub(crate) encoding: CafEncoding,
    /// Frames per packet (ALAC: 4096).
    pub(crate) frames_per_packet: u32,
    /// The `kuki` chunk (ALAC's magic cookie).
    pub(crate) cookie: Option<Vec<u8>>,
    /// The ALAC packet sizes from the `pakt` chunk, up to the first zero (`alac_pakt_read_decode`
    /// and `alac_reader_calc_frames`).
    pub(crate) packet_sizes: Vec<u64>,
    /// Byte offset of the first sample (after the `data` chunk's edit count).
    pub(crate) data_offset: u64,
    /// Bytes of sample data (`psf->datalength` after `pcm_init`).
    pub(crate) data_len: u64,
    /// The file length.
    pub(crate) file_len: u64,
    /// The `info` strings mapped to libsndfile's fields, each with its last value.
    pub(crate) strings: Vec<(&'static str, String)>,
}

impl CafHeader {
    /// The frame count of a constant-size encoding: whole frames of sample data.
    pub(crate) fn pcm_frames(&self) -> u64 {
        match self.block_align() {
            0 => 0,
            align => self.data_len / align,
        }
    }

    /// Bytes per frame of a constant-size encoding (0 for ALAC).
    pub(crate) fn block_align(&self) -> u64 {
        let bytes = match self.encoding {
            CafEncoding::Pcm { bytes, .. } => bytes,
            CafEncoding::Ulaw | CafEncoding::Alaw => 1,
            CafEncoding::Alac { .. } => 0,
        };
        u64::from(bytes) * u64::from(self.channels)
    }

    /// symphonia's codec parameters for the encoding.
    pub(crate) fn codec_params(&self) -> AudioCodecParameters {
        let mut params = AudioCodecParameters::new();
        params
            .with_sample_rate(self.sample_rate)
            .with_channels(Channels::Discrete(
                u16::try_from(self.channels).unwrap_or(u16::MAX),
            ));
        match self.encoding {
            CafEncoding::Pcm {
                subtype,
                bytes,
                little_endian,
            } => {
                let id = match (subtype, little_endian) {
                    (Subtype::PcmS8, _) => codec::CODEC_ID_PCM_S8,
                    (Subtype::Pcm16, false) => codec::CODEC_ID_PCM_S16BE,
                    (Subtype::Pcm16, true) => codec::CODEC_ID_PCM_S16LE,
                    (Subtype::Pcm24, false) => codec::CODEC_ID_PCM_S24BE,
                    (Subtype::Pcm24, true) => codec::CODEC_ID_PCM_S24LE,
                    (Subtype::Pcm32, false) => codec::CODEC_ID_PCM_S32BE,
                    (Subtype::Pcm32, true) => codec::CODEC_ID_PCM_S32LE,
                    (Subtype::Float, false) => codec::CODEC_ID_PCM_F32BE,
                    (Subtype::Float, true) => codec::CODEC_ID_PCM_F32LE,
                    (_, false) => codec::CODEC_ID_PCM_F64BE,
                    (_, true) => codec::CODEC_ID_PCM_F64LE,
                };
                params
                    .for_codec(id)
                    .with_bits_per_sample(bytes * 8)
                    .with_max_frames_per_packet(PCM_PACKET_FRAMES);
            }
            CafEncoding::Ulaw => {
                params
                    .for_codec(codec::CODEC_ID_PCM_MULAW)
                    .with_max_frames_per_packet(PCM_PACKET_FRAMES);
            }
            CafEncoding::Alaw => {
                params
                    .for_codec(codec::CODEC_ID_PCM_ALAW)
                    .with_max_frames_per_packet(PCM_PACKET_FRAMES);
            }
            CafEncoding::Alac { bits } => {
                params
                    .for_codec(codec::CODEC_ID_ALAC)
                    .with_bits_per_sample(bits)
                    .with_max_frames_per_packet(u64::from(self.frames_per_packet));
                if let Some(cookie) = &self.cookie {
                    params.with_extra_data(cookie.clone().into_boxed_slice());
                }
            }
        }
        params
    }
}

/// An error of reading the header: libsndfile's refusal, or an I/O error.
#[derive(Debug)]
pub(crate) enum CafError {
    /// libsndfile refuses the file.
    Rejected(Rejection),
    /// The file cannot be read.
    Io(io::Error),
}

impl From<io::Error> for CafError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// `len` bytes at `pos`, as libsndfile's header reader returns them: zeros past the end.
fn read_at<R: Read + Seek>(reader: &mut R, pos: u64, len: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(len);
    reader.seek(SeekFrom::Start(pos))?;
    reader.take(len as u64).read_to_end(&mut bytes)?;
    bytes.resize(len, 0);
    Ok(bytes)
}

fn be_u32(bytes: &[u8], at: usize) -> u32 {
    bytes
        .get(at..at + 4)
        .and_then(|b| b.try_into().ok())
        .map_or(0, u32::from_be_bytes)
}

fn be_i64(bytes: &[u8], at: usize) -> i64 {
    bytes
        .get(at..at + 8)
        .and_then(|b| b.try_into().ok())
        .map_or(0, i64::from_be_bytes)
}

/// libsndfile's `psf_lrint` of a sample rate: to the nearest integer, ties to even.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "lrint: the rate is rounded first, then clamped to the u32 range"
)]
fn rate(rate: f64) -> u32 {
    let rounded = rate.round_ties_even();
    if rounded.is_nan() || rounded <= 0.0 {
        0
    } else {
        rounded.min(f64::from(u32::MAX)) as u32
    }
}

/// Reads a CAF file's header the way `caf_read_header` and the codec initializers do.
pub(crate) fn read_header<R: Read + Seek>(
    reader: &mut R,
    file_len: u64,
) -> Result<CafHeader, CafError> {
    let rejected = |reason| Err(CafError::Rejected(reason));
    let head = read_at(reader, 0, 8)?;
    if head.get(..4) != Some(b"caff") {
        return rejected(Rejection::CafNotCaf);
    }
    let desc_head = read_at(reader, 8, 20)?;
    if desc_head.get(..4) != Some(b"desc") {
        return rejected(Rejection::CafNoDesc);
    }
    let desc_size = be_i64(&desc_head, 4);
    if desc_size < 32 {
        return rejected(Rejection::Malformed);
    }
    let sample_rate = rate(f64::from_be_bytes(
        desc_head
            .get(12..20)
            .and_then(|b| b.try_into().ok())
            .unwrap_or_default(),
    ));
    let desc = read_at(reader, 28, 24)?;
    let format_id: [u8; 4] = desc
        .get(..4)
        .and_then(|b| b.try_into().ok())
        .unwrap_or_default();
    let flags = be_u32(&desc, 4);
    let bytes_per_packet = be_u32(&desc, 8);
    let frames_per_packet = be_u32(&desc, 12);
    let channels = be_u32(&desc, 16);
    let bits_per_channel = be_u32(&desc, 20);
    if channels > MAX_CHANNELS {
        return rejected(Rejection::Malformed);
    }
    let Chunks {
        cookie,
        pakt,
        data,
        strings,
    } = walk_chunks(
        reader,
        20u64.saturating_add(desc_size.unsigned_abs()),
        file_len,
        channels,
    )?;
    let Some((data_offset, length, end)) = data else {
        return rejected(Rejection::Malformed);
    };
    let encoding = encoding(
        format_id,
        flags,
        bytes_per_packet,
        channels,
        bits_per_channel,
    )
    .ok_or(CafError::Rejected(Rejection::UnsupportedEncoding))?;
    // `pcm_init`: the chunk's length when it ends inside the file, else up to the end.
    let data_len = match end {
        Some(_) => length,
        None => file_len.saturating_sub(data_offset),
    };
    let packet_sizes = match (encoding, pakt) {
        (CafEncoding::Alac { .. }, Some((offset, size))) => {
            let body = read_at(reader, offset, usize::try_from(size).unwrap_or(0))?;
            packet_sizes(&body, file_len)
        }
        _ => Vec::new(),
    };
    if let CafEncoding::Alac { .. } = encoding {
        // `alac_reader_init`: libsndfile decodes at most 4096 frames per packet, and needs the
        // packet table and the magic cookie.
        if frames_per_packet > ALAC_FRAME_LENGTH || pakt.is_none() || cookie.is_none() {
            return rejected(Rejection::Internal);
        }
    }
    Ok(CafHeader {
        sample_rate,
        channels,
        encoding,
        frames_per_packet,
        cookie,
        packet_sizes,
        data_offset,
        data_len,
        file_len,
        strings,
    })
}

/// What the chunk walk after `desc` found.
struct Chunks {
    cookie: Option<Vec<u8>>,
    /// The `pakt` chunk's body offset and size.
    pakt: Option<(u64, u64)>,
    /// The `data` chunk: the first sample's offset, libsndfile's length, and the end when the
    /// chunk ends inside the file.
    data: Option<(u64, u64, Option<u64>)>,
    strings: Vec<(&'static str, String)>,
}

/// `caf_read_header`'s chunk loop, from `pos` (after the `desc` chunk).
fn walk_chunks<R: Read + Seek>(
    reader: &mut R,
    mut pos: u64,
    file_len: u64,
    channels: u32,
) -> Result<Chunks, CafError> {
    let rejected = |reason| Err(CafError::Rejected(reason));
    let mut cookie = None;
    let mut pakt = None;
    let mut data: Option<(u64, u64, Option<u64>)> = None;
    let mut strings: Vec<(&'static str, String)> = Vec::new();
    loop {
        let header = read_at(reader, pos, 12)?;
        let marker = header.get(..4).unwrap_or_default();
        let size = be_i64(&header, 4);
        pos = pos.saturating_add(12);
        if marker == [0; 4] || size < 0 {
            break;
        }
        let size = size.unsigned_abs();
        if size > file_len {
            break;
        }
        let remaining = file_len.saturating_sub(pos);
        match marker {
            b"peak" => {
                if size != 4 + u64::from(channels) * 12 {
                    return rejected(Rejection::CafBadPeak);
                }
                pos = pos.saturating_add(size);
            }
            b"data" => {
                // The edit count, then the samples.
                let data_offset = pos.saturating_add(4);
                let available = file_len.saturating_sub(data_offset);
                let length = if size > available.saturating_add(10) {
                    i128::from(available) - 8
                } else {
                    i128::from(size) - 4
                };
                let Ok(length) = u64::try_from(length) else {
                    // libsndfile's `validate_psf`: a negative data length.
                    return rejected(Rejection::Internal);
                };
                let end = data_offset
                    .checked_add(length)
                    .filter(|&end| end < file_len);
                data = Some((data_offset, length, end));
                pos = data_offset.saturating_add(length);
            }
            b"kuki" => {
                let body = usize::try_from(size).unwrap_or(usize::MAX).min(4096);
                cookie = Some(read_at(reader, pos, body)?);
                pos = pos.saturating_add(size);
            }
            b"pakt" => {
                if size < 24 || size > remaining {
                    return rejected(Rejection::Malformed);
                }
                pakt = Some((pos, size));
                pos = pos.saturating_add(size);
            }
            b"info" => {
                if size < 4 || size > remaining {
                    return rejected(Rejection::Malformed);
                }
                let body = read_at(reader, pos, usize::try_from(size).unwrap_or(0))?;
                read_strings(body.get(4..).unwrap_or_default(), &mut strings);
                pos = pos.saturating_add(size);
            }
            _ => pos = pos.saturating_add(size),
        }
        if marker != b"data" && size >= 0xffff_ff00 {
            break;
        }
        if pos >= file_len.saturating_sub(8) {
            break;
        }
    }
    Ok(Chunks {
        cookie,
        pakt,
        data,
        strings,
    })
}

/// `decode_desc_chunk`: the encoding of the `desc` chunk, `None` for one libsndfile does not
/// read.
fn encoding(
    id: [u8; 4],
    flags: u32,
    bytes_per_packet: u32,
    channels: u32,
    bits: u32,
) -> Option<CafEncoding> {
    let little_endian = flags & 2 != 0;
    let pcm = |subtype, bytes: u32| {
        (bytes_per_packet == bytes * channels).then_some(CafEncoding::Pcm {
            subtype,
            bytes,
            little_endian,
        })
    };
    match (&id, flags & 1 != 0, bits) {
        (b"alac", _, _) => match flags {
            1 => Some(CafEncoding::Alac { bits: 16 }),
            2 => Some(CafEncoding::Alac { bits: 20 }),
            3 => Some(CafEncoding::Alac { bits: 24 }),
            4 => Some(CafEncoding::Alac { bits: 32 }),
            _ => None,
        },
        (b"lpcm", true, 32) => pcm(Subtype::Float, 4),
        (b"lpcm", true, 64) => pcm(Subtype::Double, 8),
        (b"lpcm", false, 32) => pcm(Subtype::Pcm32, 4),
        (b"lpcm", false, 24) => pcm(Subtype::Pcm24, 3),
        (b"lpcm", false, 16) => pcm(Subtype::Pcm16, 2),
        (b"lpcm", false, 8) => pcm(Subtype::PcmS8, 1),
        (b"alaw", _, 8) => Some(CafEncoding::Alaw),
        (b"ulaw", _, 8) => Some(CafEncoding::Ulaw),
        _ => None,
    }
}

/// `caf_read_strings`: `key\0value\0` pairs; a key read twice keeps its last value.
fn read_strings(body: &[u8], strings: &mut Vec<(&'static str, String)>) {
    let mut parts = body.split(|&b| b == 0);
    while let (Some(key), Some(value)) = (parts.next(), parts.next()) {
        let key = String::from_utf8_lossy(key);
        let field = match key.as_ref() {
            "comments" => Some("comment"),
            other => METADATA_FIELDS
                .iter()
                .copied()
                .find(|field| *field == other),
        };
        if let Some(field) = field {
            strings.retain(|(name, _)| *name != field);
            strings.push((field, String::from_utf8_lossy(value).into_owned()));
        }
    }
}

/// The ALAC packet sizes libsndfile counts (`alac_pakt_read_decode`, `alac_reader_calc_frames`):
/// variable-length integers after the 24-byte `pakt` header, up to the first zero or an
/// unreadable integer, and up to and including the first size that is not below the file
/// length.
fn packet_sizes(pakt: &[u8], file_len: u64) -> Vec<u64> {
    let mut sizes = Vec::new();
    let mut at = 24;
    while at < pakt.len() {
        let mut value = 0u64;
        let mut count = 0;
        let mut valid = false;
        while let Some(&byte) = pakt.get(at + count) {
            value = (value << 7) | u64::from(byte & 0x7F);
            count += 1;
            if count > 5 {
                break;
            }
            if byte & 0x80 == 0 {
                valid = true;
                break;
            }
        }
        // libsndfile's `value` is a `uint32_t`.
        let value = value & u64::from(u32::MAX);
        if !valid || value == 0 {
            break;
        }
        at += count;
        sizes.push(value);
        if value >= file_len {
            break;
        }
    }
    sizes
}

/// symphonia's channel index for each ALAC element channel, by channel count (its
/// `map_channels`, for the default layouts the cookie implies).
const ALAC_CHANNEL_MAPS: [&[usize]; 8] = [
    &[0],
    &[0, 1],
    &[2, 0, 1],
    &[2, 0, 1, 3],
    &[2, 0, 1, 3, 4],
    &[2, 0, 1, 4, 5, 3],
    &[2, 0, 1, 5, 6, 4, 3],
    &[2, 4, 5, 0, 1, 6, 7, 3],
];

/// What libsndfile's ALAC decoder does differently from symphonia's (see the module
/// documentation): the channel order of more than two channels, and the second channel of an
/// uncompressed channel pair above 16 bits.
#[derive(Debug, Clone)]
pub(crate) struct AlacQuirks {
    bits: u32,
    channels: usize,
    frames_per_packet: u32,
    scratch: Vec<f64>,
}

/// A bit field as a count (saturating on 32-bit targets).
fn to_usize(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

/// An MSB-first bit reader over an ALAC packet.
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn read(&mut self, count: usize) -> Option<u64> {
        let mut value = 0u64;
        for _ in 0..count {
            let byte = *self.data.get(self.pos / 8)?;
            let bit = (byte >> (7 - self.pos % 8)) & 1;
            value = (value << 1) | u64::from(bit);
            self.pos += 1;
        }
        Some(value)
    }

    fn skip(&mut self, count: usize) -> Option<()> {
        let end = self.pos.checked_add(count)?;
        (end <= self.data.len() * 8).then(|| self.pos = end)
    }
}

impl AlacQuirks {
    /// The quirks of an ALAC stream with `bits` per sample, `channels` channels and
    /// `frames_per_packet` frames in a full packet.
    pub(crate) fn new(bits: u32, channels: usize, frames_per_packet: u32) -> Self {
        Self {
            bits,
            channels,
            frames_per_packet,
            scratch: Vec::new(),
        }
    }

    /// Applies the quirks to `frames`, symphonia's interleaved decoding of `packet`.
    pub(crate) fn apply(&mut self, packet: &[u8], frames: &mut [f64]) {
        let channels = self.channels;
        if let Some(map) = ALAC_CHANNEL_MAPS
            .get(channels.wrapping_sub(1))
            .filter(|_| channels > 2)
        {
            for frame in frames.chunks_exact_mut(channels) {
                self.scratch.clear();
                self.scratch.extend_from_slice(frame);
                for (out, &from) in frame.iter_mut().zip(map.iter()) {
                    *out = self.scratch.get(from).copied().unwrap_or(0.0);
                }
            }
        }
        if self.bits <= 16 {
            return;
        }
        for (channel, count) in self.escape_pairs(packet) {
            let second = channel + 1;
            for frame in frames.chunks_exact_mut(channels).take(count) {
                if let Some(sample) = frame.get_mut(second) {
                    *sample = self.libsndfile_second(*sample);
                }
            }
        }
    }

    /// The first channel and the frame count of each uncompressed channel pair element before
    /// the packet's first compressed element (`alac_decode`'s element loop).
    fn escape_pairs(&self, packet: &[u8]) -> Vec<(usize, usize)> {
        let mut bits = Bits {
            data: packet,
            pos: 0,
        };
        let mut channel = 0;
        let mut pairs = Vec::new();
        let width = self.bits as usize;
        while let Some(tag) = bits.read(3) {
            match tag {
                // SCE, CPE, LFE.
                0 | 1 | 3 => {
                    let pair = tag == 1;
                    let Some(header) = bits.read(16).and_then(|_| bits.read(4)) else {
                        break;
                    };
                    let samples = if header & 0b1000 != 0 {
                        bits.read(32).map_or(0, to_usize)
                    } else {
                        self.frames_per_packet as usize
                    };
                    if header & 1 == 0 {
                        // A compressed element: its length is known only after decoding it.
                        break;
                    }
                    let per_frame = if pair { 2 * width } else { width };
                    if pair {
                        pairs.push((channel, samples));
                    }
                    if bits.skip(samples.saturating_mul(per_frame)).is_none() {
                        break;
                    }
                    channel += if pair { 2 } else { 1 };
                }
                // DSE: instance, alignment flag, count (extended at 255), then the bytes.
                4 => {
                    let Some(aligned) = bits.read(4).and_then(|_| bits.read(1)) else {
                        break;
                    };
                    let mut count = to_usize(bits.read(8).unwrap_or(0));
                    if count == 255 {
                        count += to_usize(bits.read(8).unwrap_or(0));
                    }
                    if aligned == 1 {
                        bits.pos = bits.pos.div_ceil(8) * 8;
                    }
                    if bits.skip(count * 8).is_none() {
                        break;
                    }
                }
                // FIL: count (extended at 15), then the bytes.
                6 => {
                    let mut count = to_usize(bits.read(4).unwrap_or(0));
                    if count == 15 {
                        count += to_usize(bits.read(8).unwrap_or(0)).saturating_sub(1);
                    }
                    if bits.skip(count * 8).is_none() {
                        break;
                    }
                }
                // END, and CCE and PCE, which libsndfile does not decode.
                _ => break,
            }
        }
        pairs
    }

    /// libsndfile's value for the second channel of an uncompressed pair, from the correct
    /// value `sample` (a left-justified `bits`-bit integer over 2^31).
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the sample is a left-justified 32-bit integer over 2^31, exactly"
    )]
    fn libsndfile_second(&self, sample: f64) -> f64 {
        let bits = self.bits;
        let shift = 32 - bits;
        let extra = bits - 16;
        let word = (sample * 2_147_483_648.0) as i64 as i32;
        // The `bits`-bit field as stored, its upper 16 bits and the rest.
        let field = (word.cast_unsigned() >> shift) & ((1u64 << bits) - 1) as u32;
        let upper = field >> extra;
        let lower = field & ((1u32 << extra) - 1);
        let value = (upper >> shift) | lower;
        f64::from(value.wrapping_shl(shift).cast_signed()) / 2_147_483_648.0
    }
}

/// The packets of a CAF file's audio, for symphonia's decoders.
pub(crate) struct CafPackets {
    file: BufReader<File>,
    layout: Layout,
    /// The index of the next packet.
    next: u64,
}

enum Layout {
    /// Linear PCM and G.711: `frames` frames of `align` bytes from `offset`, in packets of
    /// [`PCM_PACKET_FRAMES`].
    Constant {
        offset: u64,
        align: u64,
        frames: u64,
    },
    /// ALAC: one packet of up to `frames_per_packet` frames per size, from `offsets`, read into
    /// `buffer` (libsndfile's `byte_buffer`, zero at first), of which the decoder sees `window`
    /// bytes: the most an element of the stream can take.
    Alac {
        offsets: Vec<u64>,
        sizes: Vec<u64>,
        frames_per_packet: u64,
        file_len: u64,
        buffer: Vec<u8>,
        window: usize,
    },
}

impl CafPackets {
    /// The packets of `header`'s audio in `file`.
    pub(crate) fn new(file: File, header: &CafHeader) -> Self {
        let layout = if let CafEncoding::Alac { .. } = header.encoding {
            let mut offsets = Vec::with_capacity(header.packet_sizes.len());
            let mut offset = header.data_offset;
            for size in &header.packet_sizes {
                offsets.push(offset);
                offset = offset.saturating_add(*size);
            }
            // Uncompressed samples of every channel at 32 bits, and the element headers.
            let window = usize::try_from(header.frames_per_packet)
                .unwrap_or(usize::MAX)
                .saturating_mul(usize::try_from(header.channels).unwrap_or(usize::MAX))
                .saturating_mul(4)
                .saturating_add(1024)
                .min(ALAC_BYTE_BUFFER);
            Layout::Alac {
                offsets,
                sizes: header.packet_sizes.clone(),
                frames_per_packet: u64::from(header.frames_per_packet),
                file_len: header.file_len,
                buffer: Vec::new(),
                window,
            }
        } else {
            Layout::Constant {
                offset: header.data_offset,
                align: header.block_align(),
                frames: header.pcm_frames(),
            }
        };
        Self {
            file: BufReader::new(file),
            layout,
            next: 0,
        }
    }

    /// The number of ALAC packets (0 for the other encodings).
    pub(crate) fn alac_packets(&self) -> u64 {
        match &self.layout {
            Layout::Alac { sizes, .. } => sizes.len() as u64,
            Layout::Constant { .. } => 0,
        }
    }

    /// The bytes of the next packet; `None` at the end, or at an ALAC packet the file does not
    /// hold in full or that exceeds libsndfile's buffer (libsndfile stops reading there). An
    /// ALAC packet comes with the buffer's bytes after it (see the module documentation).
    pub(crate) fn next_packet(&mut self) -> io::Result<Option<Vec<u8>>> {
        let too_large = || io::Error::new(io::ErrorKind::InvalidData, "packet too large");
        match &mut self.layout {
            Layout::Constant {
                offset,
                align,
                frames,
            } => {
                let first = self.next.saturating_mul(PCM_PACKET_FRAMES);
                if first >= *frames {
                    return Ok(None);
                }
                let count = (*frames - first).min(PCM_PACKET_FRAMES);
                let len = usize::try_from(count * *align).map_err(|_| too_large())?;
                self.file.seek(SeekFrom::Start(*offset + first * *align))?;
                let mut packet = vec![0; len];
                self.file.read_exact(&mut packet)?;
                self.next += 1;
                Ok(Some(packet))
            }
            Layout::Alac {
                offsets,
                sizes,
                file_len,
                buffer,
                window,
                ..
            } => {
                let index = usize::try_from(self.next).unwrap_or(usize::MAX);
                let (Some(&offset), Some(&size)) = (offsets.get(index), sizes.get(index)) else {
                    return Ok(None);
                };
                let len = usize::try_from(size).map_err(|_| too_large())?;
                if offset.saturating_add(size) > *file_len || len > ALAC_BYTE_BUFFER {
                    return Ok(None);
                }
                let seen = len.max(*window);
                if buffer.len() < seen {
                    buffer.resize(seen, 0);
                }
                self.file.seek(SeekFrom::Start(offset))?;
                self.file
                    .read_exact(buffer.get_mut(..len).ok_or_else(too_large)?)?;
                self.next += 1;
                Ok(Some(buffer.get(..seen).unwrap_or_default().to_vec()))
            }
        }
    }

    /// Moves to the packet that holds `frame` and returns that packet's first frame.
    pub(crate) fn seek(&mut self, frame: u64) -> u64 {
        let per_packet = match &self.layout {
            Layout::Constant { .. } => PCM_PACKET_FRAMES,
            Layout::Alac {
                frames_per_packet, ..
            } => (*frames_per_packet).max(1),
        };
        self.next = frame / per_packet;
        self.next * per_packet
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{AlacQuirks, CafEncoding, CafError, packet_sizes, rate, read_header};
    use crate::error::Rejection;
    use crate::format::Subtype;

    fn chunk(id: [u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = id.to_vec();
        out.extend_from_slice(&i64::try_from(body.len()).unwrap().to_be_bytes());
        out.extend_from_slice(body);
        out
    }

    fn desc(
        format: [u8; 4],
        flags: u32,
        bytes: u32,
        frames: u32,
        channels: u32,
        bits: u32,
    ) -> Vec<u8> {
        let mut body = 48_000f64.to_be_bytes().to_vec();
        body.extend_from_slice(&format);
        for value in [flags, bytes, frames, channels, bits] {
            body.extend_from_slice(&value.to_be_bytes());
        }
        chunk(*b"desc", &body)
    }

    fn file(chunks: &[Vec<u8>]) -> Vec<u8> {
        let mut out = b"caff\0\x01\0\0".to_vec();
        for c in chunks {
            out.extend_from_slice(c);
        }
        out
    }

    fn header(bytes: &[u8]) -> Result<super::CafHeader, CafError> {
        read_header(&mut Cursor::new(bytes), bytes.len() as u64)
    }

    fn rejection(bytes: &[u8]) -> Rejection {
        match header(bytes) {
            Err(CafError::Rejected(reason)) => reason,
            other => panic!("expected a rejection, got {other:?}"),
        }
    }

    #[test]
    fn rates_round_to_even() {
        assert_eq!(rate(44_100.5), 44_100);
        assert_eq!(rate(44_101.5), 44_102);
        assert_eq!(rate(48_000.0), 48_000);
        assert_eq!(rate(-1.0), 0);
        assert_eq!(rate(f64::NAN), 0);
    }

    #[test]
    fn linear_pcm_takes_its_layout_and_strings() {
        let mut info = 3u32.to_be_bytes().to_vec();
        info.extend_from_slice(b"title\0First\0comments\0Note\0title\0Last\0");
        let bytes = file(&[
            desc(*b"lpcm", 2, 4, 1, 2, 16),
            chunk(*b"info", &info),
            chunk(*b"data", &[0; 4 + 40]),
        ]);
        let h = header(&bytes).unwrap();
        assert_eq!(
            h.encoding,
            CafEncoding::Pcm {
                subtype: Subtype::Pcm16,
                bytes: 2,
                little_endian: true
            }
        );
        assert_eq!((h.sample_rate, h.channels, h.pcm_frames()), (48_000, 2, 10));
        assert_eq!(
            h.strings,
            vec![("comment", "Note".to_owned()), ("title", "Last".to_owned())]
        );
    }

    #[test]
    fn a_short_data_chunk_follows_libsndfiles_length_rule() {
        let mut bytes = file(&[
            desc(*b"lpcm", 0, 2, 1, 1, 16),
            chunk(*b"data", &[0; 4 + 100]),
        ]);
        // Missing 6 bytes: the declared length counts, limited to the file.
        bytes.truncate(bytes.len() - 6);
        assert_eq!(header(&bytes).unwrap().pcm_frames(), 47);
        // Missing 7 bytes: the length is the file's minus 8.
        bytes.truncate(bytes.len() - 1);
        assert_eq!(header(&bytes).unwrap().pcm_frames(), 42);
    }

    #[test]
    fn refusals_carry_libsndfiles_reasons() {
        assert_eq!(rejection(b"RIFF\0\0\0\0"), Rejection::CafNotCaf);
        assert_eq!(
            rejection(&file(&[chunk(*b"data", &[0; 8])])),
            Rejection::CafNoDesc
        );
        let no_data = file(&[desc(*b"lpcm", 0, 2, 1, 1, 16), chunk(*b"free", &[0; 4])]);
        assert_eq!(rejection(&no_data), Rejection::Malformed);
        let aac = file(&[desc(*b"aac ", 0, 0, 1024, 2, 0), chunk(*b"data", &[0; 8])]);
        assert_eq!(rejection(&aac), Rejection::UnsupportedEncoding);
        let negative = file(&[desc(*b"lpcm", 0, 2, 1, 1, 16), chunk(*b"data", &[0; 3])]);
        assert_eq!(rejection(&negative), Rejection::Internal);
        let mut minus_one = file(&[desc(*b"lpcm", 0, 2, 1, 1, 16), chunk(*b"data", &[0; 8])]);
        let at = minus_one.len() - 16;
        minus_one[at..at + 8].copy_from_slice(&(-1i64).to_be_bytes());
        assert_eq!(rejection(&minus_one), Rejection::Malformed);
        let peak = file(&[
            desc(*b"lpcm", 0, 2, 1, 1, 16),
            chunk(*b"peak", &[0; 4]),
            chunk(*b"data", &[0; 8]),
        ]);
        assert_eq!(rejection(&peak), Rejection::CafBadPeak);
    }

    #[test]
    fn libsndfiles_second_channel_of_an_uncompressed_pair() {
        let scale = 2_147_483_648.0;
        let quirks = AlacQuirks::new(24, 2, 4096);
        // -2 (0xFFFFFE): upper 16 bits 0xFFFF, lower 0xFE: (0xFFFF >> 8) | 0xFE = 0xFF.
        assert_eq!(
            quirks.libsndfile_second(f64::from(-2 << 8) / scale),
            f64::from(255 << 8) / scale
        );
        // Small positive values keep their value.
        assert_eq!(
            quirks.libsndfile_second(f64::from(22 << 8) / scale),
            f64::from(22 << 8) / scale
        );
        let wide = AlacQuirks::new(32, 2, 4096);
        // 32 bits: the upper half ORed into the lower.
        assert_eq!(
            wide.libsndfile_second(f64::from(0x1234_5678) / scale),
            f64::from(0x1234 | 0x5678) / scale
        );
    }

    #[test]
    fn escape_pairs_are_found_before_the_first_compressed_element() {
        // A CPE (tag 1) with the escape flag and a partial frame of 2 samples at 20 bits, then END.
        let mut bits = String::new();
        bits.push_str("001"); // CPE
        bits.push_str(&"0".repeat(16)); // instance, unused
        bits.push_str("1001"); // partial, no shift, escape
        bits.push_str(&"0".repeat(30)); // 2 samples, in 32 bits
        bits.push_str("10");
        bits.push_str(&"0".repeat(2 * 2 * 20));
        bits.push_str("111"); // END
        while !bits.len().is_multiple_of(8) {
            bits.push('0');
        }
        let packet: Vec<u8> = bits
            .as_bytes()
            .chunks(8)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap())
            .collect();
        assert_eq!(
            AlacQuirks::new(20, 2, 4096).escape_pairs(&packet),
            vec![(0, 2)]
        );
        // A compressed SCE first stops the walk.
        let compressed = [0b0000_0000, 0, 0b0000_0000];
        assert_eq!(
            AlacQuirks::new(20, 3, 4096).escape_pairs(&compressed),
            vec![]
        );
    }

    #[test]
    fn more_than_two_channels_come_out_in_element_order() {
        let mut quirks = AlacQuirks::new(16, 6, 4096);
        // symphonia's order for 5.1: L R C LFE Ls Rs; the file's: C L R Ls Rs LFE.
        let mut frame = [1.0, 2.0, 0.0, 5.0, 3.0, 4.0];
        quirks.apply(&[0xE0], &mut frame);
        assert_eq!(frame, [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn packet_tables_stop_at_zero_or_a_bad_integer() {
        let mut pakt = vec![0; 24];
        pakt.extend_from_slice(&[0x8C, 0x48, 0x10, 0x00, 0x00]);
        assert_eq!(packet_sizes(&pakt, 1 << 20), vec![1608, 16]);
        // A size at or above the file length is the last one counted.
        assert_eq!(packet_sizes(&pakt, 1000), vec![1608]);
        let mut unterminated = vec![0; 24];
        unterminated.extend_from_slice(&[0x81, 0x82]);
        assert_eq!(packet_sizes(&unterminated, 1 << 20), Vec::<u64>::new());
    }
}
