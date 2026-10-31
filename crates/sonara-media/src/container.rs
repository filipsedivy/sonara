//! Header parsing that symphonia 0.6.1 does not provide (QG-13: no maintained crate covers it;
//! symphonia's demuxers are used for everything else):
//!
//! - **RF64 and Wave64**: symphonia has no demuxer for either (its `WavReader` accepts only the
//!   `RIFF` marker), and `hound` reads plain RIFF only. The header gives the PCM layout for
//!   [`crate::pcm`].
//! - **The `fmt ` tag of RIFF WAVE**: libsndfile names a file `WAVEX` when its format tag is
//!   `WAVE_FORMAT_EXTENSIBLE`, which symphonia does not expose.
//! - **Text chunks** that libsndfile reports as strings: RIFF `LIST`/`INFO` (symphonia 0.6.1's
//!   `WavReader` parses the list and then drops it), and AIFF `NAME`, `AUTH`, `ANNO`, `(c) ` and
//!   `APPL`.
//! - **The loop of a RIFF `smpl` chunk** (RIFF and RF64), which symphonia does not read: the SFZ
//!   loader plays it as a sample's default loop.
//!
//! CAF files and Ogg Opus have readers of their own ([`crate::caf`], [`crate::opus`]).
//!
//! Every reader works on a `Read + Seek` source and never trusts a size beyond the file length.

use std::io::{self, Read, Seek, SeekFrom};

/// The container family, from the first 16 bytes of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    /// `RIFF` … `WAVE`.
    Riff,
    /// `RF64` … `WAVE`.
    Rf64,
    /// Sony Wave64: the `riff` GUID.
    W64,
    /// `FORM` … `AIFF` or `AIFC`.
    Aiff,
    /// `caff`, with the `desc` chunk first.
    Caf,
    /// An Ogg stream whose first packet is `OpusHead`.
    OggOpus,
    /// Anything else: symphonia's probe decides.
    Other,
}

/// The Wave64 GUID of the `riff` header.
const W64_RIFF: [u8; 16] = [
    0x72, 0x69, 0x66, 0x66, 0x2E, 0x91, 0xCF, 0x11, 0xA5, 0xD6, 0x28, 0xDB, 0x04, 0xC1, 0x00, 0x00,
];
/// The Wave64 GUID of the `wave` form.
const W64_WAVE: [u8; 16] = [
    0x77, 0x61, 0x76, 0x65, 0xF3, 0xAC, 0xD3, 0x11, 0x8C, 0xD1, 0x00, 0xC0, 0x4F, 0x8E, 0xDB, 0x8A,
];
/// The Wave64 GUID suffix shared by `fmt `, `data` and the other standard chunks.
const W64_SUFFIX: [u8; 12] = [
    0xF3, 0xAC, 0xD3, 0x11, 0x8C, 0xD1, 0x00, 0xC0, 0x4F, 0x8E, 0xDB, 0x8A,
];
/// `WAVE_FORMAT_EXTENSIBLE`.
pub(crate) const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
/// `WAVE_FORMAT_PCM`.
pub(crate) const WAVE_FORMAT_PCM: u16 = 0x0001;
/// `WAVE_FORMAT_IEEE_FLOAT`.
pub(crate) const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;

/// Classifies a file by its first bytes (any number; an Ogg Opus file needs its first page's
/// segment table and the start of its first packet).
pub(crate) fn sniff(head: &[u8]) -> Family {
    let at = |range: std::ops::Range<usize>| head.get(range);
    if head.get(..16) == Some(&W64_RIFF[..]) {
        return Family::W64;
    }
    if head.starts_with(b"OggS") {
        let packet = 27 + usize::from(head.get(26).copied().unwrap_or(0));
        if head.get(packet..packet + 8) == Some(b"OpusHead") {
            return Family::OggOpus;
        }
    }
    match (at(0..4), at(8..12)) {
        (Some(b"RIFF"), Some(b"WAVE")) => Family::Riff,
        (Some(b"RF64"), Some(b"WAVE")) => Family::Rf64,
        (Some(b"FORM"), Some(b"AIFF" | b"AIFC")) => Family::Aiff,
        // libsndfile recognises CAF only with `desc` as the first chunk (`guess_file_type`).
        (Some(b"caff"), Some(b"desc")) => Family::Caf,
        _ => Family::Other,
    }
}

