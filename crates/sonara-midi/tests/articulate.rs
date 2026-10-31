//! Articulation inference, passages and the sampler's `articulation` lane, through the public API.
//!
//! Most lines play at 60 BPM, where a beat lasts one second, so the rules' thresholds in seconds
//! read directly as beats.

use proptest::collection::vec;
use proptest::prelude::*;
use sonara_core::midi::{MidiNote, sort_notes};
use sonara_core::model::automation::CurveKind;
use sonara_core::plugins::{SAMPLER_ORDINARIO, SamplerChoices, sampler_descriptor};
use sonara_core::time::{TempoMap, TempoPoint, TimeSignatureMap};
use sonara_midi::orchestration::fallback::Emulation;
use sonara_midi::orchestration::{Articulation, instrument};
use sonara_midi::performance::{
    Articulated, DynamicsRequest, HumanizeRequest, NoteArticulation, PartId, PartLine,
    PerformanceError, Role, Rule, ScoreContext, Seed, SoundCaps, Source, articulate,
    articulation_choice, perform,
};

use Articulation::{
    ConSordino, Legato, Marcato, Pizzicato, Portato, Roll, Spiccato, Staccatissimo, Staccato,
    Stopped, Sustain, Tenuto, Tremolo,
};

/// One engine block at the slowest supported sample rate, in seconds.
const BLOCK_S: f64 = 512.0 / 22_050.0;

fn score_at(bpm: f64, origin: f64) -> ScoreContext {
    ScoreContext::new(
        TempoMap::constant(bpm).expect("a valid tempo"),
        TimeSignatureMap::default(),
        origin,
    )
    .expect("a valid origin")
}

fn score() -> ScoreContext {
    score_at(60.0, 0.0)
}

fn note(id: &str, pitch: i64, start: f64, length: f64) -> MidiNote {
    MidiNote::new(id, pitch, start, length, 80, 0).expect("a valid note")
}

fn loud(id: &str, pitch: i64, start: f64, length: f64, velocity: i64) -> MidiNote {
    MidiNote::new(id, pitch, start, length, velocity, 0).expect("a valid note")
}

fn written(id: &str, pitch: i64, start: f64, length: f64, articulation: &str) -> MidiNote {
    note(id, pitch, start, length)
        .with_articulation(Some(articulation.to_owned()))
        .expect("a vocabulary id")
}

fn line(instrument_id: &str, notes: Vec<MidiNote>, listed: &[Articulation]) -> PartLine {
    PartLine {
        part: PartId::new("part_1").expect("a valid part id"),
        instrument: instrument(instrument_id).expect("an instrument type"),
        role: Role::Melody,
        notes,
        caps: SoundCaps {
            articulations: listed.to_vec(),
            ..SoundCaps::default()
        },
        dynamics: DynamicsRequest::default(),
        humanize: HumanizeRequest::EXACT,
    }
}

fn run(line: &PartLine) -> Articulated {
    articulate(line, &score()).expect("a valid line")
}

fn of<'a>(result: &'a Articulated, id: &str) -> &'a NoteArticulation {
    result
        .articulations
        .iter()
        .find(|articulation| articulation.note == id)
        .unwrap_or_else(|| panic!("no note {id}"))
}

#[test]
fn articulation_choices_index_the_samplers_parameter() {
    let descriptor = sampler_descriptor(&SamplerChoices {
        presets: vec!["lib/Violin".to_owned()],
        instrument_types: Vec::new(),
        articulations: Articulation::ALL
            .iter()
            .map(|articulation| articulation.id().to_owned())
            .collect(),
    })
    .expect("a preset to play");
    let choices = &descriptor
        .param("articulation")
        .expect("the parameter")
        .choices;
    let choice = |articulation| {
        choices
            .get(usize::from(articulation_choice(articulation)))
            .map(String::as_str)
    };
    assert_eq!(choice(None), Some(SAMPLER_ORDINARIO));
    for articulation in Articulation::ALL {
        assert_eq!(
            choice(Some(articulation)),
            Some(articulation.id()),
            "{}",
            articulation.id()
        );
    }
    // The released indices, which automation lanes store: never 0, `auto`.
    assert_eq!(articulation_choice(None), 1);
    assert_eq!(articulation_choice(Some(Sustain)), 2);
    assert_eq!(articulation_choice(Some(Legato)), 3);
    assert_eq!(articulation_choice(Some(Staccato)), 4);
    assert_eq!(articulation_choice(Some(Roll)), 16);
    assert_eq!(articulation_choice(Some(Articulation::Whisper)), 78);
}

