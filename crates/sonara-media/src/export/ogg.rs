//! Ogg Vorbis and Ogg Opus export, with permissively licensed codecs linked statically:
//! `vorbis_rs` (BSD-3-Clause; it builds the aoTuV/Lancer variant of libvorbis and libogg) and
//! the `opus` crate (MIT/Apache-2.0) over libopus (BSD-3-Clause, built by `opusic-sys`), muxed
//! with the `ogg` crate (BSD-3-Clause). The settings follow libsndfile 1.2.2 for the prototype
//! (`ogg_vorbis.c`, `ogg_opus.c`, `ogg_vcomment.c`; reimplemented, no LGPL code copied):
//!
//! - **Vorbis:** VBR quality mode with quality `1 - level` (clamped to 0..1); comments
//!   `ENCODER=libsndfile`, then the strings as `TITLE`, `COPYRIGHT`, `SOFTWARE`, `ARTIST`,
//!   `COMMENT`, `DATE`, `ALBUM`, `LICENSE`, `Tracknumber` and `Genre` in the order they were
//!   set; one analysis call per written block.
//! - **Opus:** the libopus encoder for 1 or 2 channels at the file's rate (8, 12, 16, 24 or
//!   48 kHz) with `OPUS_APPLICATION_AUDIO`, complexity 10, an LSB depth of 24 (float input),
//!   and the bitrate `int((1 - level) * 250000 + 6000) * channels`; 20 ms packets; an
//!   `OpusHead` page (version 1, the pre-skip `lookahead * 48000 / rate`, the input rate, gain
//!   0, mapping family 0) and an `OpusTags` page (the libopus version as vendor, the strings
//!   with `ENCODER` for the software string, then `(n + 512 + 255) / 255 * 255 - 1` zero bytes
//!   after the `n` bytes of tags); a page ends when it holds a second of audio; at the end, the
//!   rest of the buffer is padded with silence, more packets are encoded until the granule
//!   position covers the pre-skip, the last one shortened to the smallest valid duration, and
//!   its granule position marks the true end. As in libsndfile, every packet after the first
//!   of that tail re-encodes the same buffer.
//!
//! The codec versions differ from the prototype's (libvorbis 1.3.7, libopus 1.4), so the
//! encoded bytes differ; the parity target is the decoded audio, compared statistically.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::num::{NonZeroU8, NonZeroU32};
use std::path::{Path, PathBuf};

use ogg::writing::{PacketWriteEndInfo, PacketWriter};
use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoder, VorbisEncoderBuilder};

use super::ExportError;
use super::strings::{StrKind, Strings};

/// Opus packets are 20 ms.
const OPUS_PACKET_MS: u32 = 20;
/// libsndfile's default page latency: a page ends when it holds 1000 ms (at 48 kHz).
const OPUS_PAGE_LATENCY: u64 = 1000 * 48;
/// The `OpusTags` padding target (as `oggenc`).
const OPUS_COMMENT_PAD: usize = 512;
/// The largest Opus packet of one stream (`1275 * 3 + 7`, from opus-tools).
const OPUS_MAX_PACKET: usize = 1275 * 3 + 7;

fn vorbis_error(e: impl std::fmt::Display) -> ExportError {
    ExportError::Encoder {
        format: "ogg",
        message: e.to_string(),
    }
}

fn opus_error(e: impl std::fmt::Display) -> ExportError {
    ExportError::Encoder {
        format: "opus",
        message: e.to_string(),
    }
}

/// One Ogg Vorbis file being written.
pub(crate) struct VorbisWriter {
    encoder: VorbisEncoder<BufWriter<File>>,
    path: PathBuf,
    channels: usize,
    planar: Vec<Vec<f32>>,
}

