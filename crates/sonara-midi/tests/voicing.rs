//! Contract tests of the voicing and doubling helpers through the public API.

use proptest::prelude::*;
use sonara_core::ids::SequentialIds;
use sonara_core::midi::MidiNote;
use sonara_core::time::{TempoMap, TimeSignatureMap, TimeSignaturePoint};
use sonara_midi::generators::ChordSlot;
use sonara_midi::orchestration::rules::{
    LOW_DISSONANCE, LOW_INTERVAL_LIMITS, Limit, NamedDoubling, RuleId, Spacing, WOODWIND_ORDER,
};
use sonara_midi::orchestration::{
    Family, InstrumentType, Part, PitchModel, PitchRange, SoundingPitch, ensemble, instrument,
    instruments,
};
use sonara_midi::performance::voicing::{
    Carrier, ChordOptions, DEFAULT_VELOCITY, Density, MelodyDoubling, RegisterFit, Remark,
    TimpaniOptions, Voiced, VoicingError, WindLayout, WindOptions, brass_chorale, fit_register,
    horns, melody_doublings, strings_divisi, timpani, woodwind_pairs,
};
use sonara_midi::performance::{PartId, PerformanceError, Role, ScoreContext};
use sonara_midi::theory::{parse_chord, parse_key};

fn part(slug: &str) -> PartId {
    PartId::new(slug).expect("a valid part id")
}

fn carrier(slug: &str, instrument: InstrumentType) -> Carrier {
    Carrier::new(part(slug), instrument)
}

/// One chord per bar of 4/4 from beat 0.
fn bars(symbols: &[&str]) -> Vec<ChordSlot> {
    symbols
        .iter()
        .zip(0_u32..)
        .map(|(symbol, bar)| ChordSlot {
            chord: parse_chord(symbol).expect("a valid chord"),
            start: f64::from(bar) * 4.0,
            length: 4.0,
        })
        .collect()
}

fn four_horns() -> Vec<Carrier> {
    ["horn_1", "horn_2", "horn_3", "horn_4"]
        .into_iter()
        .map(|slug| carrier(slug, InstrumentType::HornF))
        .collect()
}

/// The pitches of `part`, in note order.
fn pitches(voiced: &Voiced, slug: &str) -> Vec<i64> {
    voiced
        .notes(slug)
        .unwrap_or_else(|| panic!("{slug} is a part of the result"))
        .iter()
        .map(|note| i64::from(note.pitch))
        .collect()
}

/// The comfortable range of `instrument`, or its playable range when none is known.
fn comfortable(instrument: InstrumentType) -> PitchRange {
    match instrument.get().pitch() {
        PitchModel::Pitched(pitched) => pitched
            .comfortable
            .map_or(pitched.playable.range, |range| range.range),
        PitchModel::Unpitched => panic!("{instrument:?} is unpitched"),
    }
}

fn holds(range: PitchRange, pitch: i64) -> bool {
    range.low().get() <= pitch && pitch <= range.high().get()
}

/// Whether two notes sounding together clear the low-interval limit of their interval and keep
/// a dissonance at or above D3, written from the rules' tables.
fn clear(lower: i64, upper: i64) -> bool {
    let semitones = upper - lower;
    let limit = u8::try_from(semitones)
        .ok()
        .and_then(|semitones| LOW_INTERVAL_LIMITS.of(semitones));
    let above = match limit {
        Some(Limit::Lowest(pitch)) => lower >= pitch.get(),
        Some(Limit::Unlimited) | None => true,
    };
    let dissonant = [1, 2, 5, 6, 10, 11, 13, 14].contains(&semitones);
    above && !(dissonant && lower < LOW_DISSONANCE.value.get())
}

/// Every note of every part sounding at `beat`, with its part.
fn sounding_at(voiced: &Voiced, beat: f64) -> Vec<(String, i64)> {
    voiced
        .parts
        .iter()
        .flat_map(|part| {
            part.notes
                .iter()
                .filter(move |note| note.start <= beat && beat < note.end())
                .map(move |note| (part.part.to_string(), i64::from(note.pitch)))
        })
        .collect()
}

fn ids() -> SequentialIds {
    SequentialIds::new(0)
}

// A theme in D minor, eight bars at one chord per bar.
const THEME: [&str; 8] = ["Dm", "Bb", "Gm", "A", "Dm", "Gm", "A7", "Dm"];

#[test]
fn horns_voice_dm_as_f4_d4_a3_d3() {
    // TST-10: four horns carrying the bass voice D minor as F4, D4, A3, D3 and G minor after it
    // as B♭4, G4, D4, G3, the order from the top 1, 3, 2, 4.
    let voiced = horns(
        &bars(&["Dm", "Gm"]),
        &four_horns(),
        None,
        ChordOptions::default(),
        &mut ids(),
    )
    .expect("the horns voice the chords");
    assert_eq!(pitches(&voiced, "horn_1"), [65, 70]);
    assert_eq!(pitches(&voiced, "horn_3"), [62, 67]);
    assert_eq!(pitches(&voiced, "horn_2"), [57, 62]);
    assert_eq!(pitches(&voiced, "horn_4"), [50, 55]);
    assert!(voiced.remarks.is_empty(), "{:?}", voiced.remarks);
    // Every note lasts its chord at the default velocity on channel 0, with a fresh id.
    let notes = voiced.notes("horn_1").expect("horn 1 plays");
    assert_eq!((notes[0].start, notes[0].length), (0.0, 4.0));
    assert_eq!((notes[1].start, notes[1].length), (4.0, 4.0));
    assert!(
        notes
            .iter()
            .all(|note| note.velocity == DEFAULT_VELOCITY && note.channel == 0)
    );
    let mut all_ids: Vec<&str> = voiced
        .parts
        .iter()
        .flat_map(|part| part.notes.iter().map(|note| note.id.as_str()))
        .collect();
    all_ids.sort_unstable();
    all_ids.dedup();
    assert_eq!(all_ids.len(), 8);
}

#[test]
fn horn_pairs_keep_the_eroica_order_and_their_tessitura() {
    // TST-10: Forsyth on the Eroica, the 3rd horn above the 2nd throughout (VOI-07); no player
    // leaps an octave between chords (PLY-13) and every note is comfortable.
    let voiced = horns(
        &bars(&THEME),
        &four_horns(),
        None,
        ChordOptions::default(),
        &mut ids(),
    )
    .expect("the horns voice the theme");
    let [h1, h2, h3, h4] =
        ["horn_1", "horn_2", "horn_3", "horn_4"].map(|slug| pitches(&voiced, slug));
    assert_eq!(h1.len(), THEME.len());
    let range = comfortable(InstrumentType::HornF);
    for chord in 0..THEME.len() {
        assert!(h1[chord] > h3[chord] && h3[chord] > h2[chord] && h2[chord] > h4[chord]);
        for horn in [&h1, &h2, &h3, &h4] {
            assert!(holds(range, horn[chord]), "{chord}: {}", horn[chord]);
            if chord > 0 {
                assert!((horn[chord] - horn[chord - 1]).abs() <= 12);
            }
        }
        // Horn 4 carries the chord's bass.
        let chord_root = parse_chord(THEME[chord]).expect("a valid chord").root;
        assert_eq!(h4[chord].rem_euclid(12), chord_root);
    }
}

