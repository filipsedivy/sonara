//! Humanization through the public API: the deviations of each family, drawn from the part's own
//! stream, the attack compensation of the sound, and the limits both keep, with the lanes that
//! follow the note-ons as played.

use proptest::collection::vec;
use proptest::prelude::*;
use sonara_core::midi::MidiNote;
use sonara_core::time::{TempoMap, TimeSignatureMap};
use sonara_midi::orchestration::{Articulation, instrument};
use sonara_midi::performance::dynamics::value_at;
use sonara_midi::performance::humanize::{
    BRASS_PROFILE, KEYBOARD_PROFILE, PERCUSSION_PROFILE, Profile, SECTION_PROFILE, SOLO_PROFILE,
    TIMING_BOUND, profile,
};
use sonara_midi::performance::{
    Compensation, Dynamic, DynamicsKind, DynamicsRequest, HumanizeRequest, LegatoKind, NoteTiming,
    PartId, PartLine, Performed, Role, ScoreContext, Seed, SoundCaps, articulate,
    articulation_choice, perform,
};

use Articulation::{Legato, Spiccato, Staccato};

/// One engine block at the slowest supported sample rate, in seconds.
const BLOCK_S: f64 = 512.0 / 22_050.0;

/// Deviations without attack compensation.
const DEVIATIONS: HumanizeRequest = HumanizeRequest {
    deviations: true,
    attack_compensation: false,
    solo: false,
};

/// Attack compensation without deviations.
const COMPENSATION: HumanizeRequest = HumanizeRequest {
    deviations: false,
    attack_compensation: true,
    solo: false,
};

fn part(slug: &str) -> PartId {
    PartId::new(slug).expect("a valid part id")
}

/// A score at `bpm` whose clip starts at timeline beat `origin`.
fn score_at(bpm: f64, origin: f64) -> ScoreContext {
    ScoreContext::new(
        TempoMap::constant(bpm).expect("a valid tempo"),
        TimeSignatureMap::default(),
        origin,
    )
    .expect("a valid origin")
}

/// A score at 60 BPM, where a beat lasts a second.
fn second_beats() -> ScoreContext {
    score_at(60.0, 0.0)
}

fn note(id: &str, pitch: i64, start: f64, length: f64) -> MidiNote {
    MidiNote::new(id, pitch, start, length, 80, 0).expect("a valid note")
}

fn written(id: &str, pitch: i64, start: f64, length: f64, articulation: &str) -> MidiNote {
    note(id, pitch, start, length)
        .with_articulation(Some(articulation.to_owned()))
        .expect("a vocabulary id")
}

fn line(
    slug: &str,
    instrument_id: &str,
    notes: Vec<MidiNote>,
    caps: SoundCaps,
    humanize: HumanizeRequest,
) -> PartLine {
    PartLine {
        part: part(slug),
        instrument: instrument(instrument_id).expect("an instrument type"),
        role: Role::Melody,
        notes,
        caps,
        dynamics: DynamicsRequest::default(),
        humanize,
    }
}

fn run(line: &PartLine, score: &ScoreContext, seed: i64) -> Performed {
    perform(line, score, Seed::new(seed)).expect("a valid line")
}

fn by_id<'a>(notes: &'a [MidiNote], id: &str) -> &'a MidiNote {
    notes
        .iter()
        .find(|note| note.id == id)
        .unwrap_or_else(|| panic!("{id} is missing"))
}

fn timing<'a>(performed: &'a Performed, id: &str) -> &'a NoteTiming {
    performed
        .humanized
        .notes
        .iter()
        .find(|timing| timing.note == id)
        .unwrap_or_else(|| panic!("{id} has no timing"))
}

fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "{what}: {actual} is not {expected}"
    );
}

/// A velocity as the sampler plays it: rounded half to even, within 1-127.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is rounded and clamped to 1-127 first"
)]
fn sounding(velocity: f64) -> u8 {
    velocity.round_ties_even().clamp(1.0, 127.0) as u8
}

