//! Dynamics through the public API: the level table, phrases and their shapes, the velocities a
//! performance writes, the points of the sampler's `dynamics` lane and the controller events of
//! the part's clip that can take the lane's place.
//!
//! The worked numbers come from a string melody, a horn chord that swells and a timpani roll at
//! 72 BPM in 4/4, the tempo of an orchestration of an eight-bar theme in D minor.

use proptest::collection::vec;
use proptest::prelude::*;
use sonara_core::midi::{ControlKind, MidiControlEvent, MidiNote};
use sonara_core::model::automation::{AutomationPoint, CurveKind};
use sonara_core::time::{TempoMap, TimeSignatureMap};
use sonara_midi::orchestration::rules::DynamicBand;
use sonara_midi::orchestration::{Articulation, instrument};
use sonara_midi::performance::check::velocity_band;
use sonara_midi::performance::dynamics::{
    EXPRESSION, LEVELS, Level, MODULATION, Remark, cc_value, level_velocity, marking, part_marking,
    value_at,
};
use sonara_midi::performance::{
    Dynamic, DynamicsKind, DynamicsRequest, HumanizeRequest, LegatoKind, PartId, PartLine,
    PerformanceError, Performed, Role, ScoreContext, Seed, SoundCaps, perform,
};

use Dynamic::{Forte, Fortissimo, MezzoForte, MezzoPiano, Pianissimo, Piano};

/// One engine block at the slowest supported sample rate, in seconds.
const BLOCK_S: f64 = 512.0 / 22_050.0;

/// Two blocks at 72 BPM, in beats: how far a held value leads its note-on.
const LEAD_72: f64 = 2.0 * BLOCK_S * 72.0 / 60.0;

/// One block at 72 BPM, in beats: the closest two controller events along a curve may lie.
const BLOCK_72: f64 = BLOCK_S * 72.0 / 60.0;

fn score_at(bpm: f64, origin: f64) -> ScoreContext {
    ScoreContext::new(
        TempoMap::constant(bpm).expect("a valid tempo"),
        TimeSignatureMap::default(),
        origin,
    )
    .expect("a valid origin")
}

fn score() -> ScoreContext {
    score_at(72.0, 0.0)
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

/// A sound of two velocity layers split between 62 and 63, as VSCO's string sections have.
fn two_layers(articulations: &[Articulation]) -> SoundCaps {
    SoundCaps {
        articulations: articulations.to_vec(),
        layers: vec![62, 127],
        ..SoundCaps::default()
    }
}

fn line(
    instrument_id: &str,
    role: Role,
    notes: Vec<MidiNote>,
    caps: SoundCaps,
    dynamics: DynamicsRequest,
) -> PartLine {
    PartLine {
        part: PartId::new("part_1").expect("a valid part id"),
        instrument: instrument(instrument_id).expect("an instrument type"),
        role,
        notes,
        caps,
        dynamics,
        // The dynamics are checked on the notes as written.
        humanize: HumanizeRequest::EXACT,
    }
}

fn run(line: &PartLine) -> Performed {
    perform(line, &score(), Seed::default()).expect("a valid line")
}

fn request(marking: Dynamic) -> DynamicsRequest {
    DynamicsRequest {
        marking,
        ..DynamicsRequest::default()
    }
}

fn point(beat: f64, value: f64, curve: CurveKind) -> AutomationPoint {
    AutomationPoint { beat, value, curve }
}

/// Asserts that two lanes have the same points, beats and values within 1e-9.
fn assert_lane(actual: &[AutomationPoint], expected: &[AutomationPoint]) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "lane {actual:?} against {expected:?}"
    );
    for (got, want) in actual.iter().zip(expected) {
        assert!(
            (got.beat - want.beat).abs() < 1e-9
                && (got.value - want.value).abs() < 1e-9
                && got.curve == want.curve,
            "point {got:?} against {want:?} in {actual:?}"
        );
    }
}

fn velocities(performed: &Performed) -> Vec<u8> {
    performed.notes.iter().map(|note| note.velocity).collect()
}

fn played(performed: &Performed) -> Vec<u8> {
    performed
        .dynamics
        .notes
        .iter()
        .map(|level| level.velocity)
        .collect()
}

#[test]
fn level_table_round_trips() {
    // Each marking's level velocity reads back as the marking, and the bands meet without a gap.
    for (row, &dynamic) in LEVELS.iter().zip(&Dynamic::ALL) {
        assert_eq!(row.marking, dynamic);
        assert_eq!(level_velocity(dynamic), row.velocity);
        assert_eq!(marking(row.velocity), dynamic);
        assert_eq!(marking(row.lowest), dynamic);
        assert_eq!(marking(row.highest), dynamic);
        assert_eq!(Level::of(dynamic).velocity(), row.velocity);
        assert_eq!(Level::of(dynamic).marking(), dynamic);
        assert!((Level::of(dynamic).cc() - cc_value(dynamic)).abs() < 1e-12);
    }
    let table: Vec<(u8, u8, u8, f64)> = LEVELS
        .iter()
        .map(|row| (row.lowest, row.highest, row.velocity, row.cc))
        .collect();
    assert_eq!(
        table,
        [
            (1, 39, 32, 0.15),
            (40, 54, 47, 0.30),
            (55, 69, 62, 0.45),
            (70, 84, 76, 0.60),
            (85, 104, 95, 0.75),
            (105, 127, 115, 0.90),
        ]
    );
    assert_eq!(marking(0), Pianissimo);
    assert_eq!(marking(127), Fortissimo);
    // The rule checks read a velocity's band through the same table.
    for velocity in 0..=127 {
        assert_eq!(velocity_band(velocity), marking(velocity).band());
    }
    assert_eq!(velocity_band(54), DynamicBand::Soft);
    assert_eq!(velocity_band(55), DynamicBand::Medium);
    assert_eq!(velocity_band(85), DynamicBand::Loud);
}

#[test]
fn levels_between_markings_interpolate_their_velocities() {
    // Half a step below mf is 69, between mp's 62 and mf's 76.
    assert_eq!(Level::from_steps(2.5).velocity(), 69);
    // 54.5 and 85.5 round as Python rounds, to the even neighbour.
    assert_eq!(Level::from_steps(1.5).velocity(), 54);
    assert_eq!(Level::from_steps(3.5).velocity(), 86);
    assert!((Level::from_steps(2.5).cc() - 0.525).abs() < 1e-12);
    // Levels beyond the table read as its ends.
    assert_eq!(Level::from_steps(-1.0), Level::of(Pianissimo));
    assert_eq!(Level::from_steps(9.0), Level::of(Fortissimo));
    assert_eq!(Level::from_steps(f64::NAN), Level::of(Pianissimo));
    assert_eq!(Level::of(Forte).steps(), 4.0);
}