#[test]
fn three_and_two_horns_read_1_3_2_and_1_2() {
    let three = &four_horns()[..3];
    let voiced = horns(
        &bars(&["C", "F"]),
        three,
        None,
        ChordOptions::default(),
        &mut ids(),
    )
    .expect("three horns voice the chords");
    let [h1, h2, h3] = ["horn_1", "horn_2", "horn_3"].map(|slug| pitches(&voiced, slug));
    for chord in 0..2 {
        assert!(h1[chord] > h3[chord] && h3[chord] > h2[chord]);
    }
    let voiced = horns(
        &bars(&["C", "F"]),
        &four_horns()[..2],
        None,
        ChordOptions::default(),
        &mut ids(),
    )
    .expect("two horns voice the chords");
    let [h1, h2] = ["horn_1", "horn_2"].map(|slug| pitches(&voiced, slug));
    // Two voices carrying the bass sound the root and the third.
    assert_eq!(
        h2.iter().map(|p| p.rem_euclid(12)).collect::<Vec<_>>(),
        [0, 5]
    );
    assert!(h1[0] > h2[0] && h1[1] > h2[1]);
    assert_eq!(h1[0].rem_euclid(12), 4);
}

#[test]
fn a_bassoon_doubles_the_horns_bass_an_octave_below() {
    // TST-10: Rimsky-Korsakov, Ch. III, pp. 83 and 90: a bassoon doubles the bass of a four-horn
    // chord an octave below.
    let bassoon = carrier("bassoon_1", InstrumentType::Bassoon);
    let voiced = horns(
        &bars(&["Dm", "Gm"]),
        &four_horns(),
        Some(&bassoon),
        ChordOptions::default(),
        &mut ids(),
    )
    .expect("the horns voice the chords");
    assert_eq!(pitches(&voiced, "bassoon_1"), [38, 43]);
    assert_eq!(pitches(&voiced, "horn_4"), [50, 55]);
    let doublings: Vec<_> = voiced.doublings().collect();
    assert_eq!(doublings.len(), 1);
    let doubling = doublings[0];
    assert_eq!(
        (doubling.part.as_str(), doubling.of.as_str()),
        ("bassoon_1", "horn_4")
    );
    assert_eq!(doubling.octaves, -1);
    let named: &NamedDoubling = doubling.named.expect("the table names the doubling");
    assert!(named.builds_bass);
    assert_eq!(named.spacings, [Spacing::Octaves(1)]);
    // Without the bass, the bassoon has nothing to double and rests.
    let voiced = horns(
        &bars(&["Dm"]),
        &four_horns(),
        Some(&bassoon),
        ChordOptions {
            carries_bass: false,
            ..ChordOptions::default()
        },
        &mut ids(),
    )
    .expect("the horns voice the chord");
    assert_eq!(pitches(&voiced, "bassoon_1"), Vec::<i64>::new());
    assert_eq!(
        voiced.remarks,
        [Remark::Tacet {
            part: part("bassoon_1"),
            rule: None
        }]
    );
}

#[test]
fn horns_reject_what_they_cannot_voice() {
    // TST-03: one input per error.
    let chords = bars(&["C"]);
    let options = ChordOptions::default();
    let one = &four_horns()[..1];
    assert_eq!(
        horns(&chords, one, None, options, &mut ids()),
        Err(VoicingError::PartCount {
            min: 2,
            max: 4,
            given: 1
        })
    );
    let mut five = four_horns();
    five.push(carrier("horn_5", InstrumentType::HornF));
    assert!(matches!(
        horns(&chords, &five, None, options, &mut ids()),
        Err(VoicingError::PartCount { given: 5, .. })
    ));
    let mut with_trumpet = four_horns();
    with_trumpet[3] = carrier("trumpet_1", InstrumentType::TrumpetBb);
    assert_eq!(
        horns(&chords, &with_trumpet, None, options, &mut ids()),
        Err(VoicingError::WrongInstrument {
            part: part("trumpet_1"),
            instrument: InstrumentType::TrumpetBb
        })
    );
    let oboe = carrier("oboe_1", InstrumentType::Oboe);
    assert!(matches!(
        horns(&chords, &four_horns(), Some(&oboe), options, &mut ids()),
        Err(VoicingError::WrongInstrument { .. })
    ));
    let twice = carrier("horn_1", InstrumentType::Bassoon);
    assert_eq!(
        horns(&chords, &four_horns(), Some(&twice), options, &mut ids()),
        Err(VoicingError::DuplicatePart(part("horn_1")))
    );
    let silent = ChordOptions {
        velocity: 0,
        ..options
    };
    assert_eq!(
        horns(&chords, &four_horns(), None, silent, &mut ids()),
        Err(VoicingError::InvalidVelocity(0))
    );
    let mut early = bars(&["C"]);
    early[0].start = -1.0;
    assert!(matches!(
        horns(&early, &four_horns(), None, options, &mut ids()),
        Err(VoicingError::InvalidSlot { .. })
    ));
    let mut empty = bars(&["C"]);
    empty[0].length = 0.0;
    assert!(matches!(
        horns(&empty, &four_horns(), None, options, &mut ids()),
        Err(VoicingError::InvalidSlot { .. })
    ));
}

/// The brass of the Romantic orchestra template, in score order.
fn romantic_brass() -> Vec<Carrier> {
    let romantic = ensemble("romantic_orchestra").expect("the Romantic orchestra exists");
    romantic
        .parts()
        .iter()
        .filter(|template| template.instrument.family() == Family::Brass)
        .map(|template| Carrier::try_from(template).expect("a template slug is a part id"))
        .collect()
}

#[test]
fn a_brass_chorale_puts_trumpets_on_top_and_tuba_under_the_bass_trombone() {
    // TST-10: Rimsky-Korsakov, Ch. III, Harmony in the brass: trumpets on top, the third
    // trombone and the tuba in octaves as the bass, horns doubling the inner parts in pairs.
    let brass = romantic_brass();
    let slugs: Vec<&str> = brass.iter().map(|c| c.part.as_str()).collect();
    assert_eq!(
        slugs,
        [
            "horn_1",
            "horn_2",
            "horn_3",
            "horn_4",
            "trumpet_1",
            "trumpet_2",
            "trombone_1",
            "trombone_2",
            "trombone_3",
            "tuba"
        ]
    );
    let chords = bars(&THEME);
    let voiced = brass_chorale(&chords, &brass, ChordOptions::default(), &mut ids())
        .expect("the brass voice the theme");
    assert!(voiced.remarks.is_empty(), "{:?}", voiced.remarks);
    let part_pitches = |slug: &str| pitches(&voiced, slug);
    let (t1, t2) = (part_pitches("trumpet_1"), part_pitches("trumpet_2"));
    let (b1, b2, b3) = (
        part_pitches("trombone_1"),
        part_pitches("trombone_2"),
        part_pitches("trombone_3"),
    );
    let tuba = part_pitches("tuba");
    for chord in 0..THEME.len() {
        // Close order from the top: trumpets, then trombones, the bass on the bass trombone.
        assert!(t1[chord] > t2[chord] && t2[chord] > b1[chord] && b1[chord] > b2[chord]);
        assert!(b2[chord] > b3[chord]);
        assert_eq!(tuba[chord], b3[chord] - 12);
        let root = parse_chord(THEME[chord]).expect("a valid chord").root;
        assert_eq!(b3[chord].rem_euclid(12), root);
        // Every note sounding together clears its low-interval limit (SPC-01).
        let mut sounding: Vec<i64> = sounding_at(&voiced, chords[chord].start)
            .into_iter()
            .map(|(_, pitch)| pitch)
            .collect();
        sounding.sort_unstable();
        for (index, &lower) in sounding.iter().enumerate() {
            for &upper in &sounding[index + 1..] {
                assert!(
                    upper == lower || clear(lower, upper),
                    "{chord}: {sounding:?}"
                );
            }
        }
    }
    // The horns play in unison pairs, 1 with 3 and 2 with 4, never one horn alone (BAL-02).
    assert_eq!(part_pitches("horn_1"), part_pitches("horn_3"));
    assert_eq!(part_pitches("horn_2"), part_pitches("horn_4"));
    assert_eq!(part_pitches("horn_1"), t2);
    assert_eq!(part_pitches("horn_2"), b1);
    let mut doubled: Vec<(&str, &str, i8)> = voiced
        .doublings()
        .map(|d| (d.part.as_str(), d.of.as_str(), d.octaves))
        .collect();
    doubled.sort_unstable();
    assert_eq!(
        doubled,
        [
            ("horn_1", "trumpet_2", 0),
            ("horn_2", "trombone_1", 0),
            ("horn_3", "trumpet_2", 0),
            ("horn_4", "trombone_1", 0),
            ("tuba", "trombone_3", -1),
        ]
    );
    // The tuba under the bass trombone is the rules' named bass doubling.
    let tuba_row = voiced
        .doublings()
        .find(|d| d.part.as_str() == "tuba")
        .and_then(|d| d.named)
        .expect("the table names the trombone and tuba in octaves");
    assert!(tuba_row.builds_bass);
}