/// What the rule table implies for a note of `length` seconds whose next onset follows `gap`
/// seconds after its own, or that ends its phrase (`gap` is `None`): for a string instrument
/// (`bowed`) or a wind instrument.
fn expected(length: f64, gap: Option<f64>, bowed: bool) -> (Rule, Option<Articulation>) {
    let duty = gap.map_or(1.0, |gap| length / gap);
    if length < 0.12 {
        return (
            Rule::VeryShort,
            Some(if bowed { Spiccato } else { Staccatissimo }),
        );
    }
    if length < 0.25 || (duty < 0.5 && length < 0.4) {
        let fast = gap.is_some_and(|gap| gap <= 0.2);
        return (
            Rule::Short,
            Some(if bowed && fast { Spiccato } else { Staccato }),
        );
    }
    if (0.5..0.85).contains(&duty) {
        return (Rule::Separated, Some(if bowed { Portato } else { Tenuto }));
    }
    (Rule::Ordinary, None)
}

#[test]
fn inference_rules_are_exhaustive_over_lengths_and_duties() {
    let lengths = [0.05, 0.1, 0.15, 0.2, 0.3, 0.35, 0.5, 0.8, 1.2];
    let duties = [0.25, 0.45, 0.55, 0.7, 0.8, 0.9, 0.95];
    for (instrument_id, bowed) in [("violin", true), ("horn_f", false), ("flute", false)] {
        for length in lengths {
            for duty in duties {
                let next = length / duty;
                let notes = vec![note("n_a", 60, 0.0, length), note("n_b", 62, next, 2.0)];
                let result = run(&line(instrument_id, notes, &[]));
                // The next onset counts only before the note's end plus a beat's rest.
                let gap = (next < length + 1.0).then_some(next);
                let (rule, asked) = expected(length, gap, bowed);
                let first = of(&result, "n_a");
                let case = format!("{instrument_id}, {length} s, duty {duty}");
                assert_eq!(first.source, Source::Inferred(rule), "{case}");
                assert_eq!(first.asked, asked, "{case}");
                // The sound lists nothing, so every note plays the ordinary sound.
                assert_eq!(first.plays, None, "{case}");
                // The second note ends the line: long, nothing follows.
                let last = of(&result, "n_b");
                assert_eq!(last.source, Source::Inferred(Rule::Ordinary), "{case}");
                assert_eq!(last.asked, None, "{case}");
            }
        }
    }
}

#[test]
fn voices_and_struck_instruments_shape_their_notes_their_own_way() {
    // A very short note and a separated one, then a long one.
    let notes = || {
        vec![
            note("n_a", 60, 0.0, 0.1),
            note("n_b", 62, 0.5, 0.6),
            note("n_c", 64, 1.5, 2.0),
        ]
    };
    let choir = run(&line("choir_soprano", notes(), &[]));
    assert_eq!(of(&choir, "n_a").asked, Some(Staccato));
    assert_eq!(of(&choir, "n_b").asked, Some(Tenuto));
    assert_eq!(of(&choir, "n_c").asked, None);
    // A struck or plucked note's length plays its shape: no switch for any of them.
    for struck in ["piano_concert_grand", "harp", "xylophone"] {
        let result = run(&line(struck, notes(), &[Staccato, Tenuto]));
        for articulation in &result.articulations {
            assert_eq!(
                articulation.source,
                Source::Inferred(Rule::Ordinary),
                "{struck}"
            );
            assert_eq!(articulation.plays, None, "{struck}");
        }
    }
}

#[test]
fn an_overlap_at_another_pitch_is_legato_on_both_notes() {
    let joined = vec![
        note("n_a", 60, 0.0, 1.05),
        note("n_b", 62, 1.0, 1.0),
        note("n_c", 64, 2.0, 1.0),
    ];
    let result = run(&line("violin", joined.clone(), &[Legato]));
    for id in ["n_a", "n_b"] {
        let articulation = of(&result, id);
        assert_eq!(articulation.source, Source::Inferred(Rule::Joined), "{id}");
        assert_eq!(articulation.plays, Some(Legato), "{id}");
    }
    // `n_b` ends where `n_c` starts: touching is not joining.
    assert_eq!(of(&result, "n_c").source, Source::Inferred(Rule::Ordinary));
    assert_eq!(
        result
            .passages
            .iter()
            .map(|passage| passage.articulation)
            .collect::<Vec<_>>(),
        [Some(Legato), None]
    );
    // The notes come back as they were given: the overlap is the player's.
    assert_eq!(result.notes, joined);

    // The same pitch is a repetition, not a join.
    let repeated = vec![note("n_a", 60, 0.0, 1.05), note("n_b", 60, 1.0, 1.0)];
    let result = run(&line("violin", repeated, &[Legato]));
    assert!(
        result
            .articulations
            .iter()
            .all(|articulation| articulation.asked.is_none())
    );

    // A chord still sounding when the next note starts is no single line.
    let chord = vec![
        note("n_a", 60, 0.0, 1.05),
        note("n_d", 67, 0.0, 1.05),
        note("n_b", 62, 1.0, 1.0),
    ];
    let result = run(&line("violin", chord, &[Legato]));
    assert!(
        result
            .articulations
            .iter()
            .all(|articulation| articulation.asked.is_none())
    );

    // A piano's overlapping notes are finger legato, its ordinary sound.
    let result = run(&line(
        "piano_concert_grand",
        vec![note("n_a", 60, 0.0, 1.05), note("n_b", 62, 1.0, 1.0)],
        &[Legato],
    ));
    assert!(
        result
            .articulations
            .iter()
            .all(|articulation| articulation.asked.is_none())
    );
}

