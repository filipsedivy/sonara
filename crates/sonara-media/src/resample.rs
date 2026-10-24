//! Streaming resampling with libsoxr's VHQ recipe, call for call as
//! python-soxr 1.1 does it for `soxr.ResampleStream(in_rate, out_rate, channels,
//! dtype=…, quality="VHQ").resample_chunk(block, last=…)` (`src/soxr_ext.cpp`,
//! `CSoxr::process` and `CSoxr::_flush`), in `float32` for the decode cache
//! ([`ResampleStream`]) and in `float64` for the exporter ([`ResampleStream64`]):
//!
//! - the input is fed in pieces of `max(1000, 48000 * in_rate / out_rate)` frames;
//! - the output buffer starts at `soxr_delay() + frames * out_rate / in_rate + 1` frames, sized
//!   in bytes (`sizeof` the sample type) to the next power of two from 1024, and doubles while
//!   libsoxr fills it;
//! - `last` drains the resampler (`soxr_process` with no input) until it returns no frames.
//!
//! libsoxr's output does not depend on how its input is split (checked against python-soxr on
//! the recording host), so the replay matters for the number of frames per call only; it is kept
//! anyway, so that every call sees the same buffer sizes as the prototype's.

use crate::codec_ffi::{Soxr, SoxrLibrary, SoxrSample};
use crate::error::MediaError;

/// A streaming VHQ resampler for interleaved `f32` frames (the decode cache).
pub type ResampleStream = SoxrStream<f32>;

/// A streaming VHQ resampler for interleaved `f64` frames (the exporter's second pass).
pub type ResampleStream64 = SoxrStream<f64>;

/// A streaming VHQ resampler for interleaved frames of `S` (`f32` or `f64`).
pub struct SoxrStream<S: SoxrSample> {
    soxr: Soxr<S>,
    channels: usize,
    /// `out_rate / in_rate`.
    oi_ratio: f64,
    /// Input frames per `soxr_process` call.
    div_len: usize,
    /// python-soxr's `_y_buf`, `_y_buf_bytes` and `_olen` (frames).
    buffer: Vec<S>,
    buffer_bytes: usize,
    olen: usize,
    ended: bool,
}

impl<S: SoxrSample> std::fmt::Debug for SoxrStream<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoxrStream")
            .field("channels", &self.channels)
            .field("oi_ratio", &self.oi_ratio)
            .field("ended", &self.ended)
            .finish_non_exhaustive()
    }
}

impl<S: SoxrSample> SoxrStream<S> {
    /// `sizeof` the sample type: python-soxr sizes its output buffer in bytes.
    const SAMPLE_BYTES: usize = std::mem::size_of::<S>();

    /// A resampler from `in_rate` to `out_rate` Hz for `channels` interleaved channels.
    ///
    /// # Errors
    ///
    /// [`MediaError::InvalidArgument`] for a zero rate or channel count, and
    /// [`MediaError::Codec`] when libsoxr cannot be loaded or rejects the configuration.
    pub fn new(in_rate: u32, out_rate: u32, channels: usize) -> Result<Self, MediaError> {
        if in_rate == 0 || out_rate == 0 {
            return Err(MediaError::InvalidArgument(
                "Sample rate should be over 0".to_owned(),
            ));
        }
        if channels == 0 {
            return Err(MediaError::InvalidArgument(format!(
                "Channel num({channels}) out of limit. Should be in [1, 65536]"
            )));
        }
        let in_rate = f64::from(in_rate);
        let out_rate = f64::from(out_rate);
        let soxr = Soxr::new_vhq(SoxrLibrary::shared()?, in_rate, out_rate, channels)?;
        // `std::max(1000., 48000 * _in_rate / _out_rate)`, stored in a `size_t`.
        let div_len = float_to_len((48_000.0 * in_rate / out_rate).max(1000.0));
        Ok(Self {
            soxr,
            channels,
            oi_ratio: out_rate / in_rate,
            div_len,
            buffer: Vec::new(),
            buffer_bytes: 0,
            olen: 0,
            ended: false,
        })
    }

    /// The version of the loaded libsoxr, for example `"libsoxr-0.1.3"`.
    ///
    /// # Errors
    ///
    /// [`MediaError::Codec`] when libsoxr cannot be loaded.
    pub fn libsoxr_version() -> Result<String, MediaError> {
        Ok(SoxrLibrary::shared()?.version())
    }

    /// The arithmetic engine libsoxr selected, for example `"cr64"` (double precision).
    #[must_use]
    pub fn engine(&self) -> String {
        self.soxr.engine()
    }

    /// The output frames still pending inside the resampler (`soxr_delay`).
    #[must_use]
    pub fn delay(&self) -> f64 {
        self.soxr.delay()
    }

