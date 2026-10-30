//! Contract tests of the rule predicates and the harp and timpani derivations through the public
//! API. Each rule has a passage that fires it and one that stays quiet, written by hand from the
//! rule's check text and the thresholds it names (TST-10).
//!
//! The score runs at crotchet = 120 in 4/4 unless a test says otherwise, so a beat lasts 500 ms,
//! the moderate tempo at which the sources' spans are measured.

use proptest::prelude::*;
use sonara_core::midi::MidiNote;
use sonara_core::time::{TempoMap, TimeSignatureMap};
use sonara_midi::orchestration::InstrumentType as I;
use sonara_midi::orchestration::rules::{DynamicBand, Level, RuleId, RuleInput};
use sonara_midi::orchestration::{InstrumentType, LetterName, SoundingPitch};
use sonara_midi::performance::check::{
    Accidental, CHECKED_RULES, CheckError, Hit, Line, PedalSetting, Shortfall, check, check_rule,
    harp_pedals, timpani_tunings, velocity_band,
};
use sonara_midi::performance::{PartId, ScoreContext};

fn part(slug: &str) -> PartId {
    PartId::new(slug).expect("a valid part id")
}

/// A note at mezzo-forte (velocity 80).
fn note(id: &str, pitch: i64, start: f64, length: f64) -> MidiNote {
    MidiNote::new(id, pitch, start, length, 80, 0).expect("a valid note")
}

/// A note at `velocity`.
fn at(velocity: i64, id: &str, pitch: i64, start: f64, length: f64) -> MidiNote {
    MidiNote::new(id, pitch, start, length, velocity, 0).expect("a valid note")
}

/// A note played with `articulation`.
fn played(articulation: &str, id: &str, pitch: i64, start: f64, length: f64) -> MidiNote {
    note(id, pitch, start, length)
        .with_articulation(Some(articulation.to_owned()))
        .expect("a vocabulary id")
}

/// `count` notes of `pitch`, `step` beats apart and `length` long, from beat `start`; ids
/// `{prefix}0`, `{prefix}1`, ...
fn run(prefix: &str, pitch: i64, start: f64, step: f64, length: f64, count: u32) -> Vec<MidiNote> {
    (0..count)
        .map(|index| {
            note(
                &format!("{prefix}{index}"),
                pitch,
                start + f64::from(index) * step,
                length,
            )
        })
        .collect()
}

/// A line of notes `pitches`, one per beat from beat 0, a beat long; ids `{prefix}0`, ...
fn line_of(prefix: &str, pitches: &[i64]) -> Vec<MidiNote> {
    pitches
        .iter()
        .zip(0_u32..)
        .map(|(&pitch, index)| note(&format!("{prefix}{index}"), pitch, f64::from(index), 1.0))
        .collect()
}

fn score() -> ScoreContext {
    ScoreContext::new(
        TempoMap::constant(120.0).expect("a valid tempo"),
        TimeSignatureMap::default(),
        0.0,
    )
    .expect("a valid origin")
}

/// The hits of `rule` on `parts`, each a part id, its instrument and its notes.
fn hits(rule: RuleId, parts: &[(&PartId, InstrumentType, &[MidiNote])]) -> Vec<Hit> {
    let lines: Vec<Line<'_>> = parts
        .iter()
        .map(|&(id, instrument, notes)| Line::new(id, instrument, notes))
        .collect();
    check_rule(rule, &lines, &score()).expect("the inputs are valid")
}

/// The note ids of each hit.
fn notes_of(hits: &[Hit]) -> Vec<Vec<&str>> {
    hits.iter()
        .map(|hit| hit.notes.iter().map(String::as_str).collect())
        .collect()
}

/// The levels of the hits.
fn levels(hits: &[Hit]) -> Vec<Level> {
    hits.iter().map(|hit| hit.level).collect()
}

// The checks as a whole.

#[test]
fn the_checked_rules_need_only_what_a_project_holds() {
    // The rules checked from a project read no role, harmony, weights, register factors, harp
    // pedals or timpani tuning: a project stores none of them.
    let missing = [
        RuleInput::Role,
        RuleInput::Harmony,
        RuleInput::Weights,
        RuleInput::RegisterFactors,
        RuleInput::HarpPedals,
        RuleInput::TimpaniTuning,
    ];
    for rule in CHECKED_RULES {
        for input in missing {
            assert!(!rule.get().inputs().contains(&input), "{}", rule.code());
        }
    }
    // Listed once each, in the catalogue's order.
    assert!(CHECKED_RULES.is_sorted());
    let codes: Vec<&str> = CHECKED_RULES.iter().map(|rule| rule.code()).collect();
    assert_eq!(
        codes,
        [
            "RNG-01", "RNG-02", "RNG-03", "SPC-01", "SPC-02", "DBL-02", "DBL-03", "DBL-04",
            "DBL-09", "VOI-12", "PLY-01", "PLY-02", "PLY-04", "PLY-05", "PLY-06", "PLY-12",
            "PLY-14", "PLY-15", "TEX-01", "TEX-02",
        ]
    );
}

#[test]
fn velocity_bands_split_at_mp_and_f() {
    // TST-10: pp 1-39 and p 40-54 are soft, mp 55-69 and mf 70-84 medium, f and ff loud.
    let cases = [
        (1, DynamicBand::Soft),
        (54, DynamicBand::Soft),
        (55, DynamicBand::Medium),
        (84, DynamicBand::Medium),
        (85, DynamicBand::Loud),
        (127, DynamicBand::Loud),
    ];
    for (velocity, band) in cases {
        assert_eq!(velocity_band(velocity), band, "{velocity}");
    }
}

#[test]
fn check_runs_every_rule_in_catalogue_order_and_repeats_itself() {
    // A horn out of range low and high, and a held, loud, high passage: RNG-01, RNG-02, RNG-03
    // and PLY-02 each find something.
    let horn = part("horn_1");
    let mut notes = vec![note("low", 30, 0.0, 1.0)];
    notes.extend(
        (0..18).map(|index| at(100, &format!("h{index}"), 74, 1.0 + f64::from(index), 1.0)),
    );
    notes.push(note("top", 80, 19.0, 1.0));
    let lines = [Line::new(&horn, I::HornF, &notes)];
    let first = check(&lines, &score()).expect("valid inputs");
    let rules: Vec<RuleId> = first.iter().map(|hit| hit.rule).collect();
    assert!(rules.is_sorted(), "{rules:?}");
    for rule in [RuleId::Rng01, RuleId::Rng02, RuleId::Rng03, RuleId::Ply02] {
        assert!(rules.contains(&rule), "{rule:?} in {rules:?}");
    }
    // TST-04: the same inputs give the same hits, notes given in any order.
    let mut reversed = notes.clone();
    reversed.reverse();
    let again = check(&[Line::new(&horn, I::HornF, &reversed)], &score()).expect("valid inputs");
    assert_eq!(first, again);
}