#[test]
fn humanization_is_on_by_default() {
    assert_eq!(
        HumanizeRequest::default(),
        HumanizeRequest {
            deviations: true,
            attack_compensation: true,
            solo: false,
        }
    );
    assert_eq!(
        HumanizeRequest::EXACT,
        HumanizeRequest {
            deviations: false,
            attack_compensation: false,
            solo: false,
        }
    );
}

#[test]
fn each_family_has_its_deviations() {
    let expected = [
        (SECTION_PROFILE, (8.0, 4, 0.05, 2.0, false)),
        (SOLO_PROFILE, (6.0, 5, 0.04, 2.0, false)),
        (BRASS_PROFILE, (8.0, 5, 0.04, 2.0, false)),
        (PERCUSSION_PROFILE, (3.0, 6, 0.0, 0.0, false)),
        (KEYBOARD_PROFILE, (4.0, 6, 0.03, 2.0, true)),
    ];
    for (profile, (timing_ms, velocity, length, spread_ms, voiced_chords)) in expected {
        assert_eq!(
            profile,
            Profile {
                timing_ms,
                velocity,
                length,
                spread_ms,
                voiced_chords,
            }
        );
    }
    let of = |id: &str, request: HumanizeRequest| {
        profile(instrument(id).expect("an instrument type"), request)
    };
    let solo = HumanizeRequest {
        solo: true,
        ..HumanizeRequest::default()
    };
    let section = HumanizeRequest::default();
    // A string instrument plays as a section unless the request says one player plays it.
    for id in ["violin", "viola", "cello", "double_bass"] {
        assert_eq!(of(id, section), SECTION_PROFILE, "{id}");
        assert_eq!(of(id, solo), SOLO_PROFILE, "{id}");
    }
    // A choir is always a section and a solo voice always one singer.
    assert_eq!(of("choir_soprano", solo), SECTION_PROFILE);
    assert_eq!(of("voice_tenor", section), SOLO_PROFILE);
    for (id, expected) in [
        ("flute", SOLO_PROFILE),
        ("bassoon", SOLO_PROFILE),
        ("horn_f", BRASS_PROFILE),
        ("tuba", BRASS_PROFILE),
        ("timpani", PERCUSSION_PROFILE),
        ("carillon", PERCUSSION_PROFILE),
        ("piano_concert_grand", KEYBOARD_PROFILE),
        ("organ_pipe", KEYBOARD_PROFILE),
        ("harp", KEYBOARD_PROFILE),
        ("guitar_classical", KEYBOARD_PROFILE),
    ] {
        assert_eq!(of(id, section), expected, "{id}");
        assert_eq!(of(id, solo), expected, "{id}");
    }
}

#[test]
fn an_exact_performance_plays_the_notes_as_written() {
    let notes = vec![
        note("n_1", 60, 0.0, 1.0),
        note("n_2", 64, 1.0, 1.0),
        note("n_3", 67, 2.0, 2.0),
    ];
    let caps = SoundCaps {
        attack_ms: vec![(None, 40.0)],
        ..SoundCaps::default()
    };
    let exact = line("violins_1", "violin", notes, caps, HumanizeRequest::EXACT);
    let performed = run(&exact, &second_beats(), 9);
    let articulated = articulate(&exact, &second_beats()).expect("a valid line");
    let timeless = |notes: &[MidiNote]| -> Vec<(String, f64, f64)> {
        notes
            .iter()
            .map(|note| (note.id.clone(), note.start, note.length))
            .collect()
    };
    assert_eq!(timeless(&performed.notes), timeless(&articulated.notes));
    assert_eq!(performed.passages, articulated.passages);
    assert_eq!(performed.humanized.request, HumanizeRequest::EXACT);
    for timing in &performed.humanized.notes {
        assert_eq!(timing.compensation, Compensation::Off);
        assert_eq!(
            (timing.offset_ms, timing.velocity, timing.length),
            (0.0, 0, 0.0)
        );
    }
    assert_eq!(performed.humanized.uncompensated().count(), 0);
}

