//! The Export Audio dialog of an interface: its
//! fields ([`ExportForm`], validated here, TYP-06), its presets, the outputs, render request,
//! normalization and stems they describe ([`ExportPlan`]), the files an export would replace,
//! and the status-bar message of a finished export ([`export_message`]).
//!
//! The outputs follow the prototype's `ExportDialog.outputs()` (`crates/sonara-app/tests/
//! gui_dialogs.rs` compares them with the prototype's for every preset and more): the path
//! without its extension, `~` expanded, gets each format's extension; FLAC takes at most 24 bits;
//! float samples only in the PCM containers; a lossy format takes the bitrate; WAV-family outputs
//! that would share a file get a `-<container>` suffix. Stems go to `<first output's stem>
//! stems` next to the first output, in its format.
//!
//! **G1, fixed.** The prototype's "Podcast/Web (MP3 192 + Opus)" preset left the
//! lossy bitrate as it was (320 kbps by default), because its MP3 entry carried no bitrate. Here
//! it sets 192 kbps.
//!
//! **G2, fixed.** The prototype exported stems with `overwrite=True`, replacing
//! existing stem files without asking, while it asked before replacing a mix file. Here
//! [`ExportPlan::existing_files`] lists the stem files too, and both exports replace files only
//! after the user agreed.
//!
//! **G13, fixed (owner, 2026-10-23).** The prototype's "Archive (FLAC 24-bit +
//! 32-bit float WAV)" preset left the bit depth at 24-bit, because `_apply_preset` took the first
//! entry's depth (the FLAC's), so its WAV was 24-bit integer. Here a preset with a float entry
//! selects 32-bit float: the WAV is 32-bit float, and the FLAC stays 24-bit, as FLAC takes at
//! most 24 bits.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sonara_core::error::DomainError;
use sonara_core::model::project::Project;
use sonara_core::model::track::TrackKind;

use crate::error::AppResult;
use crate::events::{ERROR_MS, JobNotice, StatusMessage};
use crate::jobs::Job;
use crate::paths::{expand_user, split_suffix, with_suffix};
use crate::render::{
    Container, Dither, ExportSettings, Normalize, NormalizeSpec, OutputSpec, RenderRequest,
    RenderService, StemOptions, Tail, stem_paths,
};
use crate::settings_dialog::{Choice, qt_round};

/// The formats, in the dialog's order, with their labels; it offers those this build writes.
pub const FORMATS: [(Container, &str); 10] = [
    (Container::Wav, "WAV"),
    (Container::Bwf, "Broadcast WAV"),
    (Container::Aiff, "AIFF"),
    (Container::Flac, "FLAC"),
    (Container::Mp3, "MP3"),
    (Container::Ogg, "Ogg Vorbis"),
    (Container::Opus, "Opus"),
    (Container::Rf64, "RF64 (large WAV)"),
    (Container::W64, "Wave64"),
    (Container::Caf, "CAF"),
];
/// The sample rates besides the project's.
pub const RATES: [u32; 4] = [44_100, 48_000, 88_200, 96_000];
/// The lossy bitrates in kbps.
pub const BITRATES: [u32; 6] = [320, 256, 192, 160, 128, 96];
/// The tails besides "auto" and "none", in seconds.
pub const TAIL_SECONDS: [f64; 2] = [2.0, 5.0];
/// The normalization target's range in dB or LUFS.
pub const TARGET_DB: (f64, f64) = (-60.0, 0.0);
/// The true-peak ceiling's range in dBTP.
pub const CEILING_DB: (f64, f64) = (-20.0, 0.0);
/// The preset combo's first entry, which changes nothing.
pub const CUSTOM: &str = "Custom";

/// One format of a preset: the container, its PCM depth (0: none), float, and a sample rate
/// (above 1000) or a lossy bitrate (below 1000).
type PresetEntry = (Container, u32, bool, Option<u32>);

