//! Uncompressed little-endian PCM from an RF64 or Wave64 file, whose headers
//! [`crate::container`] parses (symphonia 0.6.1 has no demuxer for either, QG-13).
//!
//! Samples convert to `f64` with libsndfile's normalization (`pcm.c`, `float32.c`,
//! `double64.c`): `(u8 - 128) / 128`, `s8 / 128`, `s16 / 2^15`, `s24 / 2^23`, `s32 / 2^31`;
//! floats are widened unscaled. Every one of these is exact in `f64`, and casting the result to
//! `f32` gives libsndfile's `float` read (a power-of-two scale commutes with rounding), so one
//! conversion serves both the direct reader and the decode cache.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::container::{WAVE_FORMAT_IEEE_FLOAT, WAVE_FORMAT_PCM, WaveHeader};
use crate::error::MediaError;
use crate::format::Subtype;

/// The sample encodings the PCM reader decodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Encoding {
    U8,
    S16,
    S24,
    S32,
    F32,
    F64,
}

impl Encoding {
    /// The encoding of a RIFF-family `fmt ` chunk, if the reader supports it.
    pub(crate) fn of(header: &WaveHeader) -> Option<Self> {
        let format = &header.format;
        let bytes = usize::from(format.bits_per_sample.div_ceil(8));
        let fits = usize::from(format.block_align) == bytes * usize::from(format.channels);
        if !fits || format.channels == 0 {
            return None;
        }
        match (format.tag, format.bits_per_sample) {
            (WAVE_FORMAT_PCM, 8) => Some(Self::U8),
            (WAVE_FORMAT_PCM, 16) => Some(Self::S16),
            (WAVE_FORMAT_PCM, 24) => Some(Self::S24),
            (WAVE_FORMAT_PCM, 32) => Some(Self::S32),
            (WAVE_FORMAT_IEEE_FLOAT, 32) => Some(Self::F32),
            (WAVE_FORMAT_IEEE_FLOAT, 64) => Some(Self::F64),
            _ => None,
        }
    }

    /// libsndfile's subtype name for the encoding.
    pub(crate) const fn subtype(self) -> Subtype {
        match self {
            Self::U8 => Subtype::PcmU8,
            Self::S16 => Subtype::Pcm16,
            Self::S24 => Subtype::Pcm24,
            Self::S32 => Subtype::Pcm32,
            Self::F32 => Subtype::Float,
            Self::F64 => Subtype::Double,
        }
    }

    const fn bytes(self) -> usize {
        match self {
            Self::U8 => 1,
            Self::S16 => 2,
            Self::S24 => 3,
            Self::S32 | Self::F32 => 4,
            Self::F64 => 8,
        }
    }

    /// Converts packed little-endian samples to `f64`; `out` holds one value per sample.
    fn decode(self, bytes: &[u8], out: &mut [f64]) {
        let width = self.bytes();
        for (value, b) in out.iter_mut().zip(bytes.chunks_exact(width)) {
            *value = match self {
                Self::U8 => f64::from(i16::from(b[0]) - 128) / 128.0,
                Self::S16 => f64::from(i16::from_le_bytes([b[0], b[1]])) / 32_768.0,
                // Sign-extend through the top byte of an `i32`.
                Self::S24 => {
                    f64::from(i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) / 8_388_608.0
                }
                Self::S32 => {
                    f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])) / 2_147_483_648.0
                }
                Self::F32 => f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                Self::F64 => f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
            };
        }
    }
}

/// A seekable PCM stream over the data chunk of an RF64 or Wave64 file.
pub(crate) struct PcmStream {
    file: BufReader<File>,
    path: PathBuf,
    encoding: Encoding,
    channels: usize,
    block_align: usize,
    data_offset: u64,
    frames: u64,
    /// The next frame [`PcmStream::read`] returns.
    position: u64,
    /// Whether the file cursor is at `position`.
    positioned: bool,
    scratch: Vec<u8>,
}