#[test]
fn accompaniment_plays_one_marking_below_the_melody() {
    for role in Role::ALL {
        let below = matches!(role, Role::Harmony | Role::Bass | Role::Pedal);
        let expected = if below { MezzoPiano } else { MezzoForte };
        assert_eq!(part_marking(MezzoForte, role), expected, "{role:?}");
    }
    assert_eq!(part_marking(Fortissimo, Role::Bass), Forte);
    assert_eq!(part_marking(Piano, Role::Pedal), Pianissimo);
    assert_eq!(part_marking(Pianissimo, Role::Harmony), Pianissimo);
}

/// A violin melody in two four-bar phrases, the second peaking on the A5 of bar 7.
fn melody() -> Vec<MidiNote> {
    vec![
        note("n_01", 69, 0.0, 2.0),
        note("n_02", 72, 2.0, 2.0),
        note("n_03", 74, 4.0, 4.0),
        note("n_04", 72, 8.0, 2.0),
        note("n_05", 69, 10.0, 4.0),
        note("n_06", 74, 16.0, 4.0),
        note("n_07", 77, 20.0, 4.0),
        note("n_08", 81, 24.0, 2.0),
        note("n_09", 77, 26.0, 2.0),
        note("n_10", 74, 28.0, 4.0),
    ]
}

#[test]
fn ratio_lane_puts_note_on_at_level_velocity() {
    let line = line(
        "violin",
        Role::Melody,
        melody(),
        two_layers(&[Articulation::Legato]),
        request(MezzoForte),
    );
    let performed = run(&line);
    let dynamics = &performed.dynamics;
    // Two arcs from mf: written at f, the level of their peaks.
    assert_eq!(dynamics.marking, MezzoForte);
    let phrases: Vec<(f64, f64, Option<f64>, Option<u8>)> = dynamics
        .phrases
        .iter()
        .map(|phrase| (phrase.start, phrase.end, phrase.peak, phrase.written))
        .collect();
    assert_eq!(
        phrases,
        [
            (0.0, 14.0, Some(4.0), Some(95)),
            (16.0, 32.0, Some(24.0), Some(95)),
        ]
    );
    assert_eq!(velocities(&performed), [95; 10]);
    // The arcs at each note-on: mf, rising on a raised cosine to f at the peak, then easing to
    // half a step below mf (69) at the phrase's end. Between two markings the velocity is
    // interpolated: 3.5 steps is 85.5, which rounds to 86.
    let at_onsets = [76, 86, 95, 85, 76, 76, 86, 95, 91, 81];
    assert_eq!(played(&performed), at_onsets);
    let level = |velocity: u8| f64::from(velocity) / 95.0;
    let mut expected = vec![point(0.0, level(76), CurveKind::Smooth)];
    for (&onset, &velocity) in [2.0, 4.0, 8.0, 10.0].iter().zip(&at_onsets[1..5]) {
        expected.push(point(onset - LEAD_72, level(velocity), CurveKind::Hold));
        expected.push(point(onset, level(velocity), CurveKind::Smooth));
    }
    expected.push(point(14.0, level(69), CurveKind::Hold));
    for (&onset, &velocity) in [16.0, 20.0, 24.0, 26.0, 28.0].iter().zip(&at_onsets[5..]) {
        expected.push(point(onset - LEAD_72, level(velocity), CurveKind::Hold));
        expected.push(point(onset, level(velocity), CurveKind::Smooth));
    }
    expected.push(point(32.0, level(69), CurveKind::Hold));
    assert_lane(&performed.dynamics_lane(), &expected);
    // The lane runs from 76/95 = 0.80 up to 1.0 and down to 69/95 = 0.73.
    let values: Vec<f64> = dynamics.lane.iter().map(|point| point.value).collect();
    let lowest = values.iter().copied().fold(f64::INFINITY, f64::min);
    let highest = values.iter().copied().fold(0.0, f64::max);
    assert!((lowest - 69.0 / 95.0).abs() < 1e-12 && (highest - 1.0).abs() < 1e-12);
    // Whichever block holds a note-on reads its level: every note-on lands at 76-95, in the
    // loud layer of the two.
    for (note, &velocity) in performed.notes.iter().zip(&at_onsets) {
        for before in [0.0, 0.5, 1.0] {
            let read = value_at(&dynamics.lane, note.start - before * LEAD_72 / 2.0);
            let on = (f64::from(note.velocity) * read).round_ties_even();
            assert!((on - f64::from(velocity)).abs() < 1e-9, "{}", note.id);
        }
        assert_eq!(line.caps.layer(velocity), Some(63..=127));
    }
}

#[test]
fn a_swell_on_a_held_horn_chord_rises_one_marking_and_back() {
    let mut notes: Vec<MidiNote> = (0..8)
        .map(|bar| note(&format!("n_{bar}"), 65, f64::from(bar) * 4.0, 4.0))
        .collect();
    notes[6] = written("n_6", 67, 24.0, 4.0, "crescendo_swell");
    let line = line(
        "horn_f",
        Role::Harmony,
        notes,
        two_layers(&[Articulation::Sustain]),
        request(MezzoForte),
    );
    let performed = run(&line);
    // The horns play one marking below the melody, mp (62), and the swell peaks at mf (76): the
    // phrase is written at 76 and the lane holds 62/76 = 0.82, rising to 1.0 at 60 % of the chord.
    assert_eq!(performed.dynamics.marking, MezzoPiano);
    assert_eq!(velocities(&performed), [76; 8]);
    assert_eq!(played(&performed), [62; 8]);
    let held = 62.0 / 76.0;
    assert_lane(
        &performed.dynamics_lane(),
        &[
            point(0.0, held, CurveKind::Hold),
            point(24.0, held, CurveKind::Smooth),
            point(26.4, 1.0, CurveKind::Smooth),
            point(28.0 - LEAD_72, held, CurveKind::Hold),
        ],
    );
    // +3.5 dB at the peak, the sampler's square law of a held note's gain.
    let peak_db = 40.0 * (1.0 / held).log10();
    assert!((peak_db - 3.537).abs() < 1e-3, "{peak_db}");
    // The same shape on a sound without the parameter drives the track's volume.
    let volume = performed
        .dynamics
        .volume_lane(-6.0)
        .expect("velocity mode has a ratio lane");
    let volumes: Vec<f64> = volume.iter().map(|point| point.value).collect();
    assert!((volumes[0] - (-6.0 - peak_db)).abs() < 1e-9);
    assert!((volumes[2] - -6.0).abs() < 1e-9);
}

