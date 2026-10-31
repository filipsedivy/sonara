//! Quality checks (SON-P18): concrete, explained findings about a project and about rendered
//! audio, instead of one opaque score.
//!
//! Every [`Finding`] says what is wrong (`message`), why it matters (`explanation`) and where
//! (`track_id`, `clip_id`, `note_ids`). Its [`Severity`] separates what breaks the result
//! (`Error`) from what probably does (`Warning`) and from advice (`Recommendation`).
//!
//! - [`check_project`] reads a project and never changes it.
//! - [`audio_findings`] gives the analyzer's issues ([`sonara_dsp::analysis::AnalysisReport`])
//!   the same shape, with an explanation each.
//! - [`orchestration::check_orchestration`] checks the tracks that play a part against the
//!   orchestration rules, with the rule's id as the code.
//!
//! It is a library API: no agent tool, CLI command or schema field is added (CTR-01), and no UI
//! is involved.

pub mod orchestration;

use serde_json::{Value, json};
use sonara_core::midi::MidiNote;
use sonara_core::model::clip::Clip;
use sonara_core::model::project::ProjectData;
use sonara_core::model::track::{Track, TrackKind};
use sonara_dsp::analysis::{AnalysisReport, Issue, IssueCode, Severity as AudioSeverity};

/// A fader level above this is a clipping risk, in dB.
const HOT_FADER_DB: f64 = 6.0;
/// A clip with at least this many notes of one velocity sounds mechanical.
const UNIFORM_VELOCITY_NOTES: usize = 8;

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// The result is broken: something cannot sound or cannot be rendered as intended.
    Error,
    /// The result probably is not what was meant.
    Warning,
    /// Advice; nothing is wrong.
    Recommendation,
}

impl Severity {
    /// The name in the JSON report.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Recommendation => "recommendation",
        }
    }
}

/// One explained finding.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    /// How serious it is.
    pub severity: Severity,
    /// A stable code, such as `INSTRUMENT_MISSING`.
    pub code: &'static str,
    /// What is wrong, naming the thing.
    pub message: String,
    /// Why it matters and what to do.
    pub explanation: String,
    /// The track concerned, if one.
    pub track_id: Option<String>,
    /// The clip concerned, if one.
    pub clip_id: Option<String>,
    /// The notes concerned.
    pub note_ids: Vec<String>,
}

impl Finding {
    fn new(severity: Severity, code: &'static str, message: String, explanation: &str) -> Self {
        Self {
            severity,
            code,
            message,
            explanation: explanation.to_owned(),
            track_id: None,
            clip_id: None,
            note_ids: Vec::new(),
        }
    }

    fn on_track(mut self, track: &Track) -> Self {
        self.track_id = Some(track.id.clone());
        self
    }

    fn in_clip(mut self, clip_id: &str) -> Self {
        self.clip_id = Some(clip_id.to_owned());
        self
    }

    fn with_notes<'a>(mut self, notes: impl IntoIterator<Item = &'a MidiNote>) -> Self {
        self.note_ids = notes.into_iter().map(|n| n.id.clone()).collect();
        self
    }

    /// The finding as JSON.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "severity": self.severity.as_str(),
            "code": self.code,
            "message": self.message,
            "explanation": self.explanation,
            "track_id": self.track_id,
            "clip_id": self.clip_id,
            "note_ids": self.note_ids,
        })
    }
}

/// The findings of a check, most serious first, in project order within a severity.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QualityReport {
    /// The findings.
    pub findings: Vec<Finding>,
}

impl QualityReport {
    /// How many findings have `severity`.
    #[must_use]
    pub fn count(&self, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }

    /// Whether the check found no error: the project can be rendered as it is.
    #[must_use]
    pub fn is_sound(&self) -> bool {
        self.count(Severity::Error) == 0
    }

    /// The report as JSON, with the counts per severity.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "sound": self.is_sound(),
            "errors": self.count(Severity::Error),
            "warnings": self.count(Severity::Warning),
            "recommendations": self.count(Severity::Recommendation),
            "findings": self.findings.iter().map(Finding::to_json).collect::<Vec<_>>(),
        })
    }

    fn sorted(mut self) -> Self {
        self.findings.sort_by_key(|f| f.severity);
        self
    }
}