/// The presets (`PRESETS`), with G1's bitrate.
const PRESETS: [(&str, &[PresetEntry]); 5] = [
    ("Master (WAV 24-bit)", &[(Container::Wav, 24, false, None)]),
    (
        "Streaming (-14 LUFS, WAV + MP3 320)",
        &[
            (Container::Wav, 24, false, None),
            (Container::Mp3, 0, false, Some(320)),
        ],
    ),
    (
        "CD (WAV 16-bit 44.1 kHz)",
        &[(Container::Wav, 16, false, Some(44_100))],
    ),
    (
        "Archive (FLAC 24-bit + 32-bit float WAV)",
        &[
            (Container::Flac, 24, false, None),
            (Container::Wav, 32, true, None),
        ],
    ),
    (
        "Podcast/Web (MP3 192 + Opus)",
        &[
            // The prototype's entry had no bitrate.
            (Container::Mp3, 0, false, Some(192)),
            (Container::Opus, 0, false, None),
        ],
    ),
];

/// The formats this build writes (`sf_supported_containers`): every container, MP3 and FLAC
/// only when their libraries load.
#[must_use]
pub fn available_formats() -> Vec<Container> {
    sonara_media::export::available_containers()
}

/// The names of the presets, "Custom" first.
#[must_use]
pub fn preset_names() -> Vec<&'static str> {
    std::iter::once(CUSTOM)
        .chain(PRESETS.iter().map(|(name, _)| *name))
        .collect()
}

/// The PCM bit depth combo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BitDepth {
    /// "16-bit".
    #[serde(rename = "16")]
    Int16,
    /// "24-bit".
    #[serde(rename = "24")]
    Int24,
    /// "32-bit float".
    #[serde(rename = "32f")]
    Float32,
}

impl BitDepth {
    const fn depth(self) -> u32 {
        match self {
            Self::Int16 => 16,
            Self::Int24 => 24,
            Self::Float32 => 32,
        }
    }
}

/// The range combo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportRange {
    /// "Entire song".
    Song,
    /// "Loop region".
    Loop,
}

/// "Auto (until silence)" and "None" of the tail combo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NamedTail {
    /// Until the effects decay to silence.
    Auto,
    /// Exactly at the range end.
    None,
}

/// The tail combo: named, or 2 or 5 seconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExportTail {
    /// Auto or none.
    Named(NamedTail),
    /// Seconds after the range.
    Seconds(f64),
}

/// The normalization combo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizeChoice {
    /// "None".
    None,
    /// "Peak".
    Peak,
    /// "True peak".
    TruePeak,
    /// "Loudness (LUFS)".
    Loudness,
}

impl From<NormalizeChoice> for Normalize {
    fn from(choice: NormalizeChoice) -> Self {
        match choice {
            NormalizeChoice::None => Self::None,
            NormalizeChoice::Peak => Self::Peak,
            NormalizeChoice::TruePeak => Self::TruePeak,
            NormalizeChoice::Loudness => Self::Loudness,
        }
    }
}

/// The Export Audio dialog's fields, as it shows them and sends them back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportForm {
    /// The file without its extension (`~` allowed).
    pub path: String,
    /// The preset shown ("Custom" or a preset's name); choosing one is [`Self::with_preset`].
    pub preset: String,
    /// The checked formats (`wav`, `mp3`, …).
    pub formats: Vec<String>,
    /// The PCM bit depth.
    pub bit_depth: BitDepth,
    /// The sample rate; `None` for the project's.
    pub sample_rate: Option<u32>,
    /// 1 (mono) or 2 (stereo).
    pub channels: u32,
    /// The lossy bitrate in kbps.
    pub bitrate_kbps: u32,
    /// TPDF dither when reducing the bit depth.
    pub dither: bool,
    /// The song or the loop region.
    pub range: ExportRange,
    /// The tail.
    pub tail: ExportTail,
    /// The normalization.
    pub normalize: NormalizeChoice,
    /// The normalization target in dB or LUFS.
    pub target: f64,
    /// The true-peak ceiling in dBTP.
    pub ceiling: f64,
    /// Whether each track is exported as a stem too.
    pub stems: bool,
    /// Whether the stems go through the master's processing.
    pub through_master: bool,
    /// The tracks checked in the stems list, in the list's order.
    pub stem_tracks: Vec<String>,
}