#[test]
fn check_refuses_invalid_inputs() {
    // TST-03.
    let violin = part("violins_1");
    let notes = [note("a", 60, 0.0, 1.0)];
    let twice = [
        Line::new(&violin, I::Violin, &notes),
        Line::new(&violin, I::Viola, &notes),
    ];
    assert_eq!(
        check(&twice, &score()),
        Err(CheckError::DuplicatePart(violin.clone()))
    );
    let mut broken = note("bad", 60, 0.0, 1.0);
    broken.start = f64::NAN;
    let mut silent = note("silent", 60, 0.0, 1.0);
    silent.velocity = 0;
    for bad in [broken, silent] {
        let id = bad.id.clone();
        let notes = [bad];
        assert_eq!(
            check(&[Line::new(&violin, I::Violin, &notes)], &score()),
            Err(CheckError::InvalidNote(id))
        );
    }
    for rule in [RuleId::Rng04, RuleId::Ply07, RuleId::Tex06] {
        assert_eq!(
            check_rule(rule, &[], &score()),
            Err(CheckError::Unchecked(rule))
        );
    }
    assert_eq!(check(&[], &score()), Ok(Vec::new()));
}

// Range.

#[test]
fn rng_01_fires_below_and_above_the_playable_range() {
    // The violin plays G3-A7 (55-105): G3 is the open G string.
    let violin = part("violins_1");
    let notes = [
        note("low", 54, 0.0, 1.0),
        note("ok", 60, 1.0, 1.0),
        note("high_1", 106, 2.0, 1.0),
        note("high_2", 107, 3.0, 1.0),
    ];
    let found = hits(RuleId::Rng01, &[(&violin, I::Violin, &notes)]);
    assert_eq!(notes_of(&found), [vec!["low"], vec!["high_1", "high_2"]]);
    assert_eq!(levels(&found), [Level::Error, Level::Error]);
    assert_eq!(found[0].parts, [violin]);
    assert!(found[0].detail.contains("below"), "{}", found[0].detail);
    assert!(found[1].detail.contains("above"), "{}", found[1].detail);
    assert_eq!((found[1].start, found[1].end), (2.0, 4.0));
}

#[test]
fn rng_01_stays_quiet_inside_the_range_and_on_unpitched_instruments() {
    let violin = part("violins_1");
    let snare = part("snare");
    let inside = [note("g", 55, 0.0, 1.0), note("a", 105, 1.0, 1.0)];
    let drum = [note("d", 10, 0.0, 1.0)];
    let found = hits(
        RuleId::Rng01,
        &[(&violin, I::Violin, &inside), (&snare, I::SnareDrum, &drum)],
    );
    assert_eq!(found, []);
}

#[test]
fn rng_02_fires_outside_the_comfortable_range() {
    // The horn plays B1-F5 (35-77) and is comfortable from F2 to C5 (41-72).
    let horn = part("horn_1");
    let notes = [
        note("low", 38, 0.0, 1.0),
        note("ok", 60, 1.0, 1.0),
        note("high_1", 74, 2.0, 1.0),
        note("high_2", 75, 3.0, 1.0),
    ];
    let found = hits(RuleId::Rng02, &[(&horn, I::HornF, &notes)]);
    assert_eq!(notes_of(&found), [vec!["low"], vec!["high_1", "high_2"]]);
    assert_eq!(levels(&found), [Level::Warning, Level::Warning]);
    assert!(
        found[0].detail.contains("F2 (41) to C5 (72)"),
        "{}",
        found[0].detail
    );
}

#[test]
fn rng_02_stays_quiet_inside_the_comfortable_range_and_leaves_rng_01_its_notes() {
    let horn = part("horn_1");
    let notes = [
        note("f2", 41, 0.0, 1.0),
        note("c5", 72, 1.0, 1.0),
        note("off", 30, 2.0, 1.0),
    ];
    assert_eq!(hits(RuleId::Rng02, &[(&horn, I::HornF, &notes)]), []);
}

#[test]
fn rng_03_fires_on_high_brass_held_longer_than_two_bars() {
    // The B♭ trumpet is comfortable up to B♭5 (82). Nine crotchets on C6 (84) last 4.5 s, more
    // than two bars of moderate 4/4 (4 s).
    let trumpet = part("trumpet_1");
    let notes = run("c", 84, 0.0, 1.0, 1.0, 9);
    let found = hits(RuleId::Rng03, &[(&trumpet, I::TrumpetBb, &notes)]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].notes.len(), 9);
    assert_eq!(found[0].level, Level::Warning);
    assert!(found[0].detail.contains("4.5 s"), "{}", found[0].detail);
}

#[test]
fn rng_03_stays_quiet_for_two_bars_a_rest_or_a_lower_note() {
    let trumpet = part("trumpet_1");
    // Exactly two bars.
    let two_bars = run("c", 84, 0.0, 1.0, 1.0, 8);
    // Nine crotchets broken by a quaver rest (250 ms) after the fifth.
    let mut rested = run("a", 84, 0.0, 1.0, 1.0, 5);
    rested.extend(run("b", 84, 5.5, 1.0, 1.0, 4));
    // Nine crotchets with a note inside the comfortable range in the middle.
    let mut lower = run("a", 84, 0.0, 1.0, 1.0, 4);
    lower.push(note("mid", 70, 4.0, 1.0));
    lower.extend(run("b", 84, 5.0, 1.0, 1.0, 4));
    // The same high notes on a violin are no brass.
    let violin = part("violins_1");
    let long = run("v", 84, 0.0, 1.0, 1.0, 12);
    for notes in [&two_bars, &rested, &lower] {
        assert_eq!(hits(RuleId::Rng03, &[(&trumpet, I::TrumpetBb, notes)]), []);
    }
    assert_eq!(hits(RuleId::Rng03, &[(&violin, I::Violin, &long)]), []);
}

// Spacing.

#[test]
fn spc_01_fires_below_an_intervals_low_limit_and_grades_by_dynamic_and_family() {
    // A minor third stays clear from C3 (48); A2-C3 lies below.
    let cello = part("cellos");
    let low = [note("a2", 45, 0.0, 2.0), note("c3", 48, 0.0, 2.0)];
    let found = hits(RuleId::Spc01, &[(&cello, I::Cello, &low)]);
    assert_eq!(notes_of(&found), [vec!["a2", "c3"]]);
    assert_eq!(levels(&found), [Level::Hint]);
    // Loud, the same third is a warning.
    let loud = [at(100, "a2", 45, 0.0, 2.0), at(100, "c3", 48, 0.0, 2.0)];
    assert_eq!(
        levels(&hits(RuleId::Spc01, &[(&cello, I::Cello, &loud)])),
        [Level::Warning]
    );
    // So is the third in the brass, across two parts.
    let trombone = part("trombone_1");
    let tuba = part("tuba");
    let upper = [note("c3", 48, 0.0, 2.0)];
    let lower = [note("a2", 45, 1.0, 2.0)];
    let found = hits(
        RuleId::Spc01,
        &[
            (&trombone, I::TromboneTenor, &upper),
            (&tuba, I::Tuba, &lower),
        ],
    );
    assert_eq!(levels(&found), [Level::Warning]);
    assert_eq!(found[0].parts, [trombone, tuba]);
    assert_eq!((found[0].start, found[0].end), (0.0, 3.0));
}

