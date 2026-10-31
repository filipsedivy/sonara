//! Measures how the notes of a loaded patch sound, from its decoded samples: how long a key press
//! takes to be heard, and how long a held note can last. It reads a [`SampleBank`] that
//! [`super::load_sfz`] built, so it runs off the real-time thread, after the samples are decoded.
//!
//! **Onset.** A region's onset is the time from the key press to the first frame of its sample,
//! counted from its `offset`, whose level on either channel comes within [`ONSET_LEVEL_DB`] of
//! the loudest frame the region plays, plus its `ampeg_attack`. The sample is timed as the
//! sampler steps through it at the region's `pitch_keycenter`, or the key of its range nearest to
//! it, with its `tune`.
//!
//! **Hold.** A region a key press starts plays while the key is held until its sample ends, unless
//! it loops. A held note longer than the longest such region falls silent before its note-off,
//! so [`SoundReport::max_hold`] is that longest time, taken at the lowest key of each region,
//! where the sampler steps through the sample slowest. A region that loops while held, as the
//! sampler loops it, sounds for as long as the key is held, so then there is no limit.

use std::time::Duration;

use sonara_dsp::bank::{LoopMode, Region, Sample, SampleBank};

use super::sounds_on_press;

/// The level, in dB relative to a region's loudest frame, at which its onset is taken.
///
/// Relative, so that a soft velocity layer recorded quietly is measured like a loud one: an
/// absolute level would find no onset, or a late one, in a quiet layer. 20 dB below its peak a
/// note stands clear of a recording's noise floor while a slow bowed or blown attack is still
/// rising, so the measurement grows with the attack a listener hears. The level is an estimate,
/// to be refined by listening; no published method fixes it.
pub const ONSET_LEVEL_DB: f64 = -20.0;

/// What the decoded samples of a patch show about how its notes sound ([`measure`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundReport {
    /// The onset of each region a key press starts with no other key down (an attack or `first`
    /// region) that a press can sound (see [`super::PatchReport::sampled`]), in patch order. A
    /// region whose sample stays silent from its `offset` on has no onset and no entry. `legato`
    /// regions, which a press starts while another key is held, play transitions, not attacks,
    /// and have none either.
    pub onsets: Vec<RegionOnset>,
    /// The longest time a region a key press starts (attack, `first` or `legato`) plays while
    /// the key is held, from its `offset` to its end, at the lowest key it plays. `None` when one
    /// of these regions loops while the key is held, so a note sounds for as long as it is held,
    /// or when none of them plays a frame.
    pub max_hold: Option<Duration>,
}

/// The onset of one region and what selects the region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionOnset {
    /// The switch key the region plays under (SFZ `sw_last`); `None` for a region that plays
    /// under every switch.
    pub switch: Option<u8>,
    /// The lowest velocity of the region.
    pub vel_lo: u8,
    /// The highest velocity of the region.
    pub vel_hi: u8,
    /// The time from the key press to the region's onset, its `ampeg_attack` included.
    pub onset: Duration,
}

/// Measures the onsets and the longest hold of the regions of `bank` that a key press sounds.
///
/// Reads every frame each of those regions plays, so it takes time in proportion to the decoded
/// samples. A sample that cannot be timed (no channel, a rate of 0, a `tune` that is not a
/// number) gives its regions no onset and no hold.
#[must_use]
pub fn measure(bank: &SampleBank) -> SoundReport {
    let mut onsets = Vec::new();
    let mut longest: Option<Duration> = None;
    let mut loops = false;
    for region in &bank.regions {
        let Some(sample) = bank.samples.get(region.sample) else {
            continue;
        };
        if region.key_lo > region.key_hi
            || !sounds_on_press(region, sample.frames(), &bank.keyswitches)
        {
            continue;
        }
        if region.trigger.starts_on_press(false)
            && let Some(onset) = onset(region, sample)
        {
            onsets.push(RegionOnset {
                switch: region.sw_last,
                vel_lo: region.vel_lo,
                vel_hi: region.vel_hi,
                onset,
            });
        }
        if loops_while_held(region, sample) {
            loops = true;
        } else {
            longest = longest.max(hold(region, sample));
        }
    }
    SoundReport {
        onsets,
        max_hold: if loops { None } else { longest },
    }
}