#[test]
fn an_accented_short_note_is_marcato_only_when_the_sound_has_it() {
    // Medians of the phrase: 80; the accent is 20 steps above it.
    let notes = || {
        vec![
            note("n_a", 60, 0.0, 1.0),
            loud("n_b", 62, 1.0, 0.3, 100),
            note("n_c", 64, 2.0, 1.0),
            note("n_d", 65, 3.0, 1.0),
            loud("n_e", 67, 4.0, 0.3, 99),
            note("n_f", 69, 5.0, 1.0),
        ]
    };
    let with = run(&line("trumpet_c", notes(), &[Marcato, Staccato]));
    assert_eq!(of(&with, "n_b").source, Source::Inferred(Rule::Accented));
    assert_eq!(of(&with, "n_b").plays, Some(Marcato));
    // 19 steps above the median is no accent: a short, detached note.
    assert_eq!(of(&with, "n_e").source, Source::Inferred(Rule::Short));
    assert_eq!(of(&with, "n_e").plays, Some(Staccato));
    let without = run(&line("trumpet_c", notes(), &[Staccato]));
    assert_eq!(of(&without, "n_b").source, Source::Inferred(Rule::Short));
    assert_eq!(of(&without, "n_b").plays, Some(Staccato));
    // The velocity stays as written either way.
    assert_eq!(
        with.notes
            .iter()
            .find(|n| n.id == "n_b")
            .map(|n| n.velocity),
        Some(100)
    );
}

/// `count` strokes of `pitch` every `step` beats, each `step` long.
fn strokes(count: usize, pitch: i64, step: f64) -> Vec<MidiNote> {
    (0..count)
        .map(|index| {
            let at = f64::from(u32::try_from(index).expect("few strokes")) * step;
            note(&format!("s_{index}"), pitch, at, step)
        })
        .collect()
}

#[test]
fn repeated_strokes_merge_into_a_tremolo_or_roll_the_sound_plays() {
    for (instrument_id, stroke) in [
        ("violin", Tremolo),
        ("timpani", Roll),
        ("glockenspiel", Tremolo),
    ] {
        let result = run(&line(instrument_id, strokes(8, 72, 0.0625), &[stroke]));
        assert_eq!(result.notes.len(), 1, "{instrument_id}");
        let held = result.notes.first().expect("one note");
        assert_eq!(
            (held.id.as_str(), held.pitch, held.start, held.length),
            ("s_0", 72, 0.0, 0.5),
            "{instrument_id}"
        );
        let articulation = of(&result, "s_0");
        assert_eq!(articulation.source, Source::Inferred(Rule::Strokes));
        assert_eq!(articulation.asked, Some(stroke));
        assert_eq!(articulation.plays, Some(stroke));
        assert_eq!(
            articulation.merged,
            (0..8).map(|index| format!("s_{index}")).collect::<Vec<_>>()
        );
        assert_eq!(result.passages.len(), 1);
    }
    // Without the articulation the strokes stay as written, each a very short note.
    let result = run(&line("violin", strokes(8, 72, 0.0625), &[Spiccato]));
    assert_eq!(result.notes, strokes(8, 72, 0.0625));
    assert!(result.articulations.iter().all(|articulation| {
        articulation.source == Source::Inferred(Rule::VeryShort)
            && articulation.plays == Some(Spiccato)
            && articulation.merged.is_empty()
    }));
    // Three strokes are too few, and strokes 0.1 s apart too slow.
    assert_eq!(
        run(&line("violin", strokes(3, 72, 0.0625), &[Tremolo]))
            .notes
            .len(),
        3
    );
    assert_eq!(
        run(&line("violin", strokes(6, 72, 0.1), &[Tremolo]))
            .notes
            .len(),
        6
    );
    // A woodwind's repeated notes are tonguing, never merged.
    assert_eq!(
        run(&line("flute", strokes(8, 72, 0.0625), &[Tremolo]))
            .notes
            .len(),
        8
    );
    // A stroke with its own articulation breaks the run: 4 + 1 + 3.
    let mut broken = strokes(8, 72, 0.0625);
    if let Some(stroke) = broken.get_mut(4) {
        stroke.articulation = Some("staccato".to_owned());
    }
    let result = run(&line("violin", broken, &[Tremolo, Staccato]));
    assert_eq!(
        result
            .notes
            .iter()
            .map(|n| n.id.as_str())
            .collect::<Vec<_>>(),
        ["s_0", "s_4", "s_5", "s_6", "s_7"]
    );
    assert_eq!(of(&result, "s_0").merged.len(), 4);
    assert_eq!(of(&result, "s_4").source, Source::Written);
}

