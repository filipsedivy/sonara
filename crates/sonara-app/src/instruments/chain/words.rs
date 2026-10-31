//! What the chain tells the user, in plain words.

use sonara_core::midi::note_name;
use sonara_midi::orchestration::fallback::{Emulation, Manner, Neighbour, Scope};
use sonara_midi::orchestration::{
    Articulation, InstrumentType, PitchRange, Request, SoundingPitch,
};

use super::tables::Builtin;
use super::{Substitute, playable};
use crate::instruments::manifest::{EnsembleKind, Verdict};
use crate::instruments::model::{InstrumentId, InstrumentPreset};

/// The English name of `instrument`, for example `Horn in F`.
fn name(instrument: InstrumentType) -> &'static str {
    instrument.get().name()
}

/// `text` with its first letter in upper case.
fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// An articulation's id as words: `sul_ponticello` is "sul ponticello".
pub(super) fn articulation(articulation: Articulation) -> String {
    articulation.id().replace('_', " ")
}

/// A request as words: its articulation, mute and vibrato, as in "sustain con sordino"; the
/// ordinary sound alone is "ordinario".
pub(super) fn request(request: Request) -> String {
    let words: Vec<String> = [
        request.primary(),
        request.mute().articulation(),
        request.vibrato().articulation(),
    ]
    .into_iter()
    .flatten()
    .map(articulation)
    .collect();
    if words.is_empty() {
        "ordinario".to_owned()
    } else {
        words.join(" ")
    }
}

/// What plays, as words: "its ordinary sound" for the ordinary sound alone.
fn played(plays: Request) -> String {
    if plays == Request::default() {
        "its ordinary sound".to_owned()
    } else {
        request(plays)
    }
}

/// `n` as an ordinal: 32 is "32nd".
fn ordinal(n: u8) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// A rewrite of the notes as words, as in "each note shortened to a quarter of its length".
pub(super) fn emulation(emulation: Emulation) -> String {
    match emulation {
        Emulation::Overlap => "each note overlapping the next".to_owned(),
        Emulation::Shorten { divisor: 2 } => "each note shortened to half its length".to_owned(),
        Emulation::Shorten { divisor: 4 } => {
            "each note shortened to a quarter of its length".to_owned()
        }
        Emulation::Shorten { divisor } => {
            format!("each note shortened to 1/{divisor} of its length")
        }
        Emulation::Accent => "a dynamic step louder".to_owned(),
        Emulation::Soften => "a dynamic step softer".to_owned(),
        Emulation::Repeat { per_whole_note } => {
            format!("as repeated {} notes", ordinal(per_whole_note))
        }
        Emulation::Alternate { slowest, fastest } if slowest == fastest => {
            format!("as a trill written out at {slowest} notes per second")
        }
        Emulation::Alternate { slowest, fastest } => {
            format!("as a trill written out at {slowest} to {fastest} notes per second")
        }
    }
}

/// A kind of passage as words.
pub(super) const fn scope(scope: Scope) -> &'static str {
    match scope {
        Scope::ShortNotes => "short notes",
        Scope::SingleLines => "single lines",
        Scope::TremoloPassages => "tremolo passages",
        Scope::Accents => "accented notes",
        Scope::Chords => "chords",
        Scope::Arpeggios => "arpeggios",
        Scope::SingleStrokes => "single strokes",
    }
}

/// A sounding pitch in scientific pitch notation, as in "C4".
fn note(pitch: SoundingPitch) -> String {
    u8::try_from(pitch.get()).map_or_else(|_| pitch.get().to_string(), note_name)
}

/// Keys as words: "key 38", or "keys 41, 43 and 45".
fn keys(keys: &[u8]) -> String {
    match keys {
        [] => "no key".to_owned(),
        [key] => format!("key {key}"),
        [rest @ .., last] => {
            let rest: Vec<String> = rest.iter().map(u8::to_string).collect();
            format!("keys {} and {last}", rest.join(", "))
        }
    }
}

/// Step 2: "Spiccato is not available for Violin; playing staccato."
pub(super) fn substitution(
    requested: Request,
    substitute: &Substitute,
    instrument: InstrumentType,
) -> String {
    let mut text = format!(
        "{} is not available for {}; playing {}",
        sentence(&request(requested)),
        name(instrument),
        played(substitute.plays)
    );
    for rewrite in &substitute.rewrites {
        text.push_str(", ");
        text.push_str(&emulation(*rewrite));
    }
    if let Some(only) = substitute.only {
        text.push_str(" (for ");
        text.push_str(scope(only));
        text.push_str(" only)");
    }
    text.push('.');
    text
}

/// Step 1, a preset that plays partly: "Cello Section (lib/Cello) plays partly: a; b."
pub(super) fn partly(preset: &InstrumentPreset, verdict: &Verdict) -> String {
    if verdict.lost.is_empty() {
        format!("{} ({}) plays partly.", preset.display_name, preset.id)
    } else {
        format!(
            "{} ({}) plays partly: {}.",
            preset.display_name,
            preset.id,
            verdict.lost.join("; ")
        )
    }
}

/// Step 1, a preset whose sampled keys do not hold the part's notes.
pub(super) fn unsampled(preset: &InstrumentPreset, span: PitchRange) -> String {
    format!(
        "{} ({}) has no samples for some notes from {} to {}; those notes do not sound.",
        preset.display_name,
        preset.id,
        note(span.low()),
        note(span.high())
    )
}

