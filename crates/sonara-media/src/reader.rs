//! Random-access readers over project media at the project rate (`audio/engine/media.py`):
//! the direct reader for uncompressed files already at the project rate, the cached reader over
//! a decode-cache entry, and the silent reader that stands in for missing or unreadable media.
//!
//! Every reader returns interleaved `f64` frames and fills the part of a window outside the
//! media with zeros, so a window may start before the media or end after it.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::error::MediaError;
use crate::stream::AudioStream;

/// Silence in place of missing or unreadable media (`media.SilentReader`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SilentReader {
    channels: usize,
    frames: u64,
    reason: String,
}

impl SilentReader {
    /// Silence of `frames` frames; the channel count is at least 1.
    #[must_use]
    pub fn new(channels: usize, frames: u64, reason: impl Into<String>) -> Self {
        Self {
            channels: channels.max(1),
            frames,
            reason: reason.into(),
        }
    }

    /// Why the media is silent: `"missing"` or `"unreadable: …"`.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// Reads an uncompressed file at the project rate by seeking (`media.DirectReader`).
pub struct DirectReader {
    stream: AudioStream,
    channels: usize,
    frames: u64,
}

impl std::fmt::Debug for DirectReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirectReader")
            .field("channels", &self.channels)
            .field("frames", &self.frames)
            .finish_non_exhaustive()
    }
}

impl DirectReader {
    /// Opens `path` for direct reading.
    ///
    /// # Errors
    ///
    /// The errors of opening and inspecting the file, [`MediaError::Unsupported`] when its codec
    /// cannot be decoded, and [`MediaError::Malformed`] when its header states no length.
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        let mut stream = AudioStream::open(path)?.stream;
        stream.check_decodable()?;
        let frames = stream
            .frames()
            .ok_or_else(|| MediaError::malformed(path, "the file states no length"))?;
        Ok(Self {
            channels: stream.channels(),
            frames,
            stream,
        })
    }
}

/// Reads a decode-cache entry: raw interleaved little-endian `f32` (`media.CachedReader`).
///
/// The prototype memory-maps the file. `memmap2`'s mapping is an `unsafe` call, which RT-10
/// does not allow in this module, so the reader uses positional reads of the same layout; the
/// values are identical.
#[derive(Debug)]
pub struct CachedReader {
    file: Option<BufReader<File>>,
    path: PathBuf,
    channels: usize,
    frames: u64,
    bytes: Vec<u8>,
}

impl CachedReader {
    /// Opens the raw file of a cache entry with the sidecar's channel and frame counts.
    ///
    /// # Errors
    ///
    /// [`MediaError::Io`] when the file cannot be opened, and [`MediaError::Cache`] when it is
    /// shorter than `frames × channels` samples (where `numpy.memmap` fails as well).
    pub fn open(raw: &Path, channels: usize, frames: u64) -> Result<Self, MediaError> {
        let channels = channels.max(1);
        let file = if frames == 0 {
            None
        } else {
            let file = File::open(raw).map_err(|e| MediaError::io("open", raw, e))?;
            let len = file
                .metadata()
                .map_err(|e| MediaError::io("inspect", raw, e))?
                .len();
            let needed =
                frames
                    .checked_mul(channels as u64 * 4)
                    .ok_or_else(|| MediaError::Cache {
                        path: raw.to_owned(),
                        message: "frame count overflows".to_owned(),
                    })?;
            if len < needed {
                return Err(MediaError::Cache {
                    path: raw.to_owned(),
                    message: format!("{len} bytes for {frames} frames of {channels} channels"),
                });
            }
            Some(BufReader::new(file))
        };
        Ok(Self {
            file,
            path: raw.to_owned(),
            channels,
            frames,
            bytes: Vec::new(),
        })
    }
}

/// One of the three readers.
#[derive(Debug)]
pub enum MediaReader {
    /// Silence for missing or unreadable media.
    Silent(SilentReader),
    /// Direct reading of the original file.
    Direct(DirectReader),
    /// Reading of a decode-cache entry.
    Cached(CachedReader),
}

/// The window `[start, start + count)` clipped to `[0, frames)`, as offsets: `(first frame,
/// frames, offset of the first frame inside the window)`.
fn clip(start: i64, count: usize, frames: u64) -> Option<(u64, usize, usize)> {
    let start = i128::from(start);
    let a = start.max(0);
    let b = (start + count as i128).min(i128::from(frames));
    (b > a).then(|| {
        (
            u64::try_from(a).unwrap_or(0),
            usize::try_from(b - a).unwrap_or(0),
            usize::try_from(a - start).unwrap_or(0),
        )
    })
}

impl MediaReader {
    /// Channel count.
    #[must_use]
    pub fn channels(&self) -> usize {
        match self {
            Self::Silent(r) => r.channels,
            Self::Direct(r) => r.channels,
            Self::Cached(r) => r.channels,
        }
    }