impl ExportForm {
    /// The dialog's fields as it opens on `project`: `<default_dir>/<project name>`, WAV, 24
    /// bits, the project's rate, stereo, 320 kbps, dither, the entire song with an automatic
    /// tail, no normalization (−14 and −1 dB), no stems, every stem track checked.
    #[must_use]
    pub fn new(project: &Project, default_dir: &Path) -> Self {
        Self {
            path: default_dir.join(&project.name).display().to_string(),
            preset: CUSTOM.to_owned(),
            formats: vec![Container::Wav.value().to_owned()],
            bit_depth: BitDepth::Int24,
            sample_rate: None,
            channels: 2,
            bitrate_kbps: 320,
            dither: true,
            range: ExportRange::Song,
            tail: ExportTail::Named(NamedTail::Auto),
            normalize: NormalizeChoice::None,
            target: -14.0,
            ceiling: -1.0,
            stems: false,
            through_master: false,
            stem_tracks: stem_tracks(project).map(|track| track.0).collect(),
        }
    }

    /// The fields after choosing the preset `name` (`_apply_preset`): its formats checked
    /// (those of `available`), the bit depth it names (32-bit float when any of its formats is
    /// float), the rate it names, its lossy bitrate when it has one, and loudness
    /// normalization to −14 LUFS for the streaming preset. "Custom" and an
    /// unknown name change only the preset shown.
    #[must_use]
    pub fn with_preset(&self, name: &str, available: &[Container]) -> Self {
        let mut form = Self {
            preset: name.to_owned(),
            ..self.clone()
        };
        let Some((_, entries)) = PRESETS.iter().find(|(preset, _)| *preset == name) else {
            return form;
        };
        form.formats = FORMATS
            .iter()
            .map(|(container, _)| *container)
            .filter(|c| available.contains(c) && entries.iter().any(|entry| entry.0 == *c))
            .map(|c| c.value().to_owned())
            .collect();
        let depth = entries
            .iter()
            .map(|entry| entry.1)
            .find(|&d| d != 0)
            .unwrap_or(24);
        // A 32-bit float entry selects float, wherever it is in the preset; the
        // prototype took the first entry's depth only.
        let float = entries.iter().any(|entry| entry.2 && entry.1 == 32);
        form.bit_depth = if float {
            BitDepth::Float32
        } else if depth == 16 {
            BitDepth::Int16
        } else {
            BitDepth::Int24
        };
        form.sample_rate = entries
            .iter()
            .find_map(|entry| entry.3.filter(|&r| r > 1000))
            .filter(|rate| RATES.contains(rate));
        if let Some(kbps) = entries
            .iter()
            .find_map(|entry| entry.3.filter(|&r| r < 1000))
            .filter(|kbps| BITRATES.contains(kbps))
        {
            form.bitrate_kbps = kbps;
        }
        if name.contains("Streaming") {
            form.normalize = NormalizeChoice::Loudness;
            form.target = -14.0;
        }
        form
    }

