//! A compact plain-text reference of the instruments, for the system prompt of an agent that
//! composes in Sonara: every instrument id in score order with its English and Czech name, its
//! sounding ranges, its transposition and the articulation ids a note of it may carry.
//!
//! The text is rendered from the same records the orchestration planner and the checks read
//! ([`instruments`]), so it cannot disagree with them. It is deterministic: the same build
//! renders the same text, byte for byte.
//!
//! **Compactness.** Every turn of an agent carries the text, so it is short:
//!
//! - an instrument's list holds the ids a note may carry; the mute and vibrato ids, which hold
//!   over a passage and which a note refuses, are named once for all instruments;
//! - an instrument whose list equals one already written says `as <id>`;
//! - an instrument whose list differs from a written list of its own family by at most
//!   four ids says `as <id> without … plus …`, when that is shorter;
//! - the comfortable range is given only where it differs from the playable one, and the
//!   transposition only where it is not 0; a Czech name only where it is not the English one.

use super::articulation::{Articulation, Axis};
use super::instrument::{Family, Instrument, PitchModel, Pitched};
use super::{InstrumentType, instruments};

/// The most ids by which a list may differ from the list it is written against.
const MAX_DIFFERENCES: usize = 4;

/// What the articulation field says for an instrument whose notes carry no id.
const NO_IDS: &str = "none";

/// A list written in full: the instrument it belongs to and the ids.
struct Written {
    id: &'static str,
    family: Family,
    ids: Vec<Articulation>,
}