impl VorbisWriter {
    /// Creates `path` for a Vorbis stream at `level` (quality `1 - level`) with stream serial
    /// `serial`.
    pub(crate) fn create(
        path: &Path,
        sample_rate: u32,
        channels: usize,
        level: f64,
        strings: &Strings,
        serial: u32,
    ) -> Result<Self, ExportError> {
        let rate = NonZeroU32::new(sample_rate).ok_or_else(|| vorbis_error("zero sample rate"))?;
        let ch = u8::try_from(channels)
            .ok()
            .and_then(NonZeroU8::new)
            .ok_or_else(|| vorbis_error("bad channel count"))?;
        let file = File::create(path).map_err(|e| ExportError::io("create", path, e))?;
        let sink = BufWriter::with_capacity(1 << 18, file);
        let mut builder = VorbisEncoderBuilder::new_with_serial(
            rate,
            ch,
            sink,
            i32::from_ne_bytes(serial.to_ne_bytes()),
        );
        builder
            .bitrate_management_strategy(VorbisBitrateManagementStrategy::QualityVbr {
                target_quality: vorbis_quality(level),
            })
            .minimum_page_data_size(None);
        builder
            .comment_tag("ENCODER", "libsndfile")
            .map_err(vorbis_error)?;
        for (kind, text) in strings.iter() {
            let name = match kind {
                StrKind::Title => "TITLE",
                StrKind::Copyright => "COPYRIGHT",
                StrKind::Software => "SOFTWARE",
                StrKind::Artist => "ARTIST",
                StrKind::Comment => "COMMENT",
                StrKind::Date => "DATE",
                StrKind::Album => "ALBUM",
                StrKind::License => "LICENSE",
                StrKind::TrackNumber => "Tracknumber",
                StrKind::Genre => "Genre",
            };
            builder
                .comment_tag(name, String::from_utf8_lossy(text))
                .map_err(vorbis_error)?;
        }
        let encoder = builder.build().map_err(vorbis_error)?;
        Ok(Self {
            encoder,
            path: path.to_owned(),
            channels,
            planar: vec![Vec::new(); channels],
        })
    }

    /// Encodes one block of interleaved samples.
    pub(crate) fn write(&mut self, samples: &[f32]) -> Result<(), ExportError> {
        for (c, channel) in self.planar.iter_mut().enumerate() {
            channel.clear();
            channel.extend(samples.iter().skip(c).step_by(self.channels));
        }
        self.encoder
            .encode_audio_block(&self.planar)
            .map_err(vorbis_error)
    }

    /// Ends the stream.
    pub(crate) fn finish(self) -> Result<(), ExportError> {
        let mut sink = self.encoder.finish().map_err(vorbis_error)?;
        sink.flush()
            .map_err(|e| ExportError::io("write", &self.path, e))
    }
}

/// libsndfile's Vorbis quality for a compression level: `1 - level`, clamped to 0..1.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the quality is a float in 0..1 for libvorbis"
)]
fn vorbis_quality(level: f64) -> f32 {
    (1.0 - level).clamp(0.0, 1.0) as f32
}

/// The Opus bitrate libsndfile sets for a compression level, in bits per second.
#[expect(
    clippy::cast_possible_truncation,
    reason = "C's double-to-int conversion truncates; the value is below 256001"
)]
pub(crate) fn opus_bitrate(level: f64, channels: usize) -> i32 {
    let per_channel = ((1.0 - level) * 250_000.0 + 6000.0) as i32;
    per_channel.saturating_mul(i32::try_from(channels).unwrap_or(1))
}

/// One Ogg Opus file being written.
pub(crate) struct OpusWriter {
    writer: PacketWriter<'static, BufWriter<File>>,
    path: PathBuf,
    encoder: opus::Encoder,
    serial: u32,
    channels: usize,
    /// 48000 / rate.
    sr_factor: u64,
    /// Frames per packet at the file's rate.
    len: usize,
    buffer: Vec<f32>,
    /// Frames in `buffer`.
    loc: usize,
    /// Granule position (48 kHz) after the last encoded packet.
    pkt_pos: u64,
    /// Granule position of the last page end.
    pg_pos: u64,
    preskip: u64,
    /// The last encoded packet, written when the next one decides how its page ends.
    pending: Option<(Vec<u8>, u64)>,
    packet: Vec<u8>,
    strings: Strings,
    started: bool,
}

impl OpusWriter {
    /// Creates `path` for an Opus stream at `sample_rate` (8, 12, 16, 24 or 48 kHz).
    pub(crate) fn create(
        path: &Path,
        sample_rate: u32,
        channels: usize,
        level: f64,
        strings: Strings,
        serial: u32,
    ) -> Result<Self, ExportError> {
        if !matches!(sample_rate, 8000 | 12_000 | 16_000 | 24_000 | 48_000) {
            return Err(opus_error(format!(
                "sample rate {sample_rate} is not an Opus rate"
            )));
        }
        let opus_channels = match channels {
            1 => opus::Channels::Mono,
            2 => opus::Channels::Stereo,
            _ => return Err(opus_error("Opus export supports 1 or 2 channels")),
        };
        let mut encoder = opus::Encoder::new(sample_rate, opus_channels, opus::Application::Audio)
            .map_err(opus_error)?;
        encoder.set_complexity(10).map_err(opus_error)?;
        let lookahead = encoder.get_lookahead().map_err(opus_error)?;
        let sr_factor = u64::from(48_000 / sample_rate);
        let preskip = u64::try_from(lookahead).unwrap_or(0) * sr_factor;
        encoder
            .set_bitrate(opus::Bitrate::Bits(opus_bitrate(level, channels)))
            .map_err(opus_error)?;
        encoder.set_lsb_depth(24).map_err(opus_error)?;
        let len = usize::try_from(OPUS_PACKET_MS * sample_rate / 1000).unwrap_or(960);
        let file = File::create(path).map_err(|e| ExportError::io("create", path, e))?;
        Ok(Self {
            writer: PacketWriter::new(BufWriter::with_capacity(1 << 18, file)),
            path: path.to_owned(),
            encoder,
            serial,
            channels,
            sr_factor,
            len,
            buffer: vec![0.0; len * channels],
            loc: 0,
            pkt_pos: 0,
            pg_pos: 0,
            preskip,
            pending: None,
            packet: vec![0; OPUS_MAX_PACKET],
            strings,
            started: false,
        })
    }