/// Checks `project` for technical errors and a few musical problems. Reads only.
#[must_use]
pub fn check_project(project: &ProjectData) -> QualityReport {
    let mut findings = Vec::new();
    for track in &project.tracks {
        check_track(track, &mut findings);
    }
    check_solo(&project.tracks, &mut findings);
    QualityReport { findings }.sorted()
}

fn check_track(track: &Track, out: &mut Vec<Finding>) {
    if track.kind == TrackKind::Instrument && track.instrument.is_none() {
        out.push(
            Finding::new(
                Severity::Error,
                "INSTRUMENT_MISSING",
                format!("Instrument track '{}' has no instrument.", track.name),
                "Notes on this track make no sound and a render leaves it silent. Choose an instrument for the track.",
            )
            .on_track(track),
        );
    }
    if track.mixer.volume_db > HOT_FADER_DB {
        out.push(
            Finding::new(
                Severity::Warning,
                "FADER_HOT",
                format!(
                    "Track '{}' is at {:+.1} dB, above {:+.1} dB.",
                    track.name, track.mixer.volume_db, HOT_FADER_DB
                ),
                "A fader this high adds gain on top of the track's own level and makes clipping at the master likely. Lower it, or lower the sources instead.",
            )
            .on_track(track),
        );
    }
    for clip in &track.clips {
        let Clip::Midi(clip) = clip else { continue };
        if clip.muted {
            continue;
        }
        if clip.notes.is_empty() {
            out.push(
                Finding::new(
                    Severity::Recommendation,
                    "EMPTY_CLIP",
                    format!(
                        "MIDI clip '{}' on '{}' has no notes.",
                        clip.name, track.name
                    ),
                    "An empty clip plays nothing. Delete it, or fill it.",
                )
                .on_track(track)
                .in_clip(&clip.id),
            );
            continue;
        }
        check_notes(
            track,
            clip.id.as_str(),
            clip.length,
            clip.loop_length,
            clip.content_offset,
            &clip.notes,
            out,
        );
    }
}

fn check_notes(
    track: &Track,
    clip_id: &str,
    clip_length: f64,
    loop_length: Option<f64>,
    content_offset: f64,
    notes: &[MidiNote],
    out: &mut Vec<Finding>,
) {
    // Notes that begin after the end of a plain clip are never played.
    if loop_length.is_none() && content_offset == 0.0 {
        let silent: Vec<&MidiNote> = notes.iter().filter(|n| n.start >= clip_length).collect();
        if !silent.is_empty() {
            out.push(
                Finding::new(
                    Severity::Warning,
                    "NOTE_AFTER_CLIP_END",
                    format!(
                        "{} note(s) in clip '{clip_id}' start after the clip's end at beat {clip_length}.",
                        silent.len()
                    ),
                    "A clip plays only up to its end, so these notes stay silent. Lengthen the clip or move the notes.",
                )
                .on_track(track)
                .in_clip(clip_id)
                .with_notes(silent),
            );
        }
    }
    // Two notes of one pitch and channel that sound at once.
    let mut identical: Vec<&MidiNote> = Vec::new();
    let mut overlapping: Vec<&MidiNote> = Vec::new();
    for (i, a) in notes.iter().enumerate() {
        for b in &notes[i + 1..] {
            if a.pitch != b.pitch || a.channel != b.channel {
                continue;
            }
            if (a.start - b.start).abs() < 1e-9 {
                identical.push(b);
            } else if a.start < b.start + b.length && b.start < a.start + a.length {
                overlapping.push(b);
            }
        }
    }
    if !identical.is_empty() {
        out.push(
            Finding::new(
                Severity::Error,
                "DUPLICATE_NOTE",
                format!("{} note(s) in clip '{clip_id}' repeat a pitch at the same start.", identical.len()),
                "The same note twice at the same moment doubles its level and, in a MIDI file, ends early. Delete the duplicates.",
            )
            .on_track(track)
            .in_clip(clip_id)
            .with_notes(identical),
        );
    }
    if !overlapping.is_empty() {
        out.push(
            Finding::new(
                Severity::Warning,
                "OVERLAPPING_NOTES",
                format!("{} note(s) in clip '{clip_id}' overlap an earlier note of the same pitch.", overlapping.len()),
                "The second note retriggers the first, and exported MIDI files pair note-offs in order, so lengths may change. Shorten the first note or move the second.",
            )
            .on_track(track)
            .in_clip(clip_id)
            .with_notes(overlapping),
        );
    }
    if notes.len() >= UNIFORM_VELOCITY_NOTES
        && notes.iter().all(|n| n.velocity == notes[0].velocity)
    {
        out.push(
            Finding::new(
                Severity::Recommendation,
                "UNIFORM_VELOCITY",
                format!(
                    "All {} notes in clip '{clip_id}' have velocity {}.",
                    notes.len(),
                    notes[0].velocity
                ),
                "Identical velocities sound mechanical. Humanize the clip or accent the beat.",
            )
            .on_track(track)
            .in_clip(clip_id),
        );
    }
}