/// The reference: a level-2 heading, how to read a line and which ids hold over a passage, then
/// one line per instrument in score order. It ends with a newline.
#[must_use]
pub fn render() -> String {
    let passage: Vec<&str> = Articulation::ALL
        .iter()
        .filter(|articulation| !is_note_id(**articulation))
        .map(|articulation| articulation.id())
        .collect();
    let mut lines = vec![
        "## Instruments".to_owned(),
        String::new(),
        format!(
            "The {} instrument ids in score order: `id | English / Czech name | range | \
             articulations`. Pitches are MIDI numbers at sounding pitch (C4 = 60); write notes \
             at sounding pitch. Range: playable, [comfortable], (transposition: sounding minus \
             written in semitones, for notation only). A note's `articulation` takes an id of \
             its instrument's list, or none for the ordinary sound; `as viola` is the viola's \
             list. Mute and vibrato ids hold over a passage and a note refuses them: {}.",
            InstrumentType::ALL.len(),
            passage.join(" ")
        ),
        String::new(),
    ];
    let mut written: Vec<Written> = Vec::new();
    for instrument in instruments() {
        let own = note_ids(instrument);
        let field = articulations_text(instrument, &own, &written);
        lines.push(format!(
            "{} | {} | {} | {field}",
            instrument.id(),
            names(instrument),
            pitch(instrument)
        ));
        if !field.starts_with("as ") {
            written.push(Written {
                id: instrument.id(),
                family: instrument.family(),
                ids: own,
            });
        }
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// Whether a note may carry `articulation`: every id but the mute and vibrato ids.
const fn is_note_id(articulation: Articulation) -> bool {
    matches!(articulation.axis(), Axis::Primary)
}

/// The ids a note of `instrument` may carry, in the record's order.
fn note_ids(instrument: &Instrument) -> Vec<Articulation> {
    instrument
        .articulations()
        .iter()
        .copied()
        .filter(|articulation| is_note_id(*articulation))
        .collect()
}

/// `English / Czech`, or the English name alone where the Czech one is the same word.
fn names(instrument: &Instrument) -> String {
    let names = instrument.names();
    if names.en.to_lowercase() == names.cs.to_lowercase() {
        names.en.to_owned()
    } else {
        format!("{} / {}", names.en, names.cs)
    }
}

/// `low-high`, then `[low-high]` of a comfortable range that differs, then `(+12)` of a
/// transposition that is not 0; `unpitched` for an instrument without pitches.
fn pitch(instrument: &Instrument) -> String {
    match instrument.pitch() {
        PitchModel::Pitched(pitched) => ranges(pitched),
        PitchModel::Unpitched => "unpitched".to_owned(),
    }
}

/// The ranges and the transposition of a pitched instrument ([`pitch`]).
fn ranges(pitched: &Pitched) -> String {
    let playable = pitched.playable.range;
    let mut parts = vec![format!(
        "{}-{}",
        playable.low().get(),
        playable.high().get()
    )];
    if let Some(comfortable) = pitched.comfortable.map(|sourced| sourced.range)
        && comfortable != playable
    {
        parts.push(format!(
            "[{}-{}]",
            comfortable.low().get(),
            comfortable.high().get()
        ));
    }
    let semitones = pitched.transposition.semitones();
    if semitones != 0 {
        parts.push(format!("({semitones:+})"));
    }
    parts.join(" ")
}

/// The articulation field of `instrument`, whose note ids are `own`: `as <id>` for a list equal
/// to one of `written`, else the shortest of the whole list and `as <id> without … plus …`
/// against a written list of the same family that differs by at most [`MAX_DIFFERENCES`] ids.
fn articulations_text(
    instrument: &Instrument,
    own: &[Articulation],
    written: &[Written],
) -> String {
    if let Some(same) = written.iter().find(|other| other.ids == own) {
        return format!("as {}", same.id);
    }
    let whole = if own.is_empty() {
        NO_IDS.to_owned()
    } else {
        ids(own)
    };
    written
        .iter()
        .filter(|other| other.family == instrument.family())
        .filter_map(|other| difference(other, own))
        .filter(|text| text.len() < whole.len())
        .min_by_key(String::len)
        .unwrap_or(whole)
}

/// `as <id> without … plus …`: `own` written against the list of `base`, or `None` when they
/// differ by more than [`MAX_DIFFERENCES`] ids.
fn difference(base: &Written, own: &[Articulation]) -> Option<String> {
    let without: Vec<Articulation> = base
        .ids
        .iter()
        .copied()
        .filter(|articulation| !own.contains(articulation))
        .collect();
    let plus: Vec<Articulation> = own
        .iter()
        .copied()
        .filter(|articulation| !base.ids.contains(articulation))
        .collect();
    if without.len() + plus.len() > MAX_DIFFERENCES {
        return None;
    }
    let mut parts = vec![format!("as {}", base.id)];
    if !without.is_empty() {
        parts.push(format!("without {}", ids(&without)));
    }
    if !plus.is_empty() {
        parts.push(format!("plus {}", ids(&plus)));
    }
    Some(parts.join(" "))
}

/// The ids, space-separated, in the order given.
fn ids(articulations: &[Articulation]) -> String {
    let ids: Vec<&str> = articulations.iter().map(|a| a.id()).collect();
    ids.join(" ")
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::orchestration::{Mute, Request, Vibrato, instrument};

    /// The instrument lines of the reference: id and the fields after it.
    fn rows(text: &str) -> Vec<(&str, Vec<&str>)> {
        text.lines()
            .filter(|line| line.contains(" | ") && !line.contains('`'))
            .map(|line| {
                let mut fields = line.split(" | ");
                let id = fields.next().expect("split yields at least one part");
                (id, fields.collect())
            })
            .collect()
    }

    /// The articulation set each line states, with `as <id> without … plus …` resolved against
    /// the lines before it.
    fn read_back(text: &str) -> BTreeMap<String, BTreeSet<String>> {
        let mut lists: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (id, fields) in rows(text) {
            let field = fields.last().expect("an articulation field");
            let set = match field.strip_prefix("as ") {
                Some(rest) => {
                    let mut words = rest.split(' ');
                    let base = words.next().expect("a base id");
                    let mut set = lists
                        .get(base)
                        .unwrap_or_else(|| panic!("{id}: {base} is not an earlier line"))
                        .clone();
                    let mut adding = None;
                    for word in words {
                        match word {
                            "without" => adding = Some(false),
                            "plus" => adding = Some(true),
                            id if adding == Some(true) => assert!(set.insert(id.to_owned())),
                            id if adding == Some(false) => assert!(set.remove(id)),
                            other => panic!("{other} follows no keyword"),
                        }
                    }
                    set
                }
                None if *field == NO_IDS => BTreeSet::new(),
                None => field.split(' ').map(str::to_owned).collect(),
            };
            lists.insert(id.to_owned(), set);
        }
        lists
    }

    #[test]
    fn every_instrument_has_one_line_in_score_order() {
        let text = render();
        let ids: Vec<&str> = rows(&text).iter().map(|(id, _)| *id).collect();
        let expected: Vec<&str> = InstrumentType::ALL.iter().map(|t| t.id()).collect();
        assert_eq!(ids, expected);
    }

    #[test]
    fn a_line_holds_the_names_ranges_and_transposition_of_the_record() {
        let text = render();
        let rows = rows(&text);
        let row = |id: &str| -> Vec<&str> {
            rows.iter()
                .find(|(row_id, _)| *row_id == id)
                .map_or_else(|| panic!("{id} has no line"), |(_, fields)| fields.clone())
        };
        // Oracles: the records in `data/` (the violin plays G3 to A7, comfortably to A6; the B♭
        // clarinet sounds a major second below written, the horn in F a fifth below, the
        // piccolo an octave above; the snare drum has no pitch; the piano's comfortable range
        // is its whole range).
        assert_eq!(row("violin")[..2], ["Violin / housle", "55-105 [55-93]"]);
        assert_eq!(row("clarinet_bb")[1], "50-94 [50-91] (-2)");
        assert_eq!(
            row("horn_f")[..2],
            ["Horn in F / lesní roh", "35-77 [41-72] (-7)"]
        );
        assert_eq!(row("piccolo")[1], "74-108 [80-106] (+12)");
        assert_eq!(
            row("snare_drum")[..2],
            ["Snare drum / malý buben", "unpitched"]
        );
        assert_eq!(row("piano_concert_grand")[1], "21-108");
        // The Czech name is left out where it is the English one.
        assert_eq!(row("marimba")[0], "Marimba");
        // Equal lists refer to the first instrument that has it.
        assert_eq!(row("viola")[2], "as violin");
        assert_eq!(row("double_bass")[2], "as violin without su_una_corda");
        for (id, fields) in &rows {
            let record = instrument(id).expect("a rendered id is a known id");
            assert_eq!(fields.len(), 3, "{id}");
            assert_eq!(fields[1], pitch(record), "{id}");
        }
    }

    #[test]
    fn every_instrument_reads_back_the_ids_its_notes_may_carry() {
        let lists = read_back(&render());
        assert_eq!(lists.len(), InstrumentType::ALL.len());
        for (id, set) in &lists {
            let record = instrument(id).expect("a rendered id is a known id");
            // Oracle: the record's articulations without its mute and vibrato ids, which
            // `Request::new` refuses as a note's articulation.
            let expected: BTreeSet<String> = record
                .articulations()
                .iter()
                .filter(|a| Request::new(Some(**a), Mute::Open, Vibrato::Ordinary).is_ok())
                .map(|a| a.id().to_owned())
                .collect();
            assert_eq!(*set, expected, "{id}");
        }
        // The horn's stopped and muted sounds are passage states, not note ids.
        let horn = &lists["horn_f"];
        assert!(horn.contains("staccato") && !horn.contains("stopped"));
        assert!(!horn.contains("con_sordino"));
    }

    #[test]
    fn a_list_is_written_against_a_whole_list_of_its_own_family() {
        let text = render();
        let rows = rows(&text);
        for (id, fields) in &rows {
            let Some(rest) = fields[2].strip_prefix("as ") else {
                continue;
            };
            let base = rest.split(' ').next().expect("a base id");
            let (_, base_fields) = rows
                .iter()
                .find(|(row_id, _)| *row_id == base)
                .expect("the base has a line");
            assert!(
                !base_fields[2].starts_with("as "),
                "{id}: {base} is itself a reference"
            );
            let family = |id: &str| instrument(id).expect("known").family();
            if rest.contains(' ') {
                assert_eq!(family(id), family(base), "{id} against {base}");
            }
        }
    }

    #[test]
    fn the_passage_ids_are_the_mutes_and_vibratos() {
        let text = render();
        let (_, listed) = text
            .split_once("a note refuses them: ")
            .expect("the list is there");
        let listed = listed.split_once('.').expect("the sentence ends").0;
        let listed: BTreeSet<&str> = listed.split(' ').collect();
        let expected: BTreeSet<&str> = Mute::ALL
            .iter()
            .filter_map(|mute| mute.articulation())
            .chain(Vibrato::ALL.iter().filter_map(|v| v.articulation()))
            .map(Articulation::id)
            .collect();
        assert_eq!(listed, expected);
    }

    #[test]
    fn the_rendering_is_deterministic_and_ends_with_one_newline() {
        let text = render();
        assert_eq!(text, render());
        assert!(text.starts_with("## Instruments\n"));
        assert!(text.ends_with('\n') && !text.ends_with("\n\n"));
    }
}