    fn io(&self, e: std::io::Error) -> ExportError {
        ExportError::io("write", &self.path, e)
    }

    /// The `OpusHead` and `OpusTags` pages.
    fn start(&mut self, sample_rate_hint: u32) -> Result<(), ExportError> {
        let mut head = Vec::with_capacity(19);
        head.extend_from_slice(b"OpusHead");
        head.push(1);
        head.push(u8::try_from(self.channels).unwrap_or(2));
        head.extend_from_slice(
            &u16::try_from(self.preskip)
                .unwrap_or(u16::MAX)
                .to_le_bytes(),
        );
        head.extend_from_slice(&sample_rate_hint.to_le_bytes());
        head.extend_from_slice(&0i16.to_le_bytes());
        head.push(0);
        self.writer
            .write_packet(head, self.serial, PacketWriteEndInfo::EndPage, 0)
            .map_err(|e| self.io(e))?;
        let tags = opus_tags(&self.strings, opus::version());
        self.writer
            .write_packet(tags, self.serial, PacketWriteEndInfo::EndPage, 0)
            .map_err(|e| self.io(e))?;
        self.started = true;
        Ok(())
    }

    fn sample_rate(&self) -> u32 {
        u32::try_from(48_000 / self.sr_factor.max(1)).unwrap_or(48_000)
    }

    /// Buffers one block of interleaved samples, encoding full 20 ms packets.
    pub(crate) fn write(&mut self, samples: &[f32]) -> Result<(), ExportError> {
        if !self.started {
            self.start(self.sample_rate())?;
        }
        let mut rest = samples;
        while !rest.is_empty() {
            if self.loc >= self.len {
                self.write_out()?;
            }
            let room = (self.len - self.loc) * self.channels;
            let take = room.min(rest.len());
            let (head, tail) = rest.split_at(take);
            let start = self.loc * self.channels;
            if let Some(slot) = self.buffer.get_mut(start..start + take) {
                slot.copy_from_slice(head);
            }
            self.loc += take / self.channels;
            rest = tail;
        }
        Ok(())
    }

    /// Encodes `frames` frames from the start of the buffer.
    fn encode(&mut self, frames: usize) -> Result<Vec<u8>, ExportError> {
        let input = self
            .buffer
            .get(..frames * self.channels)
            .ok_or_else(|| opus_error("short buffer"))?;
        let bytes = self
            .encoder
            .encode_float(input, &mut self.packet)
            .map_err(opus_error)?;
        Ok(self.packet.get(..bytes).unwrap_or_default().to_vec())
    }

    /// `ogg_opus_write_out`: one full packet; the page holding the previous packets ends when
    /// a second of audio has accumulated since the last page end.
    fn write_out(&mut self) -> Result<(), ExportError> {
        let packet = self.encode(self.len)?;
        self.pkt_pos += u64::try_from(self.len).unwrap_or(0) * self.sr_factor;
        let end_page = self.pkt_pos - self.pg_pos >= OPUS_PAGE_LATENCY;
        if end_page {
            self.pg_pos = self.pkt_pos;
        }
        self.push(packet, self.pkt_pos, end_page)?;
        self.loc = 0;
        Ok(())
    }

    /// Writes the pending packet (ending its page if asked) and keeps `packet` pending.
    fn push(
        &mut self,
        packet: Vec<u8>,
        granule: u64,
        end_previous_page: bool,
    ) -> Result<(), ExportError> {
        if let Some((previous, previous_granule)) = self.pending.take() {
            let info = if end_previous_page {
                PacketWriteEndInfo::EndPage
            } else {
                PacketWriteEndInfo::NormalPacket
            };
            self.writer
                .write_packet(previous, self.serial, info, previous_granule)
                .map_err(|e| self.io(e))?;
        }
        self.pending = Some((packet, granule));
        Ok(())
    }