#[test]
fn spc_01_stays_quiet_at_the_limit_for_octaves_and_for_notes_apart() {
    let cello = part("cellos");
    let at_limit = [note("c3", 48, 0.0, 1.0), note("eb3", 51, 0.0, 1.0)];
    let octave = [note("low", 30, 0.0, 1.0), note("high", 42, 0.0, 1.0)];
    let fifth = [note("bb1", 34, 0.0, 1.0), note("f2", 41, 0.0, 1.0)];
    let apart = [note("a2", 45, 0.0, 1.0), note("c3", 48, 1.0, 1.0)];
    for notes in [&at_limit, &octave, &fifth, &apart] {
        assert_eq!(hits(RuleId::Spc01, &[(&cello, I::Cello, notes)]), []);
    }
}

#[test]
fn spc_02_fires_on_a_low_dissonance_between_two_parts() {
    // A fourth from G2 (43), below D3 (50): a hint across families, a warning within one.
    let cellos = part("cellos");
    let bassoon = part("bassoon_1");
    let basses = part("basses");
    let g2 = [note("g2", 43, 0.0, 1.0)];
    let c3 = [note("c3", 48, 0.0, 1.0)];
    let mixed = hits(
        RuleId::Spc02,
        &[(&cellos, I::Cello, &g2), (&bassoon, I::Bassoon, &c3)],
    );
    assert_eq!(notes_of(&mixed), [vec!["g2", "c3"]]);
    assert_eq!(levels(&mixed), [Level::Hint]);
    let strings = hits(
        RuleId::Spc02,
        &[(&cellos, I::Cello, &c3), (&basses, I::DoubleBass, &g2)],
    );
    assert_eq!(levels(&strings), [Level::Warning]);
}

#[test]
fn spc_02_stays_quiet_in_one_part_for_consonances_and_from_d3() {
    let cellos = part("cellos");
    let bassoon = part("bassoon_1");
    let one_part = [note("g2", 43, 0.0, 1.0), note("c3", 48, 0.0, 1.0)];
    assert_eq!(hits(RuleId::Spc02, &[(&cellos, I::Cello, &one_part)]), []);
    let cases = [(43, 50), (50, 55), (40, 52)];
    for (lower, upper) in cases {
        let low = [note("low", lower, 0.0, 1.0)];
        let high = [note("high", upper, 0.0, 1.0)];
        assert_eq!(
            hits(
                RuleId::Spc02,
                &[(&cellos, I::Cello, &low), (&bassoon, I::Bassoon, &high)]
            ),
            [],
            "{lower}-{upper}"
        );
    }
}

#[test]
fn ply_12_fires_on_timpani_struck_a_third_apart() {
    let timpani = part("timpani");
    let notes = [
        note("a2", 45, 0.0, 1.0),
        note("c3", 48, 0.0, 1.0),
        note("e3", 52, 2.0, 1.0),
    ];
    let found = hits(RuleId::Ply12, &[(&timpani, I::Timpani, &notes)]);
    assert_eq!(notes_of(&found), [vec!["a2", "c3"]]);
    assert_eq!(levels(&found), [Level::Hint]);
}

#[test]
fn ply_12_stays_quiet_for_fifths_and_for_thirds_in_turn() {
    let timpani = part("timpani");
    let fifth = [note("a2", 45, 0.0, 1.0), note("e3", 52, 0.0, 1.0)];
    let in_turn = [note("a2", 45, 0.0, 1.0), note("c3", 48, 1.0, 1.0)];
    for notes in [&fifth, &in_turn] {
        assert_eq!(hits(RuleId::Ply12, &[(&timpani, I::Timpani, notes)]), []);
    }
    // A third on a marimba is no timpani.
    let marimba = part("marimba");
    let third = [note("a", 60, 0.0, 1.0), note("c", 64, 0.0, 1.0)];
    assert_eq!(hits(RuleId::Ply12, &[(&marimba, I::Marimba, &third)]), []);
}

// Doubling.

#[test]
fn dbl_02_fires_when_a_lower_woodwind_doubles_a_higher_one_from_above() {
    // A bassoon an octave above a clarinet for four notes, then a third above an oboe.
    let clarinet = part("clarinet_1");
    let bassoon = part("bassoon_1");
    let below = line_of("cl", &[50, 52, 53, 55]);
    let above = line_of("bn", &[62, 64, 65, 67]);
    let found = hits(
        RuleId::Dbl02,
        &[
            (&clarinet, I::ClarinetBb, &below),
            (&bassoon, I::Bassoon, &above),
        ],
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].notes.len(), 8);
    assert_eq!(found[0].parts, [clarinet, bassoon.clone()]);
    assert_eq!(found[0].level, Level::Hint);
    let oboe = part("oboe_1");
    let oboe_line = line_of("ob", &[60, 62, 64, 65]);
    let thirds = line_of("bn", &[64, 65, 67, 69]);
    let found = hits(
        RuleId::Dbl02,
        &[
            (&oboe, I::Oboe, &oboe_line),
            (&bassoon, I::Bassoon, &thirds),
        ],
    );
    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn dbl_02_stays_quiet_in_the_natural_order_and_for_short_runs() {
    let clarinet = part("clarinet_1");
    let bassoon = part("bassoon_1");
    let natural = (
        line_of("cl", &[62, 64, 65, 67]),
        line_of("bn", &[50, 52, 53, 55]),
    );
    let short = (line_of("cl", &[50, 52, 53]), line_of("bn", &[62, 64, 65]));
    for (clarinet_notes, bassoon_notes) in [natural, short] {
        let found = hits(
            RuleId::Dbl02,
            &[
                (&clarinet, I::ClarinetBb, &clarinet_notes),
                (&bassoon, I::Bassoon, &bassoon_notes),
            ],
        );
        assert_eq!(found, []);
    }
}