#[test]
fn a_recorded_swell_needs_no_lane_swell() {
    let notes = vec![written("n_1", 65, 0.0, 4.0, "crescendo_swell")];
    let caps = two_layers(&[Articulation::Sustain, Articulation::CrescendoSwell]);
    let performed = run(&line(
        "horn_f",
        Role::Harmony,
        notes,
        caps,
        request(MezzoForte),
    ));
    assert_lane(
        &performed.dynamics_lane(),
        &[point(0.0, 1.0, CurveKind::Hold)],
    );
    assert_eq!(velocities(&performed), [62]);
}

#[test]
fn a_request_for_swells_swells_held_accompaniment_notes_only() {
    let notes = vec![
        note("n_1", 60, 0.0, 1.0),
        note("n_2", 60, 1.0, 1.0),
        note("n_3", 62, 2.0, 4.0),
    ];
    let swelling = DynamicsRequest {
        swells: true,
        ..request(MezzoForte)
    };
    let pad = run(&line(
        "violin",
        Role::Pedal,
        notes.clone(),
        two_layers(&[]),
        swelling,
    ));
    // The held note swells from mp (62) to mf (76) at 2 + 0.6 × 4 beats.
    let lane = pad.dynamics_lane();
    assert!(
        lane.iter()
            .any(|point| (point.beat - 4.4).abs() < 1e-9 && (point.value - 1.0).abs() < 1e-12)
    );
    assert_eq!(velocities(&pad), [76, 76, 76]);
    assert_eq!(played(&pad), [62, 62, 62]);
    // A melody does not swell on request: its phrases arc instead.
    let tune = run(&line(
        "violin",
        Role::Melody,
        notes,
        two_layers(&[]),
        swelling,
    ));
    assert!(
        tune.dynamics
            .phrases
            .iter()
            .all(|phrase| phrase.peak.is_some())
    );
    assert!(
        !tune
            .dynamics_lane()
            .iter()
            .any(|point| (point.beat - 4.4).abs() < 1e-9)
    );
}

/// Timpani hits on D2 and A2 in bars 1, 4, 5 and 7, and a roll on D2 through bar 8.
fn timpani() -> Vec<MidiNote> {
    vec![
        note("n_1", 38, 0.0, 1.0),
        note("n_2", 45, 12.0, 1.0),
        note("n_3", 38, 16.0, 1.0),
        note("n_4", 45, 24.0, 1.0),
        written("n_5", 38, 28.0, 4.0, "roll"),
    ]
}

#[test]
fn a_timpani_roll_crescendo_is_back_loaded_and_never_dips_a_ringing_hit() {
    let to_forte = DynamicsRequest {
        roll_to: Some(Forte),
        ..request(MezzoForte)
    };
    let performed = run(&line(
        "timpani",
        Role::Bass,
        timpani(),
        two_layers(&[Articulation::Roll]),
        to_forte,
    ));
    // Every note is written at f (95), the roll's target, and the lane holds 62/95 (mp) through
    // the hits; struck notes ring, so the phrases share one written velocity.
    assert_eq!(velocities(&performed), [95; 5]);
    assert_eq!(played(&performed), [62; 5]);
    assert!(
        performed
            .dynamics
            .phrases
            .iter()
            .all(|phrase| phrase.written == Some(95))
    );
    let held = 62.0 / 95.0;
    // A third of the rise, two thirds of a step above mp (71), at 70 % of the roll.
    assert_lane(
        &performed.dynamics_lane(),
        &[
            point(0.0, held, CurveKind::Hold),
            point(28.0, held, CurveKind::Smooth),
            point(30.8, 71.0 / 95.0, CurveKind::Smooth),
            point(32.0, 1.0, CurveKind::Hold),
        ],
    );
    // +7.4 dB over the roll, within the +12 dB a held note may rise.
    let rise_db = 40.0 * (1.0 / held).log10();
    assert!((rise_db - 7.41).abs() < 5e-3, "{rise_db}");
    assert_eq!(performed.dynamics.remarks, []);
}

#[test]
fn a_roll_crescendo_stops_where_a_held_level_has_doubled() {
    // pp (32) to ff (115) would rise 3.6 times; the roll stops at 64, twice its note-on.
    let to_fortissimo = DynamicsRequest {
        marking: Piano,
        swells: false,
        roll_to: Some(Fortissimo),
    };
    let notes = vec![written("n_1", 38, 0.0, 4.0, "roll")];
    let performed = run(&line(
        "timpani",
        Role::Bass,
        notes,
        two_layers(&[Articulation::Roll]),
        to_fortissimo,
    ));
    let reached = Level::from_steps(2.0 + 2.0 / 14.0);
    assert_eq!(reached.velocity(), 64);
    assert_eq!(
        performed.dynamics.remarks,
        [Remark::BoundedRoll {
            note: "n_1".to_owned(),
            asked: Fortissimo,
            reached,
        }]
    );
    assert_eq!(velocities(&performed), [64]);
    let lane = performed.dynamics_lane();
    let first = lane.first().map(|point| point.value);
    let last = lane.last().map(|point| point.value);
    assert_eq!(first, Some(0.5));
    assert_eq!(last, Some(1.0));
}

#[test]
fn rolls_hold_their_level_without_a_target_and_return_after_a_crescendo() {
    let notes = vec![
        written("n_1", 38, 0.0, 2.0, "roll"),
        note("n_2", 45, 2.0, 1.0),
        note("n_3", 38, 3.0, 1.0),
    ];
    let caps = two_layers(&[Articulation::Roll]);
    let flat = run(&line(
        "timpani",
        Role::Bass,
        notes.clone(),
        caps.clone(),
        request(MezzoForte),
    ));
    assert_lane(&flat.dynamics_lane(), &[point(0.0, 1.0, CurveKind::Hold)]);
    assert_eq!(velocities(&flat), [62, 62, 62]);
    // A crescendo to f, after which the hits play mp again: the lane falls back before them.
    let to_forte = DynamicsRequest {
        roll_to: Some(Forte),
        ..request(MezzoForte)
    };
    let swelling = run(&line("timpani", Role::Bass, notes, caps, to_forte));
    assert_eq!(played(&swelling), [62, 62, 62]);
    let lane = swelling.dynamics_lane();
    assert!((value_at(&lane, 2.0 - LEAD_72) - 62.0 / 95.0).abs() < 1e-12);
    assert!((value_at(&lane, 2.0 - 2.0 * LEAD_72) - 62.0 / 95.0) > 0.1);
}