#[test]
fn horn_pairs_fill_a_small_chorale_and_a_lone_horn_rests() {
    // Two trumpets, two horns and one trombone: the pair takes the third part from the top.
    let brass = [
        carrier("trumpet_1", InstrumentType::TrumpetBb),
        carrier("trumpet_2", InstrumentType::TrumpetBb),
        carrier("horn_1", InstrumentType::HornF),
        carrier("horn_2", InstrumentType::HornF),
        carrier("trombone_1", InstrumentType::TromboneTenor),
    ];
    let voiced = brass_chorale(
        &bars(&["C", "F", "G", "C"]),
        &brass,
        ChordOptions::default(),
        &mut ids(),
    )
    .expect("the brass voice the chords");
    let horn_1 = pitches(&voiced, "horn_1");
    assert_eq!(horn_1, pitches(&voiced, "horn_2"));
    let (t2, b1) = (
        pitches(&voiced, "trumpet_2"),
        pitches(&voiced, "trombone_1"),
    );
    for chord in 0..4 {
        assert!(t2[chord] > horn_1[chord] && horn_1[chord] > b1[chord]);
    }
    // Three horns: Horns 1 and 3 pair up and Horn 2 rests rather than hold a part alone.
    let brass = [
        carrier("horn_1", InstrumentType::HornF),
        carrier("horn_2", InstrumentType::HornF),
        carrier("horn_3", InstrumentType::HornF),
        carrier("trumpet_1", InstrumentType::TrumpetC),
        carrier("trumpet_2", InstrumentType::TrumpetC),
        carrier("trombone_1", InstrumentType::TromboneTenor),
        carrier("trombone_2", InstrumentType::TromboneBass),
        carrier("tuba", InstrumentType::Tuba),
    ];
    let voiced = brass_chorale(
        &bars(&["C", "G7", "C"]),
        &brass,
        ChordOptions::default(),
        &mut ids(),
    )
    .expect("the brass voice the chords");
    assert_eq!(pitches(&voiced, "horn_1"), pitches(&voiced, "horn_3"));
    assert_eq!(
        voiced.remarks,
        [Remark::Tacet {
            part: part("horn_2"),
            rule: Some(RuleId::Bal02)
        }]
    );
    assert_eq!(voiced.remarks[0].rule(), Some(RuleId::Bal02));
}

#[test]
fn a_brass_chorale_rejects_what_it_cannot_voice() {
    // TST-03.
    let chords = bars(&["C"]);
    let options = ChordOptions::default();
    let two = [
        carrier("trumpet_1", InstrumentType::TrumpetBb),
        carrier("trombone_1", InstrumentType::TromboneTenor),
    ];
    assert_eq!(
        brass_chorale(&chords, &two, options, &mut ids()),
        Err(VoicingError::TooFewVoices {
            needed: 3,
            formed: 2
        })
    );
    let with_violin = [
        carrier("trumpet_1", InstrumentType::TrumpetBb),
        carrier("violins_1", InstrumentType::Violin),
    ];
    assert_eq!(
        brass_chorale(&chords, &with_violin, options, &mut ids()),
        Err(VoicingError::WrongInstrument {
            part: part("violins_1"),
            instrument: InstrumentType::Violin
        })
    );
    let twice = [
        carrier("trumpet_1", InstrumentType::TrumpetBb),
        carrier("trumpet_1", InstrumentType::TrumpetC),
    ];
    assert_eq!(
        brass_chorale(&chords, &twice, options, &mut ids()),
        Err(VoicingError::DuplicatePart(part("trumpet_1")))
    );
}

fn line(pitches: &[i64]) -> Vec<MidiNote> {
    pitches
        .iter()
        .zip(0_u32..)
        .map(|(&pitch, index)| {
            MidiNote::new(
                format!("n_{index}"),
                pitch,
                f64::from(index),
                1.0,
                70 + i64::from(index),
                2,
            )
            .expect("a valid note")
            .with_articulation(Some("staccato".to_owned()))
            .expect("a vocabulary id")
        })
        .collect()
}

#[test]
fn fit_register_moves_a_harmony_line_by_whole_octaves() {
    // TST-10: a flute line under its comfortable range (60-94) moves up one octave as a whole.
    let flute = instrument("flute").expect("the flute exists");
    let notes = line(&[55, 57, 59]);
    let RegisterFit::Moved {
        notes: moved,
        shift,
        folded,
    } = fit_register(&notes, flute, Role::Harmony)
    else {
        panic!("the line moves");
    };
    assert_eq!(shift, 12);
    assert_eq!(folded, Vec::<String>::new());
    assert_eq!(
        moved.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        [67, 69, 71]
    );
    // Only the pitch moves: id, timing, velocity, channel and articulation are kept.
    for (before, after) in notes.iter().zip(&moved) {
        assert_eq!(
            MidiNote {
                pitch: before.pitch + 12,
                ..before.clone()
            },
            *after
        );
    }
}

#[test]
fn fit_register_folds_the_notes_a_shift_leaves_outside() {
    // A bassoon line (comfortable 34-72) too wide for any one shift: up an octave, then the top
    // note down two octaves on its own.
    let bassoon = instrument("bassoon").expect("the bassoon exists");
    let RegisterFit::Moved {
        notes,
        shift,
        folded,
    } = fit_register(&line(&[30, 50, 75]), bassoon, Role::Bass)
    else {
        panic!("the line moves");
    };
    assert_eq!(shift, 12);
    assert_eq!(
        notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        [42, 62, 63]
    );
    assert_eq!(folded, ["n_2"]);
}

#[test]
fn fit_register_never_moves_a_melody() {
    // A melody is never moved silently: the caller asks, with the move to propose.
    let flute = instrument("flute").expect("the flute exists");
    for role in [Role::Melody, Role::CounterMelody] {
        assert_eq!(
            fit_register(&line(&[55, 57, 59]), flute, role),
            RegisterFit::Ask {
                shift: 12,
                outside: 3
            }
        );
    }
    // A line inside the range, an unpitched instrument and an empty line need nothing.
    assert_eq!(
        fit_register(&line(&[60, 72]), flute, Role::Harmony),
        RegisterFit::Fits
    );
    let snare = instrument("snare_drum").expect("the snare drum exists");
    assert_eq!(
        fit_register(&line(&[38]), snare, Role::Rhythm),
        RegisterFit::Fits
    );
    assert_eq!(fit_register(&[], flute, Role::Harmony), RegisterFit::Fits);
}

