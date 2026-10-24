//! Ogg Opus files, read the way libsndfile 1.2.2 reads them (`ogg_opus.c`, `ogg_vcomment.c`).
//! symphonia 0.6.1 demuxes Ogg Opus but has no Opus decoder.
//!
//! The `ogg` crate demuxes the pages and libopus's multistream decoder decodes the packets
//! ([`OpusMsDecoder`], in the codec FFI module), as in libsndfile:
//!
//! - **Header.** The first page holds only `OpusHead` (version 0 or 1). Mapping family 0 has at
//!   most two channels; the others carry their stream map, which libopus applies. The output gain
//!   of the header is applied by the decoder.
//! - **Rate.** The lowest Opus rate at or above the header's input rate: 48 kHz above 24 kHz,
//!   24 kHz above 16 kHz, 16 kHz above 12 kHz, 12 kHz above 8 kHz, else 8 kHz.
//! - **Frames.** The last granule position minus the first page's start offset and the
//!   pre-skip, at the decoding rate. Decoding drops the pre-skip packet by packet and trims the
//!   last page to its granule position; a mid-stream page whose packets outrun its granule
//!   position is malformed.
//! - **Strings** come from `OpusTags`: names compared after upper-casing, `ENCODER` as the
//!   software, and a field read twice keeps its last value.
//!
//! A packet libopus cannot decode, or a mid-stream page whose packets outrun its granule
//! position, ends the stream there, as libsndfile's read stops (and soundfile returns the frames
//! read so far). A seek decodes again from the start, so every read equals the sequential decode
//! the decode cache makes (libsndfile's own seek pre-rolls from an earlier page instead).

use std::fs::File;
use std::io::{BufReader, SeekFrom};
use std::path::Path;

use ogg::reading::PacketReader;

use crate::codec_ffi::{OPUS_MAX_FRAMES, OpusMsDecoder};
use crate::error::{MediaError, Rejection};

/// libsndfile's Vorbis comment names for its string fields (`vorbiscomment_mapping`).
const COMMENT_FIELDS: [(&str, &str); 10] = [
    ("TITLE", "title"),
    ("COPYRIGHT", "copyright"),
    ("ENCODER", "software"),
    ("ARTIST", "artist"),
    ("COMMENT", "comment"),
    ("DATE", "date"),
    ("ALBUM", "album"),
    ("LICENSE", "license"),
    ("TRACKNUMBER", "tracknumber"),
    ("GENRE", "genre"),
];

/// Bytes of the window the last page is searched for, doubled until a page is found.
const TAIL_WINDOW: u64 = 64 * 1024;

/// What `OpusHead` says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpusHead {
    /// Output channels.
    pub(crate) channels: u8,
    /// Samples at 48 kHz to drop at the start.
    pub(crate) preskip: u16,
    /// The input rate the encoder was given.
    pub(crate) input_rate: u32,
    /// Output gain, dB in Q7.8.
    pub(crate) gain: i16,
    /// Streams, and the stereo streams among them.
    pub(crate) streams: u8,
    /// Coupled (stereo) streams.
    pub(crate) coupled: u8,
    /// The stream of each output channel.
    pub(crate) mapping: Vec<u8>,
}