#[test]
fn dbl_03_fires_on_two_flutes_in_octaves_without_strings() {
    let flute_1 = part("flute_1");
    let flute_2 = part("flute_2");
    let upper = line_of("a", &[84, 86, 88, 89]);
    let lower = line_of("b", &[72, 74, 76, 77]);
    let found = hits(
        RuleId::Dbl03,
        &[(&flute_1, I::Flute, &upper), (&flute_2, I::Flute, &lower)],
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].parts, [flute_1, flute_2]);
    assert_eq!(found[0].level, Level::Hint);
}

#[test]
fn dbl_03_stays_quiet_with_strings_doubling_or_two_colours() {
    let flute_1 = part("flute_1");
    let flute_2 = part("flute_2");
    let violins = part("violins_1");
    let upper = line_of("a", &[84, 86, 88, 89]);
    let lower = line_of("b", &[72, 74, 76, 77]);
    let strings = line_of("v", &[72, 74, 76, 77]);
    let doubled = hits(
        RuleId::Dbl03,
        &[
            (&flute_1, I::Flute, &upper),
            (&flute_2, I::Flute, &lower),
            (&violins, I::Violin, &strings),
        ],
    );
    assert_eq!(doubled, []);
    let oboe = part("oboe_1");
    let colours = hits(
        RuleId::Dbl03,
        &[(&flute_1, I::Flute, &upper), (&oboe, I::Oboe, &lower)],
    );
    assert_eq!(colours, []);
}

#[test]
fn dbl_04_fires_on_a_woodwind_an_octave_above_a_trombone() {
    let flute = part("flute_1");
    let trombone = part("trombone_1");
    let flute_line = line_of("f", &[60, 62, 64, 65]);
    let trombone_line = line_of("t", &[48, 50, 52, 53]);
    let found = hits(
        RuleId::Dbl04,
        &[
            (&flute, I::Flute, &flute_line),
            (&trombone, I::TromboneTenor, &trombone_line),
        ],
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].level, Level::Warning);
}

#[test]
fn dbl_04_stays_quiet_with_a_trumpet_or_two_octaves_apart() {
    let flute = part("flute_1");
    let trombone = part("trombone_1");
    let trumpet = part("trumpet_1");
    let flute_line = line_of("f", &[60, 62, 64, 65]);
    let trombone_line = line_of("t", &[48, 50, 52, 53]);
    let trumpet_line = line_of("p", &[60, 62, 64, 65]);
    let with_trumpet = hits(
        RuleId::Dbl04,
        &[
            (&flute, I::Flute, &flute_line),
            (&trombone, I::TromboneTenor, &trombone_line),
            (&trumpet, I::TrumpetBb, &trumpet_line),
        ],
    );
    assert_eq!(with_trumpet, []);
    let high = line_of("f", &[72, 74, 76, 77]);
    let two_octaves = hits(
        RuleId::Dbl04,
        &[
            (&flute, I::Flute, &high),
            (&trombone, I::TromboneTenor, &trombone_line),
        ],
    );
    assert_eq!(two_octaves, []);
}

#[test]
fn dbl_09_fires_on_strings_and_brass_in_unison_without_woodwind() {
    let violins = part("violins_1");
    let horn = part("horn_1");
    let melody = [60, 62, 64, 65, 67];
    let found = hits(
        RuleId::Dbl09,
        &[
            (&violins, I::Violin, &line_of("v", &melody)),
            (&horn, I::HornF, &line_of("h", &melody)),
        ],
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].notes.len(), 10);
    assert_eq!(found[0].level, Level::Hint);
}

#[test]
fn dbl_09_stays_quiet_with_a_woodwind_in_the_unison() {
    let violins = part("violins_1");
    let horn = part("horn_1");
    let oboe = part("oboe_1");
    let melody = [60, 62, 64, 65, 67];
    let found = hits(
        RuleId::Dbl09,
        &[
            (&violins, I::Violin, &line_of("v", &melody)),
            (&horn, I::HornF, &line_of("h", &melody)),
            (&oboe, I::Oboe, &line_of("o", &melody)),
        ],
    );
    assert_eq!(found, []);
}

// String technique.

#[test]
fn voi_12_fires_on_a_held_or_soft_triple_stop() {
    let violins = part("violins_1");
    // Three notes held a crotchet (500 ms), longer than a quaver (250 ms).
    let held = [
        note("g", 55, 0.0, 1.0),
        note("d", 62, 0.0, 1.0),
        note("b", 71, 0.0, 1.0),
    ];
    let found = hits(RuleId::Voi12, &[(&violins, I::Violin, &held)]);
    assert_eq!(notes_of(&found), [vec!["g", "d", "b"]]);
    assert_eq!(levels(&found), [Level::Warning]);
    // Played quickly but at p.
    let soft = [
        at(40, "g", 55, 0.0, 0.25),
        at(40, "d", 62, 0.0, 0.25),
        at(40, "b", 71, 0.0, 0.25),
    ];
    assert_eq!(
        hits(RuleId::Voi12, &[(&violins, I::Violin, &soft)]).len(),
        1
    );
}

#[test]
fn voi_12_stays_quiet_for_a_quick_chord_and_a_held_double_stop() {
    let violins = part("violins_1");
    let quick = [
        note("g", 55, 0.0, 0.5),
        note("d", 62, 0.0, 0.5),
        note("b", 71, 0.0, 0.5),
    ];
    let double = [note("g", 55, 0.0, 4.0), note("d", 62, 0.0, 4.0)];
    for notes in [&quick[..], &double[..]] {
        assert_eq!(hits(RuleId::Voi12, &[(&violins, I::Violin, notes)]), []);
    }
}

#[test]
fn ply_04_fires_when_a_mute_change_has_too_little_time() {
    let violins = part("violins_1");
    // Putting the string mute on needs two bars of moderate time (4 s); one second is free.
    let on = [
        note("open", 60, 0.0, 4.0),
        played("con_sordino", "muted", 60, 6.0, 1.0),
    ];
    let found = hits(RuleId::Ply04, &[(&violins, I::Violin, &on)]);
    assert_eq!(notes_of(&found), [vec!["open", "muted"]]);
    assert_eq!(levels(&found), [Level::Warning]);
    assert!(found[0].detail.contains("put on"), "{}", found[0].detail);
    // Taking it off needs seven beats (3.5 s); 3.25 s are free.
    let off = [
        played("con_sordino", "muted", 60, 0.0, 1.0),
        note("open", 60, 7.5, 1.0),
    ];
    let found = hits(RuleId::Ply04, &[(&violins, I::Violin, &off)]);
    assert!(found[0].detail.contains("taken off"), "{}", found[0].detail);
    // A trumpet changing one mute for another needs both times.
    let trumpet = part("trumpet_1");
    let change = [
        played("mute_cup", "cup", 70, 0.0, 1.0),
        played("mute_harmon", "harmon", 70, 10.0, 1.0),
    ];
    let found = hits(RuleId::Ply04, &[(&trumpet, I::TrumpetBb, &change)]);
    assert!(found[0].detail.contains("changed"), "{}", found[0].detail);
}

