//! What the check of a patch finds: facts read from the patch file and the headers of its samples
//! (`sonara_media::sfz::inspect`), without decoding audio, and what decoding the samples measured
//! (`sonara_media::sfz::measure`, [`SoundFacts`]). A manifest or a profile makes claims; these
//! are facts, so no claim overrides them, and where the two disagree the facts lower the preset's
//! verdict ([`super::InstrumentPreset::effective_verdict`]). The one claim that replaces a
//! measurement is an articulation's stated `attack_ms`
//! ([`super::InstrumentPreset::articulation_attacks`]): an onset depends on the level at which
//! it is taken, and the library knows its recordings better than one level does.

use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use sonara_media::sfz::{MAX_BANK_BYTES, PatchReport, SoundReport};

use super::manifest::{ArticulationEntry, ArticulationVia, VerdictState};

/// What the structural check found in a patch, copied from the SFZ reader's report so that the
/// catalog can serialize it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PatchFacts {
    /// The `<region>` headers after the includes are read, whether they can play or not.
    pub regions: usize,
    /// The regions the sampler loads: those whose sample is there and readable, or `*silence`.
    pub playable_regions: usize,
    /// The keys a press sounds on, ascending, as runs of neighbouring keys with the same velocity
    /// layers.
    pub sampled: Vec<KeyZone>,
    /// The keys that switch articulations, ascending.
    pub switch_keys: Vec<u8>,
    /// The switch the patch starts on (SFZ `sw_default`), if it names one.
    pub default_switch: Option<u8>,
    /// What the patch writes that the sampler does not play: unsupported opcodes and values,
    /// and skipped headers.
    pub ignored_opcodes: Vec<String>,
    /// The sample files the regions name that do not exist; their regions are left out.
    pub missing_samples: Vec<PathBuf>,
    /// The sample files that exist but cannot be read, or are not mono or stereo; the patch
    /// does not load while one is there.
    pub unreadable_samples: Vec<PathBuf>,
    /// One note per sample left out, with the number of its regions, in words for the user.
    pub notes: Vec<String>,
    /// The memory the decoded samples would take, in bytes, each file counted once.
    pub decoded_bytes: usize,
    /// Whether the patch is expected to load: a region can play, no sample file is unreadable,
    /// and the decoded samples fit the sampler's memory cap. Loading can still fail on a sample
    /// whose header reads but whose audio does not decode.
    pub loadable: bool,
    /// What decoding the samples measured, when the check loaded the patch with a loader that
    /// measures ([`super::PatchLoader::load`]); `None` otherwise, and for a patch that does not
    /// load.
    pub sound: Option<SoundFacts>,
}

/// What decoding the samples of a patch measured: how long its notes take to be heard and how
/// long one can be held (`sonara_media::sfz::measure`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SoundFacts {
    /// The attack of each region a key press starts with no other key down, in patch order: a
    /// region that a press sounds and whose sample is not silent. Its onset is the first frame
    /// that comes within 20 dB of the region's loudest one (`sonara_media::sfz::ONSET_LEVEL_DB`).
    pub attacks: Vec<RegionAttack>,
    /// The longest time a region a key press starts plays while the key is held, at the lowest
    /// key it plays, in milliseconds rounded up, at least 1: a note held longer falls silent
    /// before its note-off. `None` when one of those regions loops while held, so a note sounds
    /// as long as it is held, or when no region sounds on a press.
    pub max_hold_ms: Option<u32>,
}

/// The attack of one region and what selects the region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RegionAttack {
    /// The switch key the region plays under (SFZ `sw_last`); `None` for a region that plays
    /// under every switch.
    pub switch: Option<u8>,
    /// The velocities the region plays at.
    pub velocities: VelocityRange,
    /// The time from the key press to the region's onset, its SFZ `ampeg_attack` included, in
    /// milliseconds rounded to the nearest.
    pub attack_ms: u32,
}

/// A run of neighbouring keys that sound with the same velocity layers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyZone {
    /// The lowest key of the run.
    pub lo: u8,
    /// The highest key of the run.
    pub hi: u8,
    /// The velocity ranges within which the same regions play, ascending.
    pub layers: Vec<VelocityRange>,
}

/// The velocities `lo..=hi`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct VelocityRange {
    /// The lowest velocity.
    pub lo: u8,
    /// The highest velocity.
    pub hi: u8,
}