    /// The output files (`outputs()`), in the dialog's order of the formats.
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` for a format that is unknown or not in `available`, an empty path, a
    /// rate, bitrate or channel count the dialog does not offer, or no format at all ("choose at
    /// least one format").
    pub fn outputs(&self, available: &[Container]) -> AppResult<Vec<OutputSpec>> {
        for value in &self.formats {
            if !FORMATS
                .iter()
                .any(|(c, _)| c.value() == value && available.contains(c))
            {
                return Err(
                    DomainError::invalid(format!("{value} is not a format to export")).into(),
                );
            }
        }
        if self.sample_rate.is_some_and(|rate| !RATES.contains(&rate)) {
            return Err(
                DomainError::invalid("the sample rate is not one the dialog offers").into(),
            );
        }
        if !BITRATES.contains(&self.bitrate_kbps) {
            return Err(DomainError::invalid("the bitrate is not one the dialog offers").into());
        }
        if !matches!(self.channels, 1 | 2) {
            return Err(DomainError::invalid("the channels are mono or stereo").into());
        }
        if self.path.trim().is_empty() {
            return Err(DomainError::invalid("choose the file to export to").into());
        }
        let base = expand_user(Path::new(&self.path));
        if base.file_name().is_none() {
            return Err(DomainError::invalid(format!("{} has an empty name", self.path)).into());
        }
        let depth = self.bit_depth.depth();
        let float = self.bit_depth == BitDepth::Float32;
        let mut specs: Vec<OutputSpec> = Vec::new();
        for (container, _) in FORMATS {
            if !self.formats.iter().any(|value| value == container.value()) {
                continue;
            }
            let lossy = container.is_lossy();
            specs.push(OutputSpec {
                path: with_suffix(&base, container.extension()),
                container,
                bit_depth: if container == Container::Flac && depth == 32 {
                    24
                } else {
                    depth
                },
                floating_point: float && container != Container::Flac && !lossy,
                sample_rate: self.sample_rate,
                channels: Some(self.channels),
                bitrate_kbps: lossy.then_some(self.bitrate_kbps),
                variable_bitrate: false,
                dither: if self.dither {
                    Dither::Tpdf
                } else {
                    Dither::None
                },
            });
        }
        if specs.is_empty() {
            return Err(DomainError::invalid("choose at least one format").into());
        }
        // Two WAV-family outputs would share the extension.
        let mut seen: Vec<PathBuf> = Vec::with_capacity(specs.len());
        for spec in &mut specs {
            if seen.contains(&spec.path) {
                let name = spec
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let (stem, suffix) = split_suffix(&name);
                let renamed = format!("{stem}-{}{suffix}", spec.container.value());
                spec.path = spec.path.with_file_name(renamed);
            }
            seen.push(spec.path.clone());
        }
        Ok(specs)
    }

    /// The render request (`request()`): the entire song or the loop region of `project`, with
    /// the tail.
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` for a tail the dialog does not offer.
    pub fn request(&self, project: &Project) -> AppResult<RenderRequest> {
        let tail = match self.tail {
            ExportTail::Named(NamedTail::Auto) => Tail::Auto,
            ExportTail::Named(NamedTail::None) => Tail::None,
            ExportTail::Seconds(seconds) if TAIL_SECONDS.contains(&seconds) => {
                Tail::Seconds(seconds)
            }
            ExportTail::Seconds(seconds) => {
                return Err(DomainError::invalid(format!(
                    "a tail of {seconds} s is not one the dialog offers"
                ))
                .into());
            }
        };
        let mut request = RenderRequest {
            tail,
            ..RenderRequest::default()
        };
        if self.range == ExportRange::Loop {
            request.start_beat = project.transport.loop_start;
            request.end_beat = Some(project.transport.loop_end);
        }
        Ok(request)
    }

    /// The normalization (`normalize_spec()`): the target and the ceiling clamped and held with
    /// two decimals, as the dialog's spin boxes hold them.
    #[must_use]
    pub fn normalize_spec(&self) -> NormalizeSpec {
        let spin = |value: f64, (low, high): (f64, f64)| {
            if value.is_finite() {
                qt_round(value.clamp(low, high), 2)
            } else {
                high
            }
        };
        NormalizeSpec {
            mode: self.normalize.into(),
            target_db: spin(self.target, TARGET_DB),
            true_peak_ceiling_db: Some(spin(self.ceiling, CEILING_DB)),
        }
    }

