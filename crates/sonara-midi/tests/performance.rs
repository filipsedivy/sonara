//! Contract tests of the performance layer's types and `perform` through the public API.

use std::error::Error as _;

use proptest::collection::vec;
use proptest::prelude::*;
use sonara_core::midi::MidiNote;
use sonara_core::time::{TempoMap, TimeSignatureMap};
use sonara_midi::orchestration::{Articulation, PitchRange, ensembles, instrument};
use sonara_midi::performance::{
    CapsError, Dynamic, Dynamics, DynamicsKind, DynamicsRequest, HumanizeRequest, Humanized,
    LegatoKind, MAX_PART_ID_BYTES, PartId, PartLine, PerformanceError, Performed, Role,
    ScoreContext, Seed, SoundCaps, perform,
};

/// A change made to a sound's capabilities.
type Change = fn(&mut SoundCaps);

fn part(slug: &str) -> PartId {
    PartId::new(slug).expect("a valid part id")
}

fn note(id: &str, pitch: i64, start: f64, length: f64) -> MidiNote {
    MidiNote::new(id, pitch, start, length, 80, 0).expect("a valid note")
}

fn score() -> ScoreContext {
    ScoreContext::new(
        TempoMap::constant(72.0).expect("a valid tempo"),
        TimeSignatureMap::default(),
        0.0,
    )
    .expect("a valid origin")
}

fn line(slug: &str, notes: Vec<MidiNote>) -> PartLine {
    PartLine {
        part: part(slug),
        instrument: instrument("violin").expect("violin is an instrument type"),
        role: Role::Melody,
        notes,
        caps: SoundCaps::default(),
        dynamics: DynamicsRequest::default(),
        humanize: HumanizeRequest::default(),
    }
}

/// The facts of a two-layer string section patch with overlap legato.
fn section_caps() -> SoundCaps {
    SoundCaps {
        articulations: vec![Articulation::Legato, Articulation::Staccato],
        legato: LegatoKind::Overlap { crossfade_ms: 90.0 },
        layers: vec![62, 127],
        dynamics: DynamicsKind::Velocity,
        attack_ms: vec![(None, 40.0), (Some(Articulation::Staccato), 5.0)],
        max_hold_s: Some(11.2),
        sampled: Some(PitchRange::new(55, 100)),
    }
}

#[test]
fn every_ensemble_part_slug_is_a_part_id() {
    for ensemble in ensembles() {
        for template_part in ensemble.parts() {
            let id = PartId::new(template_part.slug)
                .unwrap_or_else(|error| panic!("{}: {error}", template_part.slug));
            assert_eq!(id.as_str(), template_part.slug);
            assert_eq!(id.to_string(), template_part.slug);
        }
    }
}

#[test]
fn part_ids_reject_what_is_not_a_slug() {
    let too_long = "a".repeat(MAX_PART_ID_BYTES + 1);
    for slug in [
        "",
        "Horn_3",
        "horn-3",
        "horn 3",
        "hörn",
        "horn_3\n",
        too_long.as_str(),
    ] {
        assert_eq!(
            PartId::new(slug),
            Err(PerformanceError::InvalidPartId(slug.to_owned())),
            "{slug:?}"
        );
    }
    let longest = "a".repeat(MAX_PART_ID_BYTES);
    assert_eq!(part(&longest).as_str(), longest);
    assert_eq!(part("horn_13").as_str(), "horn_13");
    assert_eq!(
        PartId::new("Horn 3").expect_err("not a slug").to_string(),
        "part id \"Horn 3\" must be 1-64 bytes of lower-case letters, digits and underscores"
    );
}

#[test]
fn roles_round_trip_through_their_slugs() {
    let slugs: Vec<&str> = Role::ALL.iter().map(|role| role.slug()).collect();
    assert_eq!(
        slugs,
        [
            "melody",
            "counter_melody",
            "harmony",
            "bass",
            "rhythm",
            "colour",
            "pedal"
        ]
    );
    for role in Role::ALL {
        assert_eq!(Role::from_slug(role.slug()), Some(role));
    }
    for slug in ["Melody", "counter-melody", "color", ""] {
        assert_eq!(Role::from_slug(slug), None, "{slug:?}");
    }
}

