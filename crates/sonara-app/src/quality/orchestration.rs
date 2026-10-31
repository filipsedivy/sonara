//! Orchestration findings: the orchestration rules that a project's notes can be checked against,
//! run over the tracks that play a part, and reported as [`Finding`]s whose code is the rule's
//! id, such as `RNG-01`.
//!
//! A track takes part when it plays a part (project schema version 2) whose instrument the
//! orchestration metadata knows. A part that names an unknown instrument is reported when the
//! project opens ([`crate::vocabulary::warnings`]) and is left out here, as is every track without
//! a part: a track name or a sound is no statement of what the music is written for.
//!
//! Each such track is one line of [`check`]: the notes of its unmuted MIDI clips where the
//! timeline plays them, so the rules that compare parts see the notes that sound together. A
//! looped clip repeats its content, a note crossing the clip's end is cut there, and a note before
//! the clip's start or after its end is not played (the notes as a MIDI file export writes them).
//! The project's tempo and time signatures measure the times and bars the rules compare.
//!
//! The rules read what a note stores: its sounding pitch, its velocity as its dynamic
//! ([`velocity_band`](sonara_midi::performance::check::velocity_band)) and its own articulation.
//! The sampler's `articulation` and `dynamics` parameters and their automation are a sound's
//! settings, not the written part, and are not read.
//!
//! It reads the project and never changes it. Like the other quality checks it is a library API:
//! no agent tool, CLI command or schema field is added (CTR-01).

use std::collections::BTreeSet;

use sonara_core::EPSILON;
use sonara_core::midi::MidiNote;
use sonara_core::model::clip::Clip;
use sonara_core::model::project::ProjectData;
use sonara_core::model::track::Track;
use sonara_core::time::TimeSignatureMap;
use sonara_midi::orchestration::rules::{Level, Rule};
use sonara_midi::orchestration::{InstrumentType, Provenance};
use sonara_midi::performance::check::{CheckError, Hit, Line, check};
use sonara_midi::performance::{PartId, ScoreContext};

use super::{Finding, QualityReport, Severity};

/// Checks the tracks of `project` that play a part against the orchestration rules a project's
/// notes can be checked against
/// ([`CHECKED_RULES`](sonara_midi::performance::check::CHECKED_RULES)). Reads only.
///
/// Each finding's code is the rule's id (`RNG-01`); a rule's error, warning and hint are an
/// [`Severity::Error`], a [`Severity::Warning`] and a [`Severity::Recommendation`]. The finding is
/// placed on the first track the rule names, with every note concerned and with the clip when
/// they all lie in one; its message names every track concerned and the bars, and its
/// explanation says what the rule looks for and the source it rests on, or that it is an
/// estimate. The findings come most serious first, then by rule in the catalogue's order, then
/// by where they start. The same project always gives the same report (TST-04).
///
/// A project without such a track gives no finding.
///
/// # Errors
/// [`CheckError::InvalidNote`] for a note the model would refuse, a pitch above 127 or a velocity
/// of 0 or above 127 written into the project's fields directly, which a validated project never
/// holds.
///
/// # Panics
/// Never: the score starts at beat 0, a valid origin, and each part's id is `part_` and its
/// index, a valid part id.
pub fn check_orchestration(project: &ProjectData) -> Result<QualityReport, CheckError> {
    let parts = played_parts(project);
    if parts.is_empty() {
        return Ok(QualityReport::default());
    }
    let lines: Vec<Line<'_>> = parts
        .iter()
        .map(|part| Line::new(&part.id, part.instrument, &part.notes))
        .collect();
    let score = ScoreContext::new(
        project.tempo_map.clone(),
        project.time_signatures.clone(),
        0.0,
    )
    .expect("beat 0 is a finite beat >= 0, the only origin ScoreContext requires");
    let findings = check(&lines, &score)?
        .iter()
        .map(|hit| finding(hit, &parts, &project.time_signatures))
        .collect();
    Ok(QualityReport { findings }.sorted())
}

/// One track that plays a part, as the rules read it.
struct PlayedPart<'p> {
    /// The track.
    track: &'p Track,
    /// The part's id within the check: unique per track, never shown.
    id: PartId,
    /// The instrument the part names.
    instrument: InstrumentType,
    /// The notes as the timeline plays them, at their timeline beats.
    notes: Vec<MidiNote>,
    /// The clip of each note of `notes`, by index.
    clips: Vec<&'p str>,
}