    /// The stem tracks (`stem_track_ids()`): `None` without stems.
    #[must_use]
    pub fn stem_track_ids(&self) -> Option<&[String]> {
        self.stems.then_some(self.stem_tracks.as_slice())
    }

    /// Everything the export does, for `project` with the formats of `available`.
    ///
    /// # Errors
    ///
    /// The errors of [`Self::outputs`] and [`Self::request`]; `NOT_FOUND` for a stem track
    /// that `project` does not have.
    pub fn plan(&self, project: &Project, available: &[Container]) -> AppResult<ExportPlan> {
        let outputs = self.outputs(available)?;
        let request = self.request(project)?;
        let stems = match (self.stem_track_ids(), outputs.first()) {
            (Some(tracks), Some(first)) if !tracks.is_empty() => {
                let name = first
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let directory = first
                    .path
                    .with_file_name(format!("{} stems", split_suffix(&name).0));
                let files = stem_paths(project, &directory, Some(tracks), first.container)?;
                Some(StemsPlan {
                    directory,
                    track_ids: tracks.to_vec(),
                    template: first.clone(),
                    through_master: self.through_master,
                    files,
                })
            }
            _ => None,
        };
        Ok(ExportPlan {
            outputs,
            request,
            normalize: self.normalize_spec(),
            stems,
        })
    }
}

/// The audio, instrument and bus tracks (the stems list): ID and name.
fn stem_tracks(project: &Project) -> impl Iterator<Item = (String, String)> + '_ {
    project
        .tracks
        .iter()
        .filter(|t| {
            matches!(
                t.kind,
                TrackKind::Audio | TrackKind::Instrument | TrackKind::Bus
            )
        })
        .map(|t| (t.id.clone(), t.name.clone()))
}

/// The stems of an export.
#[derive(Debug, Clone, PartialEq)]
pub struct StemsPlan {
    /// `<first output's stem> stems` beside the first output.
    pub directory: PathBuf,
    /// The tracks, in order.
    pub track_ids: Vec<String>,
    /// The first output, whose format the stems take.
    pub template: OutputSpec,
    /// Whether they go through the master.
    pub through_master: bool,
    /// The files the stems export writes ([`stem_paths`]).
    pub files: Vec<PathBuf>,
}

/// What an export of the dialog does.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportPlan {
    /// The mix's files.
    pub outputs: Vec<OutputSpec>,
    /// The render request.
    pub request: RenderRequest,
    /// The normalization of the mix.
    pub normalize: NormalizeSpec,
    /// The stems, when they are exported too.
    pub stems: Option<StemsPlan>,
}

impl ExportPlan {
    /// The files that exist already: the mix's, then the stems'. The dialog asks before
    /// it replaces them.
    #[must_use]
    pub fn existing_files(&self) -> Vec<PathBuf> {
        let stems = self.stems.iter().flat_map(|stems| stems.files.iter());
        self.outputs
            .iter()
            .map(|spec| &spec.path)
            .chain(stems)
            .filter(|path| path.exists())
            .cloned()
            .collect()
    }

    /// Queues the mix's export job and, with stems, the stems' job; `overwrite` replaces
    /// existing files (the user agreed to replace [`Self::existing_files`]).
    ///
    /// # Errors
    ///
    /// The errors of [`RenderService::export_job`] and [`RenderService::export_stems_job`]; the
    /// mix's job may be queued when the stems' fails.
    pub fn submit(self, render: &RenderService, overwrite: bool) -> AppResult<Vec<Job>> {
        let mut jobs = vec![render.export_job(
            self.outputs,
            self.request.clone(),
            ExportSettings {
                normalize: self.normalize,
                metadata: Vec::new(),
                overwrite,
            },
        )?];
        if let Some(stems) = self.stems {
            jobs.push(render.export_stems_job(
                stems.directory,
                Some(stems.track_ids),
                stems.template,
                self.request,
                StemOptions {
                    through_master: stems.through_master,
                    overwrite,
                },
            )?);
        }
        Ok(jobs)
    }
}

