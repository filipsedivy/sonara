//! The orchestration ids a tool argument names, checked against the orchestration metadata of
//! `sonara_midi::orchestration` at the boundary (TYP-06): a note's articulation and the
//! instrument type of a track's part.
//!
//! The project format checks only the form of these ids, so a tool is the place where an id
//! that names nothing is refused, with "did you mean" suggestions. A suggestion is only ever
//! offered, never taken: `Staccato` and `pizz` are refused like any other id that is not in the
//! metadata, and the error names `staccato` and `pizzicato`.

use serde_json::Value;
use sonara_core::error::DomainError;
use sonara_core::pyfmt::str_repr;
use sonara_midi::orchestration::{
    Articulation, Axis, InstrumentType, Lang, Mention, Target, fold, resolve,
};

/// The most suggestions one unknown id gets.
const MAX_SUGGESTIONS: usize = 3;

/// The fewest letters an id needs before ids a few edits away or starting with it are offered:
/// shorter ids are within an edit or two of too many others.
const MIN_SUGGESTION_LETTERS: usize = 4;

/// The articulation `id` names, when a note may carry it: an id of the vocabulary whose axis is
/// a note's own (primary) articulation.
///
/// # Errors
///
/// [`sonara_core::error::ErrorCode::InvalidArgument`]:
///
/// - `unknown articulation '<id>'` for an id the vocabulary does not have, with the closest
///   note articulations in the message and as detail `suggestions`, and every note
///   articulation as detail `available`;
/// - `articulation '<id>' is a mute (or a vibrato setting), which holds over a passage, not a
///   note's articulation` for a mute or vibrato id.
pub(crate) fn articulation(id: &str) -> Result<Articulation, DomainError> {
    let Some(articulation) = Articulation::from_id(id) else {
        let available = note_articulations();
        return Err(unknown(
            "articulation",
            id,
            suggestions(id, &[], &available),
            &available,
        ));
    };
    match articulation.axis() {
        Axis::Primary => Ok(articulation),
        Axis::Mute(_) => Err(passage_state(id, "a mute")),
        Axis::Vibrato(_) => Err(passage_state(id, "a vibrato setting")),
    }
}

/// The instrument type `id` names.
///
/// # Errors
///
/// [`sonara_core::error::ErrorCode::InvalidArgument`]: `unknown instrument '<id>'` for an id
/// the metadata does not have, with suggestions in the message and as detail `suggestions`
/// (first the instrument types an instrument name in the text names, such as `horn_f` for
/// "French horn", then ids close to it), and every instrument type id as detail `available`.
pub(crate) fn instrument(id: &str) -> Result<InstrumentType, DomainError> {
    InstrumentType::from_id(id).ok_or_else(|| {
        let available: Vec<&'static str> = InstrumentType::ALL.iter().map(|t| t.id()).collect();
        let named: Vec<&'static str> = resolve(id, &Lang::ALL)
            .iter()
            .flat_map(Mention::targets)
            .filter_map(|target| match target {
                Target::Instrument(instrument) => Some(instrument.id()),
                Target::Category(_) | Target::Family(_) => None,
            })
            .collect();
        unknown(
            "instrument",
            id,
            suggestions(id, &named, &available),
            &available,
        )
    })
}

/// The ids a note may carry as its articulation, in vocabulary order.
fn note_articulations() -> Vec<&'static str> {
    Articulation::ALL
        .iter()
        .filter(|articulation| matches!(articulation.axis(), Axis::Primary))
        .map(|articulation| articulation.id())
        .collect()
}

fn passage_state(id: &str, what: &str) -> DomainError {
    DomainError::invalid(format!(
        "articulation {} is {what}, which holds over a passage, not a note's articulation",
        str_repr(id)
    ))
    .with_detail("articulation", id)
}