#[test]
fn a_roll_during_which_another_note_starts_keeps_its_level() {
    // TST-06: a diminuendo roll overlapped by a second roll used to return to the phrase's level
    // under the second one, which then rose more than twice its note-on's level while held.
    let notes = vec![
        written("n_1", 36, 3.25, 4.0, "roll"),
        written("n_2", 36, 7.0, 2.0, "roll"),
        note("n_3", 36, 7.25, 0.25),
    ];
    let to_pianissimo = DynamicsRequest {
        roll_to: Some(Pianissimo),
        ..request(MezzoForte)
    };
    let performed = run(&line(
        "timpani",
        Role::Rhythm,
        notes,
        two_layers(&[Articulation::Roll]),
        to_pianissimo,
    ));
    // The first roll has a note starting during it, so it holds mf; the second, the same.
    assert_lane(
        &performed.dynamics_lane(),
        &[point(3.25, 1.0, CurveKind::Hold)],
    );
    assert_eq!(played(&performed), [76, 76, 76]);
}

#[test]
fn controller_mode_writes_the_tables_values_and_level_velocities() {
    let notes = vec![note("n_1", 60, 0.0, 2.0), note("n_2", 62, 2.0, 2.0)];
    let caps = SoundCaps {
        dynamics: DynamicsKind::Cc,
        ..SoundCaps::default()
    };
    let pad = run(&line(
        "violin",
        Role::Harmony,
        notes.clone(),
        caps.clone(),
        request(MezzoForte),
    ));
    assert_lane(&pad.dynamics_lane(), &[point(0.0, 0.45, CurveKind::Hold)]);
    assert_eq!(velocities(&pad), [62, 62]);
    assert_eq!(pad.dynamics.phrases.first().and_then(|p| p.written), None);
    assert_eq!(pad.dynamics.volume_lane(0.0), None);
    // A melody's arc from mf peaks at f's 0.75.
    let tune = run(&line(
        "violin",
        Role::Melody,
        notes,
        caps,
        request(MezzoForte),
    ));
    let lane = tune.dynamics_lane();
    assert!((lane.first().map_or(0.0, |p| p.value) - 0.60).abs() < 1e-12);
    assert!(lane.iter().any(|p| (p.value - 0.75).abs() < 1e-12));
    assert_eq!(velocities(&tune), [76, 95]);
}

#[test]
fn fixed_mode_carries_the_level_on_the_lane_alone() {
    let notes = vec![note("n_1", 60, 0.0, 2.0)];
    let caps = SoundCaps {
        dynamics: DynamicsKind::Fixed(64),
        ..SoundCaps::default()
    };
    let soft = run(&line(
        "violin",
        Role::Harmony,
        notes.clone(),
        caps.clone(),
        request(MezzoForte),
    ));
    assert_lane(
        &soft.dynamics_lane(),
        &[point(0.0, 62.0 / 64.0, CurveKind::Hold)],
    );
    assert_eq!(velocities(&soft), [62]);
    assert_eq!(played(&soft), [62]);
    assert_eq!(soft.dynamics.remarks, []);
    // Louder than the fixed velocity cannot be played: f plays at 64, and the result says so.
    let loud_part = run(&line(
        "violin",
        Role::Harmony,
        notes,
        caps,
        request(Fortissimo),
    ));
    assert_lane(
        &loud_part.dynamics_lane(),
        &[point(0.0, 1.0, CurveKind::Hold)],
    );
    assert_eq!(velocities(&loud_part), [95]);
    assert_eq!(played(&loud_part), [64]);
    assert_eq!(
        loud_part.dynamics.remarks,
        [Remark::FixedCeiling {
            velocity: 64,
            wanted: 95,
        }]
    );
}

#[test]
fn accents_play_a_marking_louder_and_soft_rewrites_a_marking_softer() {
    // A velocity accent on a long note, which articulation leaves to the velocity.
    let notes = vec![
        loud("n_1", 60, 0.0, 1.0, 80),
        loud("n_2", 60, 1.0, 1.0, 80),
        loud("n_3", 60, 2.0, 1.0, 110),
        loud("n_4", 60, 3.0, 1.0, 80),
    ];
    let accented = run(&line(
        "violin",
        Role::Harmony,
        notes,
        two_layers(&[]),
        request(MezzoForte),
    ));
    assert_eq!(velocities(&accented), [62, 62, 76, 62]);
    assert_eq!(played(&accented), [62, 62, 76, 62]);
    // A marcato the sound lacks, rewritten as an accent, and a mute it lacks, rewritten softer.
    let notes = vec![
        written("n_1", 60, 0.0, 2.0, "marcato"),
        written("n_2", 62, 2.0, 2.0, "con_sordino"),
        note("n_3", 64, 4.0, 2.0),
    ];
    let rewritten = run(&line(
        "trombone_tenor",
        Role::Harmony,
        notes,
        two_layers(&[Articulation::Staccato]),
        request(MezzoForte),
    ));
    assert_eq!(velocities(&rewritten), [76, 47, 62]);
    assert_eq!(
        rewritten
            .dynamics
            .notes
            .iter()
            .map(|level| level.level.marking())
            .collect::<Vec<_>>(),
        [MezzoForte, Piano, MezzoPiano]
    );
}

/// The starts of the phrases of `notes` played as a violin melody within `score`.
fn phrase_starts(notes: Vec<MidiNote>, score: &ScoreContext) -> Vec<f64> {
    let line = line(
        "violin",
        Role::Melody,
        notes,
        two_layers(&[]),
        request(MezzoForte),
    );
    perform(&line, score, Seed::default())
        .expect("a valid line")
        .dynamics
        .phrases
        .iter()
        .map(|phrase| phrase.start)
        .collect()
}