/// The frames `region` plays of `sample`, from its `offset` to its end, each as one slice of
/// interleaved channels; `None` for a sample without channels.
fn played<'a>(
    region: &Region,
    sample: &'a Sample,
) -> Option<impl Iterator<Item = &'a [f32]> + Clone> {
    if sample.channels == 0 {
        return None;
    }
    let start = region.offset;
    let count = end(region, sample).saturating_sub(start);
    Some(
        sample
            .data
            .chunks_exact(sample.channels)
            .skip(start)
            .take(count),
    )
}

/// The frame after the last one `region` plays of `sample`, as the sampler reads its `end`: the
/// sample's own end when the region gives none.
fn end(region: &Region, sample: &Sample) -> usize {
    let frames = sample.frames();
    if region.end == 0 {
        frames
    } else {
        region.end.min(frames)
    }
}

/// The time from a key press to the onset of `region` ([`ONSET_LEVEL_DB`]); `None` when it plays
/// nothing but silence or cannot be timed.
fn onset(region: &Region, sample: &Sample) -> Option<Duration> {
    let mut frames = played(region, sample)?;
    let peak = frames.clone().map(level).fold(0.0, f64::max);
    if peak <= 0.0 {
        return None;
    }
    let threshold = peak * 10_f64.powf(ONSET_LEVEL_DB / 20.0);
    let first = frames.position(|frame| level(frame) >= threshold)?;
    let key = region.pitch_keycenter.clamp(region.key_lo, region.key_hi);
    let seconds = region.attack + to_f64(first) / frame_rate(region, sample, key)?;
    Duration::try_from_secs_f64(seconds).ok()
}

/// How long `region` plays while its key is held, at the lowest key it plays, when it does not
/// loop; `None` when it plays no frame or cannot be timed.
fn hold(region: &Region, sample: &Sample) -> Option<Duration> {
    let frames = end(region, sample).saturating_sub(region.offset);
    if frames == 0 {
        return None;
    }
    let seconds = to_f64(frames) / frame_rate(region, sample, region.key_lo)?;
    Duration::try_from_secs_f64(seconds).ok()
}

/// Whether `region` loops while its key is held, as the sampler decides it: a continuous or
/// sustain loop that ends after it starts and within the sample.
fn loops_while_held(region: &Region, sample: &Sample) -> bool {
    matches!(region.loop_mode, LoopMode::Continuous | LoopMode::Sustain)
        && region.loop_end > region.loop_start
        && region.loop_end <= sample.frames()
}

/// The sample frames a key press of `key` plays per second: the sample's rate times the
/// transposition the sampler gives the region at that key, `tune` included. `None` when that is
/// not a positive, finite number.
fn frame_rate(region: &Region, sample: &Sample, key: u8) -> Option<f64> {
    let cents = (f64::from(key) - f64::from(region.pitch_keycenter)) * 100.0 + region.tune_cents;
    let rate = f64::from(sample.rate) * (cents / 1200.0).exp2();
    (rate.is_finite() && rate > 0.0).then_some(rate)
}

/// The level of one frame: its largest absolute sample over the channels.
fn level(frame: &[f32]) -> f64 {
    frame
        .iter()
        .fold(0.0, |level, sample| level.max(f64::from(sample.abs())))
}

/// A frame count as `f64`; a count beyond `u32::MAX` frames (a day of audio at 48 kHz) is read
/// as `u32::MAX`.
fn to_f64(frames: usize) -> f64 {
    f64::from(u32::try_from(frames).unwrap_or(u32::MAX))
}