/// The deviations are the part stream's draws, in the documented order (TST-10): per onset its
/// bounded timing offset, then per note its velocity step and its length share.
#[test]
fn deviations_replay_the_parts_own_stream() {
    let notes = vec![
        note("n_1", 69, 1.0, 1.0),
        note("n_2", 72, 3.0, 1.0),
        note("n_3", 74, 5.0, 1.0),
    ];
    let violins = line(
        "violins_1",
        "violin",
        notes.clone(),
        SoundCaps::default(),
        DEVIATIONS,
    );
    let score = score_at(72.0, 0.0);
    let performed = run(&violins, &score, 5);
    let exact = run(
        &PartLine {
            humanize: HumanizeRequest::EXACT,
            ..violins.clone()
        },
        &score,
        5,
    );
    let mut stream = Seed::new(5).stream(&part("violins_1"));
    let bound = TIMING_BOUND * 0.008;
    let tempo = score.tempo();
    for written_note in &notes {
        let offset = stream.gauss(0.0, 0.008).clamp(-bound, bound);
        let step = stream.randint(-4, 4);
        let share = stream.uniform(-0.05, 0.05);
        let timing = timing(&performed, &written_note.id);
        assert_close(timing.offset_ms, 1000.0 * offset, "offset");
        assert_eq!(i64::from(timing.velocity), step);
        assert_close(timing.length, share, "length share");
        assert_eq!(timing.compensation, Compensation::Off);

        let played = by_id(&performed.notes, &written_note.id);
        let start_s = tempo.beat_to_seconds(written_note.start);
        let end_s = tempo.beat_to_seconds(written_note.end());
        assert_close(
            played.start,
            tempo.seconds_to_beat(start_s + offset),
            "start",
        );
        assert_close(
            played.end(),
            tempo.seconds_to_beat(end_s + offset + share * (end_s - start_s)),
            "end",
        );
        let written_velocity = by_id(&exact.notes, &written_note.id).velocity;
        assert_eq!(
            i64::from(played.velocity),
            i64::from(written_velocity) + step
        );
    }
}

#[test]
fn parts_and_seeds_draw_from_their_own_streams() {
    let notes: Vec<MidiNote> = (0_u8..6)
        .map(|index| {
            note(
                &format!("n_{index}"),
                60 + i64::from(index),
                f64::from(index),
                0.5,
            )
        })
        .collect();
    let offsets = |slug: &str, seed: i64| -> Vec<f64> {
        run(
            &line(
                slug,
                "violin",
                notes.clone(),
                SoundCaps::default(),
                DEVIATIONS,
            ),
            &second_beats(),
            seed,
        )
        .humanized
        .notes
        .iter()
        .map(|timing| timing.offset_ms)
        .collect()
    };
    let first = offsets("violins_1", 2);
    assert_eq!(offsets("violins_1", 2), first);
    assert_ne!(offsets("violins_2", 2), first);
    assert_ne!(offsets("violins_1", 3), first);
    // Every offset lies within the section's bound.
    assert!(first.iter().all(|offset| offset.abs() <= 16.0 + 1e-9));
}

#[test]
fn a_chord_shares_its_offset_and_a_keyboard_chord_keeps_its_voicing() {
    let chord = |prefix: &str| -> Vec<MidiNote> {
        [48, 55, 64, 72]
            .into_iter()
            .enumerate()
            .map(|(index, pitch)| note(&format!("{prefix}_{index}"), pitch, 2.0, 1.0))
            .collect()
    };
    for seed in 0..16 {
        let strings = run(
            &line(
                "violins_1",
                "violin",
                chord("v"),
                SoundCaps::default(),
                DEVIATIONS,
            ),
            &second_beats(),
            seed,
        );
        let starts: Vec<f64> = strings.notes.iter().map(|note| note.start).collect();
        let spread = starts.iter().copied().fold(f64::MIN, f64::max)
            - starts.iter().copied().fold(f64::MAX, f64::min);
        assert!(spread <= 0.004 + 1e-9, "seed {seed}: {spread}");

        let piano = run(
            &line(
                "piano",
                "piano_concert_grand",
                chord("p"),
                SoundCaps::default(),
                DEVIATIONS,
            ),
            &second_beats(),
            seed,
        );
        let by_pitch: Vec<f64> = [48, 55, 64, 72]
            .into_iter()
            .map(|pitch| {
                piano
                    .notes
                    .iter()
                    .find(|note| note.pitch == pitch)
                    .map_or(f64::NAN, |note| note.start)
            })
            .collect();
        assert!(
            by_pitch.is_sorted_by(|lower, higher| lower <= higher),
            "seed {seed}: {by_pitch:?}"
        );
    }
}