#[test]
fn an_articulation_the_sound_lacks_falls_back_through_the_vocabulary() {
    let very_short = || vec![note("n_a", 60, 0.0, 0.1), note("n_b", 62, 0.5, 2.0)];
    let plays = |instrument_id: &str, listed: &[Articulation]| {
        let result = run(&line(instrument_id, very_short(), listed));
        let articulation = of(&result, "n_a");
        (articulation.asked, articulation.plays, articulation.rewrite)
    };
    let quarter = Some(Emulation::Shorten { divisor: 4 });
    // A string's spiccato: its row's staccato, then a sustain cut short, then the ordinary sound
    // with the row's emulation.
    assert_eq!(
        plays("violin", &[Staccato, Sustain]),
        (Some(Spiccato), Some(Staccato), None)
    );
    assert_eq!(
        plays("violin", &[Sustain]),
        (Some(Spiccato), Some(Sustain), quarter)
    );
    assert_eq!(plays("violin", &[]), (Some(Spiccato), None, quarter));
    // A wind's staccatissimo refines staccato, so staccato comes first, then staccato's row.
    assert_eq!(
        plays("oboe", &[Spiccato, Staccato]),
        (Some(Staccatissimo), Some(Staccato), None)
    );
    assert_eq!(
        plays("oboe", &[Spiccato]),
        (Some(Staccatissimo), Some(Spiccato), None)
    );

    // A string's separated note: portato, then tenuto, then tenuto's row.
    let separated = || vec![note("n_a", 60, 0.0, 0.6), note("n_b", 62, 1.0, 2.0)];
    let plays = |listed: &[Articulation]| {
        let result = run(&line("cello", separated(), listed));
        let articulation = of(&result, "n_a");
        (articulation.asked, articulation.plays, articulation.rewrite)
    };
    assert_eq!(plays(&[Portato]), (Some(Portato), Some(Portato), None));
    assert_eq!(plays(&[Tenuto]), (Some(Portato), Some(Tenuto), None));
    assert_eq!(plays(&[Sustain]), (Some(Portato), Some(Sustain), None));
    assert_eq!(plays(&[Staccato]), (Some(Portato), None, None));

    // Legato on one line, without a legato patch: a sustain that the overlap joins.
    let joined = vec![note("n_a", 60, 0.0, 1.05), note("n_b", 62, 1.0, 1.0)];
    let result = run(&line("viola", joined, &[Sustain]));
    assert_eq!(of(&result, "n_a").plays, Some(Sustain));
    assert_eq!(of(&result, "n_a").rewrite, Some(Emulation::Overlap));

    // Marcato's staccato suits only short notes; a long note keeps its accent as a rewrite.
    let marked = |length| vec![written("n_a", 60, 0.0, length, "marcato")];
    let result = run(&line("trombone_tenor", marked(0.2), &[Staccato]));
    assert_eq!(of(&result, "n_a").plays, Some(Staccato));
    let result = run(&line("trombone_tenor", marked(2.0), &[Staccato]));
    assert_eq!(of(&result, "n_a").plays, None);
    assert_eq!(of(&result, "n_a").rewrite, Some(Emulation::Accent));
}

#[test]
fn a_written_articulation_overrides_inference() {
    let notes = vec![
        written("n_a", 55, 0.0, 0.1, "pizzicato"),
        written("n_b", 57, 1.0, 2.0, "con_sordino"),
        written("n_c", 59, 4.0, 2.0, "stopped"),
        written("n_d", 60, 7.0, 0.1, "legato"),
    ];
    let result = run(&line("violin", notes.clone(), &[Pizzicato, ConSordino]));
    let summary: Vec<_> = result
        .articulations
        .iter()
        .map(|articulation| (articulation.source, articulation.asked, articulation.plays))
        .collect();
    assert_eq!(
        summary,
        [
            (Source::Written, Some(Pizzicato), Some(Pizzicato)),
            (Source::Written, Some(ConSordino), Some(ConSordino)),
            // A stopped horn's sound refines the mute; the mute stands in for it.
            (Source::Written, Some(Stopped), Some(ConSordino)),
            // Asked for and unavailable: the ordinary sound, with the legato row's emulation.
            (Source::Written, Some(Legato), None),
        ]
    );
    assert_eq!(
        result.articulations.last().and_then(|a| a.rewrite),
        Some(Emulation::Overlap)
    );
    // The notes keep their own field.
    assert_eq!(result.notes, notes);
}