/// The error for an id of `kind` (`articulation`, `instrument`) that names nothing.
fn unknown(
    kind: &str,
    id: &str,
    suggestions: Vec<&'static str>,
    available: &[&'static str],
) -> DomainError {
    let offers: Vec<String> = suggestions.iter().map(|id| str_repr(id)).collect();
    let message = match offers.as_slice() {
        [] => format!("unknown {kind} {}", str_repr(id)),
        [only] => format!("unknown {kind} {}; did you mean {only}?", str_repr(id)),
        [init @ .., last] => format!(
            "unknown {kind} {}; did you mean {} or {last}?",
            str_repr(id),
            init.join(", ")
        ),
    };
    DomainError::invalid(message)
        .with_detail(kind, id)
        .with_detail("suggestions", Value::from(suggestions))
        .with_detail("available", Value::from(available.to_vec()))
}

/// The ids of `candidates` to offer for `text`, best first, at most [`MAX_SUGGESTIONS`], none
/// twice:
///
/// 1. the candidate `text` spells in another case or with spaces or hyphens for underscores
///    (`Sul Ponticello` for `sul_ponticello`);
/// 2. `named`, the candidates an instrument name in the text names;
/// 3. candidates one edit away from a text of four or five letters, or two edits away from a
///    longer one (Damerau-Levenshtein: an insertion, deletion, substitution or swap of two
///    neighbouring letters is one edit), nearest first;
/// 4. candidates that start with a text of at least four letters, an abbreviation such as
///    `pizz`.
///
/// Ties keep the order of `candidates` (TST-04).
fn suggestions(
    text: &str,
    named: &[&'static str],
    candidates: &[&'static str],
) -> Vec<&'static str> {
    let spelled = fold(text).replace(' ', "_");
    let letters = spelled.bytes().filter(u8::is_ascii_lowercase).count();
    let mut offers: Vec<&'static str> = candidates
        .iter()
        .copied()
        .filter(|candidate| *candidate == spelled)
        .collect();
    offers.extend(named.iter().copied().filter(|id| candidates.contains(id)));
    if letters >= MIN_SUGGESTION_LETTERS {
        let max = if letters <= 5 { 1 } else { 2 };
        let mut close: Vec<(usize, usize, &'static str)> = candidates
            .iter()
            .enumerate()
            .filter_map(|(order, &candidate)| {
                let distance = strsim::damerau_levenshtein(&spelled, candidate);
                (distance <= max).then_some((distance, order, candidate))
            })
            .collect();
        close.sort_unstable();
        offers.extend(close.into_iter().map(|(_, _, candidate)| candidate));
        offers.extend(
            candidates
                .iter()
                .copied()
                .filter(|candidate| candidate.starts_with(spelled.as_str())),
        );
    }
    let mut chosen: Vec<&'static str> = Vec::with_capacity(MAX_SUGGESTIONS);
    for offer in offers {
        if chosen.len() == MAX_SUGGESTIONS {
            break;
        }
        if !chosen.contains(&offer) {
            chosen.push(offer);
        }
    }
    chosen
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sonara_core::error::ErrorCode;
    use sonara_midi::orchestration::{Articulation, InstrumentType};

    use super::{articulation, instrument, note_articulations, suggestions};

    #[test]
    fn every_note_articulation_is_accepted() {
        for id in note_articulations() {
            assert_eq!(articulation(id).map(Articulation::id), Ok(id));
        }
        assert_eq!(articulation("staccato"), Ok(Articulation::Staccato));
        // Every id but the eight mutes and the two vibrato settings.
        assert_eq!(note_articulations().len(), Articulation::ALL.len() - 10);
    }

    #[test]
    fn mutes_and_vibrato_are_refused_as_note_articulations() {
        let mute = articulation("con_sordino").expect_err("a mute");
        assert_eq!(mute.code, ErrorCode::InvalidArgument);
        assert_eq!(
            mute.message,
            "articulation 'con_sordino' is a mute, which holds over a passage, not a note's \
             articulation"
        );
        assert_eq!(mute.details["articulation"], "con_sordino");
        let vibrato = articulation("non_vibrato").expect_err("a vibrato setting");
        assert_eq!(
            vibrato.message,
            "articulation 'non_vibrato' is a vibrato setting, which holds over a passage, not a \
             note's articulation"
        );
        for id in [
            "mute_cup",
            "stopped",
            "palm_mute",
            "xylophonic",
            "vibrato_on",
        ] {
            assert!(articulation(id).is_err(), "{id}");
            assert!(!note_articulations().contains(&id), "{id}");
        }
    }

    #[test]
    fn an_unknown_articulation_names_the_closest_ids() {
        let err = articulation("stacato").expect_err("misspelt");
        assert_eq!(err.code, ErrorCode::InvalidArgument);
        assert_eq!(
            err.message,
            "unknown articulation 'stacato'; did you mean 'staccato'?"
        );
        assert_eq!(err.details["articulation"], "stacato");
        assert_eq!(err.details["suggestions"], json!(["staccato"]));
        assert_eq!(err.details["available"], json!(note_articulations()));
        assert_eq!(err.details["available"][0], "sustain");
        let err = articulation("xyzzy_plonk").expect_err("nothing close");
        assert_eq!(err.message, "unknown articulation 'xyzzy_plonk'");
        assert_eq!(err.details["suggestions"], json!([]));
    }

    #[test]
    fn suggestions_are_ranked_and_capped() {
        let ids = note_articulations();
        // Another spelling of an id comes first, and is never taken as the id.
        assert_eq!(
            suggestions("Sul Ponticello", &[], &ids)[0],
            "sul_ponticello"
        );
        assert_eq!(suggestions("col-legno", &[], &ids)[0], "col_legno");
        // An abbreviation finds the ids it starts.
        assert_eq!(suggestions("pizz", &[], &ids), ["pizzicato"]);
        assert_eq!(
            suggestions("trem", &[], &ids),
            ["tremolo", "tremolo_measured"]
        );
        // Nearest first; ties in vocabulary order; at most three.
        assert_eq!(
            suggestions("trill", &[], &ids),
            ["trill", "trill_half", "trill_whole"]
        );
        assert_eq!(suggestions("harmonic", &[], &ids)[0], "harmonics");
        // Short words get only the exact spelling.
        assert_eq!(suggestions("arc", &[], &ids), Vec::<&str>::new());
        assert_eq!(suggestions("", &[], &ids), Vec::<&str>::new());
        // A passage state is never offered for a note.
        assert!(!suggestions("con sordino", &[], &ids).contains(&"con_sordino"));
    }

    #[test]
    fn an_unknown_instrument_names_what_its_words_name() {
        let err = instrument("French horn").expect_err("a name, not an id");
        assert_eq!(err.code, ErrorCode::InvalidArgument);
        assert_eq!(
            err.message,
            "unknown instrument 'French horn'; did you mean 'horn_f'?"
        );
        assert_eq!(err.details["instrument"], "French horn");
        assert_eq!(err.details["suggestions"], json!(["horn_f"]));
        let ids: Vec<&str> = InstrumentType::ALL.iter().map(|t| t.id()).collect();
        assert_eq!(err.details["available"], json!(ids));
        let err = instrument("Violin").expect_err("an id in capitals");
        assert_eq!(err.details["suggestions"][0], "violin");
        let err = instrument("violn").expect_err("misspelt");
        assert_eq!(err.details["suggestions"][0], "violin");
        assert_eq!(instrument("horn_f"), Ok(InstrumentType::HornF));
        assert_eq!(instrument("violin"), Ok(InstrumentType::Violin));
    }
}
