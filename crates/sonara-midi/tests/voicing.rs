//! Contract tests of the voicing and doubling helpers through the public API.

use proptest::prelude::*;
use sonara_core::ids::SequentialIds;
use sonara_core::midi::MidiNote;
use sonara_midi::generators::ChordSlot;
use sonara_midi::orchestration::rules::{
    LOW_DISSONANCE, LOW_INTERVAL_LIMITS, Limit, NamedDoubling, RuleId, Spacing,
};
use sonara_midi::orchestration::{
    Family, InstrumentType, Part, PitchModel, PitchRange, ensemble, instrument, instruments,
};
use sonara_midi::performance::voicing::{
    Carrier, ChordOptions, DEFAULT_VELOCITY, RegisterFit, Remark, Voiced, VoicingError,
    brass_chorale, fit_register, horns,
};
use sonara_midi::performance::{PartId, PerformanceError, Role};
use sonara_midi::theory::parse_chord;

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
    fn brass_and_horns_stay_comfortable_and_clear_the_limits(
        symbols in prop::collection::vec(0_usize..SYMBOLS.len(), 1..8),
    ) {
        // TST-09: any progression keeps every note in its part's comfortable range and every
        // two notes sounding together clear their low limits.
        let progression: Vec<&str> = symbols.iter().map(|&i| SYMBOLS[i]).collect();
        let chords = bars(&progression);
        let brass = romantic_brass();
        for voiced in [
            brass_chorale(&chords, &brass, ChordOptions::default(), &mut ids()),
            horns(&chords, &four_horns(), None, ChordOptions::default(), &mut ids()),
        ] {
            let voiced = voiced.expect("the brass voice any chord");
            for notes in &voiced.parts {
                let carrier = brass
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
