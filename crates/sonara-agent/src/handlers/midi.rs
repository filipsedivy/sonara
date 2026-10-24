//! MIDI generation (`generate_midi` of the prototype's "clips and notes" section): chords,
//! arpeggios, bass lines, drum grooves and melodies from `sonara-midi`, into a new or an
//! existing MIDI clip.

use serde_json::{Value, json};
use sonara_core::commands::clip::{AddNotes, CreateMidiClip};
use sonara_core::error::DomainError;
use sonara_core::ids::{self, IdGenerator};
use sonara_core::midi::MidiNote;
use sonara_core::model::clip::Clip;
use sonara_core::model::track::TrackKind;
use sonara_midi::drums::{DrumOptions, drum_pattern};
use sonara_midi::generators::{
    ArpeggioOptions, BasslineOptions, BeatsPerChord as ChordLengths, ChordNotesOptions, ChordSlot,
    MelodyOptions, arpeggio, bassline, chords_to_notes, melody, progression,
};
use sonara_midi::theory::{Key, parse_key};
use sonara_midi::transforms::{self, HumanizeOptions, QuantizeOptions};

use super::positions::Positions;
use crate::context::AgentContext;
use crate::contract::args::{BeatsPerChord, GenerateMidi, GeneratorKind};
use crate::registry::{ToolError, ToolResult};

/// Tolerance of the prototype's comparison of the clip length with the generated content.
const LOOP_TOLERANCE: f64 = 1e-9;

/// The kind's argument value (`a.kind`) and its title (`a.kind.title()`).
fn kind_names(kind: GeneratorKind) -> (&'static str, &'static str) {
    match kind {
        GeneratorKind::Chords => ("chords", "Chords"),
        GeneratorKind::Arpeggio => ("arpeggio", "Arpeggio"),
        GeneratorKind::Bass => ("bass", "Bass"),
        GeneratorKind::Drums => ("drums", "Drums"),
        GeneratorKind::Melody => ("melody", "Melody"),
    }
}

/// `a.style or default`: an empty style means the default.
fn style_or<'a>(style: Option<&'a str>, default: &'a str) -> &'a str {
    style.filter(|style| !style.is_empty()).unwrap_or(default)
}

/// The prototype's `_slots`: the progression laid out from beat `start`.
fn slots(args: &GenerateMidi, key: Option<Key>, start: f64) -> Result<Vec<ChordSlot>, DomainError> {
    let chords = args
        .progression
        .as_deref()
        .filter(|chords| !chords.is_empty())
        .ok_or_else(|| {
            DomainError::invalid(format!(
                "{} generation needs a progression",
                kind_names(args.kind).0
            ))
        })?;
    let lengths = match &args.beats_per_chord {
        BeatsPerChord::Uniform(length) => ChordLengths::Uniform(*length),
        BeatsPerChord::PerChord(lengths) => ChordLengths::Each(lengths),
    };
    progression(chords, key, lengths, start)
}

/// The generated notes (beats from the clip start) and the length of the content in beats.
fn generate(
    args: &GenerateMidi,
    key: Option<Key>,
    bar_length: f64,
    ids: &mut dyn IdGenerator,
) -> Result<(Vec<MidiNote>, f64), DomainError> {
    let style = args.style.as_deref();
    let center = (i64::from(args.octave) + 1) * 12;
    let velocity = i64::from(args.velocity);
    if args.kind == GeneratorKind::Drums {
        let options = DrumOptions {
            bars: i64::from(args.bars),
            beats_per_bar: bar_length,
            variation: args.variation,
            fill: args.fill,
            swing: args.swing,
            crash_on_first: false,
            seed: args.seed,
        };
        let notes = drum_pattern(ids, style_or(style, "four_on_the_floor"), &options)?;
        return Ok((notes, f64::from(args.bars) * bar_length));
    }
    let slots = slots(args, key, 0.0)?;
    let content = slots.last().map_or(0.0, |slot| slot.start + slot.length);
    let notes = match args.kind {
        GeneratorKind::Chords => {
            let options = ChordNotesOptions {
                center,
                velocity,
                rhythm: style_or(style, "sustain").parse()?,
                ..ChordNotesOptions::default()
            };
            chords_to_notes(ids, &slots, &options)?
        }
        GeneratorKind::Arpeggio => {
            let options = ArpeggioOptions {
                pattern: style_or(style, "up").parse()?,
                rate: args.rate,
                center: center + 4,
                velocity,
                seed: args.seed,
                ..ArpeggioOptions::default()
            };
            arpeggio(ids, &slots, &options)?
        }
        GeneratorKind::Bass => {
            let octave = i64::from(args.octave);
            let options = BasslineOptions {
                style: style_or(style, "root").parse()?,
                octave: if octave <= 3 { octave } else { 2 },
                rate: args.rate,
                velocity,
                seed: args.seed,
                key,
            };
            bassline(ids, &slots, &options)?
        }
        // The prototype's `else`: a melody (drums returned above).
        GeneratorKind::Melody | GeneratorKind::Drums => {
            let key = key.ok_or_else(|| DomainError::invalid("melody generation needs a key"))?;
            let options = MelodyOptions {
                low: center,
                high: center + 19,
                rhythm_unit: args.rate,
                velocity,
                seed: args.seed,
                ..MelodyOptions::default()
            };
            melody(ids, key, &slots, &options)?
        }
    };
    if args.swing > 0.0 {
        let options = QuantizeOptions {
            strength: 1.0,
            swing: args.swing,
            ends: false,
        };
        return Ok((transforms::quantize(&notes, args.rate, &options)?, content));
    }
    Ok((notes, content))
}