impl PcmStream {
    /// Opens the data chunk described by `header`.
    ///
    /// # Errors
    ///
    /// [`MediaError::Unsupported`] for an encoding other than 8/16/24/32-bit integer or
    /// 32/64-bit float PCM.
    pub(crate) fn new(file: File, path: &Path, header: &WaveHeader) -> Result<Self, MediaError> {
        let encoding = Encoding::of(header).ok_or_else(|| {
            MediaError::unsupported(
                path,
                format!(
                    "encoding (format tag {:#06x}, {} bits)",
                    header.format.tag, header.format.bits_per_sample
                ),
            )
        })?;
        Ok(Self {
            file: BufReader::new(file),
            path: path.to_owned(),
            encoding,
            channels: usize::from(header.format.channels),
            block_align: usize::from(header.format.block_align),
            data_offset: header.data_offset,
            frames: header.frames(),
            position: 0,
            positioned: false,
            scratch: Vec::new(),
        })
    }

    pub(crate) fn encoding(&self) -> Encoding {
        self.encoding
    }

    pub(crate) fn channels(&self) -> usize {
        self.channels
    }

    pub(crate) fn frames(&self) -> u64 {
        self.frames
    }

    /// Moves to `frame` (at most the end of the data).
    pub(crate) fn seek(&mut self, frame: u64) {
        let frame = frame.min(self.frames);
        if frame != self.position {
            self.position = frame;
            self.positioned = false;
        }
    }

    /// Reads up to `out.len() / channels` frames as interleaved `f64`; returns the frames read,
    /// fewer only at the end of the data.
    ///
    /// # Errors
    ///
    /// [`MediaError::Io`] when the file cannot be read.
    pub(crate) fn read(&mut self, out: &mut [f64]) -> Result<usize, MediaError> {
        let want = u64::try_from(out.len() / self.channels).unwrap_or(u64::MAX);
        let frames = want.min(self.frames - self.position);
        let frames = usize::try_from(frames).unwrap_or(usize::MAX);
        if frames == 0 {
            return Ok(0);
        }
        if !self.positioned {
            let offset = self.data_offset + self.position * self.block_align as u64;
            self.file
                .seek(SeekFrom::Start(offset))
                .map_err(|e| MediaError::io("seek in", &self.path, e))?;
            self.positioned = true;
        }
        let bytes = frames * self.block_align;
        self.scratch.resize(bytes, 0);
        if let Err(err) = self.file.read_exact(&mut self.scratch[..bytes]) {
            self.positioned = false;
            return Err(MediaError::io("read", &self.path, err));
        }
        self.encoding
            .decode(&self.scratch[..bytes], &mut out[..frames * self.channels]);
        self.position += frames as u64;
        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::Encoding;

    #[test]
    fn samples_scale_like_libsndfile() {
        let mut out = [0.0; 3];
        Encoding::U8.decode(&[0, 128, 255], &mut out);
        assert_eq!(out, [-1.0, 0.0, 127.0 / 128.0]);
        Encoding::S16.decode(&[0x00, 0x80, 0xFF, 0x7F, 0x01, 0x00], &mut out);
        assert_eq!(out, [-1.0, 32_767.0 / 32_768.0, 1.0 / 32_768.0]);
        Encoding::S24.decode(&[0, 0, 0x80, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F], &mut out);
        assert_eq!(out, [-1.0, -1.0 / 8_388_608.0, 8_388_607.0 / 8_388_608.0]);
        let mut one = [0.0; 1];
        Encoding::S32.decode(&i32::MIN.to_le_bytes(), &mut one);
        assert_eq!(one, [-1.0]);
        Encoding::F32.decode(&1.5f32.to_le_bytes(), &mut one);
        assert_eq!(one, [1.5]);
        Encoding::F64.decode(&(-0.25f64).to_le_bytes(), &mut one);
        assert_eq!(one, [-0.25]);
    }
}