    /// Resamples one block of interleaved frames; with `last`, also drains the resampler.
    /// Returns the interleaved output frames that are ready.
    ///
    /// # Errors
    ///
    /// [`MediaError::InvalidArgument`] for input after the last block or a partial frame, and
    /// [`MediaError::Codec`] with libsoxr's message.
    pub fn resample_chunk(&mut self, input: &[S], last: bool) -> Result<Vec<S>, MediaError> {
        if self.ended {
            return Err(MediaError::InvalidArgument(
                "Input after last input".to_owned(),
            ));
        }
        let channels = self.channels;
        if !input.len().is_multiple_of(channels) {
            return Err(MediaError::InvalidArgument(
                "Channel num mismatch".to_owned(),
            ));
        }
        let ilen = input.len() / channels;
        // `soxr_delay(_soxr) + ilen * _oi_ratio + 1`, stored in a `size_t`.
        let req_len = float_to_len(self.soxr.delay() + len_to_float(ilen) * self.oi_ratio + 1.0);
        self.resize(Self::SAMPLE_BYTES * req_len * channels, false);
        let mut out_pos = 0;
        let mut idx = 0;
        while idx < ilen {
            let frames = self.div_len.min(ilen - idx);
            let piece = &input[idx * channels..(idx + frames) * channels];
            let out = &mut self.buffer[out_pos * channels..self.olen * channels];
            // `process` returns at most the frames that fit, so `out_pos <= olen` holds.
            out_pos += self.soxr.process(Some(piece), out)?;
            if self.olen <= out_pos {
                // The output buffer is full: drain more without new input (python-soxr passes
                // the current input pointer with a length of zero).
                self.flush(Some(&input[idx * channels..idx * channels]), &mut out_pos)?;
            }
            idx += self.div_len;
        }
        if last {
            self.ended = true;
            self.flush(None, &mut out_pos)?;
        }
        Ok(self.buffer[..out_pos * channels].to_vec())
    }

    /// `CSoxr::_flush`: calls `soxr_process` without input until it returns no frames, growing
    /// the buffer whenever it is full. `None` signals the end of the input.
    fn flush(&mut self, input: Option<&[S]>, out_pos: &mut usize) -> Result<(), MediaError> {
        loop {
            if self.olen <= *out_pos {
                self.resize(self.buffer_bytes * 2, true);
            }
            let out = &mut self.buffer[*out_pos * self.channels..self.olen * self.channels];
            let done = self.soxr.process(input, out)?;
            *out_pos += done;
            if done == 0 {
                return Ok(());
            }
        }
    }

    /// `CSoxr::_resize_ybuf`: keeps the buffer when it is larger than `bytes`, otherwise
    /// allocates the next power of two from 1024 bytes, copying the old content if asked.
    fn resize(&mut self, bytes: usize, copy: bool) {
        if self.buffer_bytes > 0 && bytes < self.buffer_bytes {
            return;
        }
        let mut new_bytes = 1024;
        while new_bytes < bytes {
            new_bytes <<= 1;
        }
        let olen = new_bytes / (Self::SAMPLE_BYTES * self.channels);
        let mut buffer = vec![S::default(); olen * self.channels];
        if copy {
            let kept = self.buffer.len().min(buffer.len());
            buffer[..kept].copy_from_slice(&self.buffer[..kept]);
        }
        self.buffer = buffer;
        self.buffer_bytes = new_bytes;
        self.olen = olen;
    }
}

/// A frame count as `double`, as C++ converts `size_t` in arithmetic with doubles.
#[expect(
    clippy::cast_precision_loss,
    reason = "frame counts stay far below 2^53, where the conversion is exact"
)]
fn len_to_float(frames: usize) -> f64 {
    frames as f64
}

/// C++'s conversion of a non-negative `double` to `size_t`: truncation toward zero.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the C++ conversion truncates; the values are non-negative and far below 2^64"
)]
fn float_to_len(value: f64) -> usize {
    value as usize
}

#[cfg(test)]
mod tests {
    use super::{ResampleStream, float_to_len};

    #[test]
    fn piece_length_follows_python_soxr() {
        // max(1000, 48000 * in / out), truncated.
        assert_eq!(
            float_to_len((48_000.0 * 22_050.0 / 192_000.0_f64).max(1000.0)),
            5512
        );
        assert_eq!(
            float_to_len((48_000.0 * 44_100.0 / 48_000.0_f64).max(1000.0)),
            44_100
        );
        assert_eq!(
            float_to_len((48_000.0 * 22_050.0 / 2_000_000.0_f64).max(1000.0)),
            1000
        );
    }

    #[test]
    fn a_stream_ends_with_its_last_block() {
        let mut stream = ResampleStream::new(44_100, 48_000, 2).expect("libsoxr is installed");
        // VHQ needs double precision: `cr64`, or `cr64s` where libsoxr has a SIMD build.
        assert!(stream.engine().starts_with("cr64"), "{}", stream.engine());
        assert!(
            stream.resample_chunk(&[0.0; 3], false).is_err(),
            "partial frame"
        );
        let first = stream.resample_chunk(&vec![0.25; 2 * 4410], false).unwrap();
        let rest = stream.resample_chunk(&[], true).unwrap();
        // 4410 frames at 44.1 kHz are 4800 at 48 kHz, delivered across the two calls.
        let channels = 2;
        assert_eq!((first.len() + rest.len()) / channels, 4800);
        assert!(stream.delay() < 1.0);
        assert!(
            stream.resample_chunk(&[0.0; 2], false).is_err(),
            "after the last block"
        );
        assert!(
            ResampleStream::libsoxr_version()
                .unwrap()
                .starts_with("libsoxr-")
        );
    }

    #[test]
    fn invalid_configurations_are_rejected_before_libsoxr() {
        assert!(ResampleStream::new(0, 48_000, 2).is_err());
        assert!(ResampleStream::new(44_100, 0, 2).is_err());
        assert!(ResampleStream::new(44_100, 48_000, 0).is_err());
    }
}