#[test]
fn phrases_end_at_rests_sections_long_notes_and_eight_bars() {
    // Half a beat of rest: 0.75 s at 40 BPM ends the phrase, 0.25 s at 120 BPM does not.
    let breath = vec![note("n_1", 60, 0.0, 1.0), note("n_2", 62, 1.5, 1.0)];
    assert_eq!(
        phrase_starts(breath.clone(), &score_at(40.0, 0.0)),
        [0.0, 1.5]
    );
    assert_eq!(phrase_starts(breath, &score_at(120.0, 0.0)), [0.0]);
    // A whole beat of rest ends it at any tempo.
    let rest = vec![note("n_1", 60, 0.0, 1.0), note("n_2", 62, 2.0, 1.0)];
    assert_eq!(phrase_starts(rest, &score_at(240.0, 0.0)), [0.0, 2.0]);
    // A section starting at timeline beat 6, where the clip's beat 4 lies.
    let run_of = |count: u32| -> Vec<MidiNote> {
        (0..count)
            .map(|index| note(&format!("n_{index:02}"), 60, f64::from(index), 1.0))
            .collect()
    };
    let sectioned = score_at(72.0, 2.0)
        .with_sections([6.0, 0.0])
        .expect("valid section starts");
    assert_eq!(sectioned.sections(), [0.0, 6.0]);
    assert_eq!(phrase_starts(run_of(8), &sectioned), [0.0, 4.0]);
    // A note twice the median length followed by a short breath ends its phrase.
    let mut long = run_of(4);
    long.push(note("n_04", 62, 4.0, 4.0));
    long.push(note("n_05", 64, 8.25, 1.0));
    long.push(note("n_06", 65, 9.25, 1.0));
    assert_eq!(phrase_starts(long.clone(), &score()), [0.0, 8.25]);
    // Without the breath it does not.
    if let Some(next) = long.get_mut(5) {
        next.start = 8.0;
    }
    if let Some(last) = long.get_mut(6) {
        last.start = 9.0;
    }
    assert_eq!(phrase_starts(long, &score()), [0.0]);
    // Twelve bars without a rest: eight, then four.
    let held: Vec<MidiNote> = (0_u8..12)
        .map(|bar| {
            let pitch = 60 + i64::from(bar);
            note(&format!("n_{bar:02}"), pitch, f64::from(bar) * 4.0, 4.0)
        })
        .collect();
    assert_eq!(phrase_starts(held, &score()), [0.0, 32.0]);
}

#[test]
fn section_starts_must_lie_on_the_timeline() {
    for start in [f64::NAN, f64::INFINITY, -1.0] {
        let error = score()
            .with_sections([4.0, start])
            .expect_err("an invalid start");
        let PerformanceError::InvalidSection(value) = error else {
            panic!("{error:?} is not about the section start");
        };
        assert_eq!(value.to_bits(), start.to_bits());
        assert_eq!(
            error.to_string(),
            format!("a section must start at a finite beat >= 0, got {start}")
        );
    }
}

#[test]
fn the_lane_lies_on_the_timeline_after_the_clip_origin() {
    let notes = vec![note("n_1", 60, 0.0, 1.0), note("n_2", 64, 2.0, 1.0)];
    let line = line(
        "violin",
        Role::Harmony,
        notes,
        two_layers(&[]),
        request(MezzoForte),
    );
    let performed = perform(&line, &score_at(72.0, 8.0), Seed::default()).expect("a valid line");
    assert_lane(
        &performed.dynamics_lane(),
        &[point(8.0, 1.0, CurveKind::Hold)],
    );
}

/// A sound a property line may have: velocity layers, a fixed velocity or a controller.
fn kind_strategy() -> impl Strategy<Value = DynamicsKind> {
    prop_oneof![
        Just(DynamicsKind::Velocity),
        (1_u8..=127).prop_map(DynamicsKind::Fixed),
        Just(DynamicsKind::Cc),
    ]
}

fn marking_strategy() -> impl Strategy<Value = Dynamic> {
    (0_usize..6).prop_map(|index| Dynamic::ALL[index])
}

/// Notes with distinct ids on a quarter-beat grid, some rolls, some swells.
fn notes_strategy() -> impl Strategy<Value = Vec<MidiNote>> {
    vec(
        (36_i64..90, 0_u32..64, 1_u32..24, 1_i64..128, 0_u8..8),
        0..20,
    )
    .prop_map(|raw| {
        raw.into_iter()
            .enumerate()
            .map(|(index, (pitch, start, length, velocity, kind))| {
                let note = MidiNote::new(
                    format!("n_{index:03}"),
                    pitch,
                    f64::from(start) / 4.0,
                    f64::from(length) / 4.0,
                    velocity,
                    0,
                )
                .expect("a valid note");
                let articulation = match kind {
                    0 => Some("roll"),
                    1 => Some("crescendo_swell"),
                    _ => None,
                };
                note.with_articulation(articulation.map(str::to_owned))
                    .expect("a vocabulary id")
            })
            .collect()
    })
}