#[test]
fn comfortable_ranges_span_an_octave_so_every_note_can_fold_in() {
    // TST-07: the precondition of fit_register's folding.
    for record in instruments() {
        if let PitchModel::Pitched(_) = record.pitch() {
            let range = comfortable(record.instrument_type());
            assert!(
                range.high().get() - range.low().get() >= 11,
                "{}",
                record.id()
            );
        }
    }
}

/// The chord symbols the property tests draw from.
const SYMBOLS: [&str; 12] = [
    "C", "Dm", "Em", "F", "G7", "Am", "Bdim", "Bb", "Ebmaj7", "F#m7b5", "Dsus4", "Ab",
];

proptest! {
    #[test]
    fn harmony_lines_land_inside_the_range(
        pitches in prop::collection::vec(0_i64..=127, 1..12),
        index in 0_usize..85,
    ) {
        // TST-07: after fitting, every note of a harmony line is comfortable (RNG-01, RNG-02).
        let record = &instruments()[index];
        prop_assume!(matches!(record.pitch(), PitchModel::Pitched(_)));
        let range = comfortable(record.instrument_type());
        match fit_register(&line(&pitches), record, Role::Harmony) {
            RegisterFit::Fits => {
                prop_assert!(pitches.iter().all(|&p| holds(range, p)));
            }
            RegisterFit::Moved { notes, shift, .. } => {
                prop_assert_eq!(shift % 12, 0);
                for (note, &before) in notes.iter().zip(&pitches) {
                    let after = i64::from(note.pitch);
                    prop_assert!(holds(range, after), "{} -> {}", before, after);
                    prop_assert_eq!((after - before) % 12, 0);
                }
            }
            RegisterFit::Ask { .. } => prop_assert!(false, "a harmony line is never asked about"),
        }
    }

    #[test]
    fn every_family_stays_comfortable_and_clears_the_limits(
        symbols in prop::collection::vec(0_usize..SYMBOLS.len(), 1..8),
        layout in prop::sample::select(vec![WindLayout::Superposed, WindLayout::Interlocked]),
    ) {
        // TST-09: any progression keeps every note in its part's comfortable range and every
        // two notes sounding together clear their low limits, in every family.
        let progression: Vec<&str> = symbols.iter().map(|&i| SYMBOLS[i]).collect();
        let chords = bars(&progression);
        let brass = romantic_brass();
        let strings = romantic_strings();
        let mut winds = pair("oboe", InstrumentType::Oboe).to_vec();
        winds.extend(pair("clarinet", InstrumentType::ClarinetBb));
        let mut given = brass.clone();
        given.extend(strings.iter().cloned());
        given.extend(winds.iter().cloned());
        for voiced in [
            brass_chorale(&chords, &brass, ChordOptions::default(), &mut ids()),
            horns(&chords, &four_horns(), None, ChordOptions::default(), &mut ids()),
            strings_divisi(&chords, &strings, 7, ChordOptions::default(), &mut ids()),
            strings_divisi(&chords, &strings, 4, ChordOptions::default(), &mut ids()),
            woodwind_pairs(&chords, &winds, upper_options(layout), &mut ids()),
        ] {
            let voiced = voiced.expect("a family voices any chord");
            for notes in &voiced.parts {
                let carrier = given
                    .iter()
                    .find(|c| c.part == notes.part)
                    .expect("every part was given");
                let range = comfortable(carrier.instrument);
                for note in &notes.notes {
                    prop_assert!(holds(range, i64::from(note.pitch)));
                }
            }
            for slot in &chords {
                let mut sounding: Vec<i64> =
                    sounding_at(&voiced, slot.start).into_iter().map(|(_, p)| p).collect();
                sounding.sort_unstable();
                sounding.dedup();
                for (index, &lower) in sounding.iter().enumerate() {
                    for &upper in &sounding[index + 1..] {
                        prop_assert!(clear(lower, upper), "{:?} {:?}", progression, sounding);
                    }
                }
            }
        }
    }
}

#[test]
fn a_template_part_becomes_a_carrier_under_its_slug() {
    let romantic = ensemble("romantic_orchestra").expect("the Romantic orchestra exists");
    let horn = romantic
        .part("horn_3")
        .expect("the Romantic orchestra has a third horn");
    assert_eq!(
        Carrier::try_from(horn),
        Ok(carrier("horn_3", InstrumentType::HornF))
    );
    // TST-03: a part built at run time with a display name for a slug is refused.
    let named = Part {
        slug: "Horn 3",
        ..*horn
    };
    assert_eq!(
        Carrier::try_from(&named),
        Err(PerformanceError::InvalidPartId("Horn 3".to_owned()))
    );
}

/// The five string sections of the Romantic orchestra template, in score order.
fn romantic_strings() -> Vec<Carrier> {
    let romantic = ensemble("romantic_orchestra").expect("the Romantic orchestra exists");
    romantic
        .parts()
        .iter()
        .filter(|template| template.instrument.family() == Family::Strings)
        .map(|template| Carrier::try_from(template).expect("a template slug is a part id"))
        .collect()
}

/// The notes of `slug` sounding at `beat`, from the lowest.
fn chord_of(voiced: &Voiced, slug: &str, beat: f64) -> Vec<i64> {
    let mut pitches: Vec<i64> = voiced
        .notes(slug)
        .unwrap_or_else(|| panic!("{slug} is a part of the result"))
        .iter()
        .filter(|note| note.start <= beat && beat < note.end())
        .map(|note| i64::from(note.pitch))
        .collect();
    pitches.sort_unstable();
    pitches
}