#[test]
fn default_caps_know_nothing_and_pass_the_check() {
    let caps = SoundCaps::default();
    assert_eq!(caps.articulations, []);
    assert_eq!(caps.legato, LegatoKind::None);
    assert_eq!(caps.layers, [127]);
    assert_eq!(caps.dynamics, DynamicsKind::Velocity);
    assert_eq!(caps.attack_ms, []);
    assert_eq!(caps.max_hold_s, None);
    assert_eq!(caps.sampled, None);
    assert_eq!(caps.check(), Ok(()));
    assert_eq!(section_caps().check(), Ok(()));
}

#[test]
fn caps_at_their_limits_pass_the_check() {
    let limits: [Change; 6] = [
        |caps| caps.layers = vec![1, 127],
        |caps| caps.layers = vec![1, 2, 126, 127],
        |caps| caps.legato = LegatoKind::Overlap { crossfade_ms: 1e-9 },
        |caps| caps.dynamics = DynamicsKind::Fixed(1),
        |caps| caps.dynamics = DynamicsKind::Fixed(127),
        |caps| caps.attack_ms = vec![(None, 0.0), (Some(Articulation::Legato), 0.0)],
    ];
    for (index, change) in limits.into_iter().enumerate() {
        let mut caps = section_caps();
        change(&mut caps);
        assert_eq!(caps.check(), Ok(()), "limit {index}");
    }
    let mut caps = section_caps();
    caps.legato = LegatoKind::Trigger;
    caps.dynamics = DynamicsKind::Cc;
    caps.max_hold_s = Some(f64::MIN_POSITIVE);
    assert_eq!(caps.check(), Ok(()));
}

#[test]
fn caps_check_rejects_each_contradiction() {
    let cases: [(Change, CapsError); 16] = [
        (|caps| caps.layers = Vec::new(), CapsError::Layers),
        (|caps| caps.layers = vec![0, 127], CapsError::Layers),
        (|caps| caps.layers = vec![62, 62, 127], CapsError::Layers),
        (|caps| caps.layers = vec![100, 62, 127], CapsError::Layers),
        (|caps| caps.layers = vec![62, 126], CapsError::Layers),
        (
            |caps| caps.legato = LegatoKind::Overlap { crossfade_ms: 0.0 },
            CapsError::Crossfade(0.0),
        ),
        (
            |caps| caps.legato = LegatoKind::Overlap { crossfade_ms: -5.0 },
            CapsError::Crossfade(-5.0),
        ),
        (
            |caps| {
                caps.legato = LegatoKind::Overlap {
                    crossfade_ms: f64::INFINITY,
                };
            },
            CapsError::Crossfade(f64::INFINITY),
        ),
        (
            |caps| caps.dynamics = DynamicsKind::Fixed(0),
            CapsError::FixedVelocity(0),
        ),
        (
            |caps| caps.dynamics = DynamicsKind::Fixed(128),
            CapsError::FixedVelocity(128),
        ),
        (
            |caps| caps.attack_ms = vec![(None, -1.0)],
            CapsError::Attack(-1.0),
        ),
        (
            |caps| caps.attack_ms = vec![(Some(Articulation::Legato), f64::INFINITY)],
            CapsError::Attack(f64::INFINITY),
        ),
        (
            |caps| caps.attack_ms = vec![(None, 5.0), (None, 6.0)],
            CapsError::DuplicateAttack,
        ),
        (
            |caps| {
                caps.attack_ms = vec![
                    (Some(Articulation::Staccato), 1.0),
                    (None, 1.0),
                    (Some(Articulation::Staccato), 1.0),
                ];
            },
            CapsError::DuplicateAttack,
        ),
        (|caps| caps.max_hold_s = Some(0.0), CapsError::MaxHold(0.0)),
        (
            |caps| caps.max_hold_s = Some(f64::NEG_INFINITY),
            CapsError::MaxHold(f64::NEG_INFINITY),
        ),
    ];
    for (index, (change, expected)) in cases.into_iter().enumerate() {
        let mut caps = section_caps();
        change(&mut caps);
        assert_eq!(caps.check(), Err(expected), "case {index}");
    }
    // NaN never equals itself, so these cases match the variant and test the value.
    let mut caps = section_caps();
    caps.legato = LegatoKind::Overlap {
        crossfade_ms: f64::NAN,
    };
    assert!(matches!(caps.check(), Err(CapsError::Crossfade(ms)) if ms.is_nan()));
    let mut caps = section_caps();
    caps.attack_ms = vec![(None, f64::NAN)];
    assert!(matches!(caps.check(), Err(CapsError::Attack(ms)) if ms.is_nan()));
    let mut caps = section_caps();
    caps.max_hold_s = Some(f64::NAN);
    assert!(matches!(caps.check(), Err(CapsError::MaxHold(s)) if s.is_nan()));
}