proptest! {
    /// The dynamics keep their contract with the sampler (TST-07): lane values within the
    /// parameter's range, points in time order on the timeline, velocities within 1-127, every
    /// note-on reading the level it was given in whichever block holds it, and no held note rising
    /// more than twice its note-on's level.
    #[test]
    fn dynamics_lane_keeps_note_on_in_intended_layer(
        notes in notes_strategy(),
        instrument_id in prop_oneof![Just("violin"), Just("timpani"), Just("horn_f"), Just("harp")],
        role_index in 0_usize..7,
        kind in kind_strategy(),
        marking in marking_strategy(),
        roll_to in proptest::option::of(marking_strategy()),
        swells in any::<bool>(),
        origin in 0_u32..8,
    ) {
        let caps = SoundCaps {
            articulations: vec![Articulation::Roll, Articulation::Sustain],
            legato: LegatoKind::None,
            layers: vec![62, 127],
            dynamics: kind,
            ..SoundCaps::default()
        };
        let request = DynamicsRequest { marking, swells, roll_to };
        let line = line(instrument_id, Role::ALL[role_index], notes, caps.clone(), request);
        let origin = f64::from(origin);
        let performed = perform(&line, &score_at(72.0, origin), Seed::default())
            .expect("a valid line");
        let dynamics = &performed.dynamics;
        let lane = &dynamics.lane;
        prop_assert_eq!(lane.is_empty(), performed.notes.is_empty());
        prop_assert_eq!(dynamics.notes.len(), performed.notes.len());
        prop_assert!(lane.windows(2).all(|pair| pair[0].beat <= pair[1].beat), "{:?}", lane);
        prop_assert!(lane.iter().all(|point| point.beat >= origin));
        for point in lane {
            let within = match kind {
                DynamicsKind::Cc => (0.15 - 1e-12..=0.90 + 1e-12).contains(&point.value),
                _ => point.value > 0.0 && point.value <= 1.0 + 1e-12,
            };
            prop_assert!(within, "{:?}", point);
        }
        for (note, level) in performed.notes.iter().zip(&dynamics.notes) {
            prop_assert_eq!(&note.id, &level.note);
            prop_assert!((1..=127).contains(&note.velocity));
            let on = origin + note.start;
            let read = |beat: f64| value_at(lane, beat);
            match kind {
                DynamicsKind::Velocity | DynamicsKind::Fixed(_) => {
                    let base = match kind {
                        DynamicsKind::Fixed(fixed) => f64::from(fixed),
                        _ => f64::from(note.velocity),
                    };
                    // Whichever block holds the note-on reads the velocity it was given.
                    for before in [0.0, 0.5, 1.0] {
                        let at = (base * read(on - before * LEAD_72 / 2.0)).round_ties_even();
                        prop_assert!(
                            (at.max(1.0) - f64::from(level.velocity)).abs() < 1e-9,
                            "{} reads {} for {}", note.id, at, level.velocity
                        );
                    }
                    // A note written at its phrase's velocity lands in its level's layer.
                    let unaccented = dynamics.phrases.iter().any(|phrase| {
                        phrase.written == Some(note.velocity)
                            && note.start >= phrase.start - 1e-9
                            && note.start <= phrase.end + 1e-9
                    });
                    if unaccented {
                        prop_assert_eq!(
                            caps.layer(level.velocity),
                            caps.layer(level.level.velocity())
                        );
                    }
                    // A held note's level never more than doubles.
                    let held_from = read(on);
                    let end = on + note.length;
                    let highest = lane
                        .iter()
                        .filter(|point| point.beat > on && point.beat < end)
                        .map(|point| point.value)
                        .chain([read(end - 1e-6)])
                        .fold(held_from, f64::max);
                    prop_assert!(
                        highest <= 2.0 * held_from + 1e-9,
                        "{} rises from {} to {}", note.id, held_from, highest
                    );
                    if let DynamicsKind::Fixed(fixed) = kind {
                        prop_assert!(level.velocity <= fixed);
                    }
                }
                DynamicsKind::Cc => prop_assert_eq!(level.velocity, note.velocity),
            }
        }
    }
}

/// The controller value in force at content beat `beat`: the value of the last event at or
/// before it, or `None` before the first.
fn in_force(events: &[MidiControlEvent], beat: f64) -> Option<i64> {
    events
        .iter()
        .take_while(|event| event.beat <= beat + 1e-9)
        .last()
        .map(|event| event.value)
}

/// A controller value as a number.
fn number_of(value: i64) -> f64 {
    f64::from(u8::try_from(value).expect("a controller value of 0-127"))
}

/// The controller value that gives the `dynamics` value `d` its level, before rounding: the gain
/// `d²` of the sampler's square law on expression, `d` itself on a crossfade's controller.
fn exact_value(kind: DynamicsKind, d: f64) -> f64 {
    match kind {
        DynamicsKind::Cc => 127.0 * d,
        DynamicsKind::Velocity | DynamicsKind::Fixed(_) => 127.0 * d * d,
    }
}

/// The level a note-on plays at under expression `value`, as a velocity: the sampler's level
/// follows the square of the velocity, and expression is a linear gain.
fn heard(velocity: u8, value: i64) -> f64 {
    f64::from(velocity) * (number_of(value) / 127.0).sqrt()
}

#[test]
fn each_sound_carries_its_dynamics_on_one_controller() {
    assert_eq!((EXPRESSION, MODULATION), (11, 1));
    assert_eq!(DynamicsKind::Velocity.controller(), EXPRESSION);
    assert_eq!(DynamicsKind::Fixed(100).controller(), EXPRESSION);
    assert_eq!(DynamicsKind::Cc.controller(), MODULATION);
}

#[test]
fn expression_swells_a_held_chord_at_the_level_the_lane_gives_it() {
    let mut notes: Vec<MidiNote> = (0..8)
        .map(|bar| note(&format!("n_{bar}"), 65, f64::from(bar) * 4.0, 4.0))
        .collect();
    notes[6] = written("n_6", 67, 24.0, 4.0, "crescendo_swell");
    let performed = run(&line(
        "horn_f",
        Role::Harmony,
        notes,
        two_layers(&[Articulation::Sustain]),
        request(MezzoForte),
    ));
    let controller = &performed.dynamics.controller;
    assert_eq!((controller.number, controller.channel), (EXPRESSION, 0));
    let events = performed.dynamics_events();
    assert_eq!(events, controller.events);
    // The chords are written at mf (76) and hold mp (62): expression 127 · (62/76)² = 84.5,
    // which rounds to 85, from the clip's start until the swell.
    let first = events.first().expect("a value from the clip's start");
    assert_eq!((first.beat, first.value), (0.0, 85));
    assert!(events.get(1).is_some_and(|event| event.beat > 24.0));
    // The swell reaches mf, expression 127, at 60 % of the chord, and falls back to 85 before
    // the next chord, one value a step. The curve flattens towards its peak, so 127 is in force
    // from where the lane rounds to it, before the peak's own point.
    assert_eq!(in_force(&events, 26.4), Some(127));
    let peak = events
        .iter()
        .position(|event| event.value == 127)
        .expect("the swell's peak");
    let lane = &performed.dynamics.lane;
    let reached = events[peak].beat;
    assert!(reached < 26.4 && 127.0 * value_at(lane, reached).powi(2) >= 126.5);
    assert!(127.0 * value_at(lane, reached - BLOCK_72).powi(2) < 126.5);
    assert!(
        events[..=peak]
            .windows(2)
            .all(|pair| pair[0].value < pair[1].value)
    );
    assert!(
        events[peak..]
            .windows(2)
            .all(|pair| pair[0].value > pair[1].value)
    );
    let last = events.last().expect("the swell's end");
    assert_eq!(last.value, 85);
    assert!(last.beat > 26.4 && last.beat <= 28.0 - LEAD_72 + 1e-9);
    // Along the curves at most one event a block.
    assert!(
        events
            .windows(2)
            .all(|pair| pair[1].beat - pair[0].beat >= BLOCK_72 - 1e-9)
    );
    // Every chord is heard at mp: 76 at the gain 85/127 is 62.2.
    for note in &performed.notes {
        let value = in_force(&events, note.start).expect("a value at every note-on");
        assert!(
            (heard(note.velocity, value) - 62.0).abs() < 0.5,
            "{} at {value}",
            note.id
        );
    }
    // Each event is a control change the clip accepts as it stands.
    for event in &events {
        let valid = MidiControlEvent::new(event.beat, ControlKind::Cc, event.value, 11, 0)
            .expect("a valid controller event");
        assert_eq!(&valid, event);
    }
}

