//! Audio clip playback (`AudioClipSource` and `fade_curve` in `audio/engine/graph.py`).
//!
//! Not real-time code: the media readers do file I/O behind a lock (`sonara-media`), which
//! RT-03 forbids on the render thread. The graph therefore reads every audio track's clips for a
//! block before the processing path runs ([`crate::RenderGraph::read_media`]), into a buffer
//! the processing path only adds.
//!
//! Per clip, in start order: muted clips are skipped; `source_start = round(source_offset · sr
//! / asset_sr)`; the fades are `round(seconds · sr)` frames; a mono source is duplicated, a
//! source with more than two channels folds the even channels to the left and the odd ones to
//! the right as means; the gain per frame is `(gain · fade_in) · fade_out` with the curves
//! linear `x`, equal power `sin(x π/2)`, exponential `x²` and S-curve `0.5 − 0.5 cos(π x)`.
//! Audio clips do not loop.
//!
//! In real-time playback the player's prefetch thread owns the sources of the playing graph:
//! a live graph hands its sources over
//! ([`crate::RenderGraph::take_clip_sources`]), and [`AudioClipSource::signature`] tells
//! whether a rebuilt graph plays the same clips, so the prefetched audio stays valid.

use std::f64::consts::PI;
use std::sync::{Arc, MutexGuard, PoisonError};

use sonara_core::model::clip::FadeCurve;
use sonara_core::model::project::Project;
use sonara_core::model::track::{Track, db_to_gain};
use sonara_core::time::python_round;
use sonara_media::{MediaProvider, MediaReader, SharedReader};

use crate::EngineError;

/// One clip, in timeline frames at the project rate.
#[derive(Debug)]
struct ClipPlayback {
    asset_id: String,
    start: i64,
    end: i64,
    /// The reader frame that plays at `start`.
    source_start: i64,
    gain: f64,
    fade_in: i64,
    fade_in_curve: FadeCurve,
    fade_out: i64,
    fade_out_curve: FadeCurve,
    reader: SharedReader,
    channels: usize,
}

/// What makes two clips render the same audio: every field of a [`ClipPlayback`], the gain by
/// its bits and the reader by identity.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ClipSignature {
    asset_id: String,
    start: i64,
    end: i64,
    source_start: i64,
    gain: u64,
    fade_in: i64,
    fade_in_curve: FadeCurve,
    fade_out: i64,
    fade_out_curve: FadeCurve,
    reader: usize,
    channels: usize,
}

/// The clips of an [`AudioClipSource`] for comparison: two sources with equal signatures render
/// the same audio for every frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClipsSignature(Vec<ClipSignature>);

/// The audio clips of one audio track.
#[derive(Debug)]
pub(crate) struct AudioClipSource {
    /// Clips, stably sorted by start.
    clips: Vec<ClipPlayback>,
    /// The longest clip in frames.
    max_len: i64,
    /// One block of interleaved reader frames, for the widest source.
    scratch: Vec<f64>,
}

impl AudioClipSource {
    /// The clips of `track`, with the asset IDs whose media is missing: clips whose asset is
    /// not in the project (those are dropped) and assets whose file is missing or unreadable
    /// (those play silence).
    ///
    /// Setup: opens the readers, which may decode the media into the cache first.
    pub(crate) fn new(
        track: &Track,
        project: &Project,
        media: &MediaProvider,
        block_size: usize,
    ) -> (Self, Vec<String>) {
        let sample_rate = project.settings.sample_rate;
        let tempo = &project.tempo_map;
        let mut missing = Vec::new();
        let mut clips = Vec::new();
        for clip in track.audio_clips().filter(|clip| !clip.muted) {
            let Some(asset) = project.find_asset(&clip.asset_id) else {
                missing.push(clip.asset_id.clone());
                continue;
            };
            let reader = media.reader(asset, sample_rate);
            let (channels, silent) = {
                let reader = lock(&reader);
                (reader.channels(), reader.is_missing())
            };
            if silent {
                missing.push(asset.id.clone());
            }
            clips.push(ClipPlayback {
                asset_id: asset.id.clone(),
                start: tempo.beat_to_sample(clip.start, sample_rate),
                end: tempo.beat_to_sample(clip.start + clip.length, sample_rate),
                source_start: source_start(clip.source_offset, sample_rate, asset.sample_rate),
                gain: db_to_gain(clip.gain_db),
                fade_in: fade_frames(clip.fade_in.seconds, sample_rate),
                fade_in_curve: clip.fade_in.curve,
                fade_out: fade_frames(clip.fade_out.seconds, sample_rate),
                fade_out_curve: clip.fade_out.curve,
                reader,
                channels: channels.max(1),
            });
        }
        clips.sort_by_key(|clip| clip.start);
        let max_len = clips
            .iter()
            .map(|clip| clip.end - clip.start)
            .max()
            .unwrap_or(0);
        let widest = clips.iter().map(|clip| clip.channels).max().unwrap_or(1);
        let source = Self {
            clips,
            max_len,
            scratch: vec![0.0; widest.saturating_mul(block_size)],
        };
        (source, missing)
    }