/// With a solo, every track that is neither soloed nor solo-safe is silent.
fn check_solo(tracks: &[Track], out: &mut Vec<Finding>) {
    if !tracks.iter().any(|t| t.mixer.solo) {
        return;
    }
    for track in tracks {
        let has_content = track.clips.iter().any(|c| match c {
            Clip::Midi(m) => !m.notes.is_empty(),
            Clip::Audio(_) => true,
        });
        if has_content
            && !track.mixer.solo
            && !track.mixer.solo_safe
            && track.kind != TrackKind::Master
        {
            out.push(
                Finding::new(
                    Severity::Recommendation,
                    "SILENCED_BY_SOLO",
                    format!("Track '{}' is silent because another track is soloed.", track.name),
                    "A render and playback leave out tracks that are not soloed. Clear the solo before exporting.",
                )
                .on_track(track),
            );
        }
    }
}

/// The analyzer's issues in the shape of [`Finding`], each with an explanation.
#[must_use]
pub fn audio_findings(report: &AnalysisReport) -> Vec<Finding> {
    let mut findings: Vec<Finding> = report.issues().iter().map(audio_finding).collect();
    findings.sort_by_key(|f| f.severity);
    findings
}

fn audio_finding(issue: &Issue) -> Finding {
    let severity = match issue.severity {
        AudioSeverity::Error => Severity::Error,
        AudioSeverity::Warning => Severity::Warning,
        AudioSeverity::Info => Severity::Recommendation,
    };
    let (code, explanation) = match issue.code {
        IssueCode::Nonfinite => (
            "NONFINITE",
            "NaN or infinite samples are replaced by silence and signal a faulty plugin or gain stage. Find the source and fix it.",
        ),
        IssueCode::OverFullScale => (
            "OVER_FULL_SCALE",
            "Samples above 0 dBFS are cut off in fixed-point files. Lower the master or the loudest track.",
        ),
        IssueCode::Clipping => (
            "CLIPPING",
            "Runs of samples at full scale are audible distortion. Lower the level before the point where they occur.",
        ),
        IssueCode::TruePeak => (
            "TRUE_PEAK",
            "Peaks between samples above -1 dBTP can clip after lossy encoding. Leave more headroom or use a limiter with a true-peak ceiling.",
        ),
        IssueCode::VeryLoud => (
            "VERY_LOUD",
            "Integrated loudness above -8 LUFS is louder than streaming services play and leaves little dynamics. Lower the level.",
        ),
        IssueCode::Quiet => (
            "QUIET",
            "Integrated loudness below -30 LUFS is very quiet next to other music. Raise the level or normalize on export.",
        ),
        IssueCode::Silent => (
            "SILENT",
            "No part of the render is above the loudness gate, so it is silent. Check mutes, solos, instruments and clips.",
        ),
        IssueCode::Phase => (
            "PHASE",
            "Negative left/right correlation means the channels cancel in mono. Check stereo wideners and inverted polarity.",
        ),
        IssueCode::DcOffset => (
            "DC_OFFSET",
            "A constant offset wastes headroom and can click at edits. Add a high-pass at a few hertz.",
        ),
        IssueCode::LowEndHeavy => (
            "LOW_END_HEAVY",
            "The lowest octave dominates the spectrum, which muddies small speakers. Thin the low end or high-pass non-bass tracks.",
        ),
    };
    Finding::new(severity, code, issue.message.clone(), explanation)
}