#[test]
fn caps_check_reports_the_first_failure_in_its_documented_order() {
    let mut caps = SoundCaps {
        articulations: Vec::new(),
        legato: LegatoKind::Overlap { crossfade_ms: 0.0 },
        layers: Vec::new(),
        dynamics: DynamicsKind::Fixed(0),
        attack_ms: vec![(None, 1.0), (None, -1.0)],
        max_hold_s: Some(0.0),
        sampled: None,
    };
    let expected = [
        CapsError::Layers,
        CapsError::Crossfade(0.0),
        CapsError::FixedVelocity(0),
        CapsError::Attack(-1.0),
        CapsError::MaxHold(0.0),
    ];
    let fixes: [Change; 5] = [
        |caps| caps.layers = vec![127],
        |caps| caps.legato = LegatoKind::None,
        |caps| caps.dynamics = DynamicsKind::Velocity,
        |caps| caps.attack_ms = vec![(None, 1.0)],
        |caps| caps.max_hold_s = None,
    ];
    for (expected, fix) in expected.into_iter().zip(fixes) {
        assert_eq!(caps.check(), Err(expected));
        fix(&mut caps);
    }
    assert_eq!(caps.check(), Ok(()));
    // An attack entry fails on its value before it is compared with earlier entries.
    caps.attack_ms = vec![(None, 1.0), (None, -1.0)];
    assert_eq!(caps.check(), Err(CapsError::Attack(-1.0)));
}

#[test]
fn score_context_rejects_an_origin_off_the_timeline() {
    for origin in [-0.5, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            ScoreContext::new(
                TempoMap::constant(120.0).expect("tempo"),
                TimeSignatureMap::default(),
                origin
            ),
            Err(PerformanceError::InvalidOrigin(origin)),
            "{origin}"
        );
    }
    assert!(matches!(
        ScoreContext::new(
            TempoMap::constant(120.0).expect("tempo"),
            TimeSignatureMap::default(),
            f64::NAN
        ),
        Err(PerformanceError::InvalidOrigin(origin)) if origin.is_nan()
    ));
    let tempo = TempoMap::constant(96.0).expect("tempo");
    let context = ScoreContext::new(tempo.clone(), TimeSignatureMap::default(), 12.5)
        .expect("a valid origin");
    assert_eq!(context.tempo(), &tempo);
    assert_eq!(context.signatures(), &TimeSignatureMap::default());
    assert_eq!(context.origin(), 12.5);
    assert_eq!(
        ScoreContext::new(tempo, TimeSignatureMap::default(), 0.0)
            .expect("beat 0")
            .origin(),
        0.0
    );
}

#[test]
fn perform_returns_the_notes_in_project_order_with_their_fields() {
    let legato = note("n_c", 64, 1.0, 2.0)
        .with_articulation(Some("legato".to_owned()))
        .expect("a vocabulary id");
    let notes = vec![
        note("n_e", 67, 2.0, 1.0),
        legato.clone(),
        note("n_b", 60, 1.0, 1.0),
        note("n_d", 64, 1.0, 0.5),
        note("n_a", 72, 0.0, 1.0),
    ];
    let mut exact = line("violins_1", notes);
    exact.humanize = HumanizeRequest::EXACT;
    let performed = perform(&exact, &score(), Seed::default()).expect("valid caps");
    assert_eq!(performed.part, part("violins_1"));
    let order: Vec<&str> = performed.notes.iter().map(|n| n.id.as_str()).collect();
    // By start, then pitch, then id: the chord at beat 1 rises, and its two E4s keep id order.
    assert_eq!(order, ["n_a", "n_b", "n_c", "n_d", "n_e"]);
    // Every field is kept but the velocity, which the melody's arc at mf writes at f (95).
    assert_eq!(
        performed.notes.get(2),
        Some(&MidiNote {
            velocity: 95,
            ..legato
        })
    );
    assert_eq!(
        performed.notes.get(3).map(|n| n.articulation.clone()),
        Some(None)
    );
}