/// Step 3: "No solo Violin is installed; a Violin section plays the part." When a preset of the
/// size asked for is installed but plays fewer of the part's articulations (the user prefers to
/// keep them): "No solo Violin installed plays all the articulations the part asks for; …".
pub(super) fn ensemble_swap(
    instrument: InstrumentType,
    wish: EnsembleKind,
    size_installed: bool,
) -> String {
    let name = name(instrument);
    let (wished, other) = match wish {
        EnsembleKind::Solo => (format!("solo {name}"), format!("{name} section")),
        EnsembleKind::Section | EnsembleKind::SectionEmulated => {
            (format!("{name} section"), format!("solo {name}"))
        }
    };
    if size_installed {
        format!(
            "No {wished} installed plays all the articulations the part asks for; a {other} \
             plays the part."
        )
    } else {
        format!("No {wished} is installed; a {other} plays the part.")
    }
}

/// How a neighbour plays the part, as words after "plays the part".
fn manner(neighbour: &Neighbour) -> String {
    let instrument = neighbour.instrument;
    let extreme = |high: bool| {
        playable(instrument).map_or_else(String::new, |range| {
            let (which, pitch) = if high {
                ("highest", range.high())
            } else {
                ("lowest", range.low())
            };
            format!(
                ", every stroke on its {which} playable note, {}",
                note(pitch)
            )
        })
    };
    let mut text = match neighbour.manner {
        Manner::AsWritten => String::new(),
        Manner::Softer => ", a dynamic step softer".to_owned(),
        Manner::With(with) => format!(", playing {} throughout", articulation(with)),
        Manner::OnHighestNote => extreme(true),
        Manner::OnLowestNote => extreme(false),
    };
    if let Some(only) = neighbour.only {
        text.push_str(" (for ");
        text.push_str(scope(only));
        text.push_str(" only)");
    }
    text
}

/// Step 4: "No Cimbasso is installed; Tuba (vsco2ce/Tuba-KS) plays the part."
pub(super) fn neighbour(
    part: InstrumentType,
    neighbour: &Neighbour,
    preset: &InstrumentId,
) -> String {
    format!(
        "No {} is installed; {} ({preset}) plays the part{}.",
        name(part),
        name(neighbour.instrument),
        manner(neighbour)
    )
}

/// The question step 4 asks before a neighbour plays: when it asks first, because it belongs to
/// another family, loses the part's pitches or changes its role; and when the part's notes,
/// `beyond` (its span), lie outside its playable range. `None` when it asks nothing.
pub(super) fn neighbour_question(
    part: InstrumentType,
    neighbour: &Neighbour,
    preset: &InstrumentId,
    beyond: Option<PitchRange>,
) -> Option<String> {
    let other = name(neighbour.instrument);
    let mut sentences = Vec::new();
    if neighbour.asks_first {
        let reason = if part.family() != neighbour.instrument.family() {
            "it belongs to another family"
        } else if playable(part).is_some() && playable(neighbour.instrument).is_none() {
            "the part's pitches are lost"
        } else {
            "it changes the part's role"
        };
        sentences.push(format!(
            "No {} is installed; {other} ({preset}) can play the part, but {reason}. Use it?",
            name(part)
        ));
    }
    if let (Some(span), Some(range)) = (beyond, playable(neighbour.instrument)) {
        sentences.push(format!(
            "The part reaches {} to {}, beyond {other}'s playable range of {} to {}. Play it on \
             {other} with the notes outside that range moved by an octave, or leave {other} out?",
            note(span.low()),
            note(span.high()),
            note(range.low()),
            note(range.high())
        ));
    }
    (!sentences.is_empty()).then(|| sentences.join(" "))
}

/// The plugin's name and how a built-in sound plays the part, as words after "plays the part".
fn builtin_how(sound: Builtin) -> String {
    match sound {
        Builtin::Synth { .. } => String::new(),
        Builtin::SynthKey { key, .. } => format!(" with every stroke on {}", note_name(key)),
        Builtin::Drums {
            keys: drum_keys, ..
        } => format!(" on {}", keys(drum_keys)),
    }
}

/// Step 6: "No sampled sound for Violin is installed; the built-in Poly Synth preset 'Strings'
/// plays the part, a synthetic approximation."
pub(super) fn builtin(instrument: InstrumentType, sound: Builtin) -> String {
    let lost = if playable(instrument).is_some() && matches!(sound, Builtin::Drums { .. }) {
        " that loses the pitches"
    } else {
        ""
    };
    format!(
        "No sampled sound for {} is installed; the built-in {} preset '{}' plays the part{}, a \
         synthetic approximation{lost}.",
        name(instrument),
        sound.plugin_name(),
        sound.preset(),
        builtin_how(sound)
    )
}

/// The question step 6 asks before a built-in sound that changes the part's character plays.
pub(super) fn builtin_question(instrument: InstrumentType, sound: Builtin) -> String {
    format!(
        "No sampled sound for {} is installed. The built-in {} preset '{}' can play the part{}, \
         but as another kind of sound. Use it, or leave the track silent?",
        name(instrument),
        sound.plugin_name(),
        sound.preset(),
        builtin_how(sound)
    )
}

/// Step 7: "No sound source for Whip; the track is silent."
pub(super) fn silence(instrument: InstrumentType) -> String {
    format!(
        "No sound source for {}; the track is silent.",
        name(instrument)
    )
}