#[test]
fn attack_compensation_moves_the_note_on_and_keeps_the_note_off() {
    let caps = SoundCaps {
        attack_ms: vec![(None, 40.0)],
        ..SoundCaps::default()
    };
    let notes = vec![note("n_1", 60, 1.0, 1.0), note("n_2", 62, 3.0, 2.0)];
    let performed = run(
        &line("horn_1", "horn_f", notes, caps, COMPENSATION),
        &second_beats(),
        0,
    );
    for (id, start, end) in [("n_1", 1.0, 2.0), ("n_2", 3.0, 5.0)] {
        let played = by_id(&performed.notes, id);
        assert_close(played.start, start - 0.04, id);
        assert_close(played.end(), end, id);
        assert_eq!(
            timing(&performed, id).compensation,
            Compensation::Advanced {
                wanted_ms: 40.0,
                advanced_ms: 40.0
            },
            "{id}"
        );
    }
    assert_eq!(performed.humanized.uncompensated().count(), 0);
}

#[test]
fn an_unknown_onset_moves_nothing_and_is_reported() {
    let caps = SoundCaps {
        articulations: vec![Staccato],
        attack_ms: vec![(Some(Staccato), 5.0)],
        ..SoundCaps::default()
    };
    let notes = vec![
        note("n_long_1", 60, 0.5, 1.0),
        note("n_long_2", 62, 1.5, 1.0),
        written("n_short_1", 64, 4.0, 0.2, "staccato"),
        written("n_short_2", 65, 4.5, 0.2, "staccato"),
    ];
    let performed = run(
        &line("oboe_1", "oboe", notes, caps, COMPENSATION),
        &second_beats(),
        0,
    );
    for (id, start) in [("n_long_1", 0.5), ("n_long_2", 1.5)] {
        assert_eq!(by_id(&performed.notes, id).start, start, "{id}");
        assert_eq!(timing(&performed, id).compensation, Compensation::Unknown);
    }
    for (id, start) in [("n_short_1", 4.0), ("n_short_2", 4.5)] {
        assert_close(by_id(&performed.notes, id).start, start - 0.005, id);
        assert_eq!(
            timing(&performed, id).compensation,
            Compensation::Advanced {
                wanted_ms: 5.0,
                advanced_ms: 5.0
            }
        );
    }
    let short: Vec<&str> = performed
        .humanized
        .uncompensated()
        .map(|timing| timing.note.as_str())
        .collect();
    assert_eq!(short, ["n_long_1", "n_long_2"]);
}