#[test]
fn ply_04_stays_quiet_with_enough_time_or_a_hand_mute() {
    let violins = part("violins_1");
    let on = [
        note("open", 60, 0.0, 4.0),
        played("con_sordino", "muted", 60, 12.0, 1.0),
    ];
    let off = [
        played("con_sordino", "muted", 60, 0.0, 1.0),
        note("open", 60, 8.0, 1.0),
    ];
    let first = [played("con_sordino", "muted", 60, 0.0, 1.0)];
    for notes in [&on[..], &off[..], &first[..]] {
        assert_eq!(hits(RuleId::Ply04, &[(&violins, I::Violin, notes)]), []);
    }
    // A stopped horn uses the hand, which takes no time.
    let horn = part("horn_1");
    let stopped = [
        note("open", 60, 0.0, 1.0),
        played("stopped", "stopped", 60, 1.0, 1.0),
    ];
    assert_eq!(hits(RuleId::Ply04, &[(&horn, I::HornF, &stopped)]), []);
}

#[test]
fn ply_05_fires_when_the_bow_has_less_than_half_a_second() {
    let violins = part("violins_1");
    let notes = [
        played("pizzicato", "pizz", 60, 0.0, 0.5),
        note("arco", 62, 0.75, 1.0),
        played("bartok_pizzicato", "snap", 60, 4.0, 0.25),
        note("arco_2", 62, 4.5, 1.0),
    ];
    let found = hits(RuleId::Ply05, &[(&violins, I::Violin, &notes)]);
    assert_eq!(
        notes_of(&found),
        [vec!["pizz", "arco"], vec!["snap", "arco_2"]]
    );
    assert_eq!(levels(&found), [Level::Warning, Level::Warning]);
}

#[test]
fn ply_05_stays_quiet_with_half_a_second_and_from_arco_to_pizzicato() {
    let violins = part("violins_1");
    let notes = [
        played("pizzicato", "pizz", 60, 0.0, 1.0),
        note("arco", 62, 1.0, 1.0),
        played("pizzicato", "pizz_2", 60, 2.0, 1.0),
    ];
    assert_eq!(hits(RuleId::Ply05, &[(&violins, I::Violin, &notes)]), []);
}

#[test]
fn ply_06_fires_on_pizzicato_faster_than_the_instrument_plucks() {
    let violins = part("violins_1");
    // Four plucks 100 ms apart, faster than semiquavers at 120 (125 ms).
    let fast: Vec<MidiNote> = (0..4)
        .map(|index| {
            played(
                "pizzicato",
                &format!("p{index}"),
                60,
                f64::from(index) * 0.2,
                0.2,
            )
        })
        .collect();
    let found = hits(RuleId::Ply06, &[(&violins, I::Violin, &fast)]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].notes.len(), 4);
    // The double bass at 200 ms, faster than quavers at 120 (250 ms).
    let basses = part("basses");
    let walking: Vec<MidiNote> = (0..4)
        .map(|index| {
            played(
                "pizzicato",
                &format!("b{index}"),
                40,
                f64::from(index) * 0.4,
                0.4,
            )
        })
        .collect();
    assert_eq!(
        hits(RuleId::Ply06, &[(&basses, I::DoubleBass, &walking)]).len(),
        1
    );
}

#[test]
fn ply_06_stays_quiet_at_the_instruments_speed_and_for_short_runs() {
    let violins = part("violins_1");
    let semiquavers: Vec<MidiNote> = (0..8)
        .map(|index| {
            played(
                "pizzicato",
                &format!("p{index}"),
                60,
                f64::from(index) * 0.25,
                0.25,
            )
        })
        .collect();
    let three: Vec<MidiNote> = (0..3)
        .map(|index| {
            played(
                "pizzicato",
                &format!("p{index}"),
                60,
                f64::from(index) * 0.2,
                0.2,
            )
        })
        .collect();
    let at_bass_speed: Vec<MidiNote> = (0..4)
        .map(|index| {
            played(
                "pizzicato",
                &format!("p{index}"),
                60,
                f64::from(index) * 0.4,
                0.4,
            )
        })
        .collect();
    let bowed = run("a", 60, 0.0, 0.2, 0.2, 8);
    for notes in [&semiquavers, &three, &at_bass_speed, &bowed] {
        assert_eq!(hits(RuleId::Ply06, &[(&violins, I::Violin, notes)]), []);
    }
}

// Wind and brass.

#[test]
fn ply_01_fires_on_a_long_phrase_a_held_note_and_the_flutes_low_notes() {
    let flute = part("flute_1");
    // 34 crotchets without a rest last 17 s, more than 8 bars of moderate 4/4 (16 s).
    let phrase = run("f", 72, 0.0, 1.0, 1.0, 34);
    let found = hits(RuleId::Ply01, &[(&flute, I::Flute, &phrase)]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].notes.len(), 34);
    assert!(found[0].detail.contains("17.0 s"), "{}", found[0].detail);
    // One note of 21 s on the alto saxophone outlasts its breath of 10 bars (20 s) and the phrase.
    let saxophone = part("alto_sax");
    let held = [note("long", 65, 0.0, 42.0)];
    let found = hits(RuleId::Ply01, &[(&saxophone, I::SaxophoneAlto, &held)]);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found
            .iter()
            .any(|hit| hit.detail.contains("breath of 20.0 s"))
    );
    // Five notes in the flute's lowest register (B3-G4) without a rest.
    let low = run("l", 60, 0.0, 1.0, 1.0, 5);
    let found = hits(RuleId::Ply01, &[(&flute, I::Flute, &low)]);
    assert_eq!(found.len(), 1);
    assert!(
        found[0].detail.contains("lowest register"),
        "{}",
        found[0].detail
    );
}

#[test]
fn ply_01_stays_quiet_with_rests_and_within_the_breath() {
    let flute = part("flute_1");
    // 32 crotchets, then a quaver rest (250 ms), then 32 more.
    let mut rested = run("a", 72, 0.0, 1.0, 1.0, 32);
    rested.extend(run("b", 72, 32.5, 1.0, 1.0, 32));
    let four_low = run("l", 60, 0.0, 1.0, 1.0, 4);
    let mut low_with_rests = run("a", 60, 0.0, 1.0, 1.0, 4);
    low_with_rests.extend(run("b", 60, 4.5, 1.0, 1.0, 4));
    for notes in [&rested, &four_low, &low_with_rests] {
        assert_eq!(hits(RuleId::Ply01, &[(&flute, I::Flute, notes)]), []);
    }
    // The saxophone's 20 s note is a phrase of 20 s but not longer than its breath.
    let saxophone = part("alto_sax");
    let held = [note("long", 65, 0.0, 40.0)];
    let found = hits(RuleId::Ply01, &[(&saxophone, I::SaxophoneAlto, &held)]);
    assert_eq!(found.len(), 1);
    assert!(
        found[0].detail.contains("without a rest"),
        "{}",
        found[0].detail
    );
    // A violin needs no breath.
    let violins = part("violins_1");
    let long = run("v", 72, 0.0, 1.0, 1.0, 40);
    assert_eq!(hits(RuleId::Ply01, &[(&violins, I::Violin, &long)]), []);
}