#[test]
fn notes_the_vocabulary_or_the_project_rejects_are_errors() {
    let unknown = vec![
        note("n_a", 60, 0.0, 1.0)
            .with_articulation(Some("sul_tasto_molto".to_owned()))
            .expect("an id shape"),
    ];
    assert_eq!(
        articulate(&line("violin", unknown, &[]), &score()),
        Err(PerformanceError::UnknownArticulation {
            note: "n_a".to_owned(),
            id: "sul_tasto_molto".to_owned(),
        })
    );
    let cases: [fn(&mut MidiNote); 5] = [
        |n| n.start = -0.5,
        |n| n.start = f64::NAN,
        |n| n.length = 0.0,
        |n| n.length = f64::INFINITY,
        |n| n.velocity = 0,
    ];
    for (index, change) in cases.into_iter().enumerate() {
        let mut bad = note("n_bad", 60, 1.0, 1.0);
        change(&mut bad);
        let notes = vec![note("n_ok", 62, 0.0, 1.0), bad];
        assert_eq!(
            articulate(&line("violin", notes.clone(), &[]), &score()),
            Err(PerformanceError::InvalidNote("n_bad".to_owned())),
            "case {index}"
        );
        assert_eq!(
            perform(&line("violin", notes, &[]), &score(), Seed::default()),
            Err(PerformanceError::InvalidNote("n_bad".to_owned())),
            "case {index}"
        );
    }
    assert_eq!(
        PerformanceError::UnknownArticulation {
            note: "n_a".to_owned(),
            id: "x".to_owned()
        }
        .to_string(),
        "note n_a has the articulation \"x\", which is not in the vocabulary"
    );
}

#[test]
fn a_lone_implied_note_joins_its_neighbours_passage() {
    // A legato phrase, one short note, more legato: one legato passage.
    let notes = vec![
        note("n_1", 60, 0.0, 1.05),
        note("n_2", 62, 1.0, 1.0),
        note("n_3", 64, 2.5, 0.2),
        note("n_4", 65, 3.0, 1.05),
        note("n_5", 67, 4.0, 1.0),
    ];
    let result = run(&line("violin", notes, &[Legato, Staccato]));
    assert_eq!(of(&result, "n_3").plays, Some(Staccato));
    assert_eq!(result.passages.len(), 1);
    assert_eq!(
        result.passages.first().map(|p| p.articulation),
        Some(Some(Legato))
    );
    let overruled: Vec<&str> = result
        .articulations
        .iter()
        .filter(|a| a.overruled)
        .map(|a| a.note.as_str())
        .collect();
    assert_eq!(overruled, ["n_3"]);

    // A lone first note joins the passage after it.
    let notes = vec![
        note("n_1", 60, 0.0, 0.2),
        note("n_2", 62, 0.5, 1.05),
        note("n_3", 64, 1.5, 1.0),
    ];
    let result = run(&line("violin", notes, &[Legato, Staccato]));
    assert_eq!(result.passages.len(), 1);
    assert!(of(&result, "n_1").overruled);

    // Two short notes, or one lasting a beat until the next passage, keep their own passage.
    for notes in [
        vec![
            note("n_1", 60, 0.0, 1.05),
            note("n_2", 62, 1.0, 1.0),
            note("n_3", 64, 2.5, 0.2),
            note("n_4", 65, 2.8, 0.2),
            note("n_5", 67, 3.5, 1.05),
            note("n_6", 69, 4.5, 1.0),
        ],
        vec![
            note("n_1", 60, 0.0, 1.05),
            note("n_2", 62, 1.0, 1.0),
            note("n_3", 64, 2.5, 0.2),
            note("n_5", 67, 3.5, 1.05),
            note("n_6", 69, 4.5, 1.0),
        ],
    ] {
        let result = run(&line("violin", notes, &[Legato, Staccato]));
        let articulations: Vec<_> = result.passages.iter().map(|p| p.articulation).collect();
        assert_eq!(articulations, [Some(Legato), Some(Staccato), Some(Legato)]);
        assert!(result.articulations.iter().all(|a| !a.overruled));
    }

    // A written articulation keeps its passage, however short.
    let notes = vec![
        note("n_1", 60, 0.0, 1.05),
        note("n_2", 62, 1.0, 1.0),
        written("n_3", 64, 2.5, 0.2, "pizzicato"),
        note("n_4", 65, 3.0, 1.05),
        note("n_5", 67, 4.0, 1.0),
    ];
    let result = run(&line("violin", notes, &[Legato, Pizzicato]));
    let articulations: Vec<_> = result.passages.iter().map(|p| p.articulation).collect();
    assert_eq!(articulations, [Some(Legato), Some(Pizzicato), Some(Legato)]);
}

#[test]
fn a_chord_plays_what_its_top_note_needs() {
    // The top note joins the next legato; the lower note of the chord cannot.
    let notes = vec![
        note("n_low", 55, 0.0, 0.6),
        note("n_top", 67, 0.0, 1.05),
        note("n_next", 69, 1.0, 2.0),
    ];
    let result = run(&line("violin", notes, &[Legato, Portato]));
    assert_eq!(of(&result, "n_top").plays, Some(Legato));
    assert_eq!(of(&result, "n_low").plays, Some(Portato));
    assert!(of(&result, "n_low").overruled);
    assert_eq!(
        result
            .passages
            .iter()
            .map(|p| p.articulation)
            .collect::<Vec<_>>(),
        [Some(Legato)]
    );
}