#[test]
fn a_tacet_part_performs_to_no_notes() {
    let performed =
        perform(&line("double_basses", Vec::new()), &score(), Seed::new(4)).expect("valid caps");
    assert_eq!(
        performed,
        Performed {
            part: part("double_basses"),
            notes: Vec::new(),
            articulations: Vec::new(),
            passages: Vec::new(),
            dynamics: Dynamics {
                kind: DynamicsKind::Velocity,
                marking: Dynamic::MezzoForte,
                phrases: Vec::new(),
                notes: Vec::new(),
                lane: Vec::new(),
                remarks: Vec::new(),
            },
            humanized: Humanized {
                request: HumanizeRequest::default(),
                notes: Vec::new(),
            },
        }
    );
    assert_eq!(performed.articulation_lane(), []);
    assert_eq!(performed.dynamics_lane(), []);
}

#[test]
fn perform_rejects_contradictory_caps_and_keeps_the_cause() {
    let mut contradictory = line("horn_1", vec![note("n_1", 65, 0.0, 1.0)]);
    contradictory.caps.layers = vec![62, 126];
    let error = perform(&contradictory, &score(), Seed::default()).expect_err("bad layers");
    assert_eq!(error, PerformanceError::InvalidCaps(CapsError::Layers));
    assert_eq!(error.to_string(), "the sound's capabilities are invalid");
    let cause = error
        .source()
        .and_then(|source| source.downcast_ref::<CapsError>());
    assert_eq!(cause, Some(&CapsError::Layers));
}

#[test]
fn the_same_line_score_and_seed_give_the_same_performance_whatever_else_is_performed() {
    let notes = vec![
        note("n_1", 69, 0.0, 1.0),
        note("n_2", 72, 1.0, 1.0),
        note("n_3", 74, 2.0, 2.0),
    ];
    let mut violins = line("violins_1", notes.clone());
    violins.caps = section_caps();
    let first = perform(&violins, &score(), Seed::new(7)).expect("valid caps");
    for other in ["violins_2", "violas", "horn_3"] {
        perform(&line(other, notes.clone()), &score(), Seed::new(7)).expect("valid caps");
    }
    assert_eq!(perform(&violins, &score(), Seed::new(7)), Ok(first));
}

/// Notes with distinct ids on a quarter-beat grid, so starts tie often.
fn notes_strategy() -> impl Strategy<Value = Vec<MidiNote>> {
    vec((0_i64..128, 0_u32..32, 1_u32..16, 1_i64..128), 0..24).prop_map(|raw| {
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
    /// Whatever order the line lists its notes in, the performance holds the same notes in the
    /// project's note order (TST-09), humanized alike: the deviations are drawn in note order.
    /// The line's sound lists no articulation, so no stroke is merged and every note comes back
    /// with its id, pitch, channel and articulation.
    #[test]
    fn perform_ignores_the_order_of_the_input(
        (notes, shuffled) in notes_strategy().prop_flat_map(|notes| {
            let shuffled = Just(notes.clone()).prop_shuffle();
            (Just(notes), shuffled)
        })
    ) {
        let in_order = perform(&line("cellos", notes.clone()), &score(), Seed::new(1))
            .expect("valid caps");
        let reordered = perform(&line("cellos", shuffled), &score(), Seed::new(1))
            .expect("valid caps");
        prop_assert_eq!(&in_order, &reordered);
        prop_assert_eq!(in_order.notes.len(), notes.len());
        // Every note comes back with the fields humanization and the dynamics leave alone.
        for note in &notes {
            let kept = in_order.notes.iter().any(|performed| {
                performed == &MidiNote {
                    start: performed.start,
                    length: performed.length,
                    velocity: performed.velocity,
                    ..note.clone()
                }
            });
            prop_assert!(kept, "{} is missing", note.id);
        }
        let ordered = in_order.notes.windows(2).all(|pair| {
            let (a, b) = (&pair[0], &pair[1]);
            (a.start, a.pitch, &a.id) <= (b.start, b.pitch, &b.id)
        });
        prop_assert!(ordered, "{:?}", in_order.notes);
    }
}