#[test]
fn ply_02_fires_on_long_loud_playing_and_repeated_notes_outside_the_middle() {
    // The horn's middle register is the middle third of F2-C5: D♭3-D4 (51-62).
    let horn = part("horn_1");
    // 17 loud crotchets in the middle last 8.5 s, more than four bars of moderate 4/4 (8 s).
    let loud: Vec<MidiNote> = (0..17)
        .map(|index| at(100, &format!("l{index}"), 55, f64::from(index), 1.0))
        .collect();
    let found = hits(RuleId::Ply02, &[(&horn, I::HornF, &loud)]);
    assert_eq!(found.len(), 1);
    assert!(
        found[0].detail.contains("loud or high"),
        "{}",
        found[0].detail
    );
    // Four repeated A2s (45) below the middle register.
    let repeated = run("r", 45, 0.0, 1.0, 1.0, 4);
    let found = hits(RuleId::Ply02, &[(&horn, I::HornF, &repeated)]);
    assert_eq!(notes_of(&found), [vec!["r0", "r1", "r2", "r3"]]);
    assert_eq!(levels(&found), [Level::Warning]);
}

#[test]
fn ply_02_stays_quiet_in_the_middle_softer_or_with_rests() {
    let horn = part("horn_1");
    let medium = run("m", 55, 0.0, 1.0, 1.0, 20);
    let sixteen: Vec<MidiNote> = (0..16)
        .map(|index| at(100, &format!("l{index}"), 55, f64::from(index), 1.0))
        .collect();
    let three = run("r", 45, 0.0, 1.0, 1.0, 3);
    let middle = run("r", 55, 0.0, 1.0, 1.0, 6);
    let rested = run("r", 45, 0.0, 2.0, 1.0, 6);
    for notes in [&medium, &sixteen, &three, &middle, &rested] {
        assert_eq!(hits(RuleId::Ply02, &[(&horn, I::HornF, notes)]), []);
    }
}

#[test]
fn ply_14_fires_on_fast_repeated_notes_without_multiple_tonguing() {
    // Four A4s 100 ms apart, faster than semiquavers at 140 (107 ms).
    let oboe = part("oboe_1");
    let fast = run("o", 69, 0.0, 0.2, 0.2, 4);
    let found = hits(RuleId::Ply14, &[(&oboe, I::Oboe, &fast)]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].level, Level::Hint);
    let trombone = part("trombone_1");
    let low = run("t", 50, 0.0, 0.2, 0.2, 6);
    assert_eq!(
        hits(RuleId::Ply14, &[(&trombone, I::TromboneTenor, &low)]).len(),
        1
    );
}

#[test]
fn ply_14_stays_quiet_for_instruments_that_multi_tongue_and_for_slower_notes() {
    let fast = run("x", 69, 0.0, 0.2, 0.2, 8);
    let flute = part("flute_1");
    let trumpet = part("trumpet_1");
    let clarinet = part("clarinet_1");
    assert_eq!(hits(RuleId::Ply14, &[(&flute, I::Flute, &fast)]), []);
    assert_eq!(hits(RuleId::Ply14, &[(&trumpet, I::TrumpetBb, &fast)]), []);
    assert_eq!(
        hits(RuleId::Ply14, &[(&clarinet, I::ClarinetBb, &fast)]),
        []
    );
    let oboe = part("oboe_1");
    let semiquavers = run("o", 69, 0.0, 0.25, 0.25, 8);
    let three = run("o", 69, 0.0, 0.2, 0.2, 3);
    let scale = line_of("o", &[69, 71, 72, 74]);
    for notes in [&semiquavers, &three, &scale] {
        assert_eq!(hits(RuleId::Ply14, &[(&oboe, I::Oboe, notes)]), []);
    }
}

#[test]
fn ply_15_fires_on_rapid_clarinet_octave_leaps() {
    let clarinet = part("clarinet_1");
    let notes = [
        note("a", 60, 0.0, 0.25),
        note("b", 72, 0.25, 0.25),
        note("c", 60, 0.5, 0.25),
        note("d", 62, 2.0, 1.0),
    ];
    let found = hits(RuleId::Ply15, &[(&clarinet, I::ClarinetBb, &notes)]);
    assert_eq!(notes_of(&found), [vec!["a", "b", "c"]]);
    assert!(
        found[0].detail.starts_with("2 rapid leaps"),
        "{}",
        found[0].detail
    );
}

#[test]
fn ply_15_stays_quiet_for_slow_or_smaller_leaps_and_other_winds() {
    let clarinet = part("clarinet_1");
    let slow = [note("a", 60, 0.0, 0.5), note("b", 72, 0.5, 0.5)];
    let seventh = [note("a", 60, 0.0, 0.25), note("b", 71, 0.25, 0.25)];
    for notes in [&slow, &seventh] {
        assert_eq!(
            hits(RuleId::Ply15, &[(&clarinet, I::ClarinetBb, notes)]),
            []
        );
    }
    let oboe = part("oboe_1");
    let leaps = [note("a", 60, 0.0, 0.25), note("b", 72, 0.25, 0.25)];
    assert_eq!(hits(RuleId::Ply15, &[(&oboe, I::Oboe, &leaps)]), []);
}

// Texture.

#[test]
fn tex_01_fires_on_a_celesta_in_most_bars() {
    // Four bars of violins; the celesta sounds in three of them.
    let violins = part("violins_1");
    let celesta = part("celesta");
    let strings = [note("v", 60, 0.0, 16.0)];
    let colour = [
        note("a", 72, 0.0, 1.0),
        note("b", 72, 4.0, 1.0),
        note("c", 72, 8.0, 1.0),
    ];
    let found = hits(
        RuleId::Tex01,
        &[
            (&violins, I::Violin, &strings),
            (&celesta, I::Celesta, &colour),
        ],
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].parts, [celesta]);
    assert!(
        found[0].detail.contains("3 of 4 bars"),
        "{}",
        found[0].detail
    );
}