    /// `ogg_opus_flush`: encodes the buffered frames and the encoder's look-ahead, then ends
    /// the stream with the true length in the last granule position.
    pub(crate) fn finish(mut self) -> Result<(), ExportError> {
        if !self.started {
            self.start(self.sample_rate())?;
        }
        let last_granule =
            self.pkt_pos + self.sr_factor * u64::try_from(self.loc).unwrap_or(0) + self.preskip;
        let start = self.loc * self.channels;
        if let Some(rest) = self.buffer.get_mut(start..) {
            rest.fill(0.0);
        }
        let full = u64::try_from(self.len).unwrap_or(0) * self.sr_factor;
        loop {
            self.pkt_pos += full;
            let last = self.pkt_pos >= last_granule;
            let frames = if last {
                let remaining = full - (self.pkt_pos - last_granule);
                [120u64, 240, 480]
                    .into_iter()
                    .find(|&d| remaining <= d)
                    .map_or(self.len, |d| {
                        usize::try_from(d / self.sr_factor).unwrap_or(self.len)
                    })
            } else {
                self.len
            };
            let packet = self.encode(frames)?;
            let granule = if last { last_granule } else { self.pkt_pos };
            self.push(packet, granule, false)?;
            if last {
                break;
            }
        }
        if let Some((packet, granule)) = self.pending.take() {
            self.writer
                .write_packet(packet, self.serial, PacketWriteEndInfo::EndStream, granule)
                .map_err(|e| self.io(e))?;
        }
        let path = self.path.clone();
        self.writer
            .inner_mut()
            .flush()
            .map_err(|e| ExportError::io("write", &path, e))
    }
}

/// The `OpusTags` packet as libsndfile writes it.
fn opus_tags(strings: &Strings, vendor: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"OpusTags");
    out.extend_from_slice(&u32::try_from(vendor.len()).unwrap_or(0).to_le_bytes());
    out.extend_from_slice(vendor.as_bytes());
    let count_at = out.len();
    out.extend_from_slice(&[0; 4]);
    let mut count = 0u32;
    for (kind, text) in strings.iter() {
        let name = match kind {
            StrKind::Title => "TITLE",
            StrKind::Copyright => "COPYRIGHT",
            StrKind::Software => "ENCODER",
            StrKind::Artist => "ARTIST",
            StrKind::Comment => "COMMENT",
            StrKind::Date => "DATE",
            StrKind::Album => "ALBUM",
            StrKind::License => "LICENSE",
            StrKind::TrackNumber => "TRACKNUMBER",
            StrKind::Genre => "GENRE",
        };
        let len = name.len() + 1 + text.len();
        out.extend_from_slice(&u32::try_from(len).unwrap_or(0).to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.push(b'=');
        out.extend_from_slice(text);
        count += 1;
    }
    let pad = (out.len() + OPUS_COMMENT_PAD + 255) / 255 * 255 - 1;
    out.resize(out.len() + pad, 0);
    if let Some(slot) = out.get_mut(count_at..count_at + 4) {
        slot.copy_from_slice(&count.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{opus_bitrate, opus_tags, vorbis_quality};
    use crate::export::Container;
    use crate::export::strings::Strings;

    #[test]
    fn levels_map_like_libsndfile() {
        assert_eq!(opus_bitrate(0.25, 2), 387_000);
        assert_eq!(opus_bitrate(0.0, 1), 256_000);
        assert!((vorbis_quality(0.25) - 0.75).abs() < 1e-7);
        assert!((vorbis_quality(-1.0) - 1.0).abs() < 1e-7);
    }

    #[test]
    fn opus_tags_are_padded_like_libsndfile() {
        let (strings, _) = Strings::set(
            Container::Opus,
            &[("software".to_owned(), "Sonara".to_owned())],
        );
        // 66 bytes of tags, then `(66 + 512 + 255) / 255 * 255 - 1` zeros (libsndfile's
        // formula; the prototype's file with the 11-byte vendor "libopus 1.4" has 828 bytes).
        let tags = opus_tags(&strings, "libopus 1.6.1");
        assert_eq!(tags.len(), 66 + 764);
        assert_eq!(opus_tags(&strings, "libopus 1.4").len(), 828);
        assert_eq!(&tags[8..12], &13u32.to_le_bytes());
        let text = String::from_utf8_lossy(&tags);
        assert!(text.contains("ENCODER=Sonara (libsndfile-1.2.2)"));
    }
}
