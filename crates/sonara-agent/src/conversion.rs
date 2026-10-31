//! Argument conversion (the "argument conversion" helpers of the prototype's `AgentContext` in
//! `sonara.agent.tools.registry`): positions, lengths and pitches in the forms the tool schemas
//! accept (docs/agent-api.md, "Conventions"), turned into beats and MIDI pitches against a
//! project's tempo map and time signatures; and a note's articulation and a track's part, whose
//! ids are checked against the orchestration metadata.
//!
//! The functions reproduce the prototype's parsing exactly, including its quirks: text is
//! stripped and lowercased; a number in text form is read by Python's `float()` rules
//! (`"1_0"`, `"inf"`, `".5"`), and it is not range-checked the way a JSON number is.
//!
//! One difference: Python's `float()` also reads non-ASCII decimal digits (`"١٢"` is beat 12),
//! which these functions reject like any other text that is not a number.

use sonara_core::error::{DomainError, Result};
use sonara_core::ids::IdGenerator;
use sonara_core::midi::{MidiNote, parse_note_name};
use sonara_core::model::part::{self, Notation, Part};
use sonara_core::model::project::Project;
use sonara_core::pyfmt::str_repr;
use sonara_core::time::parse_position;

use crate::contract::args::{Clef, Length, NoteInput, PartSpec, Pitch, Position};
use crate::pystr::{py_float, py_strip};
use crate::vocabulary;

/// A position in beats (`AgentContext.beats`): a JSON number is beats; text is seconds
/// (`"12.5s"`), a number (`"16"`), or `"bar:beat[:tick]"` (1-based, 960 ticks per beat).
///
/// `project` is asked for only when the text needs the tempo map or the time signatures, so a
/// number converts without an open project, as in the prototype (pass
/// `|| session.require_project()`).
///
/// # Errors
///
/// [`sonara_core::error::ErrorCode::InvalidArgument`]:
///
/// - `positions must be >= 0` for a negative number (text is not range-checked);
/// - `cannot parse position '<value>'` for seconds that are not a number;
/// - the errors of [`parse_position`] (`cannot parse musical position '<text>'` with the
///   stripped, lowercased text, `bar and beat are 1-based`, `beat <n> exceeds <sig> bar`).
///
/// The error of `project` (`NO_PROJECT`) when the text needs a project.
pub fn beats<'p>(value: &Position, project: impl FnOnce() -> Result<&'p Project>) -> Result<f64> {
    let raw = match value {
        Position::Beats(beats) => {
            if *beats < 0.0 {
                return Err(DomainError::invalid("positions must be >= 0"));
            }
            return Ok(*beats);
        }
        Position::Text(raw) => raw,
    };
    let text = py_strip(raw).to_lowercase();
    if let Some(seconds) = text.strip_suffix('s')
        && !text.ends_with("bars")
    {
        let seconds = py_float(seconds).ok_or_else(|| {
            DomainError::invalid(format!("cannot parse position {}", str_repr(raw)))
        })?;
        return Ok(project()?.seconds_to_beat(seconds));
    }
    if let Some(beats) = py_float(&text) {
        return Ok(beats);
    }
    parse_position(&text, &project()?.time_signatures)
}

