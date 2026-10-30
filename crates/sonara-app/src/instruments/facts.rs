//! What the structural check of a patch finds: facts read from the patch file and the headers of
//! its samples (`sonara_media::sfz::inspect`), without decoding audio. A manifest or a profile
//! makes claims; these are facts, so no claim overrides them, and where the two disagree the
//! facts lower the preset's verdict ([`super::InstrumentPreset::effective_verdict`]).

use std::path::PathBuf;

use serde::Serialize;
use sonara_media::sfz::{MAX_BANK_BYTES, PatchReport};

use super::manifest::{ArticulationList, ArticulationVia, VerdictState};

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
        }
    }
}

impl PatchFacts {
    /// What these facts take from a preset that claims `articulations`: each finding with the
    /// best verdict it leaves, in words for the user, in this order: a patch that does not load
    /// ([`VerdictState::Not`]), the notes of the samples left out, and each claimed key switch
    /// that is not a switch key of the patch ([`VerdictState::Partly`]). The unsupported
    /// opcodes alone lower nothing: what they cost is the claim's to say.
    pub(crate) fn findings(
        &self,
        articulations: Option<&ArticulationList>,
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
                    self.decoded_bytes >> 20,
                    MAX_BANK_BYTES >> 20
                ),
            ));
        }
        findings.extend(
            self.notes
                .iter()
                .map(|note| (VerdictState::Partly, note.clone())),
        );
        if let Some(ArticulationList::Mapped(mapped)) = articulations {
            for articulation in mapped {
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
        }
        findings
    }
}