/// `opus_read_header_packet`.
fn parse_head(packet: &[u8]) -> Result<OpusHead, Rejection> {
    if packet.len() < 19 || packet.len() > 276 || packet.get(..8) != Some(b"OpusHead") {
        return Err(Rejection::Malformed);
    }
    let byte = |at: usize| packet.get(at).copied().unwrap_or(0);
    let version = byte(8);
    if version > 1 {
        return Err(Rejection::Unimplemented);
    }
    let channels = byte(9);
    let preskip = u16::from_le_bytes([byte(10), byte(11)]);
    let input_rate = u32::from_le_bytes([byte(12), byte(13), byte(14), byte(15)]);
    let gain = i16::from_le_bytes([byte(16), byte(17)]);
    let family = byte(18);
    let (streams, coupled, mapping, length) = if family == 0 {
        if channels > 2 {
            return Err(Rejection::Malformed);
        }
        let mapping = [0, 1].into_iter().take(usize::from(channels)).collect();
        (1, channels.saturating_sub(1), mapping, 19)
    } else {
        let count = usize::from(channels);
        if packet.len() < 21 + count || (family == 1 && channels > 8) {
            return Err(Rejection::Malformed);
        }
        let (streams, coupled) = (byte(19), byte(20));
        if streams < 1 || coupled > streams || u16::from(streams) + u16::from(coupled) > 255 {
            return Err(Rejection::Malformed);
        }
        let mapping: Vec<u8> = packet.get(21..21 + count).unwrap_or_default().to_vec();
        let limit = u16::from(streams) + u16::from(coupled);
        if mapping.iter().any(|&m| u16::from(m) > limit && m != 255) {
            return Err(Rejection::Malformed);
        }
        (streams, coupled, mapping, 21 + count)
    };
    // The specification makes extra bytes in the header a hard error.
    if packet.len() != length {
        return Err(Rejection::Malformed);
    }
    Ok(OpusHead {
        channels,
        preskip,
        input_rate,
        gain,
        streams,
        coupled,
        mapping,
    })
}