#[test]
fn a_note_the_overlap_legato_crossfades_into_moves_by_half_the_crossfade() {
    let caps = SoundCaps {
        articulations: vec![Legato],
        legato: LegatoKind::Overlap { crossfade_ms: 90.0 },
        attack_ms: vec![(None, 40.0), (Some(Legato), 40.0)],
        ..SoundCaps::default()
    };
    let notes = vec![
        note("n_1", 60, 1.0, 1.1),
        note("n_2", 62, 2.0, 1.1),
        note("n_3", 64, 3.0, 1.0),
    ];
    let performed = run(
        &line("violins_1", "violin", notes, caps.clone(), COMPENSATION),
        &second_beats(),
        0,
    );
    assert!(
        performed
            .articulations
            .iter()
            .all(|articulation| articulation.plays == Some(Legato))
    );
    // The first note starts with its sample's attack; the others are crossfaded into.
    for (id, start, advance) in [
        ("n_1", 1.0, 0.040),
        ("n_2", 2.0, 0.045),
        ("n_3", 3.0, 0.045),
    ] {
        assert_close(by_id(&performed.notes, id).start, start - advance, id);
    }
    // Played on a sound without overlap legato, every note has its full attack.
    let attacked = run(
        &line(
            "violins_1",
            "violin",
            vec![note("n_1", 60, 1.0, 1.1), note("n_2", 62, 2.0, 1.1)],
            SoundCaps {
                legato: LegatoKind::None,
                ..caps
            },
            COMPENSATION,
        ),
        &second_beats(),
        0,
    );
    assert_close(by_id(&attacked.notes, "n_2").start, 2.0 - 0.040, "n_2");
}

#[test]
fn the_clip_start_and_the_note_before_hold_a_note_on_back() {
    let caps = SoundCaps {
        articulations: vec![Staccato],
        attack_ms: vec![(None, 100.0), (Some(Staccato), 5.0)],
        ..SoundCaps::default()
    };
    let notes = vec![
        note("n_first", 60, 0.02, 0.5),
        written("n_short", 62, 2.0, 0.08, "staccato"),
        note("n_long", 64, 2.1, 1.0),
    ];
    let performed = run(
        &line("flute_1", "flute", notes, caps, COMPENSATION),
        &second_beats(),
        0,
    );
    // 20 ms after the clip's start, the note-on can move only to beat 0.
    assert_eq!(by_id(&performed.notes, "n_first").start, 0.0);
    assert_close(
        by_id(&performed.notes, "n_first").end(),
        0.52,
        "n_first end",
    );
    let Compensation::Advanced {
        wanted_ms,
        advanced_ms,
    } = timing(&performed, "n_first").compensation
    else {
        panic!("n_first is compensated");
    };
    assert_close(wanted_ms, 100.0, "wanted");
    assert_close(advanced_ms, 20.0, "advanced");
    // The long note stays half the written 100 ms after the short one's note-on (1.995 s).
    assert_close(by_id(&performed.notes, "n_long").start, 2.045, "n_long");
    let Compensation::Advanced { advanced_ms, .. } = timing(&performed, "n_long").compensation
    else {
        panic!("n_long is compensated");
    };
    assert_close(advanced_ms, 55.0, "advanced");
    let short: Vec<&str> = performed
        .humanized
        .uncompensated()
        .map(|timing| timing.note.as_str())
        .collect();
    assert_eq!(short, ["n_first", "n_long"]);
    // The short note ends where the long one now starts (its written end was 2.08 s).
    assert_close(
        by_id(&performed.notes, "n_short").end(),
        2.045,
        "n_short end",
    );
}

#[test]
fn a_switch_leads_the_note_on_as_played() {
    // Two passages: the staccato note, then the long note whose 100 ms attack moves its note-on
    // further ahead than the switch's two-block lead.
    let caps = SoundCaps {
        articulations: vec![Staccato],
        attack_ms: vec![(None, 100.0), (Some(Staccato), 5.0)],
        ..SoundCaps::default()
    };
    let notes = vec![
        written("n_short", 62, 1.0, 0.2, "staccato"),
        note("n_long", 64, 2.0, 1.0),
    ];
    let compensated = line("flute_1", "flute", notes, caps, COMPENSATION);
    let score = score_at(60.0, 4.0);
    let performed = run(&compensated, &score, 0);
    let [first, second] = performed.passages.as_slice() else {
        panic!("two passages: {:?}", performed.passages);
    };
    assert_eq!(first.articulation, Some(Staccato));
    assert_eq!(second.articulation, None);
    assert_close(second.start, 1.9, "the long note's note-on");
    assert_close(second.switch.beat, 4.0 + 1.9 - 2.0 * BLOCK_S, "switch");
    assert_eq!(second.switch.value, f64::from(articulation_choice(None)));
    assert!(!second.possibly_late);
    // As written, the switch would come after the note-on as played.
    let as_written = articulate(&compensated, &score).expect("a valid line");
    assert!(as_written.passages[1].switch.beat > 4.0 + 1.9);
}