#[test]
fn tex_01_stays_quiet_at_half_the_bars_and_for_other_colours() {
    let violins = part("violins_1");
    let celesta = part("celesta");
    let strings = [note("v", 60, 0.0, 16.0)];
    // Two of four bars; a note that ends on a bar line does not sound in the next bar.
    let colour = [note("a", 72, 0.0, 4.0), note("b", 72, 8.0, 4.0)];
    let found = hits(
        RuleId::Tex01,
        &[
            (&violins, I::Violin, &strings),
            (&celesta, I::Celesta, &colour),
        ],
    );
    assert_eq!(found, []);
    let flute = part("flute_1");
    let flute_notes = [note("f", 72, 0.0, 16.0)];
    assert_eq!(hits(RuleId::Tex01, &[(&flute, I::Flute, &flute_notes)]), []);
}

#[test]
fn tex_02_fires_on_all_the_percussion_at_once_and_on_brass_and_percussion_never_resting() {
    let timpani = part("timpani");
    let bass_drum = part("bass_drum");
    let snare = part("snare");
    let a = [note("t", 45, 0.0, 2.0)];
    let b = [note("b", 36, 1.0, 2.0)];
    let c = [note("s", 38, 1.5, 1.0)];
    let found = hits(
        RuleId::Tex02,
        &[
            (&timpani, I::Timpani, &a),
            (&bass_drum, I::BassDrum, &b),
            (&snare, I::SnareDrum, &c),
        ],
    );
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].start, found[0].end), (1.5, 2.0));
    assert_eq!(notes_of(&found), [vec!["t", "b", "s"]]);
    // Eight bars of horn and timpani, both in every bar.
    let horn = part("horn_1");
    let horns = run("h", 60, 0.0, 4.0, 2.0, 8);
    let drums = run("d", 45, 2.0, 4.0, 1.0, 8);
    let found = hits(
        RuleId::Tex02,
        &[(&horn, I::HornF, &horns), (&timpani, I::Timpani, &drums)],
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].notes, Vec::<String>::new());
    assert_eq!(found[0].parts, [horn, timpani]);
    assert!(found[0].detail.contains("8 bars"), "{}", found[0].detail);
}

#[test]
fn tex_02_stays_quiet_with_fewer_percussion_a_rest_or_a_short_piece() {
    let timpani = part("timpani");
    let bass_drum = part("bass_drum");
    let together = [note("t", 45, 0.0, 2.0)];
    let with = [note("b", 36, 0.0, 2.0)];
    assert_eq!(
        hits(
            RuleId::Tex02,
            &[
                (&timpani, I::Timpani, &together),
                (&bass_drum, I::BassDrum, &with)
            ]
        ),
        []
    );
    let horn = part("horn_1");
    // The horn rests in bar 5.
    let mut horns = run("h", 60, 0.0, 4.0, 2.0, 4);
    horns.extend(run("k", 60, 20.0, 4.0, 2.0, 3));
    let drums = run("d", 45, 2.0, 4.0, 1.0, 8);
    assert_eq!(
        hits(
            RuleId::Tex02,
            &[(&horn, I::HornF, &horns), (&timpani, I::Timpani, &drums)]
        ),
        []
    );
    // Seven bars.
    let short_horns = run("h", 60, 0.0, 4.0, 2.0, 7);
    let short_drums = run("d", 45, 2.0, 4.0, 1.0, 7);
    assert_eq!(
        hits(
            RuleId::Tex02,
            &[
                (&horn, I::HornF, &short_horns),
                (&timpani, I::Timpani, &short_drums)
            ]
        ),
        []
    );
}

// The harp's pedals.

/// The setting with the pedals `altered` moved, every other one natural.
fn setting(altered: &[(LetterName, Accidental)]) -> PedalSetting {
    let mut pedals = [Accidental::Natural; 7];
    for &(letter, accidental) in altered {
        pedals[letter as usize] = accidental;
    }
    PedalSetting::new(pedals)
}

#[test]
fn harp_pedals_and_timpani_tunings_cover_a_known_passage() {
    // TST-10, harp: a C major scale, then F# and G; the F pedal moves to sharp, with five beats
    // (2.5 s) between the last F and the first F#, more than a beat of moderate time.
    let mut notes = line_of("s", &[60, 62, 64, 65, 67, 69, 71]);
    notes.push(note("fs", 66, 8.0, 1.0));
    notes.push(note("g", 67, 9.0, 1.0));
    let harp = harp_pedals(&notes, &score()).expect("valid notes");
    assert_eq!(harp.passages.len(), 2);
    assert_eq!(harp.passages[0].setting, PedalSetting::NATURAL);
    assert_eq!(harp.passages[0].changed, []);
    assert_eq!((harp.passages[0].start, harp.passages[0].end), (0.0, 7.0));
    assert_eq!(
        harp.passages[1].setting,
        setting(&[(LetterName::F, Accidental::Sharp)])
    );
    assert_eq!(harp.passages[1].changed, [LetterName::F]);
    assert_eq!((harp.passages[1].start, harp.passages[1].end), (8.0, 10.0));
    assert_eq!(harp.uncovered, []);

    // Timpani: two drums on A2 and D3 alternate, then A2 and E3; the D3 drum is retuned up a whole
    // tone (400 ms) with 4.5 beats (2.25 s) free.
    let notes = [
        note("a", 45, 0.0, 0.5),
        note("d", 50, 1.0, 0.5),
        note("a2", 45, 2.0, 0.5),
        note("d2", 50, 3.0, 0.5),
        note("e", 52, 8.0, 0.5),
        note("a3", 45, 9.0, 0.5),
    ];
    let timpani = timpani_tunings(&notes, 2, &score()).expect("valid notes");
    let pitch = |note: i64| SoundingPitch::new(note);
    assert_eq!(timpani.passages.len(), 2);
    assert_eq!(timpani.passages[0].drums, [pitch(45), pitch(50)]);
    assert_eq!(timpani.passages[0].retuned, Vec::<usize>::new());
    assert_eq!(
        (timpani.passages[0].start, timpani.passages[0].end),
        (0.0, 3.5)
    );
    assert_eq!(timpani.passages[1].drums, [pitch(45), pitch(52)]);
    assert_eq!(timpani.passages[1].retuned, [1]);
    assert_eq!(timpani.uncovered, []);
}

#[test]
fn harp_pedals_prefer_a_change_that_fits_the_time() {
    // D to B then C, and C# at once after the C: C# needs the C pedal 250 ms after its last
    // pluck, D♭ the D pedal three beats after its own; D♭ is chosen and nothing is late.
    let mut notes = line_of("s", &[62, 64, 65, 67, 69, 71, 60]);
    notes.push(note("cs", 61, 6.5, 1.0));
    let harp = harp_pedals(&notes, &score()).expect("valid notes");
    assert_eq!(harp.passages.len(), 2);
    assert_eq!(
        harp.passages[1].setting,
        setting(&[(LetterName::D, Accidental::Flat)])
    );
    assert_eq!(harp.uncovered, []);
    // With D and C both just plucked, either change is late: the sharp is taken and reported.
    let mut notes = line_of("s", &[64, 65, 67, 69, 71]);
    notes.push(note("d", 62, 5.75, 0.25));
    notes.push(note("c", 60, 5.75, 0.25));
    notes.push(note("cs", 61, 6.0, 1.0));
    let harp = harp_pedals(&notes, &score()).expect("valid notes");
    assert_eq!(
        harp.passages[1].setting,
        setting(&[(LetterName::C, Accidental::Sharp)])
    );
    assert_eq!(harp.uncovered.len(), 1);
    assert_eq!(harp.uncovered[0].notes, ["cs"]);
    assert_eq!(
        harp.uncovered[0].shortfall,
        Shortfall::Time {
            needed_ms: 500,
            free_ms: 125.0
        }
    );
}