#[test]
fn expression_puts_each_note_on_of_an_arc_on_its_level() {
    let line = line(
        "violin",
        Role::Melody,
        melody(),
        two_layers(&[Articulation::Legato]),
        request(MezzoForte),
    );
    let performed = run(&line);
    let events = performed.dynamics_events();
    // Written at f (95), the note-ons play 76, 86, 95, 85, 76, 76, 86, 95, 91 and 81: expression
    // 127 · (level / 95)², rounded.
    let at_onsets: Vec<i64> = performed
        .notes
        .iter()
        .map(|note| in_force(&events, note.start).expect("a value at every note-on"))
        .collect();
    assert_eq!(at_onsets, [81, 104, 127, 102, 81, 81, 104, 127, 117, 92]);
    for ((note, level), &value) in performed
        .notes
        .iter()
        .zip(&performed.dynamics.notes)
        .zip(&at_onsets)
    {
        let gap = heard(note.velocity, value) - f64::from(level.velocity);
        assert!(gap.abs() < 0.5, "{} heard {gap} off its level", note.id);
    }
    // The first phrase ends half a step below mf (69), 127 · (69/95)² = 67.0, held through the
    // rest before the second.
    assert_eq!(in_force(&events, 15.0), Some(67));
    // Each event is the lane's value at its beat.
    for event in &events {
        let d = value_at(&performed.dynamics.lane, event.beat);
        assert_eq!(number_of(event.value), (127.0 * d * d).round(), "{event:?}");
    }
}

#[test]
fn humanized_note_ons_find_their_levels_in_force() {
    let mut line = line(
        "violin",
        Role::Melody,
        melody(),
        two_layers(&[Articulation::Legato]),
        request(MezzoForte),
    );
    line.humanize = HumanizeRequest::default();
    let performed = run(&line);
    let moved = performed
        .notes
        .iter()
        .zip(melody())
        .filter(|(played, written)| (played.start - written.start).abs() > 1e-6)
        .count();
    assert!(moved > 0, "humanization moves the note-ons");
    let events = performed.dynamics_events();
    let at_onsets: Vec<Option<i64>> = performed
        .notes
        .iter()
        .map(|note| in_force(&events, note.start))
        .collect();
    let expected = [81, 104, 127, 102, 81, 81, 104, 127, 117, 92].map(Some);
    assert_eq!(at_onsets, expected);
}

#[test]
fn modulation_carries_the_crossfade_values_of_the_level_table() {
    let notes = vec![note("n_1", 60, 0.0, 2.0), note("n_2", 62, 2.0, 2.0)];
    let caps = SoundCaps {
        dynamics: DynamicsKind::Cc,
        ..SoundCaps::default()
    };
    // A harmony part holds mp, the value 0.45: modulation 57.15, which rounds to 57.
    let pad = run(&line(
        "violin",
        Role::Harmony,
        notes.clone(),
        caps.clone(),
        request(MezzoForte),
    ));
    let controller = &pad.dynamics.controller;
    assert_eq!((controller.number, controller.channel), (MODULATION, 0));
    let held: Vec<(f64, i64)> = controller
        .events
        .iter()
        .map(|event| (event.beat, event.value))
        .collect();
    assert_eq!(held, [(0.0, 57)]);
    // A melody's arc: mf's 0.60 (76) at the first note-on, f's 0.75 (95) at the peak, and half a
    // step below mf, 0.525 (66.7, so 67), at the end; the notes keep their levels' velocities.
    let tune = run(&line(
        "violin",
        Role::Melody,
        notes,
        caps,
        request(MezzoForte),
    ));
    let events = tune.dynamics_events();
    assert_eq!(in_force(&events, 0.0), Some(76));
    assert_eq!(in_force(&events, 2.0), Some(95));
    let last = events.last().expect("the arc's end");
    assert!(last.value == 67 && last.beat <= 4.0 + 1e-9, "{last:?}");
    assert_eq!(velocities(&tune), [76, 95]);
    let peak = events
        .iter()
        .position(|event| event.value == 95)
        .expect("the peak");
    assert!(
        events[..=peak]
            .windows(2)
            .all(|pair| pair[0].value < pair[1].value)
    );
    assert!(
        events[peak..]
            .windows(2)
            .all(|pair| pair[0].value > pair[1].value)
    );
    // Each event is the value the `dynamics` parameter writes for the lane at its beat.
    for event in &events {
        let d = value_at(&tune.dynamics.lane, event.beat);
        assert_eq!(number_of(event.value), (127.0 * d).round(), "{event:?}");
    }
}

#[test]
fn a_fixed_velocity_takes_expression_at_the_lanes_gain() {
    // The sound plays 64; mp (62) is the lane's 62/64, the gain (62/64)², expression 119.2.
    let caps = SoundCaps {
        dynamics: DynamicsKind::Fixed(64),
        ..SoundCaps::default()
    };
    let performed = run(&line(
        "violin",
        Role::Harmony,
        vec![note("n_1", 60, 0.0, 2.0)],
        caps,
        request(MezzoForte),
    ));
    let controller = &performed.dynamics.controller;
    assert_eq!(controller.number, EXPRESSION);
    let events: Vec<(f64, i64)> = controller
        .events
        .iter()
        .map(|event| (event.beat, event.value))
        .collect();
    assert_eq!(events, [(0.0, 119)]);
    assert!((heard(64, 119) - 62.0).abs() < 0.5);
}