    /// Frames at the project rate.
    #[must_use]
    pub fn frames(&self) -> u64 {
        match self {
            Self::Silent(r) => r.frames,
            Self::Direct(r) => r.frames,
            Self::Cached(r) => r.frames,
        }
    }

    /// Whether this reader stands in for missing or unreadable media
    /// (`MediaProvider.is_missing`).
    #[must_use]
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Silent(_))
    }

    /// Fills `out` with the interleaved frames from `start` on; `out.len() / channels` frames
    /// are read and the part outside the media is zero.
    ///
    /// # Errors
    ///
    /// [`MediaError::InvalidArgument`] when `out` holds a partial frame, and the I/O and decode
    /// errors of reading the media.
    pub fn read_into(&mut self, start: i64, out: &mut [f64]) -> Result<(), MediaError> {
        let channels = self.channels();
        if !out.len().is_multiple_of(channels) {
            return Err(MediaError::InvalidArgument(format!(
                "a window of {} samples is not a whole number of {channels}-channel frames",
                out.len()
            )));
        }
        out.fill(0.0);
        let Some((first, frames, offset)) = clip(start, out.len() / channels, self.frames()) else {
            return Ok(());
        };
        let window = &mut out[offset * channels..(offset + frames) * channels];
        match self {
            Self::Silent(_) => Ok(()),
            Self::Direct(r) => {
                r.stream.seek(first)?;
                let mut filled = 0;
                while filled < frames {
                    let read = r.stream.read(&mut window[filled * channels..])?;
                    if read == 0 {
                        break;
                    }
                    filled += read;
                }
                Ok(())
            }
            Self::Cached(r) => r.read_window(first, window),
        }
    }

    /// The `count` interleaved frames from `start` on (`SourceReader.read`).
    ///
    /// # Errors
    ///
    /// The errors of [`MediaReader::read_into`].
    pub fn read(&mut self, start: i64, count: usize) -> Result<Vec<f64>, MediaError> {
        let mut out = vec![0.0; count * self.channels()];
        self.read_into(start, &mut out)?;
        Ok(out)
    }
}

impl CachedReader {
    fn read_window(&mut self, first: u64, window: &mut [f64]) -> Result<(), MediaError> {
        let Some(file) = &mut self.file else {
            return Ok(());
        };
        let offset = first * self.channels as u64 * 4;
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| MediaError::io("seek in", &self.path, e))?;
        self.bytes.resize(window.len() * 4, 0);
        file.read_exact(&mut self.bytes)
            .map_err(|e| MediaError::io("read", &self.path, e))?;
        let (samples, _) = self.bytes.as_chunks::<4>();
        for (value, bytes) in window.iter_mut().zip(samples) {
            *value = f64::from(f32::from_le_bytes(*bytes));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{CachedReader, MediaReader, SilentReader, clip};

    #[test]
    fn windows_clip_to_the_media() {
        assert_eq!(clip(-100, 150, 400), Some((0, 50, 100)));
        assert_eq!(clip(0, 512, 400), Some((0, 400, 0)));
        assert_eq!(clip(350, 100, 400), Some((350, 50, 0)));
        assert_eq!(clip(410, 20, 400), None);
        assert_eq!(clip(-300, 100, 400), None);
        assert_eq!(
            clip(i64::MAX, 10, u64::MAX),
            Some((i64::MAX.unsigned_abs(), 10, 0))
        );
        assert_eq!(clip(i64::MIN, 10, 100), None);
    }

    #[test]
    fn silence_has_at_least_one_channel() {
        let mut reader = MediaReader::Silent(SilentReader::new(0, 10, "missing"));
        assert_eq!(reader.channels(), 1);
        assert!(reader.is_missing());
        assert_eq!(reader.read(-5, 20).unwrap(), vec![0.0; 20]);
        assert!(reader.read_into(0, &mut [0.0; 3]).is_ok());
    }

    #[test]
    fn cached_windows_are_zero_padded() {
        let dir = tempfile::tempdir().unwrap();
        let raw = dir.path().join("a-48000.f32");
        let samples: Vec<u8> = [0.5f32, -0.5, 0.25, -0.25, 1.0, -1.0]
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();
        std::fs::write(&raw, samples).unwrap();
        let mut reader = MediaReader::Cached(CachedReader::open(&raw, 2, 3).unwrap());
        assert_eq!(
            reader.read(-1, 3).unwrap(),
            vec![0.0, 0.0, 0.5, -0.5, 0.25, -0.25]
        );
        assert_eq!(reader.read(2, 2).unwrap(), vec![1.0, -1.0, 0.0, 0.0]);
        assert!(reader.read_into(0, &mut [0.0; 3]).is_err());
        assert!(CachedReader::open(&raw, 2, 4).is_err(), "file too short");
        assert!(CachedReader::open(&dir.path().join("none"), 2, 0).is_ok());
    }
}