    /// What the source renders, for telling whether a rebuilt graph plays the same clips.
    pub(crate) fn signature(&self) -> ClipsSignature {
        ClipsSignature(
            self.clips
                .iter()
                .map(|clip| ClipSignature {
                    asset_id: clip.asset_id.clone(),
                    start: clip.start,
                    end: clip.end,
                    source_start: clip.source_start,
                    gain: clip.gain.to_bits(),
                    fade_in: clip.fade_in,
                    fade_in_curve: clip.fade_in_curve,
                    fade_out: clip.fade_out,
                    fade_out_curve: clip.fade_out_curve,
                    reader: Arc::as_ptr(&clip.reader).addr(),
                    channels: clip.channels,
                })
                .collect(),
        )
    }

    /// Adds the clips' audio for the frames `[frame, frame + out.len())` to `out`.
    ///
    /// # Errors
    ///
    /// [`EngineError::Media`] when a reader fails.
    pub(crate) fn render(&mut self, frame: i64, out: &mut [[f64; 2]]) -> Result<(), EngineError> {
        let frames = i64::try_from(out.len()).unwrap_or(i64::MAX);
        let block_end = frame.saturating_add(frames);
        let lo = self
            .clips
            .partition_point(|clip| clip.start < frame.saturating_sub(self.max_len));
        let hi = self
            .clips
            .partition_point(|clip| clip.start < block_end)
            .max(lo);
        for clip in &self.clips[lo..hi] {
            // The window `[first, last)` of the clip inside the block.
            let first = frame.max(clip.start);
            let last = block_end.min(clip.end);
            if last <= first {
                continue;
            }
            let (Ok(offset), Ok(count)) = (
                usize::try_from(first - frame),
                usize::try_from(last - first),
            ) else {
                continue;
            };
            let window = &mut self.scratch[..count * clip.channels];
            lock(&clip.reader)
                .read_into(clip.source_start + (first - clip.start), window)
                .map_err(|source| EngineError::Media {
                    asset: clip.asset_id.clone(),
                    source,
                })?;
            let sources = window.chunks_exact(clip.channels);
            let targets = &mut out[offset..offset + count];
            for ((pos, target), source) in (first..last).zip(targets).zip(sources) {
                let gain = clip.gain_at(pos, first, last);
                let [left, right] = stereo(source);
                target[0] += left * gain;
                target[1] += right * gain;
            }
        }
        Ok(())
    }
}

impl ClipPlayback {
    /// The gain of timeline frame `pos` in a window `[first, last)`. A fade applies to the
    /// whole window when any of it lies inside the fade, as in the prototype (outside the fade
    /// its curve gives exactly 1).
    fn gain_at(&self, pos: i64, first: i64, last: i64) -> f64 {
        let mut gain = self.gain;
        if self.fade_in > 0 && first < self.start + self.fade_in {
            gain *= fade_curve(ratio(pos - self.start, self.fade_in), self.fade_in_curve);
        }
        if self.fade_out > 0 && last > self.end - self.fade_out {
            gain *= fade_curve(ratio(self.end - pos, self.fade_out), self.fade_out_curve);
        }
        gain
    }
}

/// A frame of the source as stereo: mono duplicated, more than two channels folded (even
/// channels to the left, odd ones to the right, each the sequential mean, as NumPy's
/// `mean(axis=1)` over the strided view computes it).
fn stereo(frame: &[f64]) -> [f64; 2] {
    match frame {
        [mono] => [*mono, *mono],
        [l, r] => [*l, *r],
        _ => [
            mean(frame.iter().step_by(2)),
            mean(frame.iter().skip(1).step_by(2)),
        ],
    }
}