#[test]
fn harp_pedals_report_a_cluster_no_setting_plays() {
    // Six semitones from C4 need six strings, but only five of C, D, E, F and their neighbours
    // can sound them; five semitones fit (B#, C#, D, E♭, F♭).
    let six: Vec<MidiNote> = (60..66)
        .map(|pitch| note(&format!("n{pitch}"), pitch, 0.0, 1.0))
        .collect();
    let harp = harp_pedals(&six, &score()).expect("valid notes");
    assert_eq!(harp.passages, []);
    assert_eq!(harp.uncovered.len(), 1);
    assert_eq!(harp.uncovered[0].shortfall, Shortfall::Pitches);
    assert_eq!(harp.uncovered[0].notes.len(), 6);
    let harp = harp_pedals(&six[..5], &score()).expect("valid notes");
    assert_eq!(harp.passages.len(), 1);
    assert_eq!(harp.uncovered, []);
}

#[test]
fn timpani_tunings_report_too_many_pitches_and_a_hurried_retune() {
    // Two drums: a chord of three pitches cannot be played.
    let chord = [
        note("a", 45, 0.0, 1.0),
        note("d", 50, 0.0, 1.0),
        note("e", 52, 0.0, 1.0),
    ];
    let timpani = timpani_tunings(&chord, 2, &score()).expect("valid notes");
    assert_eq!(timpani.passages, []);
    assert_eq!(timpani.uncovered.len(), 1);
    assert_eq!(timpani.uncovered[0].shortfall, Shortfall::Pitches);
    // D3 ends at beat 0.5 and E3 follows at beat 1: 250 ms for a whole tone that takes 400 ms.
    let notes = [
        note("a", 45, 0.0, 0.5),
        note("d", 50, 0.0, 0.5),
        note("e", 52, 1.0, 0.5),
        note("a2", 45, 1.0, 0.5),
    ];
    let timpani = timpani_tunings(&notes, 2, &score()).expect("valid notes");
    assert_eq!(timpani.passages.len(), 2);
    assert_eq!(timpani.uncovered.len(), 1);
    assert_eq!(timpani.uncovered[0].notes, ["e"]);
    assert_eq!(
        timpani.uncovered[0].shortfall,
        Shortfall::Time {
            needed_ms: 400,
            free_ms: 250.0
        }
    );
    // Four drums hold all three pitches at once.
    let timpani = timpani_tunings(&chord, 4, &score()).expect("valid notes");
    assert_eq!(timpani.passages.len(), 1);
    assert_eq!(timpani.passages[0].drums.len(), 4);
    assert_eq!(timpani.uncovered, []);
}

#[test]
fn derivations_refuse_invalid_inputs() {
    // TST-03.
    let mut bad = note("bad", 60, 0.0, 1.0);
    bad.length = 0.0;
    let notes = [bad];
    assert_eq!(
        harp_pedals(&notes, &score()),
        Err(CheckError::InvalidNote("bad".to_owned()))
    );
    assert_eq!(
        timpani_tunings(&notes, 4, &score()),
        Err(CheckError::InvalidNote("bad".to_owned()))
    );
    assert_eq!(timpani_tunings(&[], 0, &score()), Err(CheckError::NoDrums));
    assert_eq!(
        harp_pedals(&[], &score()).map(|harp| harp.passages),
        Ok(Vec::new())
    );
}

/// Notes of pitches in `low..=high`, each starting on a whole or half beat.
fn notes_in(low: i64, high: i64) -> impl Strategy<Value = Vec<MidiNote>> {
    proptest::collection::vec((low..=high, 0_u32..64, 1_u32..4), 0..40).prop_map(|notes| {
        notes
            .into_iter()
            .enumerate()
            .map(|(index, (pitch, half_beats, length))| {
                note(
                    &format!("n{index}"),
                    pitch,
                    f64::from(half_beats) / 2.0,
                    f64::from(length) / 2.0,
                )
            })
            .collect()
    })
}

proptest! {
    #[test]
    fn every_harp_passage_sounds_its_notes(notes in notes_in(36, 96)) {
        // TST-07: each note not reported as uncoverable lies in a passage whose setting sounds
        // its pitch class; the passages follow one another.
        let harp = harp_pedals(&notes, &score()).expect("valid notes");
        for pair in harp.passages.windows(2) {
            prop_assert!(pair[0].start < pair[1].start);
        }
        for note in &notes {
            let uncovered = harp.uncovered.iter().any(|gap| {
                gap.shortfall == Shortfall::Pitches && gap.notes.contains(&note.id)
            });
            if uncovered {
                continue;
            }
            let passage = harp.passages.iter().rev().find(|passage| passage.start <= note.start);
            let class = u8::try_from(i64::from(note.pitch) % 12).expect("0-11");
            prop_assert!(passage.is_some_and(|passage| passage.setting.sounds(class)));
        }
    }

    #[test]
    fn every_timpani_passage_tunes_its_pitches(notes in notes_in(40, 57), drums in 1_u8..5) {
        // TST-07: each passage keeps at most `drums` pitches, one drum each, and every note not
        // reported as uncoverable is on a drum of its passage.
        let timpani = timpani_tunings(&notes, drums, &score()).expect("valid notes");
        for passage in &timpani.passages {
            prop_assert_eq!(passage.drums.len(), usize::from(drums));
            let tuned: Vec<SoundingPitch> = passage.drums.iter().flatten().copied().collect();
            let mut distinct = tuned.clone();
            distinct.sort();
            distinct.dedup();
            prop_assert_eq!(distinct.len(), tuned.len());
        }
        for note in &notes {
            let uncovered = timpani.uncovered.iter().any(|gap| {
                gap.shortfall == Shortfall::Pitches && gap.notes.contains(&note.id)
            });
            if uncovered {
                continue;
            }
            let passage = timpani.passages.iter().rev().find(|passage| passage.start <= note.start);
            let pitch = SoundingPitch::new(i64::from(note.pitch));
            prop_assert!(passage.is_some_and(|passage| passage.drums.contains(&pitch)));
        }
    }
}
