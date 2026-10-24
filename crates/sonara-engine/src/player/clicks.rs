//! Real-time: runs on the audio render thread (see docs/audio-engine.md);
//! [`ClickTables::new`] and [`Metronome::new`] are setup.
//!
//! The metronome (`click` and `_add_clicks` in the prototype's `player.py`). Playback only: it
//! is never exported.
//!
//! - **The clicks** are a decaying sine burst of `int(0.03 · sr)` frames:
//!   `sin(2π f t) · exp(−t / 0.006) · g`, with 1760 Hz and 0.5 on the downbeat (accent) and
//!   1320 Hz and 0.35 otherwise. They are computed once per stream, at its sample rate.
//! - **Placement** is per part of the block: a block assembled from a loop's carry has a
//!   part before the wrap and a part after it, each on its own timeline range, so every pass
//!   clicks on its own beats with its own accent. The prototype placed the clicks of a whole
//!   block on the linear range from its first tag, which misplaces them after a wrap of a loop
//!   that is not aligned to beats. Contiguous parts are merged first, so a block
//!   without a wrap is placed exactly as the prototype places it.
//! - **Mixing:** clicks longer than the rest of the block carry over to the next blocks, and
//!   every click is added to the `f64` block before the `f32` cast.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

use std::f64::consts::PI;

use super::frames::{BLOCK, BLOCK_I64};
use crate::Timeline;

/// The most clicks that sound at once (their tails overlap); more are dropped and counted.
pub(crate) const MAX_PENDING_CLICKS: usize = 32;
/// The most clicks one block starts, kept for inspection ([`super::BlockView::clicks`]).
pub(crate) const MAX_BLOCK_CLICKS: usize = 16;
/// The most parts of one block: the rest of the previous block, a loop's cut block and the
/// first block after the wrap.
pub(crate) const MAX_PARTS: usize = 3;

/// The prototype's `click(sample_rate, accent)`: the sine burst of the downbeat (`accent`) or of
/// the other beats, as `f64` with NumPy's order of operations: `t = i / sr`, then
/// `sin(((2π) · f) · t) · exp(−t / 0.006) · g`.
///
/// Setup: allocates.
#[must_use]
#[expect(
    clippy::disallowed_methods,
    reason = "setup: runs before the stream starts, off the real-time thread"
)]
pub fn click(sample_rate: u32, accent: bool) -> Vec<f64> {
    let rate = f64::from(sample_rate);
    // Python's `int(0.03 * sample_rate)` truncates.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive length truncated as Python's int() does"
    )]
    let frames = (0.03 * rate) as usize;
    let (frequency, gain) = if accent {
        (1760.0, 0.5)
    } else {
        (1320.0, 0.35)
    };
    let omega = 2.0 * PI * frequency;
    let mut samples = Vec::with_capacity(frames);
    for i in 0..frames {
        #[expect(
            clippy::cast_precision_loss,
            reason = "frame indices stay far below 2^53, where the conversion is exact"
        )]
        let t = i as f64 / rate;
        samples.push((omega * t).sin() * (-t / 0.006).exp() * gain);
    }
    samples
}

/// The two clicks of a stream, computed at its sample rate.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClickTables {
    accent: Box<[f64]>,
    beat: Box<[f64]>,
}

impl ClickTables {
    /// Setup: computes both clicks at `sample_rate`.
    pub(crate) fn new(sample_rate: u32) -> Self {
        Self {
            accent: click(sample_rate, true).into_boxed_slice(),
            beat: click(sample_rate, false).into_boxed_slice(),
        }
    }

    fn samples(&self, accent: bool) -> &[f64] {
        if accent { &self.accent } else { &self.beat }
    }
}

/// A part of an output block: frames from one contiguous timeline range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Part {
    /// The timeline frame of its first frame.
    pub(crate) tag: i64,
    /// Its frames.
    pub(crate) frames: usize,
}

/// A click: its offset from the start of the block it plays in (negative once it started in an
/// earlier block) and whether it is the downbeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Click {
    pub(crate) offset: i64,
    pub(crate) accent: bool,
}