#[test]
fn a_velocity_step_never_leaves_the_layer_its_note_on_plays() {
    // At mp a melody of single notes is written at 62, the top of the soft layer.
    let caps = SoundCaps {
        layers: vec![62, 127],
        ..SoundCaps::default()
    };
    let notes: Vec<MidiNote> = (0..8)
        .map(|index| note(&format!("n_{index}"), 67, f64::from(2 * index), 1.0))
        .collect();
    let melody = PartLine {
        dynamics: DynamicsRequest {
            marking: Dynamic::MezzoPiano,
            ..DynamicsRequest::default()
        },
        ..line("violins_1", "violin", notes, caps, DEVIATIONS)
    };
    let performed = run(&melody, &second_beats(), 1);
    let steps: Vec<i16> = performed
        .humanized
        .notes
        .iter()
        .map(|timing| timing.velocity)
        .collect();
    assert!(steps.iter().any(|&step| step > 0), "{steps:?}");
    assert!(steps.iter().any(|&step| step < 0), "{steps:?}");
    for ((played, level), step) in performed
        .notes
        .iter()
        .zip(&performed.dynamics.notes)
        .zip(steps)
    {
        let expected = if step > 0 {
            62
        } else {
            u8::try_from(62 + step).expect("1-127")
        };
        assert_eq!(played.velocity, expected, "{}", played.id);
        assert_eq!(level.velocity, expected, "{}", played.id);
    }
}

/// The facts of a string section patch with overlap legato, slow longs and quick shorts.
fn section_caps() -> SoundCaps {
    SoundCaps {
        articulations: vec![Legato, Staccato, Spiccato],
        legato: LegatoKind::Overlap { crossfade_ms: 90.0 },
        layers: vec![62, 127],
        dynamics: DynamicsKind::Velocity,
        attack_ms: vec![
            (None, 120.0),
            (Some(Legato), 120.0),
            (Some(Staccato), 10.0),
            (Some(Spiccato), 8.0),
        ],
        max_hold_s: None,
        sampled: None,
    }
}

/// Notes with distinct ids on a sixteenth grid, so that onsets tie, overlap and crowd often.
fn notes_strategy() -> impl Strategy<Value = Vec<MidiNote>> {
    vec((48_i64..84, 0_u32..64, 1_u32..12, 1_i64..128), 1..24).prop_map(|raw| {
        raw.into_iter()
            .enumerate()
            .map(|(index, (pitch, start, length, velocity))| {
                MidiNote::new(
                    format!("n_{index:03}"),
                    pitch,
                    f64::from(start) / 4.0,
                    f64::from(length) / 4.0,
                    velocity,
                    0,
                )
                .expect("a valid note")
            })
            .collect()
    })
}

