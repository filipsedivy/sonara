//! MP3 export through LAME, loaded at run time (LIC-01), driven call for
//! call as libsndfile 1.2.2 drives it for the prototype (`mpeg.c`, `mpeg_l3_encode.c`; the
//! behaviour is reimplemented here, no LGPL code is copied):
//!
//! - **Open:** `lame_init`; input and output rate and channel count; no automatic ID3 tags;
//!   the Xing/Info header stays on (the file is seekable). libsndfile then selects variable
//!   bitrate (`vbr_default`).
//! - **Quality** (soundfile sets the compression level, then the bitrate mode, which applies
//!   the level again): for variable bitrate `lame_set_VBR_quality(level * 10)`; for constant
//!   bitrate `lame_set_brate` with `320 - level * 288` kbps at 32 kHz and above (MPEG-1),
//!   `160 - level * 152` from 16 kHz (MPEG-2) and `64 - level * 56` below (MPEG-2.5), each
//!   truncated to an integer.
//! - **First write:** `lame_init_params`; `id3tag_init`; title, artist, album, date (year),
//!   comment, genre and track number in the order they were set; an `ID3v2` tag is written at
//!   the start only when LAME produces one (it does when a field does not fit `ID3v1`).
//! - **Samples:** `float`s in ±1. Stereo goes to `lame_encode_buffer_interleaved_ieee_float`
//!   in pieces of at most 2048 samples (libsndfile's buffer, never more than one MP3 frame of
//!   both channels), mono to `lame_encode_buffer_ieee_float` in pieces of one frame.
//! - **Close:** `lame_encode_flush`; the `ID3v1` trailer; then the LAME tag frame
//!   (`lame_get_lametag_frame`) over the empty Xing frame LAME wrote at the data start.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::codec_ffi::{Id3Field, Lame, LameLibrary, VbrMode};

use super::ExportError;
use super::strings::{StrKind, Strings};

/// libsndfile's `BUF_UNION` holds 2048 floats.
const FLOAT_BUFFER: usize = 2048;

/// One MP3 file being written.
pub(crate) struct Mp3Writer {
    file: BufWriter<File>,
    path: PathBuf,
    lame: Lame,
    channels: usize,
    strings: Strings,
    /// Samples per LAME call; known after `lame_init_params`.
    piece: usize,
    data_offset: u64,
    started: bool,
    out: Vec<u8>,
}

impl Mp3Writer {
    /// Opens `path` for an MP3 at `sample_rate` with `channels` (1 or 2), configured with the
    /// compression `level` and bitrate mode.
    pub(crate) fn create(
        path: &Path,
        sample_rate: u32,
        channels: usize,
        level: f64,
        variable: bool,
        strings: Strings,
    ) -> Result<Self, ExportError> {
        let mut lame = Lame::new(LameLibrary::shared()?)?;
        let rate = i32::try_from(sample_rate)
            .map_err(|_| ExportError::InvalidArgument(format!("bad sample rate {sample_rate}")))?;
        let ch = i32::try_from(channels)
            .map_err(|_| ExportError::InvalidArgument(format!("bad channel count {channels}")))?;
        lame.set_in_samplerate(rate)?;
        lame.set_num_channels(ch)?;
        lame.set_out_samplerate(rate)?;
        lame.set_write_id3tag_automatic(false);
        // `mpeg_init`: variable bitrate by default; the compression level is not set yet.
        lame.set_vbr(VbrMode::Default)?;
        // soundfile: `SFC_SET_COMPRESSION_LEVEL`, then `SFC_SET_BITRATE_MODE`, which applies
        // the level again for the new mode.
        set_quality(&mut lame, sample_rate, level)?;
        lame.set_vbr(if variable {
            VbrMode::Default
        } else {
            VbrMode::Off
        })?;
        set_quality(&mut lame, sample_rate, level)?;
        let file = File::create(path).map_err(|e| ExportError::io("create", path, e))?;
        Ok(Self {
            file: BufWriter::with_capacity(1 << 18, file),
            path: path.to_owned(),
            lame,
            channels,
            strings,
            piece: 0,
            data_offset: 0,
            started: false,
            out: Vec::new(),
        })
    }