/// The clicks sounding into the next blocks, and the clicks the last block started.
#[derive(Debug)]
pub(crate) struct Metronome {
    pending: [Click; MAX_PENDING_CLICKS],
    pending_len: usize,
    started: [Click; MAX_BLOCK_CLICKS],
    started_len: usize,
    /// Clicks dropped because more than [`MAX_PENDING_CLICKS`] sounded at once (RT-08).
    dropped: u64,
}

impl Metronome {
    /// Setup: no clicks.
    pub(crate) fn new() -> Self {
        Self {
            pending: [Click::default(); MAX_PENDING_CLICKS],
            pending_len: 0,
            started: [Click::default(); MAX_BLOCK_CLICKS],
            started_len: 0,
            dropped: 0,
        }
    }

    /// Forgets the clicks that still sound (a start or a seek: `_pending_clicks.clear()`).
    pub(crate) fn clear(&mut self) {
        self.pending_len = 0;
        self.started_len = 0;
    }

    /// The clicks the last [`Metronome::add`] started, with their offsets in that block.
    pub(crate) fn started(&self) -> &[Click] {
        self.started.get(..self.started_len).unwrap_or(&[])
    }

    /// Clicks dropped so far.
    pub(crate) fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Places the clicks of the block's `parts` (merged where contiguous) and adds every
    /// sounding click to `out`, the block before its `f32` cast.
    pub(crate) fn add(
        &mut self,
        timeline: &Timeline,
        parts: &[Part],
        tables: &ClickTables,
        out: &mut [[f64; 2]],
    ) {
        self.started_len = 0;
        let mut offset = 0_i64;
        for part in parts {
            self.place(timeline, offset, *part);
            offset = offset.saturating_add(i64::try_from(part.frames).unwrap_or(BLOCK_I64));
        }
        self.mix(tables, out);
    }

    /// `_add_clicks` on one part: the beats in `[beat(max(0, tag)), beat(max(0, tag + frames)))`,
    /// from `ceil(start − 1e-9)` in steps of `4 / denominator`, at `offset + frame(b) − tag`.
    fn place(&mut self, timeline: &Timeline, offset: i64, part: Part) {
        let frames = i64::try_from(part.frames).unwrap_or(BLOCK_I64);
        let start_beat = timeline.frame_to_beat(part.tag.max(0));
        let end_beat = timeline.frame_to_beat(part.tag.saturating_add(frames).max(0));
        let signatures = timeline.time_signatures();
        let mut beat = (start_beat - 1e-9).ceil();
        while beat < end_beat {
            let frame = offset
                .saturating_add(timeline.beat_to_frame(beat))
                .saturating_sub(part.tag);
            let signature = signatures.at_beat(beat);
            let bar_start = signatures.bar_to_beat((signatures.beat_to_bar(beat) + 1e-9).floor());
            let click = Click {
                offset: frame,
                accent: (beat - bar_start).abs() < 1e-6,
            };
            if let Some(slot) = self.started.get_mut(self.started_len) {
                *slot = click;
                self.started_len += 1;
            }
            match self.pending.get_mut(self.pending_len) {
                Some(slot) => {
                    *slot = click;
                    self.pending_len += 1;
                }
                None => self.dropped = self.dropped.wrapping_add(1),
            }
            beat += 4.0 / f64::from(signature.denominator);
        }
    }

    /// Adds the sounding clicks to `out`, in the order they started, and keeps the ones that
    /// continue into the next block.
    fn mix(&mut self, tables: &ClickTables, out: &mut [[f64; 2]]) {
        let mut kept = 0_usize;
        for index in 0..self.pending_len {
            let Some(click) = self.pending.get(index).copied() else {
                break;
            };
            let samples = tables.samples(click.accent);
            let length = i64::try_from(samples.len()).unwrap_or(i64::MAX);
            let first = click.offset.max(0);
            let last = click.offset.saturating_add(length).min(BLOCK_I64);
            if last > first {
                let (Ok(a), Ok(z), Ok(from)) = (
                    usize::try_from(first),
                    usize::try_from(last),
                    usize::try_from(first - click.offset),
                ) else {
                    continue;
                };
                let source = samples.get(from..).unwrap_or(&[]);
                for (frame, value) in out.get_mut(a..z).unwrap_or(&mut []).iter_mut().zip(source) {
                    frame[0] += *value;
                    frame[1] += *value;
                }
            }
            if click.offset.saturating_add(length) > BLOCK_I64 {
                if let Some(slot) = self.pending.get_mut(kept) {
                    *slot = Click {
                        offset: click.offset - BLOCK_I64,
                        accent: click.accent,
                    };
                }
                kept += 1;
            }
        }
        self.pending_len = kept;
    }
}