/// What the Export Audio dialog shows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExportDialogView {
    /// The fields as it opens.
    pub form: ExportForm,
    /// The formats this build writes, in order, with their labels.
    pub formats: Vec<Choice<&'static str>>,
    /// "Custom" and the presets.
    pub presets: Vec<&'static str>,
    /// "Project rate" (`null`) and the other rates.
    pub rates: Vec<Choice<Option<u32>>>,
    /// The lossy bitrates ("320 kbps").
    pub bitrates: Vec<Choice<u32>>,
    /// The stem tracks: name and ID.
    pub stem_tracks: Vec<Choice<String>>,
    /// The target's range.
    pub target_db: (f64, f64),
    /// The ceiling's range.
    pub ceiling_db: (f64, f64),
}

/// The Export Audio dialog for `project`, exporting into `default_dir` (the project's folder,
/// else the projects folder), with the formats of `available`.
#[must_use]
pub fn dialog_view(
    project: &Project,
    default_dir: &Path,
    available: &[Container],
) -> ExportDialogView {
    ExportDialogView {
        form: ExportForm::new(project, default_dir),
        formats: FORMATS
            .iter()
            .filter(|(c, _)| available.contains(c))
            .map(|(c, label)| Choice {
                label: (*label).to_owned(),
                value: c.value(),
            })
            .collect(),
        presets: preset_names(),
        rates: std::iter::once(Choice {
            label: "Project rate".to_owned(),
            value: None,
        })
        .chain(RATES.iter().map(|&rate| Choice {
            label: format!("{} kHz", khz(rate)),
            value: Some(rate),
        }))
        .collect(),
        bitrates: BITRATES
            .iter()
            .map(|&kbps| Choice {
                label: format!("{kbps} kbps"),
                value: kbps,
            })
            .collect(),
        stem_tracks: stem_tracks(project)
            .map(|(id, name)| Choice {
                label: name,
                value: id,
            })
            .collect(),
        target_db: TARGET_DB,
        ceiling_db: CEILING_DB,
    }
}

/// `f"{rate / 1000:g}"`.
fn khz(rate: u32) -> String {
    let (thousands, rest) = (rate / 1000, rate % 1000);
    if rest == 0 {
        thousands.to_string()
    } else {
        format!("{thousands}.{}", format!("{rest:03}").trim_end_matches('0'))
    }
}

/// The export job kinds whose success the status bar reports.
const EXPORT_JOBS: [&str; 2] = ["export_audio", "export_stems"];

/// The status-bar message of a finished export job (`_on_job`): "Exported N file(s)", with
/// " · L LUFS · T dBTP" when the result has both levels, for 8 s; `None` for any other job or
/// state (a failure is [`JobNotice::failure`]).
#[must_use]
pub fn export_message(job: &JobNotice) -> Option<StatusMessage> {
    if !EXPORT_JOBS.contains(&job.kind.as_str()) || job.status != "succeeded" {
        return None;
    }
    let empty = Value::Null;
    let result = job.result.as_ref().unwrap_or(&empty);
    let files = ["outputs", "stems"]
        .iter()
        .find_map(|key| result[key].as_array().filter(|items| !items.is_empty()))
        .map_or(0, Vec::len);
    let level = |key: &str| result[key].as_f64().filter(|_| result[key].is_f64());
    let extra = match (
        level("final_integrated_lufs"),
        level("final_true_peak_dbtp"),
    ) {
        (Some(lufs), Some(peak)) => format!(" · {lufs:.1} LUFS · {peak:.1} dBTP"),
        _ => String::new(),
    };
    Some(StatusMessage {
        text: format!("Exported {files} file(s){extra}"),
        duration_ms: ERROR_MS,
        error: false,
    })
}