#[test]
fn switches_lead_their_passage_by_two_blocks_or_meet_halfway() {
    let lead_beats = 2.0 * BLOCK_S; // at 60 BPM
    let notes = vec![
        note("n_1", 60, 0.0, 1.05),
        note("n_2", 62, 1.0, 1.0),
        note("n_3", 64, 2.0, 0.2),
        note("n_4", 65, 2.5, 0.2),
    ];
    let result = run(&line("violin", notes, &[Legato, Staccato]));
    let switches: Vec<_> = result.passages.iter().map(|p| p.switch).collect();
    assert_eq!(switches.len(), 2);
    let (first, second) = (switches[0], switches[1]);
    assert_eq!(first.beat, 0.0);
    assert_eq!(first.value, f64::from(articulation_choice(Some(Legato))));
    assert_eq!(first.curve, CurveKind::Hold);
    assert!(
        (second.beat - (2.0 - lead_beats)).abs() < 1e-9,
        "{second:?}"
    );
    assert_eq!(second.value, f64::from(articulation_choice(Some(Staccato))));
    assert!(result.passages.iter().all(|p| !p.possibly_late));
    assert_eq!(result.lane(), switches);

    // Note-ons 0.06 s apart: halfway, still a block ahead. 0.03 s apart: possibly late.
    for (gap, late) in [(0.06, false), (0.03, true)] {
        let notes = vec![
            written("n_1", 60, 0.0, 0.5, "pizzicato"),
            written("n_2", 62, 1.0, 0.5, "pizzicato"),
            written("n_3", 64, 1.0 + gap, 0.5, "staccato"),
            written("n_4", 65, 2.0, 0.5, "staccato"),
        ];
        let result = run(&line("violin", notes, &[Pizzicato, Staccato]));
        let second = result.passages.get(1).expect("two passages");
        assert!(
            (second.switch.beat - (1.0 + gap / 2.0)).abs() < 1e-9,
            "{gap}"
        );
        assert_eq!(second.possibly_late, late, "{gap}");
        assert!(
            second.switch.beat > 1.0,
            "after the pizzicato's last note-on"
        );
    }
}

#[test]
fn rules_and_switches_read_the_timeline_tempo_at_the_clips_origin() {
    // 60 BPM up to timeline beat 8, then 120 BPM; the clip's content beat 0 is timeline beat 6.
    let tempo = TempoMap::new(vec![
        TempoPoint::new(0.0, 60.0, false).expect("a point"),
        TempoPoint::new(8.0, 120.0, false).expect("a point"),
    ])
    .expect("a tempo map");
    let score =
        ScoreContext::new(tempo.clone(), TimeSignatureMap::default(), 6.0).expect("a valid origin");
    // Content beat 1 is at 60 BPM, content beat 4 at 120 BPM: the same 0.2 beats last 0.2 s
    // there (short) and 0.1 s here (very short).
    let notes = vec![
        note("n_1", 60, 1.0, 0.2),
        note("n_2", 62, 1.5, 2.0),
        note("n_3", 64, 4.0, 0.2),
        note("n_4", 65, 4.5, 2.0),
    ];
    let result = articulate(&line("violin", notes, &[]), &score).expect("a valid line");
    assert_eq!(of(&result, "n_1").source, Source::Inferred(Rule::Short));
    assert_eq!(of(&result, "n_3").source, Source::Inferred(Rule::VeryShort));

    let notes = vec![
        note("n_1", 60, 0.0, 1.05),
        note("n_2", 62, 1.0, 2.0),
        written("n_3", 64, 4.0, 1.0, "staccato"),
    ];
    let result =
        articulate(&line("violin", notes, &[Legato, Staccato]), &score).expect("a valid line");
    let switches = result.lane();
    assert_eq!(switches.first().map(|point| point.beat), Some(6.0));
    // Timeline beat 10 lies at 120 BPM, where two blocks are 4 BLOCK_S beats.
    let expected = tempo.seconds_to_beat(tempo.beat_to_seconds(10.0) - 2.0 * BLOCK_S);
    assert!((expected - (10.0 - 4.0 * BLOCK_S)).abs() < 1e-9);
    let second = switches.get(1).expect("two passages").beat;
    assert!((second - expected).abs() < 1e-9, "{second}");
}