const _: () = assert!(BLOCK == 512);

#[cfg(test)]
mod tests {
    use sonara_core::model::project::{Project, ProjectData};
    use sonara_core::time::{TempoMap, TimeSignatureMap, TimeSignaturePoint};

    use super::{Click, ClickTables, Metronome, Part, click};
    use crate::Timeline;

    fn timeline(bpm: f64, numerator: u32) -> Timeline {
        let mut data = ProjectData::new("prj_1", "Clicks");
        data.tempo_map = TempoMap::constant(bpm).unwrap();
        data.time_signatures =
            TimeSignatureMap::new([TimeSignaturePoint::new(0, numerator, 4).unwrap()].to_vec())
                .unwrap();
        Timeline::new(&Project::new(data).unwrap(), 48_000)
    }

    #[test]
    fn clicks_are_thirty_milliseconds_long() {
        assert_eq!(click(48_000, true).len(), 1440);
        assert_eq!(click(44_100, false).len(), 1323);
        let accent = click(48_000, true);
        assert_eq!(accent.first(), Some(&0.0));
        assert!(accent.iter().all(|x| x.abs() <= 0.5));
    }

    /// D8 on the golden `loop-metronome` loop: the wrap block holds 324 frames from 143 576 and
    /// 188 frames from the loop start 95 960. The prototype placed one unaccented click at 424
    /// (beat 6, after the loop end); per part, the downbeat of bar 2 (beat 4 at 96 000) clicks
    /// accented at 324 + 40 = 364.
    #[test]
    fn clicks_follow_the_parts_of_a_wrap_block() {
        let timeline = timeline(120.0, 4);
        let tables = ClickTables::new(48_000);
        let mut metronome = Metronome::new();
        let mut out = [[0.0; 2]; 512];
        let parts = [
            Part {
                tag: 143_576,
                frames: 324,
            },
            Part {
                tag: 95_960,
                frames: 188,
            },
        ];
        metronome.add(&timeline, &parts, &tables, &mut out);
        assert_eq!(
            metronome.started(),
            [Click {
                offset: 364,
                accent: true
            }]
        );
        assert_eq!(out[363], [0.0; 2]);
        assert_eq!(Some(&out[365][0]), tables.samples(true).get(1));
        // The tail continues into the next blocks, then stops.
        let mut next = [[0.0; 2]; 512];
        metronome.add(
            &timeline,
            &[Part {
                tag: 96_148,
                frames: 512,
            }],
            &tables,
            &mut next,
        );
        assert_eq!(metronome.started(), []);
        assert_eq!(Some(&next[0][1]), tables.samples(true).get(148));
        for tag in [96_660, 97_172] {
            metronome.add(&timeline, &[Part { tag, frames: 512 }], &tables, &mut next);
        }
        assert_eq!(
            metronome.pending_len, 0,
            "1440 frames from offset 364 end in the fourth block"
        );
    }

    /// Without a wrap, a block's clicks are the prototype's: in 3/4 every third beat is a
    /// downbeat, and a click before the block start does not click again.
    #[test]
    fn contiguous_blocks_click_on_every_beat() {
        let timeline = timeline(150.0, 3);
        let tables = ClickTables::new(48_000);
        let mut metronome = Metronome::new();
        let mut placed = Vec::new();
        for block in 0..200_i64 {
            let mut out = [[0.0; 2]; 512];
            let tag = block * 512;
            metronome.add(&timeline, &[Part { tag, frames: 512 }], &tables, &mut out);
            placed.extend(
                metronome
                    .started()
                    .iter()
                    .map(|c| (tag + c.offset, c.accent)),
            );
        }
        // 150 BPM: a beat is 19 200 frames.
        let expected: Vec<(i64, bool)> = (0..6).map(|b| (b * 19_200, b % 3 == 0)).collect();
        assert_eq!(placed, expected);
    }
}