/// The tracks of `project` that play a part whose instrument the metadata knows, in track order.
fn played_parts(project: &ProjectData) -> Vec<PlayedPart<'_>> {
    project
        .tracks
        .iter()
        .filter_map(|track| {
            let part = track.part.as_ref()?;
            InstrumentType::from_id(&part.instrument).map(|instrument| (track, instrument))
        })
        .enumerate()
        .map(|(index, (track, instrument))| {
            let (notes, clips) = timeline_notes(track);
            PlayedPart {
                track,
                id: PartId::new(&format!("part_{index}"))
                    .expect("`part_` and a decimal index are at most 26 bytes of a-z, 0-9 and _"),
                instrument,
                notes,
                clips,
            }
        })
        .collect()
}

/// The notes of `track`'s unmuted MIDI clips at the timeline beats they play, each cut at its
/// clip's end, with the clip of each.
fn timeline_notes(track: &Track) -> (Vec<MidiNote>, Vec<&str>) {
    let mut notes = Vec::new();
    let mut clips = Vec::new();
    for clip in &track.clips {
        let Clip::Midi(clip) = clip else { continue };
        if clip.muted {
            continue;
        }
        let clip_end = clip.start + clip.length;
        for (beat, note) in clip.playback_notes() {
            // The model allows a start a tolerance before beat 0; the rules take none.
            let start = beat.max(0.0);
            let end = (beat + note.length).min(clip_end);
            if end - start <= EPSILON {
                continue;
            }
            notes.push(MidiNote {
                start,
                length: end - start,
                ..note.clone()
            });
            clips.push(clip.id.as_str());
        }
    }
    (notes, clips)
}

/// The finding of `hit` on the tracks of `parts` it names.
fn finding(hit: &Hit, parts: &[PlayedPart<'_>], signatures: &TimeSignatureMap) -> Finding {
    let named: Vec<&PlayedPart<'_>> = hit
        .parts
        .iter()
        .filter_map(|id| parts.iter().find(|part| &part.id == id))
        .collect();
    let rule = hit.rule.get();
    let message = format!(
        "{} on {}, {}: {}.",
        rule.title(),
        track_names(&named),
        bars(hit.start, hit.end, signatures),
        hit.detail,
    );
    let mut found = Finding::new(
        severity(hit.level),
        rule.code(),
        message,
        &explanation(rule),
    );
    found.track_id = named.first().map(|part| part.track.id.clone());
    found.clip_id = only_clip(hit, &named).map(str::to_owned);
    let mut seen = BTreeSet::new();
    found.note_ids = hit
        .notes
        .iter()
        .filter(|id| seen.insert(id.as_str()))
        .cloned()
        .collect();
    found
}

/// The severity of a rule's level.
fn severity(level: Level) -> Severity {
    match level {
        Level::Error => Severity::Error,
        Level::Warning => Severity::Warning,
        Level::Hint => Severity::Recommendation,
    }
}

/// What `rule` looks for and what it rests on.
fn explanation(rule: &Rule) -> String {
    let basis = match rule.provenance() {
        Provenance::Cited { source } => format!("Source: {source}."),
        Provenance::Estimate { reason } => format!("An estimate: {reason}."),
    };
    format!("{} flags this when {}. {basis}", rule.code(), rule.check())
}

/// The names of the tracks of `parts`, quoted: `'Flute 1'`, `'Flute 1' and 'Flute 2'` or
/// `'Timpani', 'Bass Drum' and 'Snare'`.
fn track_names(parts: &[&PlayedPart<'_>]) -> String {
    let names: Vec<String> = parts
        .iter()
        .map(|part| format!("'{}'", part.track.name))
        .collect();
    match names.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        Some((last, _)) => last.clone(),
        None => "no track".to_owned(),
    }
}

/// The bars from timeline beat `start` to `end`, 1-based: `bar 3` or `bars 3-5`. A span that ends
/// on a bar line does not reach into the next bar.
fn bars(start: f64, end: f64, signatures: &TimeSignatureMap) -> String {
    let first = signatures.position(start).bar;
    let at_end = signatures.position(end);
    let on_bar_line = at_end.beat == 1 && at_end.fraction <= EPSILON;
    let last = if on_bar_line {
        at_end.bar - 1
    } else {
        at_end.bar
    }
    .max(first);
    if first == last {
        format!("bar {first}")
    } else {
        format!("bars {first}-{last}")
    }
}

/// The clip that holds every note of `hit`, when there is one.
fn only_clip<'p>(hit: &Hit, parts: &[&PlayedPart<'p>]) -> Option<&'p str> {
    let mut clips = BTreeSet::new();
    for part in parts {
        for (note, &clip) in part.notes.iter().zip(&part.clips) {
            if hit.notes.contains(&note.id) {
                clips.insert(clip);
            }
        }
    }
    let mut clips = clips.into_iter();
    match (clips.next(), clips.next()) {
        (Some(clip), None) => Some(clip),
        _ => None,
    }
}