#[test]
fn controller_events_lie_in_the_clip_on_the_parts_channel() {
    let on_channel = |id: &str, pitch: i64, start: f64| {
        MidiNote::new(id, pitch, start, 1.0, 80, 5).expect("a valid note")
    };
    let line = line(
        "violin",
        Role::Harmony,
        vec![on_channel("n_1", 60, 0.0), on_channel("n_2", 64, 2.0)],
        two_layers(&[]),
        request(MezzoForte),
    );
    // The clip's content starts at timeline beat 8: the lane lies on the timeline, the events in
    // the clip, at the beats of its notes.
    let performed = perform(&line, &score_at(72.0, 8.0), Seed::default()).expect("a valid line");
    assert_lane(
        &performed.dynamics_lane(),
        &[point(8.0, 1.0, CurveKind::Hold)],
    );
    let expected = MidiControlEvent {
        beat: 0.0,
        kind: ControlKind::Cc,
        value: 127,
        number: 11,
        channel: 5,
    };
    let controller = &performed.dynamics.controller;
    assert_eq!(controller.events, std::slice::from_ref(&expected));
    assert_eq!(controller.channel, 5);
    // A preset that crossfades on breath (2) takes the same values there.
    let breath = controller.on_controller(2).expect("a MIDI controller");
    assert_eq!(breath.number, 2);
    assert_eq!(breath.channel, 5);
    assert_eq!(
        breath.events,
        [MidiControlEvent {
            number: 2,
            ..expected
        }]
    );
    assert_eq!(
        controller.on_controller(127).map(|lane| lane.number),
        Some(127)
    );
    assert_eq!(controller.on_controller(128), None);
}

#[test]
fn a_part_without_notes_writes_no_controller_events() {
    for kind in [
        DynamicsKind::Velocity,
        DynamicsKind::Fixed(100),
        DynamicsKind::Cc,
    ] {
        let caps = SoundCaps {
            dynamics: kind,
            ..SoundCaps::default()
        };
        let performed = run(&line(
            "violin",
            Role::Melody,
            Vec::new(),
            caps,
            request(MezzoForte),
        ));
        let controller = &performed.dynamics.controller;
        assert_eq!(controller.number, kind.controller());
        assert_eq!(controller.channel, 0);
        assert_eq!(controller.events, []);
        assert_eq!(performed.dynamics_events(), []);
    }
}

proptest! {
    /// The controller events keep their contract (TST-07): valid clip events in strictly
    /// ascending order on the part's channel with no value repeated, each the lane's value at its
    /// beat, at most one a block along a curve, the lane's value in force at every note-on as
    /// played, and in between never further from the lane than rounding and the lane's own
    /// movement over the two blocks before.
    #[test]
    fn controller_events_follow_the_lane(
        notes in notes_strategy(),
        instrument_id in prop_oneof![Just("violin"), Just("timpani"), Just("horn_f"), Just("harp")],
        role_index in 0_usize..7,
        kind in kind_strategy(),
        marking in marking_strategy(),
        roll_to in proptest::option::of(marking_strategy()),
        swells in any::<bool>(),
        origin in 0_u32..8,
        humanized in any::<bool>(),
    ) {
        let caps = SoundCaps {
            articulations: vec![Articulation::Roll, Articulation::Sustain],
            legato: LegatoKind::None,
            layers: vec![62, 127],
            dynamics: kind,
            ..SoundCaps::default()
        };
        let request = DynamicsRequest { marking, swells, roll_to };
        let mut line = line(instrument_id, Role::ALL[role_index], notes, caps, request);
        if humanized {
            line.humanize = HumanizeRequest::default();
        }
        let origin = f64::from(origin);
        let performed = perform(&line, &score_at(72.0, origin), Seed::default())
            .expect("a valid line");
        let lane = &performed.dynamics.lane;
        let controller = &performed.dynamics.controller;
        let events = &controller.events;
        prop_assert_eq!(events.is_empty(), performed.notes.is_empty());
        prop_assert_eq!(controller.number, kind.controller());
        prop_assert_eq!(controller.channel, 0);
        // The lane's value at a content beat, as a controller value before rounding.
        let exact = |beat: f64| exact_value(kind, value_at(lane, origin + beat));
        // Where the lane has a point, and where the first value starts to hold.
        let breakpoints: Vec<f64> = lane
            .iter()
            .map(|point| point.beat - origin)
            .chain(events.first().map(|event| event.beat))
            .collect();
        let on_breakpoint = |beat: f64| breakpoints.iter().any(|&at| (at - beat).abs() < 1e-6);
        for event in events {
            let valid = MidiControlEvent::new(
                event.beat,
                ControlKind::Cc,
                event.value,
                i64::from(controller.number),
                i64::from(controller.channel),
            )
            .expect("a valid controller event");
            prop_assert_eq!(&valid, event);
            prop_assert!(
                (number_of(event.value) - exact(event.beat)).abs() <= 0.5 + 1e-6,
                "{:?} against {}", event, exact(event.beat)
            );
        }
        for pair in events.windows(2) {
            prop_assert!(pair[1].beat > pair[0].beat, "{:?}", pair);
            prop_assert_ne!(pair[0].value, pair[1].value);
            if !(on_breakpoint(pair[0].beat) && on_breakpoint(pair[1].beat)) {
                prop_assert!(pair[1].beat - pair[0].beat >= BLOCK_72 - 1e-6, "{:?}", pair);
            }
        }
        for note in &performed.notes {
            let value = in_force(events, note.start);
            prop_assert!(value.is_some(), "{} starts before the first event", note.id);
            let value = value.map_or(0.0, number_of);
            prop_assert!(
                (value - exact(note.start)).abs() <= 0.5 + 1e-6,
                "{} reads {} for {}", note.id, value, exact(note.start)
            );
        }
        if let (Some(first), Some(last)) = (events.first(), lane.last()) {
            let end = last.beat - origin + 1.0;
            let mut beat = first.beat;
            while beat <= end {
                let held = in_force(events, beat).map_or(0.0, number_of);
                let target = exact(beat);
                let from = (beat - 2.0 * BLOCK_72 - 1e-6).max(first.beat);
                // The lane runs monotonically between its points, so its movement over the
                // window shows at the window's ends and on both sides of its points.
                let drift = breakpoints
                    .iter()
                    .filter(|&&at| at > from && at <= beat)
                    .flat_map(|&at| [at, at - 1e-9])
                    .chain([from, beat])
                    .map(|at| (exact(at) - target).abs())
                    .fold(0.0, f64::max);
                prop_assert!(
                    (held - target).abs() <= 0.5 + drift + 1e-6,
                    "{} at beat {} against {} (drift {})", held, beat, target, drift
                );
                beat += BLOCK_72 / 2.0;
            }
        }
    }
}