/// A length in beats from `start` (`AgentContext.length_beats`): a JSON number is beats; text is
/// bars (`"4 bars"`, `"4bar"`, `"4b"`; across meter changes from `start`), seconds (`"30s"`;
/// across tempo changes from `start`) or a number of beats (`"8"`).
///
/// `project` supplies the tempo map and the time signatures: the open project, or the shape of
/// a project that does not exist yet (`create_project`'s `length`), which is the prototype's
/// `project=` override.
///
/// As in the prototype, the bar suffix is stripped as the character set `b`, `a`, `r`, `s`
/// (`"4ab"` is 4 bars), and text is not range-checked (`"0 bars"` is 0).
///
/// # Errors
///
/// [`sonara_core::error::ErrorCode::InvalidArgument`]: `lengths must be > 0` for a number that is
/// not positive, `cannot parse length '<value>'` for text that is none of the forms.
pub fn length_beats(value: &Length, start: f64, project: &Project) -> Result<f64> {
    let raw = match value {
        Length::Beats(beats) => {
            if *beats <= 0.0 {
                return Err(DomainError::invalid("lengths must be > 0"));
            }
            return Ok(*beats);
        }
        Length::Text(raw) => raw,
    };
    let unparsable = || DomainError::invalid(format!("cannot parse length {}", str_repr(raw)));
    let text = py_strip(raw).to_lowercase();
    if text.ends_with("bars") || text.ends_with("bar") || text.ends_with('b') {
        // `text.rstrip("bars").rstrip("b").strip()`
        let number = py_strip(text.trim_end_matches(['b', 'a', 'r', 's']));
        let bars = py_float(number).ok_or_else(unparsable)?;
        let signatures = &project.time_signatures;
        let start_bar = signatures.beat_to_bar(start);
        return Ok(signatures.bar_to_beat(start_bar + bars) - start);
    }
    if let Some(seconds) = text.strip_suffix('s') {
        let seconds = py_float(seconds).ok_or_else(unparsable)?;
        let t0 = project.beat_to_seconds(start);
        return Ok(project.seconds_to_beat(t0 + seconds) - start);
    }
    py_float(&text).ok_or_else(unparsable)
}

/// A MIDI pitch (`AgentContext.pitch`): a number 0–127, or a note name such as `C4` (= 60),
/// `F#3` or `Bb2`.
///
/// # Errors
///
/// [`sonara_core::error::ErrorCode::InvalidArgument`]: `pitch <n> is outside 0-127`, or the
/// errors of [`parse_note_name`].
pub fn pitch(value: &Pitch) -> Result<u8> {
    match value {
        Pitch::Number(number) => u8::try_from(*number)
            .ok()
            .filter(|pitch| *pitch <= 127)
            .ok_or_else(|| DomainError::invalid(format!("pitch {number} is outside 0-127"))),
        Pitch::Name(name) => parse_note_name(name),
    }
}

/// New notes with fresh IDs from `ids` (`AgentContext.notes`), in order; a negative start is
/// clamped to 0. A note's articulation must be an articulation of the orchestration vocabulary
/// that a note can carry; it is stored on the note.
///
/// # Errors
///
/// The first failing note's error: those of [`pitch`], an unknown articulation or a mute or
/// vibrato id ([`sonara_core::error::ErrorCode::InvalidArgument`] with suggestions for an
/// unknown id), and those of [`MidiNote::create`] (for example `note length must be > 0`).
pub fn notes(items: &[NoteInput], ids: &mut dyn IdGenerator) -> Result<Vec<MidiNote>> {
    items
        .iter()
        .map(|note| {
            let pitch = pitch(&note.pitch)?;
            let articulation = note
                .articulation
                .as_deref()
                .map(vocabulary::articulation)
                .transpose()?;
            MidiNote::create(
                ids,
                i64::from(pitch),
                note.start,
                note.length,
                i64::from(note.velocity),
                i64::from(note.channel),
            )?
            .with_articulation(articulation.map(|articulation| articulation.id().to_owned()))
        })
        .collect()
}

/// The part a track plays (`create_track.part`), with its instrument type checked against the
/// orchestration metadata.
///
/// # Errors
///
/// [`sonara_core::error::ErrorCode::InvalidArgument`]: an unknown instrument type, with
/// suggestions, or a broken rule of [`Part::validate`].
pub fn part(spec: &PartSpec) -> Result<Part> {
    let instrument = vocabulary::instrument(&spec.instrument)?;
    let notation = spec
        .notation
        .as_ref()
        .map_or_else(Notation::default, |notation| Notation {
            transposition: notation.transposition,
            clef: notation.clef.map(clef),
        });
    let part = Part {
        instrument: instrument.id().to_owned(),
        name: spec.name.clone(),
        players: spec.players,
        solo: spec.solo,
        player: spec.player.clone(),
        notation,
    };
    part.validate()?;
    Ok(part)
}

fn clef(clef: Clef) -> part::Clef {
    match clef {
        Clef::Treble => part::Clef::Treble,
        Clef::Treble8vb => part::Clef::Treble8vb,
        Clef::Bass => part::Clef::Bass,
        Clef::Alto => part::Clef::Alto,
        Clef::Tenor => part::Clef::Tenor,
        Clef::Percussion => part::Clef::Percussion,
        Clef::SingleLine => part::Clef::SingleLine,
    }
}