/// The `fmt ` chunk of a RIFF, RF64 or Wave64 file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WaveFormat {
    /// The format tag; for `WAVE_FORMAT_EXTENSIBLE`, the tag of the sub-format GUID.
    pub(crate) tag: u16,
    /// Whether the chunk is `WAVE_FORMAT_EXTENSIBLE`.
    pub(crate) extensible: bool,
    /// Channel count.
    pub(crate) channels: u16,
    /// Frames per second.
    pub(crate) sample_rate: u32,
    /// Bytes per frame.
    pub(crate) block_align: u16,
    /// Bits per sample (the container size).
    pub(crate) bits_per_sample: u16,
}

/// What a RIFF-family header says about the audio and its strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WaveHeader {
    /// The `fmt ` chunk.
    pub(crate) format: WaveFormat,
    /// Byte offset of the first sample.
    pub(crate) data_offset: u64,
    /// Bytes of sample data, limited to what the file holds (as libsndfile limits it).
    pub(crate) data_len: u64,
    /// The `LIST`/`INFO` strings, mapped to libsndfile's fields, in file order.
    pub(crate) strings: Vec<(&'static str, String)>,
    /// The first loop of the `smpl` chunk, for a RIFF or RF64 file that has one.
    pub(crate) sample_loop: Option<SampleLoop>,
}

/// A loop a sampler chunk (RIFF `smpl`) embeds in the file: its first and last frame, both
/// played, as the chunk writes them. The chunk's loop type (forward, alternating, backward) and
/// play count are not kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SampleLoop {
    /// The first frame of the loop.
    pub(crate) start: u32,
    /// The last frame of the loop, which is played too (the SFZ `loop_end` counts the same way).
    pub(crate) end: u32,
}

impl WaveHeader {
    /// The frame count libsndfile reports: whole frames of sample data.
    pub(crate) fn frames(&self) -> u64 {
        match self.format.block_align {
            0 => 0,
            align => self.data_len / u64::from(align),
        }
    }
}