#[test]
fn timpani_hits_then_a_roll_switch_two_blocks_before_the_roll() {
    // Hits on beat 1 of bars 1, 4, 5 and 7, a roll through bar 8 (4/4).
    let notes = vec![
        note("n_1", 38, 0.0, 1.0),
        note("n_2", 45, 12.0, 1.0),
        note("n_3", 38, 16.0, 1.0),
        note("n_4", 45, 24.0, 1.0),
        written("n_5", 38, 28.0, 4.0, "roll"),
    ];
    let result =
        articulate(&line("timpani", notes, &[Roll]), &score_at(72.0, 0.0)).expect("a valid line");
    let lane: Vec<(f64, f64)> = result
        .lane()
        .iter()
        .map(|point| (point.beat, point.value))
        .collect();
    let lead_beats = 2.0 * BLOCK_S * 72.0 / 60.0;
    assert_eq!(lane.len(), 2);
    assert_eq!(lane[0], (0.0, f64::from(articulation_choice(None))));
    assert!((lane[1].0 - (28.0 - lead_beats)).abs() < 1e-9);
    assert_eq!(lane[1].1, f64::from(articulation_choice(Some(Roll))));
}

#[test]
fn perform_carries_the_articulation_of_its_line() {
    let line = line(
        "violin",
        vec![
            note("n_2", 62, 1.0, 1.0),
            note("n_1", 60, 0.0, 1.05),
            note("n_3", 64, 2.5, 0.1),
            note("n_4", 64, 2.75, 0.1),
        ],
        &[Legato, Spiccato],
    );
    let performed = perform(&line, &score(), Seed::new(3)).expect("a valid line");
    let articulated = articulate(&line, &score()).expect("a valid line");
    // The performance keeps the articulated notes; only their velocities follow the dynamics.
    let unvoiced = |notes: &[MidiNote]| -> Vec<MidiNote> {
        notes
            .iter()
            .map(|note| MidiNote {
                velocity: 1,
                ..note.clone()
            })
            .collect()
    };
    assert_eq!(unvoiced(&performed.notes), unvoiced(&articulated.notes));
    assert_eq!(performed.articulations, articulated.articulations);
    assert_eq!(performed.passages, articulated.passages);
    assert_eq!(performed.articulation_lane(), articulated.lane());
    assert_eq!(
        performed
            .passages
            .iter()
            .map(|p| p.articulation)
            .collect::<Vec<_>>(),
        [Some(Legato), Some(Spiccato)]
    );
}

/// The articulations a property line's sound may list.
const LISTABLE: [Articulation; 10] = [
    Sustain,
    Legato,
    Staccato,
    Spiccato,
    Staccatissimo,
    Tenuto,
    Portato,
    Marcato,
    Tremolo,
    Roll,
];

const INSTRUMENTS: [&str; 6] = [
    "violin",
    "flute",
    "horn_f",
    "timpani",
    "piano_concert_grand",
    "choir_alto",
];

/// Notes on a 1/16-beat grid, few pitches so that repetitions and overlaps are common; ids carry
/// `prefix`.
fn notes_strategy(prefix: &'static str) -> impl Strategy<Value = Vec<MidiNote>> {
    vec((60_i64..64, 0_u32..64, 1_u32..24, 40_i64..128), 0..20).prop_map(move |raw| {
        raw.into_iter()
            .enumerate()
            .map(|(index, (pitch, start, length, velocity))| {
                MidiNote::new(
                    format!("{prefix}_{index:03}"),
                    pitch,
                    f64::from(start) / 16.0,
                    f64::from(length) / 16.0,
                    velocity,
                    0,
                )
                .expect("a valid note")
            })
            .collect()
    })
}

fn caps_strategy() -> impl Strategy<Value = Vec<Articulation>> {
    proptest::sample::subsequence(LISTABLE.to_vec(), 0..=LISTABLE.len())
}

/// What a note's articulation says about the note alone, without its passage.
fn own(articulation: &NoteArticulation) -> NoteArticulation {
    NoteArticulation {
        passage: 0,
        overruled: false,
        ..articulation.clone()
    }
}