impl From<&PatchReport> for PatchFacts {
    fn from(report: &PatchReport) -> Self {
        Self {
            regions: report.regions,
            playable_regions: report.playable_regions,
            sampled: report
                .sampled
                .iter()
                .map(|zone| KeyZone {
                    lo: *zone.keys.start(),
                    hi: *zone.keys.end(),
                    layers: zone
                        .layers
                        .iter()
                        .map(|layer| VelocityRange {
                            lo: layer.lo,
                            hi: layer.hi,
                        })
                        .collect(),
                })
                .collect(),
            switch_keys: report.keyswitches.keys().collect(),
            default_switch: report.keyswitches.default,
            ignored_opcodes: report.ignored_opcodes.clone(),
            missing_samples: report.missing_samples.clone(),
            unreadable_samples: report.unreadable_samples.clone(),
            notes: report.notes.clone(),
            decoded_bytes: report.decoded_bytes,
            loadable: report.is_loadable(),
            sound: None,
        }
    }
}

impl From<&SoundReport> for SoundFacts {
    fn from(report: &SoundReport) -> Self {
        Self {
            attacks: report
                .onsets
                .iter()
                .map(|onset| RegionAttack {
                    switch: onset.switch,
                    velocities: VelocityRange {
                        lo: onset.vel_lo,
                        hi: onset.vel_hi,
                    },
                    attack_ms: millis(onset.onset, false),
                })
                .collect(),
            // A region that plays a frame plays for some time, however short.
            max_hold_ms: report.max_hold.map(|hold| millis(hold, true).max(1)),
        }
    }
}

impl SoundFacts {
    /// The attack of the articulation that `via` reaches, in milliseconds: the median of the
    /// attacks of the regions that play it, the mean of the two middle ones rounded up when
    /// their number is even. `via` [`ArticulationVia::Patch`] takes every region; a key switch
    /// the regions that need it and those that play under every switch; a velocity zone the
    /// regions whose velocities overlap it. `None` when no region plays it, and for an
    /// articulation a controller selects: a controller fades or gates the regions in ways the
    /// regions alone do not say.
    pub(crate) fn attack_ms(&self, via: ArticulationVia) -> Option<u32> {
        let plays = |region: &&RegionAttack| match via {
            ArticulationVia::Patch => true,
            ArticulationVia::Keyswitch { key } => region.switch.is_none_or(|switch| switch == key),
            ArticulationVia::Velocity { lo, hi } => {
                region.velocities.lo <= hi && lo <= region.velocities.hi
            }
            ArticulationVia::Cc { .. } => false,
        };
        let mut times: Vec<u32> = self
            .attacks
            .iter()
            .filter(plays)
            .map(|region| region.attack_ms)
            .collect();
        times.sort_unstable();
        let middle = times.len() / 2;
        let upper = *times.get(middle)?;
        if times.len() % 2 == 1 {
            return Some(upper);
        }
        // An even number, at least two, so the lower middle exists.
        let lower = *times.get(middle.checked_sub(1)?)?;
        Some(lower + (upper - lower).div_ceil(2))
    }
}

/// `duration` in whole milliseconds, rounded up when `up`, else to the nearest; a duration of
/// more than `u32::MAX` milliseconds (49 days) is read as that.
fn millis(duration: Duration, up: bool) -> u32 {
    let micros = duration.as_micros();
    let ms = if up {
        micros.div_ceil(1000)
    } else {
        (micros + 500) / 1000
    };
    u32::try_from(ms).unwrap_or(u32::MAX)
}