#[test]
fn strings_divide_every_upper_section_alike() {
    // TST-10: Rimsky-Korsakov, Ch. III, String harmony, p. 70: for six or seven parts every upper
    // section divides the same way, the cellos take the bass and the double basses double it an
    // octave below.
    let strings = romantic_strings();
    let chords = bars(&THEME);
    let voiced = strings_divisi(&chords, &strings, 7, ChordOptions::default(), &mut ids())
        .expect("the strings voice the theme");
    let divided: Vec<(&str, u8)> = voiced
        .remarks
        .iter()
        .map(|remark| match remark {
            Remark::Divided { part, ways } => (part.as_str(), *ways),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(divided, [("violas", 2), ("violins_2", 2), ("violins_1", 2)]);
    assert!(
        voiced
            .remarks
            .iter()
            .all(|r| r.rule() == Some(RuleId::Voi10))
    );
    for (slot, symbol) in chords.iter().zip(THEME) {
        let beat = slot.start;
        let [v1, v2, va] = ["violins_1", "violins_2", "violas"].map(|s| chord_of(&voiced, s, beat));
        let (vc, cb) = (
            chord_of(&voiced, "cellos", beat),
            chord_of(&voiced, "basses", beat),
        );
        for halves in [&v1, &v2, &va] {
            assert_eq!(halves.len(), 2, "{symbol}");
            // The halves of a section never lie a unison or an octave apart (DBL-05, DBL-06).
            assert_ne!((halves[1] - halves[0]) % 12, 0, "{symbol} {halves:?}");
        }
        assert!(v1[0] > v2[1] && v2[0] > va[1] && va[0] > vc[0], "{symbol}");
        let root = parse_chord(symbol).expect("a valid chord").root;
        assert_eq!(vc[0].rem_euclid(12), root);
        assert_eq!(cb, [vc[0] - 12]);
    }
    let doubling = voiced
        .doublings()
        .find(|d| d.part.as_str() == "basses")
        .expect("the basses double the cellos");
    assert_eq!((doubling.of.as_str(), doubling.octaves), ("cellos", -1));
    assert!(doubling.named.is_some_and(|row| row.builds_bass));
}

#[test]
fn four_string_parts_need_no_divisi_and_fewer_notes_spare_the_first_violins() {
    let strings = romantic_strings();
    let chords = bars(&["C", "F", "G7", "C"]);
    let voiced = strings_divisi(&chords, &strings, 4, ChordOptions::default(), &mut ids())
        .expect("the strings voice the chords");
    assert!(voiced.remarks.is_empty(), "{:?}", voiced.remarks);
    for slug in ["violins_1", "violins_2", "violas", "cellos", "basses"] {
        assert_eq!(pitches(&voiced, slug).len(), 4, "{slug}");
    }
    // Two upper notes go to the second violins with the violas (Rimsky-Korsakov, Ch. I, p. 7);
    // the first violins stay free for a melody.
    let voiced = strings_divisi(&chords, &strings, 3, ChordOptions::default(), &mut ids())
        .expect("the strings voice the chords");
    assert_eq!(pitches(&voiced, "violins_1"), Vec::<i64>::new());
    assert_eq!(pitches(&voiced, "violins_2").len(), 4);
    assert_eq!(pitches(&voiced, "violas").len(), 4);
    assert_eq!(
        voiced.remarks,
        [Remark::Tacet {
            part: part("violins_1"),
            rule: None
        }]
    );
}

#[test]
fn double_basses_never_play_without_cellos() {
    // DBL-08: without cellos the violas take the bass and the double basses rest.
    let strings = [
        carrier("violins_1", InstrumentType::Violin),
        carrier("violas", InstrumentType::Viola),
        carrier("basses", InstrumentType::DoubleBass),
    ];
    let chords = bars(&["C", "G"]);
    let voiced = strings_divisi(&chords, &strings, 3, ChordOptions::default(), &mut ids())
        .expect("the strings voice the chords");
    let violas = pitches(&voiced, "violas");
    assert_eq!(
        violas.iter().map(|p| p.rem_euclid(12)).collect::<Vec<_>>(),
        [0, 7]
    );
    assert_eq!(
        voiced.remarks.last(),
        Some(&Remark::Tacet {
            part: part("basses"),
            rule: Some(RuleId::Dbl08)
        })
    );
    // Without the bass the cellos join the upper sections and the basses rest.
    let voiced = strings_divisi(
        &chords,
        &romantic_strings(),
        4,
        ChordOptions {
            carries_bass: false,
            ..ChordOptions::default()
        },
        &mut ids(),
    )
    .expect("the strings voice the chords");
    assert_eq!(pitches(&voiced, "cellos").len(), 2);
    assert_eq!(
        voiced.remarks,
        [Remark::Tacet {
            part: part("basses"),
            rule: None
        }]
    );
}

#[test]
fn strings_reject_what_they_cannot_voice() {
    // TST-03.
    let chords = bars(&["C"]);
    let options = ChordOptions::default();
    assert_eq!(
        strings_divisi(&chords, &romantic_strings(), 0, options, &mut ids()),
        Err(VoicingError::TooFewVoices {
            needed: 1,
            formed: 0
        })
    );
    let two = [
        carrier("violins_1", InstrumentType::Violin),
        carrier("cellos", InstrumentType::Cello),
    ];
    assert_eq!(
        strings_divisi(&chords, &two, 4, options, &mut ids()),
        Err(VoicingError::TooManyVoices { asked: 4, most: 3 })
    );
    let with_flute = [
        carrier("violins_1", InstrumentType::Violin),
        carrier("flute_1", InstrumentType::Flute),
    ];
    assert!(matches!(
        strings_divisi(&chords, &with_flute, 2, options, &mut ids()),
        Err(VoicingError::WrongInstrument { .. })
    ));
}

fn pair(name: &str, instrument: InstrumentType) -> [Carrier; 2] {
    [
        carrier(&format!("{name}_1"), instrument),
        carrier(&format!("{name}_2"), instrument),
    ]
}

/// Chords over a bass the strings carry.
fn upper_options(layout: WindLayout) -> WindOptions {
    WindOptions {
        chord: ChordOptions {
            carries_bass: false,
            ..ChordOptions::default()
        },
        layout,
        ..WindOptions::default()
    }
}

#[test]
fn woodwind_pairs_superpose_in_the_natural_order() {
    // TST-10: Rimsky-Korsakov, Ch. III, Wood-wind harmony: the pairs stacked in the natural
    // order, each pair's two players a concord apart (VOI-05).
    let mut winds = pair("clarinet", InstrumentType::ClarinetBb).to_vec();
    winds.extend(pair("oboe", InstrumentType::Oboe));
    let chords = bars(&THEME);
    let voiced = woodwind_pairs(
        &chords,
        &winds,
        upper_options(WindLayout::Superposed),
        &mut ids(),
    )
    .expect("the woodwinds voice the theme");
    assert!(voiced.remarks.is_empty(), "{:?}", voiced.remarks);
    let [o1, o2, c1, c2] =
        ["oboe_1", "oboe_2", "clarinet_1", "clarinet_2"].map(|slug| pitches(&voiced, slug));
    for chord in 0..THEME.len() {
        assert!(o1[chord] > o2[chord] && o2[chord] > c1[chord] && c1[chord] > c2[chord]);
        for (upper, lower) in [(o1[chord], o2[chord]), (c1[chord], c2[chord])] {
            assert!([0, 3, 4, 8, 9].contains(&((upper - lower) % 12)), "{chord}");
        }
    }
}

#[test]
fn woodwind_pairs_interlock_oboes_and_clarinets_for_a_background() {
    // TST-10: Forsyth, p. 262: 1st oboe, 1st clarinet, 2nd oboe, 2nd clarinet, the way the two
    // pairs of horns interlock.
    let mut winds = pair("oboe", InstrumentType::Oboe).to_vec();
    winds.extend(pair("clarinet", InstrumentType::ClarinetBb));
    let chords = bars(&THEME);
    let voiced = woodwind_pairs(
        &chords,
        &winds,
        upper_options(WindLayout::Interlocked),
        &mut ids(),
    )
    .expect("the woodwinds voice the theme");
    let [o1, o2, c1, c2] =
        ["oboe_1", "oboe_2", "clarinet_1", "clarinet_2"].map(|slug| pitches(&voiced, slug));
    for chord in 0..THEME.len() {
        assert!(o1[chord] > c1[chord] && c1[chord] > o2[chord] && o2[chord] > c2[chord]);
        for (upper, lower) in [(o1[chord], o2[chord]), (c1[chord], c2[chord])] {
            assert!([0, 3, 4, 8, 9].contains(&((upper - lower) % 12)), "{chord}");
        }
    }
}

#[test]
fn four_colours_are_never_close_and_three_need_a_pair() {
    // VOI-02: flute, oboe, clarinet and bassoon in one chord lie wider than an octave.
    let winds = [
        carrier("flute_1", InstrumentType::Flute),
        carrier("oboe_1", InstrumentType::Oboe),
        carrier("clarinet_1", InstrumentType::ClarinetBb),
        carrier("bassoon_1", InstrumentType::Bassoon),
    ];
    let chords = bars(&["C", "Am", "F", "G"]);
    let voiced = woodwind_pairs(&chords, &winds, WindOptions::default(), &mut ids())
        .expect("the woodwinds voice the chords");
    for slot in &chords {
        let sounding: Vec<i64> = sounding_at(&voiced, slot.start)
            .into_iter()
            .map(|(_, p)| p)
            .collect();
        let span = sounding.iter().max().unwrap() - sounding.iter().min().unwrap();
        assert!(span > 12, "{sounding:?}");
    }
    // TST-03, VOI-01: three players of three colours.
    assert_eq!(
        woodwind_pairs(&chords, &winds[..3], WindOptions::default(), &mut ids()),
        Err(VoicingError::Breaks(RuleId::Voi01))
    );
    assert_eq!(
        VoicingError::Breaks(RuleId::Voi01).to_string(),
        "the parts given would break VOI-01"
    );
    // Interlocking needs two pairs.
    assert_eq!(
        woodwind_pairs(
            &chords,
            &winds,
            upper_options(WindLayout::Interlocked),
            &mut ids()
        ),
        Err(VoicingError::NotTwoPairs)
    );
    // A saxophone has no place in the natural order.
    let sax = [carrier("alto_sax", InstrumentType::SaxophoneAlto)];
    assert!(matches!(
        woodwind_pairs(&chords, &sax, WindOptions::default(), &mut ids()),
        Err(VoicingError::WrongInstrument { .. })
    ));
    assert_eq!(
        woodwind_pairs(&chords, &[], WindOptions::default(), &mut ids()),
        Err(VoicingError::TooFewVoices {
            needed: 1,
            formed: 0
        })
    );
}

/// A melody of `pitches`, one beat each, played legato at velocity 90 on channel 1.
fn melody(pitches: &[i64]) -> Vec<MidiNote> {
    pitches
        .iter()
        .zip(0_u32..)
        .map(|(&pitch, index)| {
            MidiNote::new(format!("m_{index}"), pitch, f64::from(index), 1.0, 90, 1)
                .expect("a valid note")
                .with_articulation(Some("legato".to_owned()))
                .expect("a vocabulary id")
        })
        .collect()
}

/// A theme between A4 and A5 (MIDI 69-81).
const THEME_MELODY: [i64; 8] = [69, 74, 77, 76, 74, 81, 79, 77];

/// The parts of the Romantic orchestra whose family is one of `families`, in score order.
fn romantic(families: &[Family]) -> Vec<Carrier> {
    let romantic = ensemble("romantic_orchestra").expect("the Romantic orchestra exists");
    romantic
        .parts()
        .iter()
        .filter(|template| families.contains(&template.instrument.family()))
        .map(|template| Carrier::try_from(template).expect("a template slug is a part id"))
        .collect()
}

/// The octaves each doubling part plays above the carrier, sorted by part.
fn octaves(result: &MelodyDoubling) -> Vec<(String, i8)> {
    let mut lines: Vec<(String, i8)> = result
        .voiced
        .doublings()
        .map(|d| {
            assert_eq!(*d.of, result.carrier);
            assert_eq!(d.named, result.doubling);
            (d.part.to_string(), d.octaves)
        })
        .collect();
    lines.sort();
    lines
}

#[test]
fn strings_double_a_middle_melody_in_unison_and_at_the_octave() {
    // TST-10: Rimsky-Korsakov, Ch. II, p. 43: first and second violins in unison over violas and
    // cellos in unison an octave lower, for a melody in the middle range; the double basses rest.
    let theme = melody(&THEME_MELODY);
    let result = melody_doublings(&theme, &romantic_strings(), Density::Medium, &mut ids())
        .expect("the strings double the melody");
    assert_eq!(result.carrier.as_str(), "violins_1");
    assert_eq!(result.density, Density::Medium);
    let row = result.doubling.expect("a named doubling");
    assert_eq!(row.spacings, [Spacing::Octaves(1)]);
    assert_eq!(row.members[0].selector.ids(), [InstrumentType::Violin]);
    assert_eq!(
        row.members[1].selector.ids(),
        [InstrumentType::Viola, InstrumentType::Cello]
    );
    assert_eq!(
        octaves(&result),
        [
            ("cellos".to_owned(), -1),
            ("violas".to_owned(), -1),
            ("violins_2".to_owned(), 0)
        ]
    );
    let voiced = &result.voiced;
    assert_eq!(pitches(voiced, "violins_1"), THEME_MELODY);
    assert_eq!(pitches(voiced, "violins_2"), THEME_MELODY);
    let lower: Vec<i64> = THEME_MELODY.iter().map(|p| p - 12).collect();
    assert_eq!(pitches(voiced, "violas"), lower);
    assert_eq!(pitches(voiced, "cellos"), lower);
    assert_eq!(
        voiced.remarks,
        [Remark::Tacet {
            part: part("basses"),
            rule: None
        }]
    );
    // Each line keeps the melody's timing, velocity, channel and articulation under fresh ids.
    for (note, written) in voiced
        .notes("violas")
        .expect("the violas play")
        .iter()
        .zip(&theme)
    {
        assert_ne!(note.id, written.id);
        assert_eq!(
            (note.start, note.length, note.velocity, note.channel),
            (
                written.start,
                written.length,
                written.velocity,
                written.channel
            )
        );
        assert_eq!(note.articulation.as_deref(), Some("legato"));
    }
}

#[test]
fn a_thin_melody_is_one_colour_and_a_full_one_the_three_groups() {
    let theme = melody(&THEME_MELODY);
    let thin = melody_doublings(&theme, &romantic_strings(), Density::Thin, &mut ids())
        .expect("the strings carry the melody");
    assert_eq!(thin.doubling, None);
    assert_eq!(thin.density, Density::Thin);
    assert_eq!(octaves(&thin), Vec::<(String, i8)>::new());
    assert_eq!(pitches(&thin.voiced, "violins_1"), THEME_MELODY);
    assert_eq!(thin.voiced.remarks.len(), 4);
    // TST-10: Rimsky-Korsakov, Ch. II, p. 61: violins, a woodwind and a trumpet in unison, the
    // most natural tutti melody of the three groups.
    let tutti = romantic(&[Family::Woodwinds, Family::Brass, Family::Strings]);
    let full = melody_doublings(&theme, &tutti, Density::Full, &mut ids())
        .expect("the tutti doubles the melody");
    assert_eq!(full.carrier.as_str(), "violins_1");
    assert_eq!(full.density, Density::Full);
    let row = full.doubling.expect("a named doubling");
    assert_eq!(row.spacings, [Spacing::Unison]);
    let families: Vec<Family> = row
        .members
        .iter()
        .map(|member| member.selector.ids()[0].family())
        .collect();
    assert_eq!(
        families,
        [Family::Strings, Family::Woodwinds, Family::Brass]
    );
    let lines = octaves(&full);
    assert!(lines.iter().all(|(_, octave)| *octave == 0), "{lines:?}");
    for slug in ["violins_2", "flute_1", "trumpet_1"] {
        assert!(
            lines.iter().any(|(part, _)| part == slug),
            "{slug}: {lines:?}"
        );
    }
    // Without woodwinds and brass the strings fall back to their own doubling, and say so.
    let strings_only = melody_doublings(&theme, &romantic_strings(), Density::Full, &mut ids())
        .expect("the strings double the melody");
    assert_eq!(strings_only.density, Density::Medium);
    assert_eq!(
        strings_only.voiced.remarks.first(),
        Some(&Remark::Thinned {
            asked: Density::Full,
            written: Density::Medium
        })
    );
}

#[test]
fn a_high_violin_melody_is_doubled_an_octave_below_even_thin() {
    // RNG-04: above MIDI 90 a violin melody wants the octave below, here the second violins.
    let high = melody(&[86, 90, 93, 91]);
    let violins = [
        carrier("violins_1", InstrumentType::Violin),
        carrier("violins_2", InstrumentType::Violin),
    ];
    let result = melody_doublings(&high, &violins, Density::Thin, &mut ids())
        .expect("the violins double the melody");
    assert_eq!(result.density, Density::Thin);
    assert_eq!(octaves(&result), [("violins_2".to_owned(), -1)]);
    assert_eq!(result.voiced.remarks, []);
    // A single violin part cannot give it, and the result says so.
    let alone = melody_doublings(&high, &violins[..1], Density::Thin, &mut ids())
        .expect("the violins carry the melody");
    assert_eq!(
        alone.voiced.remarks,
        [Remark::Undoubled {
            rule: RuleId::Rng04
        }]
    );
    assert_eq!(alone.voiced.remarks[0].rule(), Some(RuleId::Rng04));
}

#[test]
fn a_double_bass_melody_is_supported_by_a_bassoon() {
    // DBL-08: no double-bass phrase without cellos or bassoons (Rimsky-Korsakov, Ch. II, p. 38).
    let low = melody(&[43, 45, 47, 50]);
    let parts = [
        carrier("basses", InstrumentType::DoubleBass),
        carrier("bassoon_1", InstrumentType::Bassoon),
    ];
    let result = melody_doublings(&low, &parts, Density::Thin, &mut ids())
        .expect("the basses carry the melody");
    assert_eq!(result.carrier.as_str(), "basses");
    assert_eq!(octaves(&result), [("bassoon_1".to_owned(), 0)]);
    let alone = melody_doublings(&low, &parts[..1], Density::Medium, &mut ids())
        .expect("the basses carry the melody");
    assert_eq!(
        alone.voiced.remarks,
        [
            Remark::Thinned {
                asked: Density::Medium,
                written: Density::Thin
            },
            Remark::Undoubled {
                rule: RuleId::Dbl08
            }
        ]
    );
}

#[test]
fn strings_and_brass_never_share_a_unison_without_woodwind() {
    // DBL-09: violins and a trumpet in unison stay apart; with only them the violins play alone.
    let parts = [
        carrier("violins_1", InstrumentType::Violin),
        carrier("trumpet_1", InstrumentType::TrumpetBb),
    ];
    let result = melody_doublings(&melody(&THEME_MELODY), &parts, Density::Medium, &mut ids())
        .expect("the violins carry the melody");
    assert_eq!(result.doubling, None);
    assert_eq!(result.density, Density::Thin);
    assert_eq!(pitches(&result.voiced, "trumpet_1"), Vec::<i64>::new());
}

#[test]
fn the_function_table_chooses_the_carrier() {
    // A tenor melody goes to the cellos before the violas; a soprano melody to the violins
    // before an instrument the table does not list for it, which carries only when alone.
    let tenor = melody(&[50, 53, 57, 55]);
    let parts = [
        carrier("violas", InstrumentType::Viola),
        carrier("cellos", InstrumentType::Cello),
    ];
    let result = melody_doublings(&tenor, &parts, Density::Thin, &mut ids())
        .expect("the strings carry the melody");
    assert_eq!(result.carrier.as_str(), "cellos");
    let soprano = melody(&THEME_MELODY);
    let parts = [
        carrier("harp", InstrumentType::Harp),
        carrier("violins_1", InstrumentType::Violin),
    ];
    let result = melody_doublings(&soprano, &parts, Density::Thin, &mut ids())
        .expect("the violins carry the melody");
    assert_eq!(result.carrier.as_str(), "violins_1");
    let result = melody_doublings(&soprano, &parts[..1], Density::Thin, &mut ids())
        .expect("the harp carries the melody");
    assert_eq!(result.carrier.as_str(), "harp");
}

#[test]
fn melody_doublings_reject_what_they_cannot_double() {
    // TST-03.
    assert_eq!(
        melody_doublings(&[], &romantic_strings(), Density::Medium, &mut ids()),
        Err(VoicingError::EmptyMelody)
    );
    assert_eq!(
        melody_doublings(
            &melody(&[100, 104]),
            &romantic_strings(),
            Density::Medium,
            &mut ids()
        ),
        Err(VoicingError::NoCarrier)
    );
    let twice = [
        carrier("violins_1", InstrumentType::Violin),
        carrier("violins_1", InstrumentType::Violin),
    ];
    assert_eq!(
        melody_doublings(&melody(&THEME_MELODY), &twice, Density::Medium, &mut ids()),
        Err(VoicingError::DuplicatePart(part("violins_1")))
    );
}

proptest! {
    #[test]
    fn a_doubled_melody_stays_comfortable_and_keeps_the_rules(
        start in 36_i64..=90,
        steps in prop::collection::vec(-4_i64..=4, 1..10),
        density in prop::sample::select(vec![Density::Thin, Density::Medium, Density::Full]),
    ) {
        // TST-09: the carrier plays the melody as written, every line moves by whole octaves
        // inside its part's comfortable range, and no doubling breaks the woodwind order
        // (DBL-02) or sets strings and brass in unison without woodwind (DBL-09).
        let mut pitch = start;
        let mut written = vec![pitch];
        for step in steps {
            pitch = (pitch + step).clamp(0, 127);
            written.push(pitch);
        }
        let tutti = romantic(&[Family::Woodwinds, Family::Brass, Family::Strings]);
        let result = match melody_doublings(&melody(&written), &tutti, density, &mut ids()) {
            Ok(result) => result,
            Err(error) => {
                // TST-03: the only refusal is a melody no part holds inside its range.
                prop_assert_eq!(error, VoicingError::NoCarrier);
                let fits = |carrier: &Carrier| {
                    let range = comfortable(carrier.instrument);
                    written.iter().all(|&p| holds(range, p))
                };
                prop_assert!(!tutti.iter().any(fits));
                return Ok(());
            }
        };
        prop_assert_eq!(pitches(&result.voiced, result.carrier.as_str()), written.clone());
        let mut lines: Vec<(InstrumentType, i8)> = Vec::new();
        for doubling in result.voiced.doublings() {
            let carrier = tutti.iter().find(|c| c.part == *doubling.part).expect("a given part");
            let played = pitches(&result.voiced, doubling.part.as_str());
            let shift = i64::from(doubling.octaves) * 12;
            let expected: Vec<i64> = written.iter().map(|p| p + shift).collect();
            prop_assert_eq!(&played, &expected);
            let range = comfortable(carrier.instrument);
            prop_assert!(played.iter().all(|&p| holds(range, p)));
            lines.push((carrier.instrument, doubling.octaves));
        }
        let main = tutti.iter().find(|c| c.part == result.carrier).expect("a given part");
        lines.push((main.instrument, 0));
        for &(upper, high) in &lines {
            for &(lower, low) in &lines {
                let winds = upper.family() == Family::Woodwinds && lower.family() == Family::Woodwinds;
                if high > low && winds {
                    prop_assert!(
                        WOODWIND_ORDER.rank(upper)
                            <= WOODWIND_ORDER.rank(lower)
                    );
                }
                if high == low && upper.family() == Family::Strings && lower.family() == Family::Brass {
                    prop_assert!(lines.iter().any(|&(other, at)| at == high && other.family() == Family::Woodwinds));
                }
            }
        }
    }
}

/// A score at 72 BPM with `signatures`, the clip starting at timeline beat `origin`.
fn score_at(signatures: TimeSignatureMap, origin: f64) -> ScoreContext {
    ScoreContext::new(
        TempoMap::constant(72.0).expect("a valid tempo"),
        signatures,
        origin,
    )
    .expect("a valid origin")
}

fn timpani_part() -> Carrier {
    carrier("timpani", InstrumentType::Timpani)
}

/// The (start, length, pitch, articulation) of every note of `part`.
fn hits(voiced: &Voiced, part: &str) -> Vec<(f64, f64, i64, Option<String>)> {
    voiced
        .notes(part)
        .expect("the part is in the result")
        .iter()
        .map(|note| {
            (
                note.start,
                note.length,
                i64::from(note.pitch),
                note.articulation.clone(),
            )
        })
        .collect()
}

#[test]
fn timpani_tune_d_and_a_hit_the_tonic_and_dominant_bars_and_roll_the_close() {
    // TST-10: in D minor the drums are D2 and A2 (Forsyth: two drums on the main harmonic notes);
    // the chords over D or A get a hit on their downbeat, the closing D minor a roll.
    let key = parse_key("D minor").expect("a valid key");
    let part = timpani(
        &bars(&THEME),
        &timpani_part(),
        &score_at(TimeSignatureMap::default(), 0.0),
        TimpaniOptions::new(key),
        &mut ids(),
    )
    .expect("the timpani play the theme");
    assert_eq!(part.drums.map(SoundingPitch::get), [38, 45]);
    let roll = Some("roll".to_owned());
    assert_eq!(
        hits(&part.voiced, "timpani"),
        [
            (0.0, 1.0, 38, None),
            (12.0, 1.0, 45, None),
            (16.0, 1.0, 38, None),
            (24.0, 1.0, 45, None),
            (28.0, 4.0, 38, roll),
        ]
    );
    assert_eq!(part.voiced.remarks, []);
    assert!(
        part.voiced
            .notes("timpani")
            .expect("the timpani play")
            .iter()
            .all(|note| note.velocity == DEFAULT_VELOCITY)
    );
}

#[test]
fn timpani_take_the_lowest_fourth_or_fifth_never_a_third() {
    // PLY-12: in every key the drums are the tonic and the dominant, a fourth or a fifth apart,
    // inside the comfortable range; C takes G2 and C3, E takes E2 and B2.
    let range = comfortable(InstrumentType::Timpani);
    let chords = bars(&["C"]);
    let score = score_at(TimeSignatureMap::default(), 0.0);
    for (name, expected) in [
        ("C", Some([43, 48])),
        ("E", Some([40, 47])),
        ("D", Some([38, 45])),
    ]
    .into_iter()
    .chain(
        ["F", "Gb", "G", "Ab", "A", "Bb", "B", "Db", "Eb"]
            .into_iter()
            .map(|name| (name, None)),
    ) {
        let key = parse_key(name).expect("a valid key");
        let part = timpani(
            &chords,
            &timpani_part(),
            &score,
            TimpaniOptions::new(key),
            &mut ids(),
        )
        .expect("the timpani tune to any key");
        let [low, high] = part.drums.map(SoundingPitch::get);
        if let Some(expected) = expected {
            assert_eq!([low, high], expected, "{name}");
        }
        assert!(holds(range, low) && holds(range, high), "{name}");
        assert!([5, 7].contains(&(high - low)), "{name}");
        let tonic = key.root;
        let classes = [low.rem_euclid(12), high.rem_euclid(12)];
        assert!(
            classes.contains(&tonic) && classes.contains(&((tonic + 7) % 12)),
            "{name}"
        );
    }
}

#[test]
fn timpani_roll_a_held_pedal_and_follow_the_metre_and_the_clip() {
    let key = parse_key("D minor").expect("a valid key");
    // A tonic pedal under changing chords is one roll through all of them.
    let pedal = timpani(
        &bars(&["Dm", "Gm/D", "A7/D", "Dm"]),
        &timpani_part(),
        &score_at(TimeSignatureMap::default(), 0.0),
        TimpaniOptions::new(key),
        &mut ids(),
    )
    .expect("the timpani play the pedal");
    assert_eq!(
        hits(&pedal.voiced, "timpani"),
        [(0.0, 16.0, 38, Some("roll".to_owned()))]
    );
    // In 3/4 a chord of two bars gets a hit on each downbeat; chords over other basses get none.
    let three_four = TimeSignatureMap::new(vec![
        TimeSignaturePoint::new(0, 3, 4).expect("a valid metre"),
    ])
    .expect("a valid map");
    let slots = vec![
        ChordSlot {
            chord: parse_chord("Dm").expect("a valid chord"),
            start: 0.0,
            length: 6.0,
        },
        ChordSlot {
            chord: parse_chord("Bb").expect("a valid chord"),
            start: 6.0,
            length: 3.0,
        },
        ChordSlot {
            chord: parse_chord("Gm").expect("a valid chord"),
            start: 9.0,
            length: 3.0,
        },
    ];
    let waltz = timpani(
        &slots,
        &timpani_part(),
        &score_at(three_four, 0.0),
        TimpaniOptions::new(key),
        &mut ids(),
    )
    .expect("the timpani play the waltz");
    assert_eq!(
        hits(&waltz.voiced, "timpani"),
        [(0.0, 1.0, 38, None), (3.0, 1.0, 38, None)]
    );
    // A clip starting on timeline beat 2 hits on the next timeline downbeat, content beat 2.
    let shifted = timpani(
        &bars(&["Dm", "Gm", "A", "Dm"]),
        &timpani_part(),
        &score_at(TimeSignatureMap::default(), 2.0),
        TimpaniOptions::new(key),
        &mut ids(),
    )
    .expect("the timpani play the chords");
    assert_eq!(
        hits(&shifted.voiced, "timpani"),
        [
            (2.0, 1.0, 38, None),
            (10.0, 1.0, 45, None),
            (12.0, 4.0, 38, Some("roll".to_owned()))
        ]
    );
}

#[test]
fn timpani_rest_without_their_notes_and_reject_another_instrument() {
    // TST-03: chords over neither drum leave the timpani silent, and the result says so.
    let key = parse_key("D minor").expect("a valid key");
    let score = score_at(TimeSignatureMap::default(), 0.0);
    let silent = timpani(
        &bars(&["Bb", "Gm", "C"]),
        &timpani_part(),
        &score,
        TimpaniOptions::new(key),
        &mut ids(),
    )
    .expect("the timpani rest");
    assert_eq!(
        silent.voiced.remarks,
        [Remark::Tacet {
            part: part("timpani"),
            rule: None
        }]
    );
    let snare = carrier("snare", InstrumentType::SnareDrum);
    assert!(matches!(
        timpani(
            &bars(&["Dm"]),
            &snare,
            &score,
            TimpaniOptions::new(key),
            &mut ids()
        ),
        Err(VoicingError::WrongInstrument { .. })
    ));
    let quiet = TimpaniOptions {
        velocity: 200,
        ..TimpaniOptions::new(key)
    };
    assert_eq!(
        timpani(&bars(&["Dm"]), &timpani_part(), &score, quiet, &mut ids()),
        Err(VoicingError::InvalidVelocity(200))
    );
    let mut endless = bars(&["Dm"]);
    endless[0].length = f64::INFINITY;
    assert!(matches!(
        timpani(
            &endless,
            &timpani_part(),
            &score,
            TimpaniOptions::new(key),
            &mut ids()
        ),
        Err(VoicingError::InvalidSlot { .. })
    ));
}