/// `vorbiscomment_read_tags` with the `OpusTags` identifier: libsndfile's fields, each with
/// its last value.
fn parse_comments(packet: &[u8]) -> Result<Vec<(&'static str, String)>, Rejection> {
    if packet.len() < 16 || packet.get(..8) != Some(b"OpusTags") {
        return Err(Rejection::Malformed);
    }
    let u32_at = |at: usize| {
        packet
            .get(at..at + 4)
            .and_then(|b| b.try_into().ok())
            .map_or(0, u32::from_le_bytes)
    };
    let mut at = 8usize;
    let vendor = u32_at(at) as usize;
    at += 4;
    if vendor > 0 {
        if at.saturating_add(vendor).saturating_add(4) > packet.len() {
            return Err(Rejection::Malformed);
        }
        at += vendor;
    }
    let count = u32_at(at);
    at += 4;
    let mut strings: Vec<(&'static str, String)> = Vec::new();
    for _ in 0..count {
        if at + 4 > packet.len() {
            return Err(Rejection::Malformed);
        }
        let len = u32_at(at) as usize;
        at += 4;
        let Some(comment) = packet.get(at..at.saturating_add(len)) else {
            return Err(Rejection::Malformed);
        };
        at += len;
        // libsndfile reads a C string: the comment ends at its first NUL.
        let comment = comment.split(|&b| b == 0).next().unwrap_or_default();
        let Some(eq) = comment.iter().position(|&b| b == b'=') else {
            continue;
        };
        let name = comment.get(..eq).unwrap_or_default().to_ascii_uppercase();
        let value = comment.get(eq + 1..).unwrap_or_default();
        if let Some((_, field)) = COMMENT_FIELDS
            .iter()
            .find(|(key, _)| key.as_bytes() == name.as_slice())
        {
            strings.retain(|(f, _)| f != field);
            strings.push((field, String::from_utf8_lossy(value).into_owned()));
        }
    }
    Ok(strings)
}

/// libsndfile's decoding rate for an input rate, as a factor of 48 kHz.
fn rate_factor(input_rate: u32) -> u32 {
    match input_rate {
        24_001.. => 1,
        16_001..=24_000 => 2,
        12_001..=16_000 => 3,
        8_001..=12_000 => 4,
        _ => 6,
    }
}

/// Samples at 48 kHz in one Opus packet, 0 for a packet libopus cannot measure.
fn packet_samples(packet: &[u8]) -> u64 {
    opus::packet::get_nb_samples(packet, 48_000).map_or(0, |n| n as u64)
}

/// The packets of one page that complete on it.
struct Page {
    packets: Vec<Vec<u8>>,
    granule: u64,
    eos: bool,
}

/// A decoded Ogg Opus stream.
pub(crate) struct OpusStream {
    reader: PacketReader<BufReader<File>>,
    serial: u32,
    head: OpusHead,
    decoder: OpusMsDecoder,
    sample_rate: u32,
    factor: u64,
    frames: u64,
    strings: Vec<(&'static str, String)>,
    /// The page being decoded, and the next of its packets.
    page: Page,
    next_packet: usize,
    /// The granule position before the next packet, and at the end of the current page.
    pkt_pos: u64,
    pg_pos: u64,
    /// Decoded frames, interleaved, of which `loc..len` are not yet returned.
    buffer: Vec<f32>,
    loc: usize,
    len: usize,
}

fn rejected(path: &Path, reason: Rejection) -> MediaError {
    MediaError::rejected(path, reason)
}

impl OpusStream {
    /// Opens `path`, whose first Ogg page carries `OpusHead`.
    ///
    /// # Errors
    ///
    /// [`MediaError::Io`] when the file cannot be read, and [`MediaError::Rejected`] for a file
    /// libsndfile refuses.
    pub(crate) fn open(path: &Path) -> Result<Self, MediaError> {
        let file = File::open(path).map_err(|e| MediaError::io("open", path, e))?;
        let file_len = file
            .metadata()
            .map_err(|e| MediaError::io("inspect", path, e))?
            .len();
        let mut reader = PacketReader::new(BufReader::new(file));
        let malformed = || rejected(path, Rejection::Malformed);
        let head_packet = reader.read_packet().ok().flatten().ok_or_else(malformed)?;
        // The first page holds exactly the header packet.
        if !head_packet.first_in_stream() || !head_packet.last_in_page() {
            return Err(malformed());
        }
        let serial = head_packet.stream_serial();
        let head = parse_head(&head_packet.data).map_err(|reason| rejected(path, reason))?;
        let tags = next_of(&mut reader, serial).ok_or_else(malformed)?;
        let strings = parse_comments(&tags.data).map_err(|reason| rejected(path, reason))?;
        let factor = rate_factor(head.input_rate);
        let sample_rate = 48_000 / factor;
        let decoder = OpusMsDecoder::new(
            sample_rate,
            head.streams,
            head.coupled,
            &head.mapping,
            head.gain,
        )
        .map_err(|_| rejected(path, Rejection::Internal))?;
        // `ogg_opus_analyze_file`: the first audio page gives the start offset.
        let page = next_page(&mut reader, serial);
        let duration: u64 = page.packets.iter().map(|p| packet_samples(p)).sum();
        if duration == 0 {
            return Err(malformed());
        }
        let start = if page.eos {
            if duration < page.granule {
                return Err(malformed());
            }
            0
        } else {
            page.granule.checked_sub(duration).ok_or_else(malformed)?
        };
        let end = last_granule(path, serial, file_len)?;
        let factor = u64::from(factor);
        let frames = end.map_or(u64::MAX, |end| {
            end.saturating_sub(start)
                .saturating_sub(u64::from(head.preskip))
                / factor
        });
        let channels = usize::from(head.channels);
        Ok(Self {
            reader,
            serial,
            pg_pos: page.granule,
            pkt_pos: start,
            page,
            next_packet: 0,
            head,
            decoder,
            sample_rate,
            factor,
            frames,
            strings,
            buffer: vec![0.0; OPUS_MAX_FRAMES * channels.max(1)],
            loc: 0,
            len: 0,
        })
    }

    /// Output channels.
    pub(crate) fn channels(&self) -> usize {
        usize::from(self.head.channels)
    }

    /// The decoding rate.
    pub(crate) fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The frame count libsndfile reports.
    pub(crate) fn frames(&self) -> u64 {
        self.frames
    }

    /// The `OpusTags` strings.
    pub(crate) fn strings(&self) -> &[(&'static str, String)] {
        &self.strings
    }

    /// Reads up to `out.len() / channels` frames as interleaved `f64` (libopus's `f32`
    /// widened); fewer only at the end of the stream.
    pub(crate) fn read(&mut self, out: &mut [f64]) -> usize {
        let channels = self.channels().max(1);
        let want = out.len() / channels;
        let mut filled = 0;
        while filled < want {
            if self.loc >= self.len && !self.refill() {
                break;
            }
            let n = (self.len - self.loc).min(want - filled);
            let src = self
                .buffer
                .get(self.loc * channels..(self.loc + n) * channels)
                .unwrap_or_default();
            let dst = out
                .get_mut(filled * channels..(filled + n) * channels)
                .unwrap_or_default();
            for (d, s) in dst.iter_mut().zip(src) {
                *d = f64::from(*s);
            }
            self.loc += n;
            filled += n;
        }
        filled
    }

    /// `ogg_opus_read_refill`: decodes the next packet; `false` at the end of the stream, which
    /// a packet libopus rejects and a malformed page also mark.
    fn refill(&mut self) -> bool {
        if self.next_packet >= self.page.packets.len() {
            let page = next_page(&mut self.reader, self.serial);
            if page.packets.is_empty() {
                return false;
            }
            self.pkt_pos = self.pg_pos;
            self.pg_pos = page.granule;
            self.page = page;
            self.next_packet = 0;
        }
        let Some(packet) = self.page.packets.get(self.next_packet) else {
            return false;
        };
        self.next_packet += 1;
        let Ok(decoded) = self.decoder.decode_float(packet, &mut self.buffer) else {
            return false;
        };
        let decoded = decoded as u64;
        let packet_end = self.pkt_pos.saturating_add(decoded * self.factor);
        let len = if packet_end <= self.pg_pos {
            decoded
        } else if self.page.eos {
            self.pg_pos.saturating_sub(self.pkt_pos) / self.factor
        } else {
            return false;
        };
        let preskip = u64::from(self.head.preskip);
        let loc = if self.pkt_pos < preskip {
            ((preskip - self.pkt_pos) / self.factor).min(len)
        } else {
            0
        };
        self.len = usize::try_from(len.min(decoded)).unwrap_or(0);
        self.loc = usize::try_from(loc).unwrap_or(0).min(self.len);
        self.pkt_pos = packet_end;
        true
    }
}

/// The next packet of stream `serial`, `None` at the end or at a damaged page.
fn next_of(reader: &mut PacketReader<BufReader<File>>, serial: u32) -> Option<ogg::Packet> {
    loop {
        let packet = reader.read_packet().ok().flatten()?;
        if packet.stream_serial() == serial {
            return Some(packet);
        }
    }
}

/// The packets of stream `serial` that complete on its next page (no packets at the end).
fn next_page(reader: &mut PacketReader<BufReader<File>>, serial: u32) -> Page {
    let mut page = Page {
        packets: Vec::new(),
        granule: 0,
        eos: false,
    };
    while let Some(packet) = next_of(reader, serial) {
        let last = packet.last_in_page();
        page.granule = packet.absgp_page();
        page.eos = packet.last_in_stream();
        page.packets.push(packet.data);
        if last {
            break;
        }
    }
    page
}

/// The granule position of the last page of stream `serial` that completes a packet
/// (`ogg_sync_last_page_before`), searched from the end in growing windows.
fn last_granule(path: &Path, serial: u32, file_len: u64) -> Result<Option<u64>, MediaError> {
    let mut window = TAIL_WINDOW;
    loop {
        let start = file_len.saturating_sub(window);
        let file = File::open(path).map_err(|e| MediaError::io("open", path, e))?;
        let mut reader = PacketReader::new(BufReader::new(file));
        reader
            .seek_bytes(SeekFrom::Start(start))
            .map_err(|e| MediaError::io("seek in", path, e))?;
        let mut last = None;
        while let Ok(Some(packet)) = reader.read_packet() {
            if packet.stream_serial() == serial && packet.last_in_page() {
                last = Some(packet.absgp_page());
            }
        }
        if last.is_some() || start == 0 {
            return Ok(last);
        }
        window = window.saturating_mul(2);
    }
}

#[cfg(test)]
mod tests {
    use super::{OpusHead, parse_comments, parse_head, rate_factor};
    use crate::error::Rejection;

    fn head(channels: u8, family: u8, tail: &[u8]) -> Vec<u8> {
        let mut packet = b"OpusHead\x01".to_vec();
        packet.push(channels);
        packet.extend_from_slice(&312u16.to_le_bytes());
        packet.extend_from_slice(&44_100u32.to_le_bytes());
        packet.extend_from_slice(&(-256i16).to_le_bytes());
        packet.push(family);
        packet.extend_from_slice(tail);
        packet
    }

    #[test]
    fn headers_follow_libsndfiles_checks() {
        assert_eq!(
            parse_head(&head(2, 0, &[])).unwrap(),
            OpusHead {
                channels: 2,
                preskip: 312,
                input_rate: 44_100,
                gain: -256,
                streams: 1,
                coupled: 1,
                mapping: vec![0, 1],
            }
        );
        assert_eq!(parse_head(&head(3, 0, &[])), Err(Rejection::Malformed));
        assert_eq!(parse_head(&head(2, 0, &[0])), Err(Rejection::Malformed));
        let six = parse_head(&head(6, 1, &[4, 2, 0, 4, 1, 2, 3, 5])).unwrap();
        assert_eq!((six.streams, six.coupled, six.mapping.len()), (4, 2, 6));
        assert_eq!(parse_head(&head(9, 1, &[9; 11])), Err(Rejection::Malformed));
        assert_eq!(
            parse_head(&head(2, 1, &[0, 0, 0, 1])),
            Err(Rejection::Malformed)
        );
        let mut future = head(2, 0, &[]);
        future[8] = 2;
        assert_eq!(parse_head(&future), Err(Rejection::Unimplemented));
        assert_eq!(parse_head(b"OpusHead"), Err(Rejection::Malformed));
    }

    #[test]
    fn rates_are_the_lowest_opus_rate_at_or_above_the_input() {
        let rates: Vec<u32> = [
            0, 8000, 8001, 12_000, 16_000, 22_050, 24_000, 44_100, 96_000,
        ]
        .into_iter()
        .map(|r| 48_000 / rate_factor(r))
        .collect();
        assert_eq!(
            rates,
            [
                8000, 8000, 12_000, 12_000, 16_000, 24_000, 24_000, 48_000, 48_000
            ]
        );
    }

    fn tags(vendor: &[u8], comments: &[&[u8]]) -> Vec<u8> {
        let mut packet = b"OpusTags".to_vec();
        packet.extend_from_slice(&u32::try_from(vendor.len()).unwrap().to_le_bytes());
        packet.extend_from_slice(vendor);
        packet.extend_from_slice(&u32::try_from(comments.len()).unwrap().to_le_bytes());
        for c in comments {
            packet.extend_from_slice(&u32::try_from(c.len()).unwrap().to_le_bytes());
            packet.extend_from_slice(c);
        }
        packet
    }

    #[test]
    fn comments_map_like_libsndfile() {
        let strings = parse_comments(&tags(
            b"v",
            &[
                b"title=a",
                b"TITLE=b",
                b"encoder=x",
                b"Software=y",
                b"novalue",
                b"genre=g\0junk",
            ],
        ))
        .unwrap();
        assert_eq!(
            strings,
            vec![
                ("title", "b".to_owned()),
                ("software", "x".to_owned()),
                ("genre", "g".to_owned())
            ]
        );
        let mut short = tags(b"v", &[b"title=a"]);
        short.truncate(short.len() - 2);
        assert_eq!(parse_comments(&short), Err(Rejection::Malformed));
        assert_eq!(
            parse_comments(b"OpusTagz\0\0\0\0\0\0\0\0"),
            Err(Rejection::Malformed)
        );
    }
}