    /// `mpeg_l3_encoder_write_id3tag`, run before the first samples.
    fn start(&mut self) -> Result<(), ExportError> {
        self.lame.init_params()?;
        let framesize = usize::try_from(self.lame.framesize()).unwrap_or(0).max(1);
        let frame_samples = framesize * self.channels;
        self.piece = if self.channels == 2 {
            FLOAT_BUFFER.min(frame_samples)
        } else {
            frame_samples
        };
        self.lame.id3tag_init();
        for (kind, text) in self.strings.iter() {
            let field = match kind {
                StrKind::Title => Id3Field::Title,
                StrKind::Artist => Id3Field::Artist,
                StrKind::Album => Id3Field::Album,
                StrKind::Date => Id3Field::Year,
                StrKind::Comment => Id3Field::Comment,
                StrKind::Genre => Id3Field::Genre,
                StrKind::TrackNumber => Id3Field::Track,
                StrKind::Copyright | StrKind::Software | StrKind::License => continue,
            };
            self.lame.id3tag_set(field, text);
        }
        let id3v2 = self.lame.id3v2_tag();
        if !id3v2.is_empty() {
            self.file
                .write_all(&id3v2)
                .map_err(|e| ExportError::io("write", &self.path, e))?;
            self.data_offset = u64::try_from(id3v2.len()).unwrap_or(0);
        }
        self.started = true;
        Ok(())
    }

    /// Encodes one block of interleaved samples (one `sf_write_float` call).
    pub(crate) fn write(&mut self, samples: &[f32]) -> Result<(), ExportError> {
        if !self.started {
            self.start()?;
        }
        self.out.clear();
        for piece in samples.chunks(self.piece.max(1)) {
            if self.channels == 2 {
                self.lame
                    .encode_interleaved(piece, piece.len() / 2, &mut self.out)?;
            } else {
                self.lame.encode_mono(piece, &mut self.out)?;
            }
        }
        self.file
            .write_all(&self.out)
            .map_err(|e| ExportError::io("write", &self.path, e))
    }

    /// Flushes the encoder, appends the `ID3v1` trailer and writes the LAME tag frame.
    pub(crate) fn finish(mut self) -> Result<(), ExportError> {
        if !self.started {
            self.start()?;
        }
        let mut tail = Vec::new();
        self.lame.flush(&mut tail)?;
        tail.extend_from_slice(&self.lame.id3v1_tag());
        let path = self.path.clone();
        let io = |e| ExportError::io("write", &path, e);
        self.file.write_all(&tail).map_err(io)?;
        let tag = self.lame.lametag_frame();
        if !tag.is_empty() {
            self.file.flush().map_err(io)?;
            self.file
                .seek(SeekFrom::Start(self.data_offset))
                .map_err(io)?;
            self.file.write_all(&tag).map_err(io)?;
        }
        self.file.flush().map_err(io)
    }
}

/// `mpeg_l3_encoder_set_quality` for the current bitrate mode.
fn set_quality(lame: &mut Lame, sample_rate: u32, level: f64) -> Result<(), ExportError> {
    match lame.vbr() {
        Some(VbrMode::Default) => {
            lame.set_vbr_quality(to_float(level * 10.0))?;
        }
        Some(mode) => {
            let kbps = cbr_kbps(sample_rate, level);
            if mode == VbrMode::Abr {
                lame.set_vbr_mean_bitrate_kbps(kbps)?;
            } else {
                lame.set_brate(kbps)?;
            }
        }
        None => {
            return Err(ExportError::Encoder {
                format: "mp3",
                message: "LAME reports an unknown bitrate mode".to_owned(),
            });
        }
    }
    Ok(())
}

/// The constant bitrate libsndfile picks for a compression `level` in kbps, truncated as C
/// converts the `double` to `int`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "C's double-to-int conversion truncates; the value is between 8 and 320"
)]
pub(crate) fn cbr_kbps(sample_rate: u32, level: f64) -> i32 {
    let kbps = if sample_rate >= 32_000 {
        320.0 - level * (320.0 - 32.0)
    } else if sample_rate >= 16_000 {
        160.0 - level * (160.0 - 8.0)
    } else {
        64.0 - level * (64.0 - 8.0)
    };
    kbps as i32
}

/// The C conversion of `compression * 10.0` to the `float` parameter.
#[expect(
    clippy::cast_possible_truncation,
    reason = "lame_set_VBR_quality takes a C float"
)]
fn to_float(v: f64) -> f32 {
    v as f32
}

#[cfg(test)]
mod tests {
    use super::cbr_kbps;

    #[test]
    fn constant_bitrates_follow_libsndfile() {
        assert_eq!(cbr_kbps(48_000, 0.0), 320);
        assert_eq!(cbr_kbps(44_100, 0.99), 34);
        assert_eq!(cbr_kbps(22_050, 0.0), 160);
        assert_eq!(cbr_kbps(11_025, 0.0), 64);
        assert_eq!(cbr_kbps(8_000, 0.5), 36);
    }
}