proptest! {
    /// A rest of at least a beat separates what the rules read: inferring a line equals inferring
    /// its two halves on their own (TST-09).
    #[test]
    fn inference_on_a_line_equals_inference_on_its_halves_split_at_a_rest(
        first in notes_strategy("a"),
        second in notes_strategy("b"),
        rest in 0_u32..8,
        bpm in prop_oneof![Just(60.0), Just(90.0), Just(132.0)],
        instrument_index in 0..INSTRUMENTS.len(),
        listed in caps_strategy(),
    ) {
        let instrument_id = INSTRUMENTS.get(instrument_index).copied().unwrap_or("violin");
        let first_end = first.iter().map(MidiNote::end).fold(0.0, f64::max);
        let offset = first_end + 1.0 + f64::from(rest) / 4.0;
        let second: Vec<MidiNote> = second
            .into_iter()
            .map(|mut note| {
                note.start += offset;
                note
            })
            .collect();
        let score = score_at(bpm, 0.0);
        let infer = |notes: Vec<MidiNote>| {
            articulate(&line(instrument_id, notes, &listed), &score).expect("a valid line")
        };
        let whole = infer(first.iter().chain(&second).cloned().collect());
        let halves = [infer(first), infer(second)];
        let whole_own: Vec<NoteArticulation> = whole.articulations.iter().map(own).collect();
        let halves_own: Vec<NoteArticulation> = halves
            .iter()
            .flat_map(|half| half.articulations.iter().map(own))
            .collect();
        prop_assert_eq!(whole_own, halves_own);
        let halves_notes: Vec<MidiNote> =
            halves.iter().flat_map(|half| half.notes.clone()).collect();
        prop_assert_eq!(whole.notes, halves_notes);
    }

    /// Only articulations the sound lists are played; passages and their switches keep the
    /// engine's timing (TST-07).
    #[test]
    fn passages_play_listed_articulations_and_switch_in_time(
        notes in notes_strategy("n"),
        origin in 0_u32..32,
        bpm in prop_oneof![Just(60.0), Just(90.0), Just(132.0), Just(200.0)],
        instrument_index in 0..INSTRUMENTS.len(),
        listed in caps_strategy(),
    ) {
        let instrument_id = INSTRUMENTS.get(instrument_index).copied().unwrap_or("violin");
        let origin = f64::from(origin) / 4.0;
        let score = score_at(bpm, origin);
        let tempo = score.tempo().clone();
        let result = articulate(&line(instrument_id, notes.clone(), &listed), &score)
            .expect("a valid line");
        let is_listed = |articulation: Option<Articulation>| {
            articulation.is_none_or(|articulation| listed.contains(&articulation))
        };
        prop_assert_eq!(result.articulations.len(), result.notes.len());
        for (note, articulation) in result.notes.iter().zip(&result.articulations) {
            prop_assert_eq!(&note.id, &articulation.note);
            prop_assert!(is_listed(articulation.plays), "{:?}", articulation);
            let passage = result.passages.get(articulation.passage);
            let inside = passage.is_some_and(|passage| {
                passage.start <= note.start + 1e-9 && note.start <= passage.last_onset + 1e-9
            });
            prop_assert!(inside, "{} lies outside its passage", note.id);
            prop_assert_eq!(
                articulation.overruled,
                passage.is_some_and(|passage| passage.articulation != articulation.plays)
            );
        }
        // Every stroke merged or kept: no note is lost.
        let merged: usize = result
            .articulations
            .iter()
            .map(|articulation| articulation.merged.len().saturating_sub(1))
            .sum();
        prop_assert_eq!(result.notes.len() + merged, notes.len());
        if !listed.contains(&Tremolo) && !listed.contains(&Roll) {
            let mut sorted = notes.clone();
            sort_notes(&mut sorted);
            prop_assert_eq!(&result.notes, &sorted);
        }
        let seconds = |beat: f64| tempo.beat_to_seconds(beat);
        for (index, passage) in result.passages.iter().enumerate() {
            prop_assert!(is_listed(passage.articulation));
            // A choice index and a beat copied unchanged: exact equality, bit for bit.
            let choice = f64::from(articulation_choice(passage.articulation));
            prop_assert_eq!(passage.switch.value.to_bits(), choice.to_bits());
            prop_assert_eq!(passage.switch.curve, CurveKind::Hold);
            prop_assert!(passage.start <= passage.last_onset);
            let first_on = origin + passage.start;
            prop_assert!(passage.switch.beat <= first_on + 1e-9);
            match index.checked_sub(1).and_then(|previous| result.passages.get(previous)) {
                None => {
                    prop_assert_eq!(passage.switch.beat.to_bits(), origin.to_bits());
                    prop_assert!(!passage.possibly_late);
                }
                Some(previous) => {
                    prop_assert_ne!(previous.articulation, passage.articulation);
                    prop_assert!(previous.last_onset < passage.start);
                    // Never at or before the previous passage's last note-on.
                    prop_assert!(passage.switch.beat > origin + previous.last_onset);
                    let lead = seconds(first_on) - seconds(passage.switch.beat);
                    prop_assert!(lead <= 2.0 * BLOCK_S + 1e-9);
                    if lead < BLOCK_S - 1e-9 {
                        prop_assert!(passage.possibly_late);
                    }
                    if lead > BLOCK_S + 1e-9 {
                        prop_assert!(!passage.possibly_late);
                    }
                }
            }
        }
    }

    /// Whatever order the line lists its notes in, the articulation is the same (TST-04).
    #[test]
    fn articulation_ignores_the_order_of_the_input(
        (notes, shuffled) in notes_strategy("n").prop_flat_map(|notes| {
            let shuffled = Just(notes.clone()).prop_shuffle();
            (Just(notes), shuffled)
        }),
        listed in caps_strategy(),
    ) {
        let in_order = run(&line("violin", notes, &listed));
        let reordered = run(&line("violin", shuffled, &listed));
        prop_assert_eq!(in_order, reordered);
    }
}