/// The sequential mean of `values`, from the first value on.
fn mean<'a>(mut values: impl Iterator<Item = &'a f64>) -> f64 {
    let Some(&first) = values.next() else {
        return 0.0;
    };
    let (sum, count) = values.fold((first, 1_u32), |(sum, count), v| (sum + v, count + 1));
    sum / f64::from(count)
}

/// Fade-in gain at normalized position `x` (`fade_curve`); fade-outs pass `(end − pos) / len`.
fn fade_curve(x: f64, curve: FadeCurve) -> f64 {
    let x = x.clamp(0.0, 1.0);
    match curve {
        FadeCurve::EqualPower => (x * (PI / 2.0)).sin(),
        FadeCurve::Exponential => x * x,
        FadeCurve::SCurve => 0.5 - 0.5 * (PI * x).cos(),
        FadeCurve::Linear => x,
    }
}

/// NumPy's `int64 / int` true division for frame counts.
#[expect(
    clippy::cast_precision_loss,
    reason = "frame counts stay far below 2^53, where the conversion is exact"
)]
fn ratio(numerator: i64, denominator: i64) -> f64 {
    numerator as f64 / denominator as f64
}

/// `round(source_offset * sample_rate / asset_rate)`: Python divides the exact integer product
/// with one rounding, which `f64` reproduces while the product stays below 2^53.
fn source_start(source_offset: i64, sample_rate: u32, asset_rate: u32) -> i64 {
    if asset_rate == 0 {
        // Audio assets always have a rate; the prototype would divide by zero here.
        return 0;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "exact below 2^53, see the function documentation"
    )]
    let product = (i128::from(source_offset) * i128::from(sample_rate)) as f64;
    python_round(product / f64::from(asset_rate))
}

/// `round(seconds * sample_rate)`.
fn fade_frames(seconds: f64, sample_rate: u32) -> i64 {
    python_round(seconds * f64::from(sample_rate))
}

/// Locks a reader, taking over its state if another thread panicked while holding it (a reader
/// is always in a usable state between calls).
fn lock(reader: &SharedReader) -> MutexGuard<'_, MediaReader> {
    reader.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use sonara_core::model::clip::FadeCurve;

    use super::{fade_curve, mean, source_start, stereo};

    #[test]
    fn fade_curves_reach_unity_and_clip() {
        for curve in [
            FadeCurve::Linear,
            FadeCurve::EqualPower,
            FadeCurve::Exponential,
            FadeCurve::SCurve,
        ] {
            assert_eq!(fade_curve(1.0, curve), 1.0, "{curve:?}");
            assert_eq!(fade_curve(2.5, curve), 1.0, "{curve:?}");
            assert_eq!(fade_curve(-1.0, curve), 0.0, "{curve:?}");
        }
        assert_eq!(fade_curve(0.5, FadeCurve::Exponential), 0.25);
        assert_eq!(
            fade_curve(0.5, FadeCurve::SCurve),
            0.5 - 0.5 * (std::f64::consts::PI * 0.5).cos()
        );
    }

    #[test]
    fn channels_fold_like_the_prototype() {
        assert_eq!(stereo(&[0.5]), [0.5, 0.5]);
        assert_eq!(stereo(&[0.1, 0.2]), [0.1, 0.2]);
        // Sums of dyadic values are exact, so the expected means are too.
        let five = [0.5, 0.25, 1.0, 0.75, 2.0];
        assert_eq!(stereo(&five), [3.5 / 3.0, 0.5]);
        assert_eq!(mean([].iter()), 0.0);
    }

    #[test]
    fn source_offsets_scale_with_ties_to_even() {
        assert_eq!(source_start(11_025, 48_000, 44_100), 12_000);
        // 1 · 48000 / 96000 = 0.5 rounds to 0, 3 · 48000 / 96000 = 1.5 to 2.
        assert_eq!(source_start(1, 48_000, 96_000), 0);
        assert_eq!(source_start(3, 48_000, 96_000), 2);
        assert_eq!(source_start(5, 48_000, 0), 0);
    }
}