impl PatchFacts {
    /// What these facts take from a preset whose patch is claimed to play `articulations`: each
    /// finding with the best verdict it leaves, in words for the user, in this order: a patch
    /// that does not load ([`VerdictState::Not`]), the notes of the samples left out, and each
    /// claimed key switch that is not a switch key of the patch ([`VerdictState::Partly`]). The
    /// unsupported opcodes alone lower nothing: what they cost is the claim's to say.
    pub(crate) fn findings<'a>(
        &self,
        articulations: impl IntoIterator<Item = &'a ArticulationEntry>,
    ) -> Vec<(VerdictState, String)> {
        let mut findings = Vec::new();
        if self.regions == 0 {
            findings.push((VerdictState::Not, "the patch has no region".to_owned()));
        } else if self.playable_regions == 0 {
            findings.push((
                VerdictState::Not,
                "no region of the patch can play".to_owned(),
            ));
        }
        if !self.unreadable_samples.is_empty() {
            findings.push((
                VerdictState::Not,
                "the patch does not load while a sample file cannot be read".to_owned(),
            ));
        }
        if self.decoded_bytes > MAX_BANK_BYTES {
            findings.push((
                VerdictState::Not,
                format!(
                    "its samples take {} MiB decoded, more than the {} MiB a patch may take",
                    self.decoded_bytes.div_ceil(1 << 20),
                    MAX_BANK_BYTES >> 20
                ),
            ));
        }
        findings.extend(
            self.notes
                .iter()
                .map(|note| (VerdictState::Partly, note.clone())),
        );
        for articulation in articulations {
            if let ArticulationVia::Keyswitch { key } = articulation.via
                && !self.switch_keys.contains(&key)
            {
                findings.push((
                    VerdictState::Partly,
                    format!(
                        "{}: key {key} is not a switch key of the patch",
                        articulation.name.id()
                    ),
                ));
            }
        }
        findings
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sonara_media::sfz::SoundReport;

    use super::{RegionAttack, SoundFacts, VelocityRange, millis};
    use crate::instruments::ArticulationVia;

    fn attacks(times: &[u32]) -> SoundFacts {
        SoundFacts {
            attacks: times
                .iter()
                .map(|&attack_ms| RegionAttack {
                    switch: None,
                    velocities: VelocityRange { lo: 0, hi: 127 },
                    attack_ms,
                })
                .collect(),
            max_hold_ms: None,
        }
    }

    /// The median of an odd number of regions is the middle one; of an even number, the mean of
    /// the two middle ones rounded up. No region, or an articulation a controller selects, has
    /// no attack (TST-10, TST-03).
    #[test]
    fn an_articulation_takes_the_median_of_its_regions() {
        let patch = ArticulationVia::Patch;
        assert_eq!(attacks(&[90, 5, 30]).attack_ms(patch), Some(30));
        assert_eq!(attacks(&[13, 10]).attack_ms(patch), Some(12));
        assert_eq!(attacks(&[10, 10, 500, 7]).attack_ms(patch), Some(10));
        assert_eq!(
            attacks(&[u32::MAX, u32::MAX]).attack_ms(patch),
            Some(u32::MAX)
        );
        assert_eq!(attacks(&[]).attack_ms(patch), None);
        assert_eq!(
            attacks(&[10]).attack_ms(ArticulationVia::Cc { cc: 1, value: 0 }),
            None
        );
        assert_eq!(
            attacks(&[10]).attack_ms(ArticulationVia::Keyswitch { key: 30 }),
            Some(10),
            "a region under every switch plays under this one"
        );
        let mut switched = attacks(&[10]);
        switched.attacks[0].switch = Some(31);
        assert_eq!(
            switched.attack_ms(ArticulationVia::Keyswitch { key: 30 }),
            None
        );
        assert_eq!(
            attacks(&[10]).attack_ms(ArticulationVia::Velocity { lo: 127, hi: 127 }),
            Some(10)
        );
        let mut soft = attacks(&[10]);
        soft.attacks[0].velocities = VelocityRange { lo: 1, hi: 63 };
        assert_eq!(
            soft.attack_ms(ArticulationVia::Velocity { lo: 64, hi: 127 }),
            None
        );
    }

    /// Attacks round to the nearest millisecond, holds up, both from whole microseconds, so a
    /// time that floating point puts a nanosecond past a millisecond is not a millisecond longer.
    /// A hold is at least 1 ms (TST-07).
    #[test]
    fn times_round_to_whole_milliseconds() {
        let hold = |hold| {
            SoundFacts::from(&SoundReport {
                onsets: Vec::new(),
                max_hold: Some(hold),
            })
            .max_hold_ms
        };
        assert_eq!(hold(Duration::from_nanos(15)), Some(1));
        assert_eq!(hold(Duration::from_millis(2_670)), Some(2_670));
        assert_eq!(millis(Duration::from_micros(1_499), false), 1);
        assert_eq!(millis(Duration::from_micros(1_500), false), 2);
        assert_eq!(millis(Duration::from_micros(1_001), true), 2);
        assert_eq!(millis(Duration::from_micros(22), true), 1);
        assert_eq!(millis(Duration::from_nanos(100_000_001), true), 100);
        assert_eq!(millis(Duration::from_nanos(99_999_999), true), 100);
        assert_eq!(millis(Duration::MAX, false), u32::MAX);
    }
}