fn read_array<const N: usize, R: Read>(reader: &mut R) -> io::Result<[u8; N]> {
    let mut bytes = [0u8; N];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

/// Reads `len` bytes at the current position, refusing sizes beyond the end of the file.
fn read_body<R: Read + Seek>(reader: &mut R, len: u64, file_len: u64) -> io::Result<Vec<u8>> {
    let pos = reader.stream_position()?;
    if len > file_len.saturating_sub(pos) {
        return Err(invalid("chunk extends beyond the end of the file"));
    }
    let len = usize::try_from(len).map_err(|_| invalid("chunk too large"))?;
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    Ok(body)
}

fn u16_le(bytes: &[u8], at: usize) -> u16 {
    bytes
        .get(at..at + 2)
        .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
}

fn u32_le(bytes: &[u8], at: usize) -> u32 {
    bytes
        .get(at..at + 4)
        .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn u64_le(bytes: &[u8], at: usize) -> u64 {
    bytes.get(at..at + 8).map_or(0, |b| {
        u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
    })
}

fn parse_fmt(body: &[u8]) -> io::Result<WaveFormat> {
    if body.len() < 16 {
        return Err(invalid("fmt chunk shorter than 16 bytes"));
    }
    let tag = u16_le(body, 0);
    let extensible = tag == WAVE_FORMAT_EXTENSIBLE;
    let resolved = if extensible {
        if body.len() < 40 {
            return Err(invalid("extensible fmt chunk shorter than 40 bytes"));
        }
        // The sub-format GUID starts at byte 24; its first two bytes are the format tag.
        u16_le(body, 24)
    } else {
        tag
    };
    Ok(WaveFormat {
        tag: resolved,
        extensible,
        channels: u16_le(body, 2),
        sample_rate: u32_le(body, 4),
        block_align: u16_le(body, 12),
        bits_per_sample: u16_le(body, 14),
    })
}

/// libsndfile's string fields for RIFF `INFO` sub-chunks (`wav.c`).
fn info_field(id: &[u8]) -> Option<&'static str> {
    Some(match id {
        b"INAM" => "title",
        b"ICOP" => "copyright",
        b"ISFT" => "software",
        b"IART" => "artist",
        b"ICMT" => "comment",
        b"ICRD" => "date",
        b"IPRD" => "album",
        b"ITRK" => "tracknumber",
        b"IGNR" => "genre",
        _ => return None,
    })
}

/// A C string from a chunk: the bytes before the first NUL, decoded as UTF-8 (soundfile decodes
/// libsndfile's strings this way, replacing invalid sequences).
fn c_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// The bytes of a `smpl` chunk before its first loop.
const SMPL_HEADER_BYTES: usize = 36;
/// The bytes of each loop of a `smpl` chunk.
const SMPL_LOOP_BYTES: usize = 24;

/// The first loop of a `smpl` chunk: the loop count is at byte 28 of the chunk, and the first
/// loop follows its 36-byte header, with its start frame at byte 8 and its end frame at byte 12
/// of the loop. `None` when the chunk lists no loop or is too short to hold one.
fn parse_smpl(body: &[u8]) -> Option<SampleLoop> {
    if u32_le(body, 28) == 0 || body.len() < SMPL_HEADER_BYTES + SMPL_LOOP_BYTES {
        return None;
    }
    Some(SampleLoop {
        start: u32_le(body, SMPL_HEADER_BYTES + 8),
        end: u32_le(body, SMPL_HEADER_BYTES + 12),
    })
}

fn parse_info(list: &[u8], strings: &mut Vec<(&'static str, String)>) {
    if list.get(..4) != Some(b"INFO") {
        return;
    }
    let mut pos = 4;
    while let (Some(id), Some(size)) = (list.get(pos..pos + 4), list.get(pos + 4..pos + 8)) {
        let size = u32::from_le_bytes([size[0], size[1], size[2], size[3]]) as usize;
        let start = pos + 8;
        let Some(body) = list.get(start..start.saturating_add(size)) else {
            break;
        };
        if let Some(field) = info_field(id) {
            strings.push((field, c_string(body)));
        }
        pos = start + size + (size & 1);
    }
}

/// Parses the header of a RIFF or RF64 WAVE file: `fmt `, the first `data` chunk, every
/// `LIST`/`INFO` chunk (also after the data, as libsndfile reads seekable files) and the first
/// loop of the first `smpl` chunk that lists one. A `smpl` chunk that cannot be read is ignored,
/// as a `LIST` chunk is.
pub(crate) fn read_riff<R: Read + Seek>(
    reader: &mut R,
    file_len: u64,
    rf64: bool,
) -> io::Result<WaveHeader> {
    reader.seek(SeekFrom::Start(12))?;
    let mut format = None;
    let mut data = None;
    let mut ds64_data_len = None;
    let mut strings = Vec::new();
    let mut sample_loop = None;
    let mut pos = 12u64;
    while pos.saturating_add(8) <= file_len {
        reader.seek(SeekFrom::Start(pos))?;
        let id: [u8; 4] = read_array(reader)?;
        let size = u64::from(u32::from_le_bytes(read_array(reader)?));
        let body_start = pos + 8;
        match &id {
            b"ds64" if rf64 => {
                let body = read_body(reader, size, file_len)?;
                ds64_data_len = Some(u64_le(&body, 8));
            }
            b"fmt " if format.is_none() => {
                format = Some(parse_fmt(&read_body(reader, size, file_len)?)?);
            }
            b"LIST" => {
                if let Ok(body) = read_body(reader, size, file_len) {
                    parse_info(&body, &mut strings);
                }
            }
            b"smpl" if sample_loop.is_none() => {
                if let Ok(body) = read_body(reader, size, file_len) {
                    sample_loop = parse_smpl(&body);
                }
            }
            b"data" if data.is_none() => {
                let declared = match ds64_data_len {
                    Some(len) if rf64 && size == u64::from(u32::MAX) => len,
                    _ => size,
                };
                let available = file_len.saturating_sub(body_start);
                let len = declared.min(available);
                data = Some((body_start, len));
                // Continue after the data, rounded to the even chunk boundary.
                pos = body_start.saturating_add(len).saturating_add(len & 1);
                continue;
            }
            _ => {}
        }
        pos = body_start.saturating_add(size).saturating_add(size & 1);
    }
    let format = format.ok_or_else(|| invalid("no fmt chunk"))?;
    let (data_offset, data_len) = data.ok_or_else(|| invalid("no data chunk"))?;
    Ok(WaveHeader {
        format,
        data_offset,
        data_len,
        strings,
        sample_loop,
    })
}

/// Parses the header of a Wave64 file: the `fmt ` and `data` chunks (libsndfile reads no
/// strings from Wave64, and no loop is read from it).
pub(crate) fn read_w64<R: Read + Seek>(reader: &mut R, file_len: u64) -> io::Result<WaveHeader> {
    reader.seek(SeekFrom::Start(24))?;
    let form: [u8; 16] = read_array(reader)?;
    if form != W64_WAVE {
        return Err(invalid("Wave64 form is not wave"));
    }
    let mut format = None;
    let mut data = None;
    let mut pos = 40u64;
    while pos.saturating_add(24) <= file_len && data.is_none() {
        reader.seek(SeekFrom::Start(pos))?;
        let guid: [u8; 16] = read_array(reader)?;
        // The size includes the 24-byte chunk header; chunks are aligned to 8 bytes.
        let size = u64::from_le_bytes(read_array(reader)?);
        if size < 24 {
            return Err(invalid("Wave64 chunk smaller than its header"));
        }
        let body_len = size - 24;
        if guid[4..] == W64_SUFFIX {
            match &guid[..4] {
                b"fmt " if format.is_none() => {
                    format = Some(parse_fmt(&read_body(reader, body_len, file_len)?)?);
                }
                b"data" => {
                    let available = file_len.saturating_sub(pos + 24);
                    data = Some((pos + 24, body_len.min(available)));
                }
                _ => {}
            }
        }
        pos = pos.saturating_add(size.saturating_add(7) & !7);
    }
    let format = format.ok_or_else(|| invalid("no fmt chunk"))?;
    let (data_offset, data_len) = data.ok_or_else(|| invalid("no data chunk"))?;
    Ok(WaveHeader {
        format,
        data_offset,
        data_len,
        strings: Vec::new(),
        sample_loop: None,
    })
}

/// libsndfile's `psf_isprint`: printable ASCII.
fn is_print(byte: u8) -> bool {
    (32..=126).contains(&byte)
}

/// The strings libsndfile reports for an AIFF file (`aiff.c`): `NAME` (title), `AUTH`
/// (artist), `ANNO` (comment), `(c) ` (copyright, with every unprintable byte replaced by `.`)
/// and `APPL` with the `m3ga` signature (software, cut at the first unprintable byte).
pub(crate) fn read_aiff_strings<R: Read + Seek>(
    reader: &mut R,
    file_len: u64,
) -> io::Result<Vec<(&'static str, String)>> {
    let mut strings = Vec::new();
    let mut pos = 12u64;
    while pos.saturating_add(8) <= file_len {
        reader.seek(SeekFrom::Start(pos))?;
        let id: [u8; 4] = read_array(reader)?;
        let size = u64::from(u32::from_be_bytes(read_array(reader)?));
        let is_text = matches!(&id, b"NAME" | b"AUTH" | b"ANNO" | b"(c) " | b"APPL");
        if is_text {
            let body = read_body(reader, size, file_len)?;
            match &id {
                b"NAME" => strings.push(("title", c_string(&body))),
                b"AUTH" => strings.push(("artist", c_string(&body))),
                b"ANNO" => strings.push(("comment", c_string(&body))),
                b"(c) " => {
                    let clean: Vec<u8> = body
                        .iter()
                        .map(|&b| if is_print(b) { b } else { b'.' })
                        .collect();
                    strings.push(("copyright", c_string(&clean)));
                }
                _ => {
                    if body.get(..4) == Some(b"m3ga") {
                        let text = &body[4..];
                        let end = text
                            .iter()
                            .position(|&b| !is_print(b))
                            .unwrap_or(text.len());
                        strings.push(("software", c_string(&text[..end])));
                    }
                }
            }
        }
        pos = pos
            .saturating_add(8)
            .saturating_add(size)
            .saturating_add(size & 1);
    }
    Ok(strings)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{
        Family, SampleLoop, WAVE_FORMAT_EXTENSIBLE, WAVE_FORMAT_PCM, read_aiff_strings, read_riff,
        read_w64, sniff,
    };

    fn riff(chunks: &[(&[u8; 4], Vec<u8>)], marker: [u8; 4]) -> Vec<u8> {
        let mut body = b"WAVE".to_vec();
        for (id, data) in chunks {
            body.extend_from_slice(*id);
            body.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
            body.extend_from_slice(data);
            if data.len() % 2 == 1 {
                body.push(0);
            }
        }
        let mut file = marker.to_vec();
        file.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
        file.extend_from_slice(&body);
        file
    }

    fn fmt(tag: u16, channels: u16, bits: u16) -> Vec<u8> {
        let align = channels * bits / 8;
        let mut f = Vec::new();
        f.extend_from_slice(&tag.to_le_bytes());
        f.extend_from_slice(&channels.to_le_bytes());
        f.extend_from_slice(&48_000u32.to_le_bytes());
        f.extend_from_slice(&(48_000 * u32::from(align)).to_le_bytes());
        f.extend_from_slice(&align.to_le_bytes());
        f.extend_from_slice(&bits.to_le_bytes());
        f
    }

    #[test]
    fn families_are_recognised_by_their_markers() {
        assert_eq!(sniff(b"RIFF\0\0\0\0WAVEfmt "), Family::Riff);
        assert_eq!(sniff(b"RF64\xff\xff\xff\xffWAVEds64"), Family::Rf64);
        assert_eq!(sniff(b"FORM\0\0\0\0AIFC"), Family::Aiff);
        assert_eq!(sniff(b"caff\0\x01\0\0desc"), Family::Caf);
        assert_eq!(sniff(b"caff\0\x01\0\0data"), Family::Other);
        let mut opus = b"OggS\0\x02".to_vec();
        opus.resize(26, 0);
        opus.extend_from_slice(b"\x01\x13OpusHead");
        assert_eq!(sniff(&opus), Family::OggOpus);
        assert_eq!(sniff(&opus[..30]), Family::Other);
        assert_eq!(sniff(b"fLaC"), Family::Other);
        assert_eq!(sniff(b""), Family::Other);
    }

    #[test]
    fn riff_header_gives_layout_and_info_strings() {
        let mut info = b"INFO".to_vec();
        for (id, text) in [
            (b"INAM", &b"Song\0"[..]),
            (b"IXYZ", b"skip\0\0"),
            (b"ITRK", b"7\0"),
        ] {
            info.extend_from_slice(id);
            info.extend_from_slice(&u32::try_from(text.len()).unwrap().to_le_bytes());
            info.extend_from_slice(text);
            if text.len() % 2 == 1 {
                info.push(0);
            }
        }
        let file = riff(
            &[
                (b"fmt ", fmt(WAVE_FORMAT_PCM, 2, 16)),
                (b"LIST", info),
                (b"data", vec![0; 40]),
            ],
            *b"RIFF",
        );
        let len = u64::try_from(file.len()).unwrap();
        let header = read_riff(&mut Cursor::new(file), len, false).unwrap();
        assert_eq!(header.format.tag, WAVE_FORMAT_PCM);
        assert!(!header.format.extensible);
        assert_eq!(header.frames(), 10);
        assert_eq!(
            header.strings,
            vec![
                ("title", "Song".to_owned()),
                ("tracknumber", "7".to_owned())
            ]
        );
    }

    #[test]
    fn a_data_chunk_beyond_the_file_is_cut_to_the_file() {
        let mut file = riff(&[(b"fmt ", fmt(WAVE_FORMAT_PCM, 1, 16))], *b"RIFF");
        file.extend_from_slice(b"data");
        file.extend_from_slice(&1000u32.to_le_bytes());
        file.extend_from_slice(&[0; 7]);
        let len = u64::try_from(file.len()).unwrap();
        let header = read_riff(&mut Cursor::new(file), len, false).unwrap();
        assert_eq!(header.data_len, 7);
        assert_eq!(header.frames(), 3);
    }

    #[test]
    fn extensible_formats_resolve_their_sub_format() {
        let mut ext = fmt(WAVE_FORMAT_EXTENSIBLE, 2, 24);
        ext.extend_from_slice(&22u16.to_le_bytes());
        ext.extend_from_slice(&24u16.to_le_bytes());
        ext.extend_from_slice(&3u32.to_le_bytes());
        ext.extend_from_slice(&3u16.to_le_bytes()); // IEEE float sub-format
        ext.extend_from_slice(&[0; 14]);
        let file = riff(&[(b"fmt ", ext), (b"data", vec![0; 12])], *b"RIFF");
        let len = u64::try_from(file.len()).unwrap();
        let header = read_riff(&mut Cursor::new(file), len, false).unwrap();
        assert!(header.format.extensible);
        assert_eq!(header.format.tag, 3);
        assert_eq!(header.frames(), 2);
    }

    /// A `smpl` chunk with the loops `(start, end)`, after a header whose other fields are set.
    fn smpl(loops: &[(u32, u32)]) -> Vec<u8> {
        let mut body = Vec::new();
        for field in [0x47_u32, 0, 22_675, 60, 0, 0, 0] {
            body.extend_from_slice(&field.to_le_bytes());
        }
        body.extend_from_slice(&u32::try_from(loops.len()).unwrap().to_le_bytes());
        body.extend_from_slice(&0_u32.to_le_bytes());
        for (id, &(start, end)) in (1_u32..).zip(loops) {
            for field in [id, 0, start, end, 0, 0] {
                body.extend_from_slice(&field.to_le_bytes());
            }
        }
        body
    }

    #[test]
    fn the_first_loop_of_a_smpl_chunk_is_read() {
        let header = |chunks: &[(&[u8; 4], Vec<u8>)]| {
            let file = riff(chunks, *b"RIFF");
            let len = u64::try_from(file.len()).unwrap();
            read_riff(&mut Cursor::new(file), len, false).unwrap()
        };
        let fmt = || (b"fmt ", fmt(WAVE_FORMAT_PCM, 1, 16));
        let data = || (b"data", vec![0; 2_000]);
        // After the data, as most editors write it; the second loop is not read.
        let looped = header(&[fmt(), data(), (b"smpl", smpl(&[(100, 899), (5, 6)]))]);
        assert_eq!(
            looped.sample_loop,
            Some(SampleLoop {
                start: 100,
                end: 899
            })
        );
        assert_eq!(looped.frames(), 1_000, "the audio is unchanged");
        let before = header(&[fmt(), (b"smpl", smpl(&[(7, 8)])), data()]);
        assert_eq!(before.sample_loop, Some(SampleLoop { start: 7, end: 8 }));
        // A chunk without loops, one cut short, and a file without the chunk have none.
        assert_eq!(
            header(&[fmt(), data(), (b"smpl", smpl(&[]))]).sample_loop,
            None
        );
        let mut short = smpl(&[(100, 899)]);
        short.truncate(50);
        assert_eq!(header(&[fmt(), data(), (b"smpl", short)]).sample_loop, None);
        assert_eq!(header(&[fmt(), data()]).sample_loop, None);
        // A later chunk gives the loop when the first lists none.
        let later = header(&[
            fmt(),
            (b"smpl", smpl(&[])),
            data(),
            (b"smpl", smpl(&[(1, 2)])),
        ]);
        assert_eq!(later.sample_loop, Some(SampleLoop { start: 1, end: 2 }));
    }

    #[test]
    fn missing_chunks_are_errors() {
        let file = riff(&[(b"data", vec![0; 4])], *b"RIFF");
        let len = u64::try_from(file.len()).unwrap();
        assert!(read_riff(&mut Cursor::new(file), len, false).is_err());
        let truncated = b"riff".to_vec();
        assert!(read_w64(&mut Cursor::new(truncated), 4).is_err());
    }

    #[test]
    fn aiff_copyright_is_sanitized_and_software_cut() {
        let mut file = b"FORM\0\0\0\0AIFF".to_vec();
        for (id, text) in [
            (b"(c) ", "© Owner".as_bytes()),
            (b"APPL", "m3gaTool välue".as_bytes()),
            (b"NAME", "Tíťle".as_bytes()),
        ] {
            file.extend_from_slice(id);
            file.extend_from_slice(&u32::try_from(text.len()).unwrap().to_be_bytes());
            file.extend_from_slice(text);
            if text.len() % 2 == 1 {
                file.push(0);
            }
        }
        let len = u64::try_from(file.len()).unwrap();
        let strings = read_aiff_strings(&mut Cursor::new(file), len).unwrap();
        assert_eq!(
            strings,
            vec![
                ("copyright", ".. Owner".to_owned()),
                ("software", "Tool v".to_owned()),
                ("title", "Tíťle".to_owned()),
            ]
        );
    }
}