proptest! {
    /// The limits of humanization hold for any line (TST-07, TST-09): valid notes in note order;
    /// moves within their bounds; the note-ons in their written order; no new overlap; every
    /// written overlap at another pitch still an overlap; each note-on reading its own level from
    /// the `dynamics` lane in the layer it played before its step; and each switch ahead of its
    /// passage's note-ons as played.
    #[test]
    fn humanization_keeps_its_limits(
        notes in notes_strategy(),
        seed in any::<i64>(),
        bpm in 50.0_f64..160.0,
        origin in 0.0_f64..8.0,
    ) {
        let score = score_at(bpm, origin);
        let tempo = score.tempo();
        let seconds = |beat: f64| tempo.beat_to_seconds(origin + beat);
        let humanized = line("violins_1", "violin", notes.clone(), section_caps(),
            HumanizeRequest::default());
        let performed = run(&humanized, &score, seed);
        let exact = run(&PartLine { humanize: HumanizeRequest::EXACT, ..humanized.clone() },
            &score, seed);
        prop_assert_eq!(performed.notes.len(), exact.notes.len());
        let ordered = performed.notes.windows(2).all(|pair| {
            (pair[0].start, pair[0].pitch, &pair[0].id) <= (pair[1].start, pair[1].pitch, &pair[1].id)
        });
        prop_assert!(ordered);

        // The section's bound: twice its timing deviation plus a chord note's spread.
        let bound = 2.0 * 0.008 + 0.002 + 1e-9;
        for (played, timing) in performed.notes.iter().zip(&performed.humanized.notes) {
            prop_assert_eq!(&played.id, &timing.note);
            let written = by_id(&exact.notes, &played.id);
            prop_assert!(played.start >= 0.0 && played.start.is_finite());
            prop_assert!(played.length > 0.0 && played.length.is_finite());
            prop_assert!((1..=127).contains(&played.velocity));
            prop_assert!(i16::from(played.velocity).abs_diff(i16::from(written.velocity)) <= 4);
            let wanted = match timing.compensation {
                Compensation::Advanced { wanted_ms, .. } => wanted_ms / 1000.0,
                Compensation::Off | Compensation::Unknown => 0.0,
            };
            let moved = seconds(played.start) - seconds(written.start);
            prop_assert!(moved >= -wanted - bound && moved <= bound,
                "{} moved {moved} s, wanted {wanted}", played.id);
        }

        for a in &exact.notes {
            for b in &exact.notes {
                if b.start <= a.start + 1e-9 {
                    continue;
                }
                let (pa, pb) = (by_id(&performed.notes, &a.id), by_id(&performed.notes, &b.id));
                prop_assert!(pa.start < pb.start, "{} no longer starts before {}", a.id, b.id);
                if a.end() <= b.start + 1e-9 {
                    prop_assert!(pa.end() <= pb.start + 1e-9,
                        "{} now overlaps {}", a.id, b.id);
                } else if a.pitch != b.pitch {
                    prop_assert!(pa.end() > pb.start, "{} no longer overlaps {}", a.id, b.id);
                }
            }
        }

        // Each note-on reads its own level from the lane, in the layer it played as written.
        let lane = performed.dynamics_lane();
        for (played, level) in performed.notes.iter().zip(&performed.dynamics.notes) {
            prop_assert_eq!(&level.note, &played.id);
            let factor = value_at(&lane, origin + played.start);
            let plays = sounding(f64::from(played.velocity) * factor);
            prop_assert_eq!(plays, level.velocity, "{}", played.id);
            let as_written = exact.dynamics.notes.iter()
                .find(|written| written.note == played.id)
                .expect("the same notes");
            let layer = section_caps().layer(as_written.velocity).expect("a layer");
            prop_assert!(layer.contains(&plays), "{} plays {plays}", played.id);
        }

        // Each switch lies after the previous passage's last note-on and ahead of its own first,
        // by at least a block unless it says it may be late.
        for (index, passage) in performed.passages.iter().enumerate() {
            let members: Vec<f64> = performed.notes.iter().zip(&performed.articulations)
                .filter(|(_, articulation)| articulation.passage == index)
                .map(|(note, _)| note.start)
                .collect();
            let first = members.iter().copied().fold(f64::INFINITY, f64::min);
            let last = members.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            // The passage's note-ons are its notes' own starts, bit for bit.
            prop_assert!(passage.start.total_cmp(&first).is_eq());
            prop_assert!(passage.last_onset.total_cmp(&last).is_eq());
            prop_assert!(passage.switch.beat <= origin + first + 1e-9);
            if index > 0 {
                let previous = &performed.passages[index - 1];
                prop_assert!(passage.switch.beat > origin + previous.last_onset);
                if !passage.possibly_late {
                    let lead = tempo.beat_to_seconds(origin + first)
                        - tempo.beat_to_seconds(passage.switch.beat);
                    prop_assert!(lead >= BLOCK_S - 1e-9, "passage {index} leads by {lead}");
                }
            }
        }
    }
}