/// The light humanization of `generate_midi` (`humanize: true`).
fn humanize_options(args: &GenerateMidi) -> HumanizeOptions {
    HumanizeOptions {
        timing: if args.kind == GeneratorKind::Drums {
            0.004
        } else {
            0.006
        },
        velocity: 6,
        length: 0.0,
        // `a.seed + 1`: Python seeds `random` with the magnitude of the integer, so the wrapped
        // `i64::MIN` (magnitude 2^63) is the prototype's seed for `i64::MAX`.
        seed: args.seed.wrapping_add(1),
    }
}

/// Adds `generated` to the existing MIDI clip `clip_id`, moved from beat `start` of the
/// timeline to the clip's own time (notes before the clip start are dropped). Returns the
/// commit's result with `clip_id`, and the notes added.
fn append(
    ctx: &AgentContext,
    positions: &Positions<'_>,
    clip_id: String,
    generated: &[MidiNote],
    start: f64,
    dry_run: bool,
) -> Result<(Value, Vec<MidiNote>), ToolError> {
    let clip_start = match positions.project()?.clip(&clip_id)?.1 {
        Clip::Midi(clip) => clip.start,
        Clip::Audio(_) => return Err(DomainError::invalid("clip_id must be a MIDI clip").into()),
    };
    let command = AddNotes {
        clip_id,
        notes: transforms::offset(generated, start - clip_start)?,
    };
    let mut out = ctx.commit(&command, dry_run)?;
    out["clip_id"] = json!(command.clip_id);
    Ok((out, command.notes))
}

/// `generate_midi`: generated material in a new MIDI clip on an instrument track, or appended
/// to an existing clip (`clip_id`). Deterministic for a seed.
///
/// The notes draw their IDs before the new clip; a clip longer than the content loops it. The
/// result adds `clip_id`, `note_count`, `content_beats` and, when there are notes,
/// `pitch_range`.
pub(super) fn generate_midi(ctx: &AgentContext, args: GenerateMidi) -> ToolResult {
    let positions = Positions::new(ctx);
    let start = positions.beats(&args.start)?;
    let project = positions.project()?;
    if project.track(&args.track_id)?.kind != TrackKind::Instrument {
        return Err(DomainError::invalid("generate_midi targets an instrument track").into());
    }
    let bar_length = project.time_signatures.at_beat(start).beats_per_bar();
    // `parse_key(a.key) if a.key else None`
    let key = args
        .key
        .as_deref()
        .filter(|key| !key.is_empty())
        .map(parse_key)
        .transpose()?;
    let mut ids = ctx.app().ids();
    let (mut generated, content) = generate(&args, key, bar_length, &mut ids)?;
    if args.humanize {
        generated = transforms::humanize(&generated, &humanize_options(&args))?;
    }
    let (mut out, notes) = if let Some(clip_id) = args.clip_id.filter(|id| !id.is_empty()) {
        append(ctx, &positions, clip_id, &generated, start, args.dry_run)?
    } else {
        let length = match &args.length {
            Some(length) => positions.length_beats(length, start)?,
            None => content,
        };
        let name = args
            .name
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| kind_names(args.kind).1.to_owned());
        let command = CreateMidiClip {
            track_id: args.track_id,
            start,
            length,
            name,
            notes: generated,
            loop_length: (length > content + LOOP_TOLERANCE).then_some(content),
            color: None,
            clip_id: ids.new_id(ids::CLIP),
        };
        let mut out = ctx.commit(&command, args.dry_run)?;
        out["clip_id"] = json!(command.clip_id);
        (out, command.notes)
    };
    out["note_count"] = json!(notes.len());
    out["content_beats"] = json!(content);
    let pitches = notes.iter().map(|note| note.pitch);
    if let (Some(low), Some(high)) = (pitches.clone().min(), pitches.max()) {
        out["pitch_range"] = json!([low, high]);
    }
    Ok(out)
}
