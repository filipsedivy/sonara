//! `sonara.sampler` against synthetic banks: layers, round robin, release, release triggers,
//! the sustain pedal, keyswitches, exclusive groups, controllers, polyphony, the articulation,
//! dynamics, mute and vibrato parameters, an automated articulation, mute or vibrato change
//! inside a block, a note's own articulation, the key offset, the overlap legato, the patches of
//! a multi-patch set and the real-time rules (TST-10: every check compares against a value that follows from the bank,
//! not "it ran"). Every block, and every parameter change of the articulation, dynamics, mute,
//! vibrato and set tests, runs under `assert_no_alloc` (RT-02).
//!
//! The samples are constants, so a voice's steady-state output is its sample value times the
//! velocity gain `(velocity / 127)²`, which the tests compute independently of the engine; a
//! legato crossfade adds the equal-power gains `cos` and `sin` of the fraction it has done.

use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2};
use std::sync::Arc;

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use sonara_core::model::device::{ParamValue, Params};
use sonara_core::plugins::{SamplerChoices, sampler_descriptor};
use sonara_dsp::bank::{
    ArticulationTarget, CcCrossfade, CcDepth, CcRange, Controllers, Crossfade, CrossfadeCurve,
    DynamicsDrive, Keyswitches, LegatoSpec, LoopMode, OffMode, Passage, Region, Sample, SampleBank,
    SamplerMapping, SamplerSetup, Selector, TargetTable, Trigger,
};
use sonara_dsp::processor::{
    BlockContext, DeviceState, EventKind, InstrumentEvent, ParamSlot, ProcessSpec, Processor,
};
use sonara_dsp::processors::{create, param_slots};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

/// The sampler's descriptor with one patch, four articulations, two mutes and three vibratos:
/// the `articulation` choices are `auto` (0), `ordinario` (1), `sustain` (2), `staccato` (3),
/// `pizzicato` (4) and `legato` (5); the `mute` choices `auto` (0), `open` (1) and
/// `con_sordino` (2); the `vibrato` choices `auto` (0), `ordinary` (1), `vibrato_on` (2) and
/// `non_vibrato` (3).
fn descriptor() -> sonara_core::plugins::PluginDescriptor {
    let owned = |ids: &[&str]| ids.iter().map(|&id| id.to_owned()).collect();
    sampler_descriptor(&SamplerChoices {
        presets: vec!["/library/violin.sfz".to_owned()],
        articulations: owned(&["sustain", "staccato", "pizzicato", "legato"]),
        mutes: owned(&["open", "con_sordino"]),
        vibratos: owned(&["ordinary", "vibrato_on", "non_vibrato"]),
        ..SamplerChoices::default()
    })
    .expect("one patch")
}

const RATE: u32 = 48_000;
const BLOCK: usize = 2_048;
/// Frames long enough to cover every test's playback.
const SAMPLE_FRAMES: usize = 96_000;

fn constant(value: f32) -> Sample {
    Sample {
        rate: RATE,
        channels: 1,
        data: vec![value; SAMPLE_FRAMES],
    }
}

fn region(sample: usize) -> Region {
    Region {
        sample,
        pitch_keycenter: 60,
        end: SAMPLE_FRAMES,
        ..Region::default()
    }
}

fn sampler(bank: SampleBank) -> Box<dyn Processor> {
    sampler_with(bank, &Params::new())
}

/// A prepared sampler playing `bank` with `params` over the defaults, with the mapping of a
/// preset about which nothing is claimed.
fn sampler_with(bank: SampleBank, params: &Params) -> Box<dyn Processor> {
    sampler_set_up(SamplerSetup::new(Arc::new(bank)), params)
}

/// A prepared sampler playing the instrument `setup` with `params` over the defaults.
fn sampler_set_up(setup: SamplerSetup, params: &Params) -> Box<dyn Processor> {
    let mut processor = create("sonara.sampler").expect("the sampler exists");
    processor.prepare(ProcessSpec {
        sample_rate: RATE,
        block_size: BLOCK,
        channels: 2,
    });
    processor.set_state(&DeviceState {
        sampler: Some(Arc::new(setup)),
        ..DeviceState::default()
    });
    let slots = param_slots(&descriptor(), params).expect("valid parameters");
    processor.set_params(&slots);
    processor
}

fn event(kind: EventKind, pitch: u8, velocity: u8) -> InstrumentEvent {
    InstrumentEvent {
        offset: 0,
        kind,
        pitch,
        velocity,
        channel: 0,
        value: 0,
        number: 0,
        articulation: None,
    }
}

fn run(processor: &mut dyn Processor, events: &[InstrumentEvent]) -> Vec<[f64; 2]> {
    let mut buffer = vec![[0.0; 2]; BLOCK];
    let ctx = BlockContext {
        frame: 0,
        sample_rate: RATE,
        beat: 0.0,
        bpm: 120.0,
        playing: true,
        sidechain: None,
        events,
    };
    assert_no_alloc(|| processor.process(&mut buffer, &ctx));
    buffer
}

/// The settled output level of a block (its last frame).
fn level(block: &[[f64; 2]]) -> f64 {
    block.last().expect("a block")[0]
}

fn gain(velocity: u8) -> f64 {
    (f64::from(velocity) / 127.0).powi(2)
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

#[test]
fn velocity_layers_select_their_sample() {
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.25)],
        regions: vec![
            Region {
                vel_hi: 63,
                ..region(0)
            },
            Region {
                vel_lo: 64,
                ..region(1)
            },
        ],
        ..SampleBank::default()
    };
    let mut soft = sampler(bank.clone());
    close(
        level(&run(&mut *soft, &[event(EventKind::NoteOn, 60, 40)])),
        0.5 * gain(40),
    );
    let mut loud = sampler(bank);
    close(
        level(&run(&mut *loud, &[event(EventKind::NoteOn, 60, 100)])),
        0.25 * gain(100),
    );
}

/// The gain of `amp_veltrack` `percent` at `velocity`, written out from the SFZ format and the
/// reference player sfizz: `1 - t (1 - v²)` for `t >= 0`, `|t| (1 - v²)` below.
fn tracked(percent: f64, velocity: u8) -> f64 {
    let (t, v2) = (percent / 100.0, gain(velocity));
    if t >= 0.0 {
        1.0 - t * (1.0 - v2)
    } else {
        -t * (1.0 - v2)
    }
}

/// A region on sample 0 with `amp_veltrack` `percent`.
fn tracking(percent: f64) -> Region {
    Region {
        amp_veltrack: percent,
        ..region(0)
    }
}

#[test]
fn velocity_tracking_matches_the_aria_measurements() {
    // The gains ARIA plays at 25, 50, 75 and 100 % for velocities 127, 96, 64, 32 and 1, in dB,
    // as sfizz's tests record them (0.1 dB is their margin).
    let measured = [
        (25.0, [0.0, -1.0, -1.8, -2.3, -2.5]),
        (50.0, [0.0, -2.1, -4.1, -5.5, -6.0]),
        (75.0, [0.0, -3.4, -7.2, -10.5, -12.0]),
        (100.0, [0.0, -4.9, -12.0, -24.0, -84.1]),
    ];
    for (percent, db) in measured {
        for (velocity, expected) in [127, 96, 64, 32, 1].into_iter().zip(db) {
            let got = 20.0 * tracking(percent).velocity_gain(velocity).log10();
            assert!(
                (got - expected).abs() <= 0.1,
                "{percent} % at {velocity}: {got} dB, not {expected}"
            );
        }
    }
}

#[test]
fn velocity_tracking_follows_the_sfz_formula() {
    for percent in [-100.0, -50.0, -25.0, 0.0, 25.0, 60.0, 100.0] {
        for velocity in [1, 32, 64, 100, 127] {
            close(
                tracking(percent).velocity_gain(velocity),
                tracked(percent, velocity),
            );
        }
    }
    // The ends: 100 % is the curve v², 0 % ignores the velocity, -100 % silences velocity 127
    // and leaves the softest notes at full gain (as sfizz's tests pin them).
    assert_eq!(tracking(100.0).velocity_gain(64), gain(64));
    assert_eq!(
        Region::default().velocity_gain(64),
        gain(64),
        "100 % is the default"
    );
    assert_eq!(tracking(0.0).velocity_gain(1), 1.0);
    assert_eq!(tracking(-100.0).velocity_gain(127), 0.0);
    assert_eq!(tracking(-100.0).velocity_gain(0), 1.0);
    // Out of range or not a number: the nearest end, or the default.
    assert_eq!(tracking(250.0).velocity_gain(64), gain(64));
    assert_eq!(tracking(-250.0).velocity_gain(127), 0.0);
    assert_eq!(tracking(f64::NAN).velocity_gain(64), gain(64));
}

#[test]
fn amp_veltrack_zero_ignores_velocity() {
    // An organ or harpsichord region (`amp_veltrack=0`) sounds the same at every velocity.
    let bank = SampleBank {
        samples: vec![constant(0.5)],
        regions: vec![tracking(0.0)],
        ..SampleBank::default()
    };
    for velocity in [1, 40, 127] {
        let mut processor = sampler(bank.clone());
        let out = run(&mut *processor, &[event(EventKind::NoteOn, 60, velocity)]);
        close(level(&out), 0.5);
    }
}

#[test]
fn partial_velocity_tracking_scales_the_voice() {
    for (percent, velocity) in [(50.0, 64), (-50.0, 64), (-100.0, 20)] {
        let bank = SampleBank {
            samples: vec![constant(0.5)],
            regions: vec![tracking(percent)],
            ..SampleBank::default()
        };
        let mut processor = sampler(bank);
        let out = run(&mut *processor, &[event(EventKind::NoteOn, 60, velocity)]);
        close(level(&out), 0.5 * tracked(percent, velocity));
    }
}

#[test]
fn a_region_tracked_to_silence_starts_no_voice_but_chokes_its_group() {
    // At -100 %, velocity 127 gives zero gain: no voice starts, but the press still switches off
    // the exclusive group it belongs to.
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.25)],
        regions: vec![
            Region {
                key_lo: 50,
                key_hi: 50,
                off_by: 1,
                ..region(0)
            },
            Region {
                key_lo: 60,
                key_hi: 60,
                group: 1,
                ..tracking(-100.0)
            },
        ],
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 50, 127)])),
        0.5,
    );
    let out = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    close(level(&out), 0.0);
    assert_eq!(processor.report().entries(), [("voices", 0.0)]);
}

/// A fade-in over `lo..hi` with `curve`.
fn fade_in(lo: u8, hi: u8, curve: CrossfadeCurve) -> Crossfade {
    Crossfade {
        in_lo: lo,
        in_hi: hi,
        curve,
        ..Crossfade::NONE
    }
}

/// A fade-out over `lo..hi` with `curve`.
fn fade_out(lo: u8, hi: u8, curve: CrossfadeCurve) -> Crossfade {
    Crossfade {
        out_lo: lo,
        out_hi: hi,
        curve,
        ..Crossfade::NONE
    }
}

/// Checks `crossfade` at each velocity from `first` against `expected`, to the five digits
/// sfizz's tests give.
fn assert_fade(crossfade: Crossfade, first: u8, expected: &[f64]) {
    for (velocity, want) in (first..).zip(expected) {
        let got = crossfade.gain(velocity);
        assert!(
            (got - want).abs() < 1e-5,
            "{crossfade:?} at {velocity}: {got}, not {want}"
        );
    }
}

#[test]
fn velocity_crossfade_gains_match_sfizz() {
    // sfizz's tests of `xfin_lovel=20 xfin_hivel=24` and `xfout_lovel=51 xfout_hivel=55`, with
    // the default equal-power curve and with `xf_velcurve=gain`: silent at or below the fade-in's
    // low velocity, full from its high one, and the other way round for the fade-out.
    assert_fade(
        fade_in(20, 24, CrossfadeCurve::Power),
        19,
        &[0.0, 0.0, 0.5, FRAC_1_SQRT_2, 0.86603, 1.0, 1.0],
    );
    assert_fade(
        fade_in(20, 24, CrossfadeCurve::Gain),
        19,
        &[0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0],
    );
    assert_fade(
        fade_out(51, 55, CrossfadeCurve::Power),
        50,
        &[1.0, 1.0, 0.86603, FRAC_1_SQRT_2, 0.5, 0.0, 0.0],
    );
    assert_fade(
        fade_out(51, 55, CrossfadeCurve::Gain),
        50,
        &[1.0, 1.0, 0.75, 0.5, 0.25, 0.0, 0.0],
    );
    // Without a crossfade every velocity plays at full gain, and the default curve is power.
    for velocity in 0..=127 {
        assert_eq!(Crossfade::NONE.gain(velocity), 1.0, "{velocity}");
    }
    assert_eq!(Crossfade::default(), Crossfade::NONE);
    assert_eq!(Region::default().velocity_crossfade, Crossfade::NONE);
    assert_eq!(CrossfadeCurve::default(), CrossfadeCurve::Power);
}

#[test]
fn where_a_fades_bounds_meet_or_cross_the_low_bound_decides() {
    let power = CrossfadeCurve::Power;
    // `xfin_lovel=70` alone (its high bound stays 0) and a fade-in of no width are steps at 70.
    for crossfade in [fade_in(70, 0, power), fade_in(70, 70, power)] {
        assert_fade(crossfade, 69, &[0.0, 1.0, 1.0]);
        assert_eq!(crossfade.gain(127), 1.0);
    }
    // `xfout_hivel=90` alone (its low bound stays 127) fades nothing, as in sfizz.
    for velocity in 0..=127 {
        assert_eq!(fade_out(127, 90, power).gain(velocity), 1.0, "{velocity}");
    }
    // A fade-out of no width, or one whose bounds cross, is full up to its low bound and silent
    // above it, as the SFZ format's bounds say (sfizz fades nothing there).
    assert_fade(fade_out(60, 60, power), 59, &[1.0, 1.0, 0.0, 0.0]);
    assert_fade(fade_out(90, 70, power), 89, &[1.0, 1.0, 0.0]);
    // A fade that ends at 127 reaches its end there, not one velocity early as in sfizz.
    assert_fade(
        fade_out(100, 127, power),
        125,
        &[(2.0_f64 / 27.0).sqrt(), (1.0_f64 / 27.0).sqrt(), 0.0],
    );
    assert_fade(
        fade_in(100, 127, power),
        125,
        &[(25.0_f64 / 27.0).sqrt(), (26.0_f64 / 27.0).sqrt(), 1.0],
    );
}

#[test]
fn complementary_crossfades_keep_power_or_amplitude() {
    // TST-07: a layer that fades out over the velocities another fades in over keeps the summed
    // power with the equal-power curve, and the summed amplitude with the gain curve, at every
    // velocity.
    for velocity in 0..=127 {
        let power = |fade: Crossfade| fade.gain(velocity).powi(2);
        let p = CrossfadeCurve::Power;
        close(power(fade_out(70, 90, p)) + power(fade_in(70, 90, p)), 1.0);
        let g = CrossfadeCurve::Gain;
        close(
            fade_out(70, 90, g).gain(velocity) + fade_in(70, 90, g).gain(velocity),
            1.0,
        );
    }
}

/// Sonatina's timpani: the soft layer (sample 0) fades out and the loud one (sample 1) fades in
/// over velocities 70 to 90, with `curve`. Neither writes a velocity range.
fn timpani(curve: CrossfadeCurve) -> SampleBank {
    SampleBank {
        samples: vec![constant(0.5), constant(0.25)],
        regions: vec![
            Region {
                velocity_crossfade: fade_out(70, 90, curve),
                ..region(0)
            },
            Region {
                velocity_crossfade: fade_in(70, 90, curve),
                ..region(1)
            },
        ],
        ..SampleBank::default()
    }
}

#[test]
fn velocity_crossfade_scales_each_layer() {
    // TST-06: both layers used to sound at full gain on every hit. Inside the crossfade each
    // sounds at its share; outside it only one starts a voice.
    let position = |velocity: u8| (f64::from(velocity) - 70.0) / 20.0;
    for (curve, share) in [
        (CrossfadeCurve::Power, f64::sqrt as fn(f64) -> f64),
        (CrossfadeCurve::Gain, |x| x),
    ] {
        for (velocity, voices) in [
            (60, 1.0),
            (70, 1.0),
            (75, 2.0),
            (80, 2.0),
            (90, 1.0),
            (110, 1.0),
        ] {
            let mut processor = sampler(timpani(curve));
            let out = run(&mut *processor, &[event(EventKind::NoteOn, 60, velocity)]);
            let x = position(velocity).clamp(0.0, 1.0);
            let expected = gain(velocity) * (0.5 * share(1.0 - x) + 0.25 * share(x));
            close(level(&out), expected);
            assert_eq!(
                processor.report().entries(),
                [("voices", voices)],
                "{curve:?} at {velocity}"
            );
        }
    }
}

#[test]
fn velocity_tracking_and_crossfade_multiply() {
    let bank = SampleBank {
        samples: vec![constant(0.5)],
        regions: vec![Region {
            amp_veltrack: 0.0,
            velocity_crossfade: fade_in(70, 90, CrossfadeCurve::Gain),
            ..region(0)
        }],
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    let out = run(&mut *processor, &[event(EventKind::NoteOn, 60, 85)]);
    close(level(&out), 0.5 * 0.75);
}

#[test]
fn a_layer_outside_its_crossfade_still_counts_its_round_robin() {
    // The first position fades in only above velocity 100: a soft press selects it, starts no
    // voice, and the next press moves on to the second position.
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.25)],
        regions: vec![
            Region {
                seq_length: 2,
                seq_position: 1,
                velocity_crossfade: fade_in(100, 110, CrossfadeCurve::Power),
                ..region(0)
            },
            Region {
                seq_length: 2,
                seq_position: 2,
                ..region(1)
            },
        ],
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    let first = run(&mut *processor, &[event(EventKind::NoteOn, 60, 50)]);
    close(level(&first), 0.0);
    assert_eq!(processor.report().entries(), [("voices", 0.0)]);
    let second = run(&mut *processor, &[event(EventKind::NoteOn, 62, 50)]);
    close(level(&second), 0.25 * gain(50));
}

#[test]
fn release_samples_follow_the_velocity_crossfade() {
    // A release region fades with the velocity the key was pressed with, like the attack.
    let mut bank = note_with_tail(0.125);
    bank.regions[1].velocity_crossfade = fade_in(70, 90, CrossfadeCurve::Gain);
    for (velocity, tail) in [
        (60, 0.0),
        (85, 0.125 * gain(85) * 0.75),
        (100, 0.125 * gain(100)),
    ] {
        let mut processor = sampler(bank.clone());
        run(&mut *processor, &[event(EventKind::NoteOn, 60, velocity)]);
        let out = run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
        close(level(&out), tail);
    }
}

#[test]
fn round_robin_alternates_between_positions() {
    let rr = |sample, position| Region {
        seq_length: 2,
        seq_position: position,
        ..region(sample)
    };
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.25)],
        regions: vec![rr(0, 1), rr(1, 2)],
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    let mut played = Vec::new();
    for _ in 0..4 {
        run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
        // Let the choke fade finish, then press the key.
        for _ in 0..2 {
            run(&mut *processor, &[]);
        }
        let out = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
        played.push(level(&out));
    }
    close(played[0], 0.5);
    close(played[1], 0.25);
    close(played[2], 0.5);
    close(played[3], 0.25);
}

#[test]
fn key_release_fades_out_and_frees_the_voice() {
    let bank = SampleBank {
        samples: vec![constant(0.5)],
        regions: vec![Region {
            release: 0.01,
            ..region(0)
        }],
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    let held = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    close(level(&held), 0.5);
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
    let faded = run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
    close(level(&faded), 0.0);
    assert!(
        faded.windows(2).all(|w| w[1][0] <= w[0][0] + 1e-12),
        "the fade only falls"
    );
    assert_eq!(processor.report().entries(), [("voices", 0.0)]);
}

#[test]
fn release_trigger_plays_the_tail_of_the_released_note() {
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.125)],
        regions: vec![
            Region {
                release: 0.001,
                ..region(0)
            },
            Region {
                trigger: Trigger::Release,
                ..region(1)
            },
        ],
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 80)]);
    let tail = run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
    // The main voice has faded; the tail plays at the velocity the note was pressed with.
    close(level(&tail), 0.125 * gain(80));
}

/// The sustain pedal (CC 64) pressed (`true`) or lifted.
fn pedal(down: bool) -> InstrumentEvent {
    InstrumentEvent {
        number: 64,
        value: if down { 127 } else { 0 },
        ..event(EventKind::Cc, 0, 0)
    }
}

/// A note with a short release over every key, and a release region on sample 1 (`tail`).
fn note_with_tail(tail: f32) -> SampleBank {
    SampleBank {
        samples: vec![constant(0.5), constant(tail)],
        regions: vec![
            Region {
                release: 0.001,
                ..region(0)
            },
            Region {
                trigger: Trigger::Release,
                ..region(1)
            },
        ],
        ..SampleBank::default()
    }
}

#[test]
fn release_samples_play_when_the_pedal_lifts() {
    // TST-06: a key released under the pedal never started its release regions, so pianos lost
    // their release and resonance tails whenever CC 64 was used.
    let mut processor = sampler(note_with_tail(0.125));
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 80)]);
    let held = run(
        &mut *processor,
        &[pedal(true), event(EventKind::NoteOff, 60, 0)],
    );
    // The pedal holds the note, and the tail waits for the pedal.
    close(level(&held), 0.5 * gain(80));
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
    let lifted = run(&mut *processor, &[pedal(false)]);
    // The note has faded; the tail plays at the velocity the note was pressed with.
    close(level(&lifted), 0.125 * gain(80));
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
}

#[test]
fn a_re_press_under_the_pedal_keeps_the_released_velocity() {
    let mut processor = sampler(note_with_tail(0.125));
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 80)]);
    run(
        &mut *processor,
        &[pedal(true), event(EventKind::NoteOff, 60, 0)],
    );
    // The key is pressed again, harder, and stays down while the pedal lifts.
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    let lifted = run(&mut *processor, &[pedal(false)]);
    // The first note fades; the held one sounds on, with the first note's tail beside it.
    close(level(&lifted), 0.5 + 0.125 * gain(80));
    assert_eq!(processor.report().entries(), [("voices", 2.0)]);
}

#[test]
fn a_key_released_twice_under_the_pedal_plays_one_tail() {
    let mut processor = sampler(note_with_tail(0.125));
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 80)]);
    run(
        &mut *processor,
        &[pedal(true), event(EventKind::NoteOff, 60, 0)],
    );
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 100)]);
    run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
    let lifted = run(&mut *processor, &[pedal(false)]);
    // Both notes fade; the tail is the later release's.
    close(level(&lifted), 0.125 * gain(100));
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
}

#[test]
fn every_key_released_under_the_pedal_plays_its_tail_once() {
    let mut processor = sampler(note_with_tail(0.125));
    run(
        &mut *processor,
        &[
            event(EventKind::NoteOn, 60, 80),
            event(EventKind::NoteOn, 64, 100),
        ],
    );
    run(
        &mut *processor,
        &[
            pedal(true),
            event(EventKind::NoteOff, 60, 0),
            event(EventKind::NoteOff, 64, 0),
        ],
    );
    let tails = 0.125 * (gain(80) + gain(100));
    close(level(&run(&mut *processor, &[pedal(false)])), tails);
    assert_eq!(processor.report().entries(), [("voices", 2.0)]);
    // A second pedal stroke finds no release left to start.
    close(
        level(&run(&mut *processor, &[pedal(true), pedal(false)])),
        tails,
    );
    assert_eq!(processor.report().entries(), [("voices", 2.0)]);
}

#[test]
fn a_key_held_through_the_pedal_starts_no_tail() {
    let mut processor = sampler(note_with_tail(0.125));
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 80)]);
    let out = run(&mut *processor, &[pedal(true), pedal(false)]);
    close(level(&out), 0.5 * gain(80));
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
}

#[test]
fn all_notes_off_forgets_the_releases_under_the_pedal() {
    let mut processor = sampler(note_with_tail(0.125));
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 80)]);
    run(
        &mut *processor,
        &[pedal(true), event(EventKind::NoteOff, 60, 0)],
    );
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    let out = run(&mut *processor, &[pedal(true), pedal(false)]);
    assert!(out.iter().flatten().all(|v| *v == 0.0), "no tail starts");
    assert_eq!(processor.report().entries(), [("voices", 0.0)]);
}

#[test]
fn a_release_under_the_pedal_keeps_the_keyswitch_it_was_released_with() {
    // The tail is the one the release selected, even when a keyswitch changes before the pedal
    // lifts.
    let tail = |sample, switch| Region {
        trigger: Trigger::Release,
        sw_last: Some(switch),
        key_lo: 36,
        ..region(sample)
    };
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.125), constant(0.25)],
        regions: vec![
            Region {
                release: 0.001,
                key_lo: 36,
                ..region(0)
            },
            tail(1, 24),
            tail(2, 25),
        ],
        keyswitches: switches(&[(24, 25)], Some(24)),
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    run(
        &mut *processor,
        &[pedal(true), event(EventKind::NoteOff, 60, 0)],
    );
    run(&mut *processor, &[event(EventKind::NoteOn, 25, 127)]);
    close(level(&run(&mut *processor, &[pedal(false)])), 0.125);
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
}

#[test]
fn each_trigger_starts_on_its_own_press_or_release() {
    // The SFZ format's `trigger` values: `attack` on every press, `first` on a press with no
    // other key down, `legato` on one with another key down; `release` and `release_key` on a
    // release only.
    let starts = |trigger: Trigger| {
        (
            trigger.starts_on_press(false),
            trigger.starts_on_press(true),
            trigger.is_release(),
        )
    };
    assert_eq!(starts(Trigger::Attack), (true, true, false));
    assert_eq!(starts(Trigger::First), (true, false, false));
    assert_eq!(starts(Trigger::Legato), (false, true, false));
    assert_eq!(starts(Trigger::Release), (false, false, true));
    assert_eq!(starts(Trigger::ReleaseKey), (false, false, true));
}

/// A `first` region on sample 0 (0.5) and a `legato` region on sample 1 (0.25) over the keys
/// from 36, with keyswitches on 24 and 25.
fn first_and_legato() -> SampleBank {
    let phrase = |sample, trigger| Region {
        trigger,
        key_lo: 36,
        ..region(sample)
    };
    SampleBank {
        samples: vec![constant(0.5), constant(0.25)],
        regions: vec![phrase(0, Trigger::First), phrase(1, Trigger::Legato)],
        keyswitches: switches(&[(24, 25)], Some(24)),
        ..SampleBank::default()
    }
}

#[test]
fn legato_region_plays_only_while_another_key_is_held() {
    // TST-06: Sonatina's legato patches played both groups on every press.
    let mut processor = sampler(first_and_legato());
    // No other key down: only the `first` region starts.
    let first = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    close(level(&first), 0.5);
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
    // 60 is held: 62 starts only the `legato` region.
    let joined = run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)]);
    close(level(&joined), 0.5 + 0.25);
    assert_eq!(processor.report().entries(), [("voices", 2.0)]);
    // Both keys up: the next press starts a new phrase.
    let released = run(
        &mut *processor,
        &[
            event(EventKind::NoteOff, 60, 0),
            event(EventKind::NoteOff, 62, 0),
        ],
    );
    close(level(&released), 0.0);
    let again = run(&mut *processor, &[event(EventKind::NoteOn, 64, 127)]);
    close(level(&again), 0.5);
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
}

#[test]
fn a_held_keyswitch_does_not_make_a_note_legato() {
    // The switch key 25 is held down while 60 is pressed: 60 still starts the phrase. Unlike
    // sfizz, which counts every key down, a switch key is no note of the phrase.
    let mut processor = sampler(first_and_legato());
    let out = run(
        &mut *processor,
        &[
            event(EventKind::NoteOn, 25, 127),
            event(EventKind::NoteOn, 60, 127),
        ],
    );
    close(level(&out), 0.5);
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
}

#[test]
fn a_key_the_pedal_holds_does_not_make_the_next_note_legato() {
    // 60 is released under the pedal and keeps sounding, but its key is up, so 62 starts a new
    // phrase with its `first` region.
    let mut processor = sampler(first_and_legato());
    run(
        &mut *processor,
        &[pedal(true), event(EventKind::NoteOn, 60, 127)],
    );
    run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
    let out = run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)]);
    close(level(&out), 0.5 + 0.5);
    assert_eq!(processor.report().entries(), [("voices", 2.0)]);
}

#[test]
fn a_key_pressed_again_while_it_is_down_joins_as_legato() {
    // Two presses of 60 without a release between them: the second counts the first, as in
    // sfizz, and starts the `legato` region.
    let mut processor = sampler(first_and_legato());
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    let out = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    close(level(&out), 0.5 + 0.25);
    assert_eq!(processor.report().entries(), [("voices", 2.0)]);
}

#[test]
fn all_notes_off_starts_the_next_phrase_with_a_first_note() {
    // A seek or a loop wrap sends all notes off: the held key is up, so the chased note after
    // it starts with its attack, not as a transition from a note that no longer sounds.
    let mut processor = sampler(first_and_legato());
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    let out = run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)]);
    close(level(&out), 0.5);
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
}

/// A note (0.5, a 1 ms release) with a `release_key` region (0.125) and a `release` region
/// (0.25) over every key.
fn note_with_both_tails() -> SampleBank {
    SampleBank {
        samples: vec![constant(0.5), constant(0.125), constant(0.25)],
        regions: vec![
            Region {
                release: 0.001,
                ..region(0)
            },
            Region {
                trigger: Trigger::ReleaseKey,
                ..region(1)
            },
            Region {
                trigger: Trigger::Release,
                ..region(2)
            },
        ],
        ..SampleBank::default()
    }
}

#[test]
fn release_key_plays_on_note_off_even_with_the_pedal_down() {
    // TST-06: Sonatina's timpani roll endings (`trigger=release_key`) sounded at the press. They
    // start at the key release whatever the pedal does, while the `release` tail waits for the
    // pedal.
    let mut processor = sampler(note_with_both_tails());
    run(
        &mut *processor,
        &[pedal(true), event(EventKind::NoteOn, 60, 80)],
    );
    let pressed = run(&mut *processor, &[]);
    close(level(&pressed), 0.5 * gain(80));
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
    let released = run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
    // The pedal holds the note; the `release_key` tail plays at the note's velocity.
    close(level(&released), (0.5 + 0.125) * gain(80));
    assert_eq!(processor.report().entries(), [("voices", 2.0)]);
    let lifted = run(&mut *processor, &[pedal(false)]);
    // The note fades; the `release` tail starts; the `release_key` tail does not start again.
    close(level(&lifted), (0.125 + 0.25) * gain(80));
    assert_eq!(processor.report().entries(), [("voices", 2.0)]);
}

#[test]
fn release_key_plays_on_note_off_without_the_pedal() {
    let mut processor = sampler(note_with_both_tails());
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 100)]);
    let released = run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
    close(level(&released), (0.125 + 0.25) * gain(100));
    assert_eq!(processor.report().entries(), [("voices", 2.0)]);
}

#[test]
fn a_release_key_tail_plays_on_through_the_next_release_of_its_key() {
    // The tail is the released note's: pressing and releasing the key again starts a second
    // tail beside it and does not fade the first.
    let bank = SampleBank {
        regions: note_with_both_tails()
            .regions
            .into_iter()
            .filter(|region| region.trigger != Trigger::Release)
            .collect(),
        ..note_with_both_tails()
    };
    let mut processor = sampler(bank);
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    let tail = run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
    close(level(&tail), 0.125);
    let pressed = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    close(level(&pressed), 0.5 + 0.125);
    let released = run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
    close(level(&released), 0.125 + 0.125);
    assert_eq!(processor.report().entries(), [("voices", 2.0)]);
}

#[test]
fn a_region_no_key_starts_matches_no_key() {
    // A key of -1 or a controller trigger (TST-06: Salamander's pedal noises sounded on every
    // note): neither its attack nor its release region starts on any of the 128 keys.
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.25)],
        regions: vec![
            Region {
                on_key: false,
                ..region(0)
            },
            Region {
                on_key: false,
                trigger: Trigger::Release,
                ..region(1)
            },
        ],
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    let mut events: Vec<_> = (0..=127)
        .map(|key| event(EventKind::NoteOn, key, 127))
        .collect();
    events.extend((0..=127).map(|key| InstrumentEvent {
        offset: BLOCK / 2,
        ..event(EventKind::NoteOff, key, 0)
    }));
    let out = run(&mut *processor, &events);
    assert!(out.iter().flatten().all(|v| *v == 0.0), "no region sounds");
    assert_eq!(processor.report().entries(), [("voices", 0.0)]);
}

#[test]
fn regions_no_key_starts_leave_the_others_playing() {
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.25), constant(0.125)],
        regions: vec![
            Region {
                release: 0.001,
                ..region(0)
            },
            Region {
                on_key: false,
                ..region(1)
            },
            Region {
                on_key: false,
                trigger: Trigger::Release,
                ..region(2)
            },
        ],
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    let held = run(&mut *processor, &[event(EventKind::NoteOn, 60, 100)]);
    close(level(&held), 0.5 * gain(100));
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
    let released = run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
    close(level(&released), 0.0);
    assert_eq!(
        processor.report().entries(),
        [("voices", 0.0)],
        "the release region does not start"
    );
}

#[test]
fn keyswitch_selects_the_articulation() {
    let articulation = |sample, switch| Region {
        sw_last: Some(switch),
        // Playing keys only: the keyswitch keys themselves make no sound.
        key_lo: 36,
        ..region(sample)
    };
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.25)],
        regions: vec![articulation(0, 24), articulation(1, 25)],
        keyswitches: switches(&[(24, 25)], Some(24)),
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)])),
        0.5,
    );
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    run(&mut *processor, &[event(EventKind::NoteOn, 25, 127)]);
    run(&mut *processor, &[]);
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)])),
        0.25,
    );
}

/// Two articulations on the keys from 36 up, selected by the keyswitch keys 24 (0.5) and 25
/// (0.25), with `default` active before any switch; key 25 also starts a region of its own
/// (0.125), as a press of a switch key that a region covers does.
fn switched_bank(default: Option<u8>) -> SampleBank {
    let articulation = |sample, switch| Region {
        sw_last: Some(switch),
        key_lo: 36,
        ..region(sample)
    };
    SampleBank {
        samples: vec![constant(0.5), constant(0.25), constant(0.125)],
        regions: vec![
            articulation(0, 24),
            articulation(1, 25),
            Region {
                key_lo: 25,
                key_hi: 25,
                ..region(2)
            },
        ],
        keyswitches: switches(&[(24, 25)], default),
        ..SampleBank::default()
    }
}

#[test]
fn all_notes_off_restores_the_default_switch() {
    // All notes off, which the engine sends when playback jumps, makes the patch's default
    // switch active again; the chase then re-sends the last keyswitch before the new position
    // (`a_keyswitch_event_selects_without_sounding`).
    let mut processor = sampler(switched_bank(Some(24)));
    // The default articulation sounds; the switch press changes only the next notes.
    let before = run(
        &mut *processor,
        &[
            event(EventKind::NoteOn, 60, 127),
            event(EventKind::NoteOn, 25, 127),
        ],
    );
    close(level(&before), 0.5 + 0.125);
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)])),
        0.5 + 0.125 + 0.25,
    );
    let silenced = run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    close(level(&silenced), 0.0);
    assert_eq!(processor.report().entries(), [("voices", 0.0)]);
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)])),
        0.5,
    );
}

#[test]
fn all_notes_off_restores_no_switch_where_the_patch_has_no_default() {
    // The oracle is the freshly loaded patch: before any switch no articulation plays.
    let mut fresh = sampler(switched_bank(None));
    let silent = run(&mut *fresh, &[event(EventKind::NoteOn, 60, 127)]);
    close(level(&silent), 0.0);
    let mut processor = sampler(switched_bank(None));
    run(&mut *processor, &[event(EventKind::KeySwitch, 25, 0)]);
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)])),
        0.25,
    );
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    let after = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    assert_eq!(after, silent);
    assert_eq!(processor.report().entries(), [("voices", 0.0)]);
}

#[test]
fn a_keyswitch_event_selects_without_sounding() {
    let mut processor = sampler(switched_bank(Some(24)));
    // A press of key 25 switches and also sounds the region on that key.
    run(&mut *processor, &[event(EventKind::NoteOn, 25, 127)]);
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
    let mut processor = sampler(switched_bank(Some(24)));
    // The keyswitch event only switches: nothing sounds, no voice starts.
    let switched = run(&mut *processor, &[event(EventKind::KeySwitch, 25, 0)]);
    assert!(switched.iter().flatten().all(|v| *v == 0.0), "{switched:?}");
    assert_eq!(processor.report().entries(), [("voices", 0.0)]);
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)])),
        0.25,
    );
    // A chase in one block: all notes off, the last switch, then the chased note.
    let chased = run(
        &mut *processor,
        &[
            event(EventKind::AllNotesOff, 0, 0),
            event(EventKind::KeySwitch, 25, 0),
            event(EventKind::NoteOn, 62, 127),
        ],
    );
    close(level(&chased), 0.25);
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
}

#[test]
fn a_keyswitch_event_of_another_key_changes_nothing() {
    let mut processor = sampler(switched_bank(Some(24)));
    run(&mut *processor, &[event(EventKind::KeySwitch, 25, 0)]);
    // Key 60 is no keyswitch key of the patch: the switch stays at 25, and nothing sounds.
    let ignored = run(&mut *processor, &[event(EventKind::KeySwitch, 60, 0)]);
    assert!(ignored.iter().flatten().all(|v| *v == 0.0));
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)])),
        0.25,
    );
    // A sampler without an instrument ignores it too.
    let mut empty = create("sonara.sampler").expect("the sampler exists");
    empty.prepare(ProcessSpec {
        sample_rate: RATE,
        block_size: BLOCK,
        channels: 2,
    });
    empty.set_params(&param_slots(&descriptor(), &Params::new()).expect("valid parameters"));
    let out = run(&mut *empty, &[event(EventKind::KeySwitch, 25, 0)]);
    assert!(out.iter().flatten().all(|v| *v == 0.0));
}

#[test]
fn an_empty_key_range_never_plays() {
    // Pins the current behaviour: a region whose lowest key lies above its highest matches no
    // key, neither its attack on a press nor its release region on a release; the others play.
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.25), constant(0.125)],
        regions: vec![
            Region {
                key_lo: 61,
                key_hi: 60,
                ..region(0)
            },
            Region {
                key_lo: 61,
                key_hi: 60,
                trigger: Trigger::Release,
                ..region(1)
            },
            Region {
                key_lo: 72,
                key_hi: 72,
                ..region(2)
            },
        ],
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    let mut events: Vec<_> = (0..=127)
        .filter(|key| *key != 72)
        .map(|key| event(EventKind::NoteOn, key, 127))
        .collect();
    events.extend(
        (0..=127)
            .filter(|key| *key != 72)
            .map(|key| InstrumentEvent {
                offset: BLOCK / 2,
                ..event(EventKind::NoteOff, key, 0)
            }),
    );
    let out = run(&mut *processor, &events);
    assert!(out.iter().flatten().all(|v| *v == 0.0), "no region sounds");
    assert_eq!(processor.report().entries(), [("voices", 0.0)]);
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 72, 127)])),
        0.125,
    );
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
}

/// Keyswitches on the keys of `ranges` (inclusive) with the default switch `default`.
fn switches(ranges: &[(u8, u8)], default: Option<u8>) -> Keyswitches {
    let mut switches = Keyswitches::NONE;
    switches.default = default;
    for (lo, hi) in ranges {
        switches.add_range(*lo, *hi);
    }
    switches
}

#[test]
fn keys_between_disjoint_switch_ranges_play_notes() {
    // Switch keys 24-25 and 30-31; the regions play on 26-29, between the ranges.
    let articulation = |sample, switch| Region {
        sw_last: Some(switch),
        key_lo: 26,
        key_hi: 29,
        ..region(sample)
    };
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.25)],
        regions: vec![articulation(0, 24), articulation(1, 30)],
        keyswitches: switches(&[(24, 25), (30, 31)], Some(24)),
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    // Key 27 lies inside 24..=31 but in neither range: it plays the default articulation
    // instead of switching to a switch no region answers.
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 27, 127)])),
        0.5,
    );
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    let switched = run(&mut *processor, &[event(EventKind::NoteOn, 30, 127)]);
    close(level(&switched), 0.0);
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 28, 127)])),
        0.25,
    );
}

#[test]
fn a_default_switch_without_switch_keys_selects_its_articulation() {
    let articulation = |sample, switch| Region {
        sw_last: Some(switch),
        ..region(sample)
    };
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.25)],
        regions: vec![articulation(0, 24), articulation(1, 25)],
        keyswitches: switches(&[], Some(25)),
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)])),
        0.25,
    );
    // Key 24 is no switch key here: it plays as a note and leaves the switch alone.
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    run(&mut *processor, &[event(EventKind::NoteOn, 24, 127)]);
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)])),
        0.25,
    );
}

#[test]
fn keyswitch_ranges_keep_only_midi_keys() {
    let mut keys = switches(&[(120, 200), (10, 5)], None);
    assert_eq!(
        keys.keys().collect::<Vec<_>>(),
        (120..=127).collect::<Vec<_>>()
    );
    assert!(!keys.contains(200), "200 is no MIDI key");
    keys.add_range(0, 0);
    assert_eq!(keys.keys().next(), Some(0));
    assert_eq!(Keyswitches::default(), Keyswitches::NONE);
}

#[test]
fn exclusive_group_chokes_the_open_note() {
    let bank = SampleBank {
        samples: vec![constant(0.5)],
        regions: vec![Region {
            group: 1,
            off_by: 1,
            ..region(0)
        }],
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    let out = run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)]);
    // The first voice fades within CHOKE_RELEASE (10 ms); one voice is left.
    close(level(&out), 0.5);
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
}

/// A sample without frames: its region starts no voice but still switches off its group.
fn silent() -> Sample {
    Sample {
        rate: RATE,
        channels: 1,
        data: Vec::new(),
    }
}

/// Key 60 plays 0.5 with `release` and is switched off by group 1 as `off_mode` says; key 62
/// is a silent region of group 1, so after its press the output is the old note's fade alone.
fn exclusive(off_mode: OffMode, release: f64) -> SampleBank {
    SampleBank {
        samples: vec![constant(0.5), silent()],
        regions: vec![
            Region {
                key_lo: 60,
                key_hi: 60,
                off_by: 1,
                off_mode,
                release,
                ..region(0)
            },
            Region {
                key_lo: 62,
                key_hi: 62,
                group: 1,
                ..region(1)
            },
        ],
        ..SampleBank::default()
    }
}

/// The output of a full-level 0.5 voice `frame` frames into a linear fade-out of `seconds`
/// that starts at the block's first frame: the level falls by `1 / (seconds · rate)` per frame.
fn faded(seconds: f64, frame: u32) -> f64 {
    0.5 * (1.0 - f64::from(frame + 1) / (seconds * f64::from(RATE))).max(0.0)
}

/// Checks every frame of `block` against [`faded`] over `seconds`.
fn assert_fades_over(block: &[[f64; 2]], seconds: f64) {
    for (frame, out) in (0_u32..).zip(block) {
        assert!(
            (out[0] - faded(seconds, frame)).abs() < 1e-9,
            "frame {frame}: {} != {}",
            out[0],
            faded(seconds, frame)
        );
    }
}

/// Presses key 60 in `processor` and lets it settle, then presses key 62 and returns that block.
fn switch_off(processor: &mut dyn Processor) -> Vec<[f64; 2]> {
    close(
        level(&run(processor, &[event(EventKind::NoteOn, 60, 127)])),
        0.5,
    );
    run(processor, &[event(EventKind::NoteOn, 62, 127)])
}

/// The parameters with the release scale at `scale`.
fn release_scale(scale: f64) -> Params {
    let mut params = Params::new();
    params.insert("release_scale".to_owned(), ParamValue::Float(scale));
    params
}

#[test]
fn off_mode_fast_keeps_the_quick_choke() {
    // The default: 10 ms (480 frames), whatever the region's release.
    let mut processor = sampler(exclusive(OffMode::Fast, 0.5));
    let out = switch_off(&mut *processor);
    assert_fades_over(&out, 0.01);
    close(out[239][0], 0.25);
    assert_eq!(processor.report().entries(), [("voices", 0.0)]);
}

#[test]
fn off_mode_normal_chokes_over_the_region_release() {
    // `normal` puts the voice into its release, exactly as a key release does: 20 ms here.
    let bank = exclusive(OffMode::Normal, 0.02);
    let mut processor = sampler(bank.clone());
    let out = switch_off(&mut *processor);
    assert_fades_over(&out, 0.02);
    close(out[479][0], 0.25);
    assert_eq!(processor.report().entries(), [("voices", 0.0)]);

    let mut released = sampler(bank.clone());
    run(&mut *released, &[event(EventKind::NoteOn, 60, 127)]);
    let key_release = run(&mut *released, &[event(EventKind::NoteOff, 60, 0)]);
    assert_eq!(out, key_release, "the same fade as the key release");

    // The release parameter scales it like any release: twice as long.
    let mut scaled = sampler_with(bank, &release_scale(2.0));
    assert_fades_over(&switch_off(&mut *scaled), 0.04);
}

#[test]
fn off_mode_time_fades_over_off_time() {
    // `time` fades over `off_time` (40 ms, 1920 frames), neither the region's 1 ms release nor
    // the 10 ms choke, and the release parameter leaves it alone.
    let bank = exclusive(OffMode::Time(0.04), 0.001);
    let mut processor = sampler(bank.clone());
    let out = switch_off(&mut *processor);
    assert_fades_over(&out, 0.04);
    close(out[959][0], 0.25);
    assert_eq!(processor.report().entries(), [("voices", 0.0)]);

    let mut scaled = sampler_with(bank, &release_scale(4.0));
    assert_fades_over(&switch_off(&mut *scaled), 0.04);
}

#[test]
fn a_zero_off_time_still_fades_without_a_click() {
    // The shortest fade-out of the sampler, 5 ms (240 frames), as for a release of 0.
    let mut processor = sampler(exclusive(OffMode::Time(0.0), 0.0));
    assert_fades_over(&switch_off(&mut *processor), 0.005);
}

#[test]
fn a_switch_off_never_lengthens_a_release_under_way() {
    // TST-07: the key's 10 ms release has begun when the group switches the voice off with a
    // 1 s `off_time`; the release keeps its pace and ends at frame 480.
    let mut processor = sampler(exclusive(OffMode::Time(1.0), 0.01));
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    let out = run(
        &mut *processor,
        &[
            event(EventKind::NoteOff, 60, 0),
            InstrumentEvent {
                offset: 100,
                ..event(EventKind::NoteOn, 62, 127)
            },
        ],
    );
    assert_fades_over(&out, 0.01);
    assert_eq!(processor.report().entries(), [("voices", 0.0)]);
}

#[test]
fn the_pedal_does_not_hold_a_switched_off_voice() {
    // A key released under the pedal is switched off with a 40 ms `off_time`; lifting the pedal
    // 100 frames later does not cut that fade to the region's 5 ms release.
    let mut processor = sampler(exclusive(OffMode::Time(0.04), 0.005));
    run(
        &mut *processor,
        &[pedal(true), event(EventKind::NoteOn, 60, 127)],
    );
    let held = run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
    close(level(&held), 0.5);
    let out = run(
        &mut *processor,
        &[
            event(EventKind::NoteOn, 62, 127),
            InstrumentEvent {
                offset: 100,
                ..pedal(false)
            },
        ],
    );
    assert_fades_over(&out, 0.04);
}

#[test]
fn a_timed_switch_off_overlaps_the_new_note() {
    // Sonatina's legato layout: every region is in group 1 and switched off by it with a timed
    // fade, so the old note fades out over `off_time` while the new one plays at full level.
    let bank = SampleBank {
        samples: vec![constant(0.5)],
        regions: vec![Region {
            group: 1,
            off_by: 1,
            off_mode: OffMode::Time(0.04),
            ..region(0)
        }],
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    let out = run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)]);
    // The new voice's 0.5 ms attack (24 frames) is over by frame 959; the old voice is at half.
    close(out[959][0], 0.5 + faded(0.04, 959));
    close(out[959][0], 0.75);
    close(level(&out), 0.5);
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
}

#[test]
fn pitch_follows_the_key_and_loops_hold() {
    // A ramp that loops: one octave up plays it twice as fast, so the loop wraps twice as often.
    let ramp = Sample {
        rate: RATE,
        channels: 1,
        data: (0..100_u8).map(|i| f32::from(i) / 100.0).collect(),
    };
    let bank = SampleBank {
        samples: vec![ramp],
        regions: vec![Region {
            loop_mode: LoopMode::Continuous,
            loop_start: 0,
            loop_end: 100,
            end: 100,
            ..region(0)
        }],
        ..SampleBank::default()
    };
    let wraps = |key| {
        let mut processor = sampler(bank.clone());
        let out = run(&mut *processor, &[event(EventKind::NoteOn, key, 127)]);
        out.windows(2).filter(|w| w[1][0] < w[0][0] - 0.5).count()
    };
    let (base, octave) = (wraps(60), wraps(72));
    assert!(base >= 15, "a 100-frame loop wraps often: {base}");
    assert!(
        octave.abs_diff(2 * base) <= 2,
        "an octave up wraps twice as often: {base} vs {octave}"
    );
}

#[test]
fn polyphony_steals_the_oldest_voice_and_never_clips() {
    let bank = SampleBank {
        samples: vec![constant(0.01)],
        regions: vec![region(0)],
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    let presses: Vec<_> = (0..80_u8)
        .map(|i| event(EventKind::NoteOn, 30 + i % 60, 127))
        .collect();
    let out = run(&mut *processor, &presses);
    assert_eq!(processor.report().entries(), [("voices", 64.0)]);
    let peak = out.iter().flatten().fold(0.0_f64, |p, v| p.max(v.abs()));
    assert!(peak <= 0.64 + 1e-9 && peak > 0.5, "peak {peak}");
}

#[test]
fn an_empty_sampler_plays_silence() {
    let mut processor = create("sonara.sampler").expect("the sampler exists");
    processor.prepare(ProcessSpec {
        sample_rate: RATE,
        block_size: BLOCK,
        channels: 2,
    });
    let slots = param_slots(&descriptor(), &Params::new()).expect("defaults");
    processor.set_params(&slots);
    let out = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    assert!(out.iter().flatten().all(|v| *v == 0.0));
}

// --- Controllers: the table, gates, triggers, crossfades and gains ---

/// Controller `number` set to `value` on `channel`.
fn cc(number: u8, value: i32, channel: u8) -> InstrumentEvent {
    InstrumentEvent {
        number,
        value,
        channel,
        ..event(EventKind::Cc, 0, 0)
    }
}

/// A key press on `channel`.
fn press(key: u8, velocity: u8, channel: u8) -> InstrumentEvent {
    InstrumentEvent {
        channel,
        ..event(EventKind::NoteOn, key, velocity)
    }
}

/// One range of `controller`, as a gate or trigger list.
fn range(controller: u8, lo: u8, hi: u8) -> Box<[CcRange]> {
    Box::new([CcRange { controller, lo, hi }])
}

/// Two layers gated by the sustain pedal: sample 0 (0.5) while it is up (0-63), sample 1
/// (0.25) while it is down (64-127). Velocity does not change their level.
fn pedal_layers() -> SampleBank {
    SampleBank {
        samples: vec![constant(0.5), constant(0.25)],
        regions: vec![
            Region {
                cc_gates: range(64, 0, 63),
                amp_veltrack: 0.0,
                ..region(0)
            },
            Region {
                cc_gates: range(64, 64, 127),
                amp_veltrack: 0.0,
                ..region(1)
            },
        ],
        ..SampleBank::default()
    }
}

/// The number of sounding voices.
fn voices(processor: &dyn Processor) -> f64 {
    processor
        .report()
        .entries()
        .iter()
        .find(|(name, _)| *name == "voices")
        .map(|(_, value)| *value)
        .expect("the sampler reports its voices")
}

#[test]
fn controllers_start_at_zero_except_volume_and_expression() {
    let initial = Controllers::INITIAL;
    assert_eq!(
        (0..=127)
            .filter(|n| initial.value(*n) != 0)
            .collect::<Vec<u8>>(),
        [7, 11]
    );
    assert_eq!((initial.value(7), initial.value(11)), (127, 127));
    assert_eq!(SampleBank::default().controllers, initial);
    // A value above 127 is read as 127; a number above 127 names no controller.
    let mut controllers = initial;
    controllers.set(1, 200);
    controllers.set(200, 5);
    assert_eq!((controllers.value(1), controllers.value(200)), (127, 0));
}

#[test]
fn cc_gate_selects_the_layer() {
    // TST-06: gated regions used to play whatever the controller said, so pedal-up and
    // pedal-down layers both sounded on every note.
    let mut processor = sampler(pedal_layers());
    let up = run(&mut *processor, &[event(EventKind::NoteOn, 60, 100)]);
    close(level(&up), 0.5);
    assert_eq!(voices(&*processor), 1.0);
    // The pedal goes down: the next press plays the other layer, and the held voice sounds on,
    // since a gate is read only when a region starts.
    let down = run(
        &mut *processor,
        &[pedal(true), event(EventKind::NoteOn, 62, 100)],
    );
    close(level(&down), 0.5 + 0.25);
    assert_eq!(voices(&*processor), 2.0);
}

#[test]
fn a_gate_reads_the_controllers_of_the_notes_channel() {
    let mut processor = sampler(pedal_layers());
    let out = run(
        &mut *processor,
        &[cc(64, 127, 3), press(60, 100, 3), press(62, 100, 0)],
    );
    // Channel 3 has its pedal down, channel 0 does not.
    close(level(&out), 0.25 + 0.5);
    assert_eq!(voices(&*processor), 2.0);
}

#[test]
fn every_gate_of_a_region_must_be_open() {
    let gated = Region {
        cc_gates: Box::new([
            CcRange {
                controller: 1,
                lo: 10,
                hi: 20,
            },
            CcRange {
                controller: 2,
                lo: 0,
                hi: 63,
            },
        ]),
        ..region(0)
    };
    let mut controllers = Controllers::INITIAL;
    assert!(!gated.gates_open(&controllers), "controller 1 is at 0");
    controllers.set(1, 15);
    assert!(gated.gates_open(&controllers));
    controllers.set(2, 64);
    assert!(!gated.gates_open(&controllers));
    // A range whose high end is below its low end holds no value; no gate is always open.
    let inverted = Region {
        cc_gates: range(1, 20, 10),
        ..region(0)
    };
    assert!((0..=127).all(|value| {
        controllers.set(1, value);
        !inverted.gates_open(&controllers)
    }));
    assert!(Region::default().gates_open(&controllers));
}

#[test]
fn the_controllers_start_where_the_patch_sets_them() {
    // `set_cc64=100`: the pedal-down layer plays until the controller moves, on every channel,
    // and a reset brings every channel back there.
    let mut bank = pedal_layers();
    bank.controllers.set(64, 100);
    let mut processor = sampler(bank);
    let first = run(&mut *processor, &[press(60, 100, 5)]);
    close(level(&first), 0.25);
    let moved = run(&mut *processor, &[cc(64, 0, 5), press(62, 100, 5)]);
    close(level(&moved), 0.25 + 0.5);
    assert_no_alloc(|| processor.reset());
    let after_reset = run(&mut *processor, &[press(60, 100, 5)]);
    close(level(&after_reset), 0.25);
}

#[test]
fn all_notes_off_lifts_the_pedal_and_keeps_the_other_controllers() {
    // Sample 0 needs the pedal down, sample 1 controller 1 at 100 or more.
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.25)],
        regions: vec![
            Region {
                cc_gates: range(64, 64, 127),
                amp_veltrack: 0.0,
                ..region(0)
            },
            Region {
                cc_gates: range(1, 100, 127),
                amp_veltrack: 0.0,
                ..region(1)
            },
        ],
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    run(
        &mut *processor,
        &[
            pedal(true),
            cc(1, 110, 0),
            event(EventKind::AllNotesOff, 0, 0),
        ],
    );
    let out = run(&mut *processor, &[event(EventKind::NoteOn, 60, 100)]);
    close(level(&out), 0.25);
}

#[test]
fn a_gated_release_region_reads_the_controllers_when_it_starts() {
    // The tail needs controller 2 at 64 or more. It is read at the release, and for a key
    // released under the pedal at the pedal's lift.
    let mut bank = note_with_tail(0.125);
    bank.regions[1].cc_gates = range(2, 64, 127);
    let tail = 0.125 * gain(80);
    let mut closed = sampler(bank.clone());
    run(&mut *closed, &[event(EventKind::NoteOn, 60, 80)]);
    let out = run(&mut *closed, &[event(EventKind::NoteOff, 60, 0)]);
    close(level(&out), 0.0);
    let mut open = sampler(bank.clone());
    run(&mut *open, &[event(EventKind::NoteOn, 60, 80)]);
    let out = run(
        &mut *open,
        &[cc(2, 127, 0), event(EventKind::NoteOff, 60, 0)],
    );
    close(level(&out), tail);
    let mut pedalled = sampler(bank);
    run(&mut *pedalled, &[event(EventKind::NoteOn, 60, 80)]);
    run(
        &mut *pedalled,
        &[pedal(true), event(EventKind::NoteOff, 60, 0)],
    );
    let out = run(&mut *pedalled, &[cc(2, 127, 0), pedal(false)]);
    close(level(&out), tail);
}

/// Salamander's pedal noises: a pedal-down noise on sample 0 (0.5) or 1 (0.25), chosen by
/// `lorand`/`hirand`, which controller 64 at 127 starts, and a pedal-up noise on sample 2
/// (0.125), which controller 64 at 0 starts. No key starts them; their keycenter is 48.
fn pedal_noises() -> SampleBank {
    let noise = |sample, value, rand_lo, rand_hi| Region {
        on_key: false,
        cc_triggers: range(64, value, value),
        cc_gates: range(64, value, value),
        rand_lo,
        rand_hi,
        pitch_keycenter: 48,
        ..region(sample)
    };
    SampleBank {
        samples: vec![constant(0.5), constant(0.25), constant(0.125)],
        regions: vec![
            noise(0, 127, 0.0, 0.5),
            noise(1, 127, 0.5, 1.0),
            noise(2, 0, 0.0, 1.0),
        ],
        ..SampleBank::default()
    }
}

#[test]
fn on_locc_region_plays_on_pedal_down() {
    // TST-06: Salamander's pedal noises used to sound on every key and never on the pedal.
    let mut processor = sampler(pedal_noises());
    let down = run(&mut *processor, &[pedal(true)]);
    // One of the two pedal-down noises, at its full level: no velocity scales it.
    let noise = level(&down);
    assert!(
        (noise - 0.5).abs() < 1e-9 || (noise - 0.25).abs() < 1e-9,
        "one noise: {noise}"
    );
    assert_eq!(voices(&*processor), 1.0);
    // The pedal goes up: the pedal-up noise starts beside it.
    let up = run(&mut *processor, &[pedal(false)]);
    close(level(&up), noise + 0.125);
    assert_eq!(voices(&*processor), 2.0);
    // A key press starts none of them, and its release ends none of them.
    let note = run(
        &mut *processor,
        &[
            event(EventKind::NoteOn, 60, 100),
            InstrumentEvent {
                offset: BLOCK / 2,
                ..event(EventKind::NoteOff, 60, 0)
            },
        ],
    );
    close(level(&note), noise + 0.125);
    assert_eq!(voices(&*processor), 2.0);
}

#[test]
fn every_controller_event_in_the_range_starts_the_region() {
    // As the SFZ format reads `on_loccN`: a second pedal-down event starts the noise again, and
    // all notes off ends what a controller started.
    let mut processor = sampler(pedal_noises());
    run(&mut *processor, &[pedal(true)]);
    run(&mut *processor, &[pedal(true)]);
    assert_eq!(voices(&*processor), 2.0);
    let silenced = run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    close(level(&silenced), 0.0);
    assert_eq!(voices(&*processor), 0.0);
}

#[test]
fn the_random_range_chooses_one_controller_region() {
    // Over several presses of the pedal both down noises play, never both at once.
    let mut processor = sampler(pedal_noises());
    let mut heard = Vec::new();
    for _ in 0..16 {
        run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
        heard.push(level(&run(&mut *processor, &[pedal(true)])));
        assert_eq!(voices(&*processor), 1.0);
    }
    let loud = heard.iter().filter(|l| (*l - 0.5).abs() < 1e-9).count();
    let soft = heard.iter().filter(|l| (*l - 0.25).abs() < 1e-9).count();
    assert_eq!(loud + soft, heard.len(), "{heard:?}");
    assert!(loud > 0 && soft > 0, "{heard:?}");
}

#[test]
fn a_controller_region_plays_at_its_keycenter() {
    // A ramp at the output rate: at its own pitch the voice reads one source frame per output
    // frame, whatever key was pressed last.
    let ramp = Sample {
        rate: RATE,
        channels: 1,
        data: (0..4_800_u16).map(|i| f32::from(i) / 4_800.0).collect(),
    };
    let bank = SampleBank {
        samples: vec![ramp.clone()],
        regions: vec![Region {
            on_key: false,
            cc_triggers: range(64, 127, 127),
            pitch_keycenter: 48,
            end: 4_800,
            ..region(0)
        }],
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    let out = run(
        &mut *processor,
        &[event(EventKind::NoteOn, 72, 127), pedal(true)],
    );
    for frame in [100, 1_000, 2_000] {
        close(out[frame][0], f64::from(ramp.data[frame]));
    }
}

#[test]
fn closed_gates_and_other_switches_keep_a_controller_region_silent() {
    // The pedal-down noise needs controller 1 at 64 or more and the keyswitch 24.
    let bank = SampleBank {
        samples: vec![constant(0.5)],
        regions: vec![Region {
            on_key: false,
            cc_triggers: range(64, 127, 127),
            cc_gates: range(1, 64, 127),
            sw_last: Some(24),
            ..region(0)
        }],
        keyswitches: switches(&[(24, 25)], Some(25)),
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    run(&mut *processor, &[pedal(true)]);
    assert_eq!(voices(&*processor), 0.0, "controller 1 is at 0");
    run(&mut *processor, &[cc(1, 100, 0), pedal(true)]);
    assert_eq!(voices(&*processor), 0.0, "switch 25 is active");
    let out = run(
        &mut *processor,
        &[event(EventKind::NoteOn, 24, 100), pedal(true)],
    );
    close(level(&out), 0.5);
}

#[test]
fn controllers_leave_the_random_choices_of_notes_alone() {
    // Two layers split by `lorand`/`hirand`: a patch without controller triggers draws its
    // random numbers for key presses only, so controller events between the notes change none
    // of its choices.
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.25)],
        regions: vec![
            Region {
                rand_hi: 0.5,
                release: 0.001,
                ..region(0)
            },
            Region {
                rand_lo: 0.5,
                release: 0.001,
                ..region(1)
            },
        ],
        ..SampleBank::default()
    };
    let notes = |with_controllers: bool| {
        let mut processor = sampler(bank.clone());
        (0..12)
            .map(|i| {
                if with_controllers {
                    run(&mut *processor, &[cc(1, i * 10, 0), pedal(false)]);
                }
                let out = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
                run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
                level(&out)
            })
            .collect::<Vec<_>>()
    };
    let plain = notes(false);
    assert!(
        plain.iter().any(|l| (*l - 0.5).abs() < 1e-9)
            && plain.iter().any(|l| (*l - 0.25).abs() < 1e-9),
        "both layers play: {plain:?}"
    );
    assert_eq!(notes(true), plain);
}

/// A fade with controller 1 over 0 to 127, in (`true`) or out, with `curve`.
fn cc_fade(fade_in: bool, curve: CrossfadeCurve) -> Box<[CcCrossfade]> {
    let fade = if fade_in {
        Crossfade {
            in_lo: 0,
            in_hi: 127,
            curve,
            ..Crossfade::NONE
        }
    } else {
        Crossfade {
            out_lo: 0,
            out_hi: 127,
            curve,
            ..Crossfade::NONE
        }
    };
    Box::new([CcCrossfade {
        controller: 1,
        fade,
    }])
}

#[test]
fn cc_crossfade_follows_the_controller() {
    // Dynamics on the modulation wheel (Sonatina's choirs): on a held note the soft layer
    // (sample 0) fades out and the loud one (sample 1) in as controller 1 rises.
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.25)],
        regions: vec![
            Region {
                cc_crossfades: cc_fade(false, CrossfadeCurve::Power),
                amp_veltrack: 0.0,
                ..region(0)
            },
            Region {
                cc_crossfades: cc_fade(true, CrossfadeCurve::Power),
                amp_veltrack: 0.0,
                ..region(1)
            },
        ],
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    let start = run(&mut *processor, &[event(EventKind::NoteOn, 60, 100)]);
    // Controller 1 starts at 0: the soft layer alone; the loud layer's voice starts silent, so
    // that the controller can raise it.
    close(level(&start), 0.5);
    assert_eq!(voices(&*processor), 2.0);
    for value in [32, 64, 127, 0] {
        // A move within a block reaches the sounding voices from the next block.
        run(&mut *processor, &[cc(1, value, 0)]);
        let out = run(&mut *processor, &[]);
        let x = f64::from(value) / 127.0;
        close(level(&out), 0.5 * (1.0 - x).sqrt() + 0.25 * x.sqrt());
    }
    assert_eq!(voices(&*processor), 2.0);
}

#[test]
fn cc_crossfade_is_block_continuous() {
    // A layer fading in with controller 1 (gain curve): a jump of the controller from 32 to 127
    // ramps linearly across the next block, from the old gain to the new one on its last frame,
    // so it never clicks.
    let bank = SampleBank {
        samples: vec![constant(0.5)],
        regions: vec![Region {
            cc_crossfades: cc_fade(true, CrossfadeCurve::Gain),
            amp_veltrack: 0.0,
            ..region(0)
        }],
        ..SampleBank::default()
    };
    let mut processor = sampler(bank);
    let from = 32.0 / 127.0;
    let start = run(
        &mut *processor,
        &[cc(1, 32, 0), event(EventKind::NoteOn, 60, 100)],
    );
    close(level(&start), 0.5 * from);
    let held = run(&mut *processor, &[cc(1, 127, 0)]);
    for frame in &held {
        close(frame[0], 0.5 * from);
    }
    let ramp = run(&mut *processor, &[]);
    for (i, frame) in (1..=BLOCK).zip(&ramp) {
        let at = from + (1.0 - from) * f64::from(u32::try_from(i).expect("a block")) / 2_048.0;
        close(frame[0], 0.5 * at);
    }
    close(level(&ramp), 0.5);
    let after = run(&mut *processor, &[]);
    for frame in &after {
        close(frame[0], 0.5);
    }
}

#[test]
fn cc_volume_changes_the_level_in_db() {
    // `volume_onccN` (`gain_ccN`): -12 dB at 127, in proportion to the value.
    let bank = SampleBank {
        samples: vec![constant(0.5)],
        regions: vec![Region {
            cc_volume: Box::new([CcDepth {
                controller: 1,
                depth: -12.0,
            }]),
            amp_veltrack: 0.0,
            ..region(0)
        }],
        ..SampleBank::default()
    };
    for value in [0, 64, 127] {
        let mut processor = sampler(bank.clone());
        let out = run(
            &mut *processor,
            &[cc(1, value, 0), event(EventKind::NoteOn, 60, 100)],
        );
        let db = -12.0 * f64::from(value) / 127.0;
        close(level(&out), 0.5 * 10_f64.powf(db / 20.0));
    }
}

#[test]
fn cc_amplitude_scales_the_level() {
    // `amplitude_onccN`: a harpsichord's release-level control (`amplitude_oncc12=100`) is
    // silent at 0 and full at 127; 50 % halves the level at 127. A voice starts even when
    // silent.
    for (depth, value) in [(100.0, 0), (100.0, 64), (100.0, 127), (50.0, 127)] {
        let bank = SampleBank {
            samples: vec![constant(0.5)],
            regions: vec![Region {
                cc_amplitude: Box::new([CcDepth {
                    controller: 12,
                    depth,
                }]),
                amp_veltrack: 0.0,
                ..region(0)
            }],
            ..SampleBank::default()
        };
        let mut processor = sampler(bank);
        let out = run(
            &mut *processor,
            &[cc(12, value, 0), event(EventKind::NoteOn, 60, 100)],
        );
        close(level(&out), 0.5 * depth / 100.0 * f64::from(value) / 127.0);
        assert_eq!(voices(&*processor), 1.0);
    }
}

#[test]
fn controller_gains_multiply() {
    let both = |controller, fade_in| CcCrossfade {
        controller,
        fade: if fade_in {
            Crossfade {
                in_lo: 0,
                in_hi: 127,
                curve: CrossfadeCurve::Gain,
                ..Crossfade::NONE
            }
        } else {
            Crossfade {
                out_lo: 0,
                out_hi: 127,
                curve: CrossfadeCurve::Gain,
                ..Crossfade::NONE
            }
        },
    };
    let depth = |controller, depth| CcDepth { controller, depth };
    let modulated = Region {
        cc_crossfades: Box::new([both(1, true), both(2, false)]),
        cc_amplitude: Box::new([depth(12, 100.0), depth(13, 50.0)]),
        cc_volume: Box::new([depth(1, 6.0), depth(2, -6.0)]),
        ..region(0)
    };
    let mut controllers = Controllers::INITIAL;
    for (number, value) in [(1, 100), (2, 27), (12, 127), (13, 64)] {
        controllers.set(number, value);
    }
    let v = |n: u8| f64::from(controllers.value(n)) / 127.0;
    let crossfades = v(1) * (1.0 - v(2));
    let amplitude = v(12) * 0.5 * v(13);
    let db = 6.0 * v(1) - 6.0 * v(2);
    close(
        modulated.controller_gain(&controllers),
        crossfades * amplitude * 10_f64.powf(db / 20.0),
    );
    // A region no controller changes gains exactly 1.
    assert_eq!(Region::default().controller_gain(&controllers), 1.0);
}

#[test]
fn the_amplitude_opcode_scales_the_region() {
    for (amplitude, voices_started) in [(100.0, 1.0), (50.0, 1.0), (0.0, 0.0)] {
        let bank = SampleBank {
            samples: vec![constant(0.5)],
            regions: vec![Region {
                amplitude,
                ..region(0)
            }],
            ..SampleBank::default()
        };
        let mut processor = sampler(bank);
        let out = run(&mut *processor, &[event(EventKind::NoteOn, 60, 100)]);
        close(level(&out), 0.5 * amplitude / 100.0 * gain(100));
        assert_eq!(voices(&*processor), voices_started, "{amplitude} %");
    }
}

/// The target that makes keyswitch `key` active.
fn switch(key: u8) -> ArticulationTarget {
    ArticulationTarget {
        member: 0,
        selector: Some(Selector::Keyswitch(key)),
        legato: false,
    }
}

#[test]
fn a_mapping_looks_up_each_choice_without_allocating() {
    let mapping = SamplerMapping::new(
        vec![None, Some(switch(24)), None, Some(switch(25))],
        DynamicsDrive::Velocity,
        None,
        0,
    );
    // The render thread reads the targets (RT-02).
    let looked_up = assert_no_alloc(|| {
        [
            mapping.target(SamplerMapping::AUTO),
            mapping.target(SamplerMapping::ORDINARIO),
            mapping.default_target(),
            mapping.target(2),
            mapping.target(3),
            mapping.target(4),
            mapping.target(u16::MAX),
        ]
    });
    assert_eq!(
        looked_up,
        [
            None,
            Some(switch(24)),
            Some(switch(24)),
            None,
            Some(switch(25)),
            None,
            None
        ],
        "auto, ordinario, the default, a choice without a target, a target, beyond the table"
    );
}

/// The target of a cell of the tables below: the switch `24 + choice`, moved by 10 under the
/// mute choice 2 and by 20 under the vibrato choice 1; none for the choice 3, and for `auto` (0)
/// but under the mute choice 2.
fn cell(choice: u16, passage: Passage) -> Option<ArticulationTarget> {
    let key = 24 + u8::try_from(choice).expect("a small choice");
    let muted = passage.mute == 2;
    let moved = key + if muted { 10 } else { 0 } + 20 * u8::from(passage.vibrato == 1);
    (choice != 3 && (choice != 0 || muted)).then(|| switch(moved))
}

#[test]
fn a_table_looks_up_each_cell_without_allocating() {
    // Four articulation choices under three mute choices and two vibrato choices.
    let table = TargetTable::tabulate(4, 3, 2, cell);
    let mapping = SamplerMapping::new(table, DynamicsDrive::Velocity, None, 0);
    let under = |mute, vibrato| Passage { mute, vibrato };
    let looked_up = assert_no_alloc(|| {
        [
            // `auto` under `auto` has no target, whatever the cell said; under a state it has.
            mapping.target(SamplerMapping::AUTO),
            mapping.target_under(SamplerMapping::AUTO, under(2, 0)),
            mapping.target_under(1, under(0, 0)),
            mapping.target_under(1, under(1, 0)),
            mapping.target_under(1, under(2, 0)),
            mapping.target_under(1, under(2, 1)),
            mapping.target_under(2, under(0, 1)),
            mapping.target_under(3, under(2, 1)),
            // A choice beyond the table has none; a state beyond it plays as `auto`.
            mapping.target_under(4, under(0, 0)),
            mapping.target_under(u16::MAX, under(u16::MAX, u16::MAX)),
            mapping.target_under(1, under(3, 0)),
            mapping.target_under(1, under(2, u16::MAX)),
        ]
    });
    assert_eq!(
        looked_up,
        [
            None,
            Some(switch(34)),
            Some(switch(25)),
            Some(switch(25)),
            Some(switch(35)),
            Some(switch(55)),
            Some(switch(46)),
            None,
            None,
            None,
            Some(switch(25)),
            Some(switch(35)),
        ]
    );
    assert_eq!(mapping.target(1), mapping.default_target());
}

#[test]
fn tables_that_select_the_same_compare_equal() {
    // A state whose every choice plays as `auto` keeps one column (TYP-10): states that
    // change nothing make the table of the articulation choices alone.
    let alone = |choice: u16, _: Passage| cell(choice, Passage::AUTO);
    let by_choice: Vec<Option<ArticulationTarget>> =
        (0..4).map(|choice| cell(choice, Passage::AUTO)).collect();
    assert_eq!(
        TargetTable::tabulate(4, 10, 4, alone),
        TargetTable::from(by_choice)
    );
    // Only the vibrato tells cells apart: the same table as one built without mute columns.
    let vibrato_only = |choice: u16, passage: Passage| cell(choice, Passage { mute: 0, ..passage });
    assert_eq!(
        TargetTable::tabulate(4, 10, 2, vibrato_only),
        TargetTable::tabulate(4, 1, 2, vibrato_only)
    );
    assert_ne!(
        TargetTable::tabulate(4, 3, 2, cell),
        TargetTable::tabulate(4, 1, 2, vibrato_only)
    );
    // No target anywhere, or only for `auto` under `auto`: the default.
    assert_eq!(
        TargetTable::tabulate(79, 10, 4, |_, _| None),
        TargetTable::default()
    );
    assert_eq!(
        TargetTable::tabulate(79, 10, 4, |choice, passage| {
            (choice == 0 && passage == Passage::AUTO).then(|| switch(24))
        }),
        TargetTable::default()
    );
    // A zero count counts as one column.
    assert_eq!(
        TargetTable::tabulate(4, 0, 0, cell),
        TargetTable::tabulate(4, 1, 1, cell)
    );
    assert_eq!(
        SamplerMapping::new(
            TargetTable::tabulate(4, 3, 2, alone),
            DynamicsDrive::Velocity,
            None,
            0
        ),
        SamplerMapping::new(
            (0..4)
                .map(|choice| cell(choice, Passage::AUTO))
                .collect::<Vec<_>>(),
            DynamicsDrive::Velocity,
            None,
            0
        )
    );
}

#[test]
fn auto_never_has_a_target() {
    let mapping = SamplerMapping::new(vec![Some(switch(24))], DynamicsDrive::Velocity, None, 0);
    assert_eq!(mapping.target(SamplerMapping::AUTO), None);
    assert_eq!(mapping, SamplerMapping::default());
}

#[test]
fn mappings_that_select_the_same_compare_equal() {
    // Trailing choices without a target do not make two mappings differ (TYP-10), so a preset
    // that maps nothing compiles to the default mapping however many choices there are.
    let mapping = |targets| SamplerMapping::new(targets, DynamicsDrive::Velocity, None, 0);
    assert_eq!(
        mapping(vec![None, Some(switch(24))]),
        mapping(vec![None, Some(switch(24)), None, None])
    );
    assert_eq!(mapping(vec![None; 79]), SamplerMapping::default());
    assert_ne!(
        mapping(vec![None, Some(switch(24))]),
        SamplerMapping::default()
    );
    assert_ne!(
        SamplerMapping::new(Vec::new(), DynamicsDrive::Fixed(100), None, 0),
        SamplerMapping::default()
    );
    assert_ne!(
        SamplerMapping::new(Vec::new(), DynamicsDrive::Velocity, None, -12),
        SamplerMapping::default()
    );
    assert_eq!(
        SamplerSetup::new(Arc::new(SampleBank::default())).mapping,
        Arc::new(SamplerMapping::default()),
        "a bank alone plays with the mapping of a preset about which nothing is claimed"
    );
}

/// Plays a phrase on `processor`: the default articulation, a switch to the second, a note in
/// it, then all notes off and a note in the default again.
fn phrase(processor: &mut dyn Processor) -> Vec<[f64; 2]> {
    let blocks: [&[InstrumentEvent]; 6] = [
        &[event(EventKind::NoteOn, 60, 100)],
        &[event(EventKind::NoteOff, 60, 0)],
        &[event(EventKind::NoteOn, 25, 127)],
        &[event(EventKind::NoteOn, 62, 64)],
        &[event(EventKind::AllNotesOff, 0, 0)],
        &[event(EventKind::NoteOn, 64, 127)],
    ];
    blocks
        .iter()
        .flat_map(|events| run(processor, events))
        .collect()
}

#[test]
fn articulation_auto_plays_the_patch_as_it_is_whatever_the_targets() {
    // `auto`, the default, has no target: the patch's own default switch and the keyswitch
    // notes decide, so the articulation targets and the legato of a preset change nothing. The
    // oracle is the same patch with the mapping of a preset about which nothing is claimed.
    // The key offset and a dynamics drive other than velocity apply to every choice, so they
    // stay at the defaults here.
    let oracle = phrase(&mut *sampler(switched_bank(Some(24))));
    let mapping = SamplerMapping::new(
        vec![
            None,
            Some(switch(25)),
            Some(switch(24)),
            Some(ArticulationTarget {
                member: 0,
                selector: Some(Selector::Velocity { lo: 1, hi: 40 }),
                legato: false,
            }),
            Some(ArticulationTarget {
                member: 0,
                selector: Some(Selector::Controller {
                    number: 32,
                    value: 3,
                }),
                legato: true,
            }),
        ],
        DynamicsDrive::Velocity,
        Some(LegatoSpec {
            crossfade_ms: 90,
            start_offset_ms: 60,
            gap_ms: 0,
        }),
        0,
    );
    let setup = SamplerSetup {
        bank: Arc::new(switched_bank(Some(24))),
        mapping: Arc::new(mapping),
    };
    let mapped = phrase(&mut *sampler_set_up(setup, &Params::new()));
    assert!(
        oracle.iter().any(|frame| frame[0] != 0.0),
        "the phrase sounds"
    );
    assert_eq!(mapped, oracle);
}

/// The parameters with the `articulation` choice `name`.
fn articulation(name: &str) -> Params {
    let mut params = Params::new();
    params.insert("articulation".to_owned(), ParamValue::Str(name.to_owned()));
    params
}

/// Sets `params` on `processor` as the engine does at a block start, inside `assert_no_alloc`
/// (RT-02); the slots are computed before, off the render thread.
fn set(processor: &mut dyn Processor, params: &Params) {
    let slots = param_slots(&descriptor(), params).expect("valid parameters");
    assert_no_alloc(|| processor.set_params(&slots));
}

/// The level of a note on key 60 at full velocity, played alone: the note is released after a
/// block and has faded before the next one.
fn note(processor: &mut dyn Processor) -> f64 {
    let quiet = run(processor, &[]);
    assert!(
        quiet.iter().flatten().all(|v| *v == 0.0),
        "nothing sounds before the note"
    );
    let sounded = level(&run(processor, &[event(EventKind::NoteOn, 60, 127)]));
    run(processor, &[event(EventKind::NoteOff, 60, 0)]);
    sounded
}

/// Four articulations on the keys from 36 up, selected by the keyswitch keys 24 (0.5, the
/// default), 25 (0.25), 26 (0.125) and 27 (0.0625), and a region on the switch keys themselves
/// (0.03125), which a press of one of them sounds.
fn four_articulations() -> SampleBank {
    let articulation = |sample, switch| Region {
        sw_last: Some(switch),
        key_lo: 36,
        ..region(sample)
    };
    SampleBank {
        samples: vec![
            constant(0.5),
            constant(0.25),
            constant(0.125),
            constant(0.0625),
            constant(0.031_25),
        ],
        regions: vec![
            articulation(0, 24),
            articulation(1, 25),
            articulation(2, 26),
            articulation(3, 27),
            Region {
                key_lo: 24,
                key_hi: 27,
                ..region(4)
            },
        ],
        keyswitches: switches(&[(24, 27)], Some(24)),
        ..SampleBank::default()
    }
}

/// The mapping of the tests' choices onto [`four_articulations`]: `ordinario` the default
/// switch 24, `sustain` the patch as it is (no selector), `staccato` the switch 26 and
/// `pizzicato` the switch 27.
fn four_targets(key_offset: i8) -> SamplerMapping {
    SamplerMapping::new(
        vec![
            None,
            Some(switch(24)),
            Some(ArticulationTarget {
                member: 0,
                selector: None,
                legato: false,
            }),
            Some(switch(26)),
            Some(switch(27)),
        ],
        DynamicsDrive::Velocity,
        None,
        key_offset,
    )
}

/// A prepared sampler playing `bank` with `mapping` and `params`.
fn mapped(bank: SampleBank, mapping: SamplerMapping, params: &Params) -> Box<dyn Processor> {
    sampler_set_up(
        SamplerSetup {
            bank: Arc::new(bank),
            mapping: Arc::new(mapping),
        },
        params,
    )
}

#[test]
fn an_articulation_switches_new_notes_without_sounding_its_key() {
    let mut processor = mapped(
        four_articulations(),
        four_targets(0),
        &articulation("staccato"),
    );
    // The target's switch is active from the start, and selecting it sounds nothing (a press
    // of key 26 would sound the region on the switch keys).
    assert_eq!(voices(&*processor), 0.0);
    close(note(&mut *processor), 0.125);
    // A held note rings on through a change; the next note plays the new target.
    let held = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    close(level(&held), 0.125);
    set(&mut *processor, &articulation("pizzicato"));
    let both = run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)]);
    close(level(&both), 0.125 + 0.0625);
    assert_eq!(voices(&*processor), 2.0, "no voice of the switch key");
}

#[test]
fn a_clip_keyswitch_after_the_target_wins_until_the_target_is_applied_again() {
    // The order on one processor, without a reset between the steps: the articulation's
    // target, then what the clip selects after it, as the engine's chase sends it.
    let mut processor = mapped(
        four_articulations(),
        four_targets(0),
        &articulation("staccato"),
    );
    close(note(&mut *processor), 0.125);
    // 1. A keyswitch from the clip after the target wins.
    run(&mut *processor, &[event(EventKind::KeySwitch, 25, 0)]);
    close(note(&mut *processor), 0.25);
    // 2. A new choice wins after the clip's keyswitch.
    set(&mut *processor, &articulation("pizzicato"));
    close(note(&mut *processor), 0.0625);
    // 3. A press of a keyswitch key in the clip wins too (and sounds its own region).
    run(&mut *processor, &[event(EventKind::NoteOn, 25, 127)]);
    run(&mut *processor, &[event(EventKind::NoteOff, 25, 0)]);
    close(note(&mut *processor), 0.25);
    // 4. All notes off alone (a jump before every keyswitch of the clip) applies the target
    //    again: not the patch's default switch, not the clip's last one.
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    close(note(&mut *processor), 0.0625);
    // 5. A chase after a keyswitch of the clip: all notes off, then the clip's last keyswitch,
    //    which wins after the target, then the chased note.
    let chased = run(
        &mut *processor,
        &[
            event(EventKind::AllNotesOff, 0, 0),
            event(EventKind::KeySwitch, 25, 0),
            event(EventKind::NoteOn, 62, 127),
        ],
    );
    close(level(&chased), 0.25);
    run(&mut *processor, &[event(EventKind::NoteOff, 62, 0)]);
    // 6. Back at `auto`, the clip's last keyswitch plays, not the forced one.
    set(&mut *processor, &articulation("auto"));
    close(note(&mut *processor), 0.25);
    // 7. In `auto`, all notes off restores the patch's default switch, as before.
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    close(note(&mut *processor), 0.5);
}

#[test]
fn a_chased_keyswitch_from_before_the_target_yields_to_it() {
    // A chase whose keyswitch the target was applied after (its event's value 1): all notes
    // off applies the target, and the keyswitch sets the patch's switch without winning over
    // it, so the chased note plays the target, as straight play did. Back at `auto`, the
    // clip's keyswitch plays (TST-06: a chased keyswitch used to win in any case).
    let mut processor = mapped(
        four_articulations(),
        four_targets(0),
        &articulation("pizzicato"),
    );
    let yielding = InstrumentEvent {
        value: 1,
        ..event(EventKind::KeySwitch, 25, 0)
    };
    let chased = run(
        &mut *processor,
        &[
            event(EventKind::AllNotesOff, 0, 0),
            yielding,
            event(EventKind::NoteOn, 62, 127),
        ],
    );
    close(level(&chased), 0.0625);
    run(&mut *processor, &[event(EventKind::NoteOff, 62, 0)]);
    close(note(&mut *processor), 0.0625);
    set(&mut *processor, &articulation("auto"));
    close(note(&mut *processor), 0.25);
    // A keyswitch that wins (value 0) still overrides the target, as before.
    set(&mut *processor, &articulation("pizzicato"));
    run(&mut *processor, &[event(EventKind::KeySwitch, 25, 0)]);
    close(note(&mut *processor), 0.25);
}

#[test]
fn a_change_of_another_parameter_does_not_apply_the_target_again() {
    // The engine sets the parameters at every block; only a change of the choice applies its
    // target, so an automated volume leaves the clip's keyswitch alone.
    let mut processor = mapped(
        four_articulations(),
        four_targets(0),
        &articulation("staccato"),
    );
    run(&mut *processor, &[event(EventKind::KeySwitch, 25, 0)]);
    let mut louder = articulation("staccato");
    louder.insert("volume".to_owned(), ParamValue::Float(6.0));
    set(&mut *processor, &louder);
    close(note(&mut *processor), 0.25 * 10_f64.powf(6.0 / 20.0));
}

#[test]
fn a_reset_and_a_new_instrument_apply_the_target_again() {
    let mut processor = mapped(
        four_articulations(),
        four_targets(0),
        &articulation("pizzicato"),
    );
    run(&mut *processor, &[event(EventKind::KeySwitch, 25, 0)]);
    close(note(&mut *processor), 0.25);
    // A reset, as on play from stop, applies the target again.
    assert_no_alloc(|| processor.reset());
    close(note(&mut *processor), 0.0625);
    run(&mut *processor, &[event(EventKind::KeySwitch, 25, 0)]);
    processor.prepare(ProcessSpec {
        sample_rate: RATE,
        block_size: BLOCK,
        channels: 2,
    });
    close(note(&mut *processor), 0.0625);
    // A new instrument whose mapping plays pizzicato on another switch.
    let mut remapped = vec![None; 5];
    if let Some(slot) = remapped.get_mut(4) {
        *slot = Some(switch(26));
    }
    processor.set_state(&DeviceState {
        sampler: Some(Arc::new(SamplerSetup {
            bank: Arc::new(four_articulations()),
            mapping: Arc::new(SamplerMapping::new(
                remapped,
                DynamicsDrive::Velocity,
                None,
                0,
            )),
        })),
        ..DeviceState::default()
    });
    close(note(&mut *processor), 0.125);
}

/// The index of the parameter `id` in the sampler's descriptor order.
fn param_index(id: &str) -> u8 {
    let index = descriptor()
        .params
        .iter()
        .position(|p| p.id == id)
        .expect("the sampler has the parameter");
    u8::try_from(index).expect("a parameter index fits a byte")
}

/// A choice event at frame `offset` of its block: the parameter `id` takes the choice with
/// index `value`, as the engine sends an automated change of the `articulation` inside a block.
fn choice_event(offset: usize, id: &str, value: i32) -> InstrumentEvent {
    InstrumentEvent {
        offset,
        value,
        number: param_index(id),
        ..event(EventKind::ParamChoice, 0, 0)
    }
}

#[test]
fn an_automated_choice_switches_new_notes_from_its_frame() {
    // The parameter plays `pizzicato` (switch 27, 0.0625). In one block, a note at frame 0, a
    // change to `staccato` (switch 26, 0.125) at frame 700 with a note sent after it at the
    // same frame, and a note at frame 1400: the note at 0 rings on, and both later notes play
    // the new choice, the one on the change's own frame included.
    let mut processor = mapped(
        four_articulations(),
        four_targets(0),
        &articulation("pizzicato"),
    );
    let out = run(
        &mut *processor,
        &[
            event(EventKind::NoteOn, 60, 127),
            choice_event(700, "articulation", 3),
            at_offset(700, event(EventKind::NoteOn, 62, 127)),
            at_offset(1_400, event(EventKind::NoteOn, 64, 127)),
        ],
    );
    let left = |frame: usize| out[frame][0];
    close(left(699), 0.0625);
    assert!(
        left(700) > 0.0625,
        "the note on the change's frame starts there"
    );
    close(left(1_399), 0.0625 + 0.125);
    close(level(&out), 0.0625 + 0.125 + 0.125);
    assert_eq!(voices(&*processor), 3.0, "no voice of a switch key");
    run(
        &mut *processor,
        &[
            event(EventKind::NoteOff, 60, 0),
            event(EventKind::NoteOff, 62, 0),
            event(EventKind::NoteOff, 64, 0),
        ],
    );
    // The choice stays, and the next block start's parameters with the same choice apply
    // nothing again.
    close(note(&mut *processor), 0.125);
    set(&mut *processor, &articulation("staccato"));
    close(note(&mut *processor), 0.125);
}

#[test]
fn the_next_parameters_apply_their_choice_after_a_choice_event() {
    // The parameters said `pizzicato` and an event in the block changed it to `staccato`: the
    // event is the stored choice, so parameters that say `pizzicato` again (a block start
    // after a seek back before the change) apply it, and so does all notes off after the
    // event the event's own choice.
    let mut processor = mapped(
        four_articulations(),
        four_targets(0),
        &articulation("pizzicato"),
    );
    run(&mut *processor, &[choice_event(100, "articulation", 3)]);
    close(note(&mut *processor), 0.125);
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    close(note(&mut *processor), 0.125);
    set(&mut *processor, &articulation("pizzicato"));
    close(note(&mut *processor), 0.0625);
}

#[test]
fn a_choice_event_of_the_choice_in_force_leaves_the_clip_keyswitch_alone() {
    // Under `staccato`, the clip selects switch 25 (0.25) after it. A choice event of
    // `staccato`, the choice in force, applies nothing again, so the clip's switch plays on;
    // a change to `pizzicato` (0.0625) applies its target; a clip keyswitch wins after it
    // again; and parameters with the choice the event left apply nothing either.
    let mut processor = mapped(
        four_articulations(),
        four_targets(0),
        &articulation("staccato"),
    );
    run(&mut *processor, &[event(EventKind::KeySwitch, 25, 0)]);
    close(note(&mut *processor), 0.25);
    run(&mut *processor, &[choice_event(0, "articulation", 3)]);
    close(note(&mut *processor), 0.25);
    run(&mut *processor, &[choice_event(0, "articulation", 4)]);
    close(note(&mut *processor), 0.0625);
    run(&mut *processor, &[event(EventKind::KeySwitch, 25, 0)]);
    close(note(&mut *processor), 0.25);
    set(&mut *processor, &articulation("pizzicato"));
    close(note(&mut *processor), 0.25);
}

#[test]
fn a_choice_event_of_another_parameter_or_no_choice_changes_nothing() {
    // `staccato` plays switch 26 (0.125) at the default volume: a choice event of
    // `instrument_type` or of the volume, and one whose value is no choice index, leave the
    // articulation, the level and the stored parameters as they are.
    let mut processor = mapped(
        four_articulations(),
        four_targets(0),
        &articulation("staccato"),
    );
    run(
        &mut *processor,
        &[
            choice_event(0, "instrument_type", 1),
            choice_event(1, "volume", 4),
            choice_event(2, "articulation", -1),
            choice_event(3, "articulation", i32::MAX),
        ],
    );
    close(note(&mut *processor), 0.125);
    // The stored parameters are unchanged: the same parameters again apply nothing, so a clip
    // keyswitch after them still plays.
    run(&mut *processor, &[event(EventKind::KeySwitch, 25, 0)]);
    set(&mut *processor, &articulation("staccato"));
    close(note(&mut *processor), 0.25);
}

/// The mapping of [`four_targets`] with passage states over [`four_articulations`]: under the
/// mute `con_sordino`, `auto`, `ordinario` and `staccato` play the switch 25 (0.25); under the
/// vibrato `vibrato_on`, `staccato` plays the switch 27 (0.0625), unless muted. Every other
/// combination plays as without a state, `pizzicato` (27) and `sustain` (the patch as it is)
/// among them.
fn passage_targets() -> SamplerMapping {
    let base = four_targets(0);
    let table = TargetTable::tabulate(5, 3, 4, |choice, passage| match choice {
        0 | 1 | 3 if passage.mute == 2 => Some(switch(25)),
        3 if passage.vibrato == 2 => Some(switch(27)),
        _ => base.target(choice),
    });
    SamplerMapping::new(table, DynamicsDrive::Velocity, None, 0)
}

/// The parameters with the `articulation`, `mute` and `vibrato` choices given.
fn stated(articulation_choice: &str, mute: &str, vibrato: &str) -> Params {
    let mut params = articulation(articulation_choice);
    params.insert("mute".to_owned(), ParamValue::Str(mute.to_owned()));
    params.insert("vibrato".to_owned(), ParamValue::Str(vibrato.to_owned()));
    params
}

#[test]
fn a_mute_or_vibrato_switches_new_notes_to_the_target_it_gives_the_articulation() {
    let mut processor = mapped(
        four_articulations(),
        passage_targets(),
        &stated("staccato", "auto", "auto"),
    );
    close(note(&mut *processor), 0.125);
    // A held note rings on through the change; the next note plays the muted staccato.
    let held = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    close(level(&held), 0.125);
    set(&mut *processor, &stated("staccato", "con_sordino", "auto"));
    let both = run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)]);
    close(level(&both), 0.125 + 0.25);
    assert_eq!(voices(&*processor), 2.0, "no voice of the switch key");
    run(
        &mut *processor,
        &[
            event(EventKind::NoteOff, 60, 0),
            event(EventKind::NoteOff, 62, 0),
        ],
    );
    // `open` and `auto` play the staccato as recorded without a mute.
    for mute in ["open", "auto"] {
        set(&mut *processor, &stated("staccato", mute, "auto"));
        close(note(&mut *processor), 0.125);
    }
    set(&mut *processor, &stated("staccato", "auto", "vibrato_on"));
    close(note(&mut *processor), 0.0625);
    // A state the mapping does not record plays as `auto`.
    set(&mut *processor, &stated("staccato", "auto", "non_vibrato"));
    close(note(&mut *processor), 0.125);
    // Neither state touches an articulation recorded without one.
    for (mute, vibrato) in [("con_sordino", "vibrato_on"), ("auto", "auto")] {
        set(&mut *processor, &stated("pizzicato", mute, vibrato));
        close(note(&mut *processor), 0.0625);
    }
}

#[test]
fn auto_plays_a_recorded_mute_and_forces_nothing_else() {
    let mut processor = mapped(
        four_articulations(),
        passage_targets(),
        &stated("auto", "con_sordino", "auto"),
    );
    close(note(&mut *processor), 0.25);
    // Back to open: nothing is forced, so the patch's default switch plays again.
    set(&mut *processor, &stated("auto", "open", "auto"));
    close(note(&mut *processor), 0.5);
    set(&mut *processor, &stated("auto", "auto", "vibrato_on"));
    close(note(&mut *processor), 0.5);
}

#[test]
fn a_state_that_changes_no_target_leaves_the_clip_keyswitch_alone() {
    // Under `pizzicato` (27), the clip selects switch 25 (0.25) after it. A mute or vibrato
    // that gives `pizzicato` no other target applies nothing, so the clip's switch plays on.
    let mut processor = mapped(
        four_articulations(),
        passage_targets(),
        &stated("pizzicato", "auto", "auto"),
    );
    run(&mut *processor, &[event(EventKind::KeySwitch, 25, 0)]);
    close(note(&mut *processor), 0.25);
    set(&mut *processor, &stated("pizzicato", "con_sordino", "auto"));
    close(note(&mut *processor), 0.25);
    set(
        &mut *processor,
        &stated("pizzicato", "con_sordino", "vibrato_on"),
    );
    close(note(&mut *processor), 0.25);
    run(
        &mut *processor,
        &[choice_event(0, "mute", 1), choice_event(1, "vibrato", 3)],
    );
    close(note(&mut *processor), 0.25);
    // A state that changes the target wins after the clip's switch: under `staccato`, the
    // clip selects 24 (0.5), and the mute then plays the muted staccato (25).
    set(&mut *processor, &stated("staccato", "auto", "auto"));
    run(&mut *processor, &[event(EventKind::KeySwitch, 24, 0)]);
    close(note(&mut *processor), 0.5);
    set(&mut *processor, &stated("staccato", "con_sordino", "auto"));
    close(note(&mut *processor), 0.25);
    // All notes off and a reset apply the target under the state again.
    run(&mut *processor, &[event(EventKind::KeySwitch, 24, 0)]);
    close(note(&mut *processor), 0.5);
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    close(note(&mut *processor), 0.25);
    run(&mut *processor, &[event(EventKind::KeySwitch, 24, 0)]);
    assert_no_alloc(|| processor.reset());
    close(note(&mut *processor), 0.25);
}

#[test]
fn a_note_plays_its_own_articulation_under_the_state() {
    // `pizzicato` (27, recorded without a mute) is the parameter's articulation under the mute:
    // an unmarked note plays it, a `staccato` note the muted staccato (25), and without the
    // mute the staccato (26).
    let mut processor = mapped(
        four_articulations(),
        passage_targets(),
        &stated("pizzicato", "con_sordino", "auto"),
    );
    close(note(&mut *processor), 0.0625);
    close(marked_note(&mut *processor, 3), 0.25);
    set(&mut *processor, &stated("pizzicato", "auto", "auto"));
    close(marked_note(&mut *processor, 3), 0.125);
    set(&mut *processor, &stated("pizzicato", "auto", "vibrato_on"));
    close(marked_note(&mut *processor, 3), 0.0625);
    close(note(&mut *processor), 0.0625);
}

#[test]
fn an_automated_mute_or_vibrato_switches_new_notes_from_its_frame() {
    // Under `staccato` (26, 0.125), one block holds a note at frame 0, a change of the mute to
    // `con_sordino` at frame 700 with a note sent after it on that frame, and a note at frame
    // 1400: both later notes play the muted staccato (25, 0.25).
    let mut processor = mapped(
        four_articulations(),
        passage_targets(),
        &stated("staccato", "auto", "auto"),
    );
    let out = run(
        &mut *processor,
        &[
            event(EventKind::NoteOn, 60, 127),
            choice_event(700, "mute", 2),
            at_offset(700, event(EventKind::NoteOn, 62, 127)),
            at_offset(1_400, event(EventKind::NoteOn, 64, 127)),
        ],
    );
    let left = |frame: usize| out[frame][0];
    close(left(699), 0.125);
    assert!(
        left(700) > 0.125,
        "the note on the change's frame starts there"
    );
    close(left(1_399), 0.125 + 0.25);
    close(level(&out), 0.125 + 0.25 + 0.25);
    run(
        &mut *processor,
        &[
            event(EventKind::NoteOff, 60, 0),
            event(EventKind::NoteOff, 62, 0),
            event(EventKind::NoteOff, 64, 0),
        ],
    );
    // The vibrato too, once the mute is back at `auto`.
    run(&mut *processor, &[choice_event(0, "mute", 0)]);
    close(note(&mut *processor), 0.125);
    run(&mut *processor, &[choice_event(0, "vibrato", 2)]);
    close(note(&mut *processor), 0.0625);
    // The events are the stored choices: parameters with the same choices apply nothing again,
    // so the clip's switch after them plays on; parameters that differ apply their target.
    run(&mut *processor, &[event(EventKind::KeySwitch, 24, 0)]);
    set(&mut *processor, &stated("staccato", "auto", "vibrato_on"));
    close(note(&mut *processor), 0.5);
    set(&mut *processor, &stated("staccato", "auto", "auto"));
    close(note(&mut *processor), 0.125);
}

/// Plays the same phrase as [`phrase`] with the switch keys of [`four_articulations`]:
/// notes, a keyswitch event, a press of a switch key, all notes off and a chase.
fn switched_phrase(processor: &mut dyn Processor) -> Vec<[f64; 2]> {
    let blocks: [&[InstrumentEvent]; 7] = [
        &[event(EventKind::NoteOn, 60, 100)],
        &[event(EventKind::KeySwitch, 26, 0)],
        &[event(EventKind::NoteOn, 62, 64)],
        &[event(EventKind::NoteOn, 25, 127)],
        &[event(EventKind::NoteOn, 64, 127)],
        &[
            event(EventKind::AllNotesOff, 0, 0),
            event(EventKind::KeySwitch, 27, 0),
            event(EventKind::NoteOn, 65, 90),
        ],
        &[event(EventKind::AllNotesOff, 0, 0)],
    ];
    blocks
        .iter()
        .flat_map(|events| run(processor, events))
        .collect()
}

#[test]
fn auto_and_a_choice_without_a_selector_play_the_clip_as_before() {
    // The oracle: the patch with the mapping of a preset about which nothing is claimed.
    let oracle = switched_phrase(&mut *sampler(four_articulations()));
    assert!(oracle.iter().any(|frame| frame[0] != 0.0));
    for choice in ["auto", "sustain"] {
        let mut processor = mapped(four_articulations(), four_targets(0), &articulation(choice));
        assert_eq!(switched_phrase(&mut *processor), oracle, "{choice}");
    }
    // A mapping with mute and vibrato targets plays the same with all three at `auto`, and with
    // `open` and `ordinary`, which give `auto` nothing to force.
    for (mute, vibrato) in [
        ("auto", "auto"),
        ("open", "ordinary"),
        ("auto", "non_vibrato"),
    ] {
        let mut processor = mapped(
            four_articulations(),
            passage_targets(),
            &stated("auto", mute, vibrato),
        );
        assert_eq!(switched_phrase(&mut *processor), oracle, "{mute} {vibrato}");
    }
    // Every choice of a mapping without targets plays as `auto`.
    for choice in ["ordinario", "staccato", "pizzicato"] {
        let mut processor = sampler_with(four_articulations(), &articulation(choice));
        assert_eq!(switched_phrase(&mut *processor), oracle, "{choice}");
    }
}

#[test]
fn a_forced_switch_plays_where_the_patch_lost_its_switch_keys() {
    // A patch whose regions name switches but which declares no keyswitch keys and no default
    // (a range the loader could not read): nothing plays in `auto`, and the target still
    // selects its regions, because a region compares its switch with the key itself.
    let bank = SampleBank {
        keyswitches: Keyswitches::NONE,
        ..four_articulations()
    };
    let mut silent = mapped(bank.clone(), four_targets(0), &articulation("auto"));
    close(note(&mut *silent), 0.0);
    let mut forced = mapped(bank, four_targets(0), &articulation("staccato"));
    close(note(&mut *forced), 0.125);
}

#[test]
fn a_release_plays_the_tail_of_the_forced_switch() {
    // Release regions of the switches 24 (0.25) and 26 (0.125) beside the attacks: a note
    // released under `staccato` (switch 26) plays that switch's tail, also when the sustain
    // pedal holds the release until it lifts.
    let tail = |sample, switch| Region {
        sw_last: Some(switch),
        trigger: Trigger::Release,
        key_lo: 36,
        ..region(sample)
    };
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.25), constant(0.125)],
        regions: vec![
            Region {
                key_lo: 36,
                release: 0.001,
                ..region(0)
            },
            tail(1, 24),
            tail(2, 26),
        ],
        keyswitches: switches(&[(24, 27)], Some(24)),
        ..SampleBank::default()
    };
    for held in [false, true] {
        let mut processor = mapped(bank.clone(), four_targets(0), &articulation("staccato"));
        let mut events = vec![pedal(held), event(EventKind::NoteOn, 60, 127)];
        run(&mut *processor, &events);
        events = vec![event(EventKind::NoteOff, 60, 0)];
        if held {
            events.push(InstrumentEvent {
                offset: BLOCK / 2,
                ..pedal(false)
            });
        }
        close(level(&run(&mut *processor, &events)), 0.125);
    }
}

#[test]
fn a_controller_starts_the_regions_of_the_forced_switch() {
    // A pedal noise of switch 26 (0.5): the pedal starts it under `staccato`, not in `auto`,
    // where the patch's default switch 24 is active.
    let bank = SampleBank {
        samples: vec![constant(0.5)],
        regions: vec![Region {
            on_key: false,
            sw_last: Some(26),
            cc_triggers: range(64, 64, 127),
            ..region(0)
        }],
        keyswitches: switches(&[(24, 27)], Some(24)),
        ..SampleBank::default()
    };
    for (choice, sounding) in [("auto", 0.0), ("staccato", 0.5)] {
        let mut processor = mapped(bank.clone(), four_targets(0), &articulation(choice));
        close(level(&run(&mut *processor, &[pedal(true)])), sounding);
    }
}

/// Two velocity layers on every key, 1-40 (0.5) and 41-127 (0.25), and a release region of the
/// soft layer (0.125).
fn two_layers() -> SampleBank {
    SampleBank {
        samples: vec![constant(0.5), constant(0.25), constant(0.125)],
        regions: vec![
            Region {
                vel_lo: 1,
                vel_hi: 40,
                ..region(0)
            },
            Region {
                vel_lo: 41,
                ..region(1)
            },
            Region {
                vel_lo: 1,
                vel_hi: 40,
                trigger: Trigger::Release,
                ..region(2)
            },
        ],
        ..SampleBank::default()
    }
}

/// `pizzicato` plays the velocity zone 1-40 of [`two_layers`]; no other choice has a target.
fn zone_target() -> SamplerMapping {
    let mut targets = vec![None; 5];
    if let Some(slot) = targets.get_mut(4) {
        *slot = Some(ArticulationTarget {
            member: 0,
            selector: Some(Selector::Velocity { lo: 1, hi: 40 }),
            legato: false,
        });
    }
    SamplerMapping::new(targets, DynamicsDrive::Velocity, None, 0)
}

#[test]
fn a_velocity_zone_moves_new_notes_into_it() {
    // `lo + (v - 1) · (hi - lo) / 126` over 1-40: 1 stays 1, 127 becomes 40, 64 becomes
    // 1 + 63 · 39 / 126 = 20 (19.5, rounded down), 100 becomes 1 + 99 · 39 / 126 = 31.
    for (pressed, moved) in [(1, 1), (64, 20), (100, 31), (127, 40)] {
        let mut processor = mapped(two_layers(), zone_target(), &articulation("pizzicato"));
        let attack = run(&mut *processor, &[event(EventKind::NoteOn, 60, pressed)]);
        close(level(&attack), 0.5 * gain(moved));
        // The release region of the zone plays at the moved velocity too.
        let release = run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
        close(level(&release), 0.125 * gain(moved));
    }
    // Back at `auto`, the note's own velocity picks the loud layer again.
    let mut processor = mapped(two_layers(), zone_target(), &articulation("pizzicato"));
    set(&mut *processor, &articulation("auto"));
    let attack = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    close(level(&attack), 0.25);
}

/// Two layers gated by controller 32 (0-63: 0.5, 64-127: 0.25), and a region that an event of
/// controller 32 from 64 up starts (0.125).
fn controller_layers() -> SampleBank {
    SampleBank {
        samples: vec![constant(0.5), constant(0.25), constant(0.125)],
        regions: vec![
            Region {
                cc_gates: range(32, 0, 63),
                amp_veltrack: 0.0,
                ..region(0)
            },
            Region {
                cc_gates: range(32, 64, 127),
                amp_veltrack: 0.0,
                ..region(1)
            },
            Region {
                on_key: false,
                cc_triggers: range(32, 64, 127),
                ..region(2)
            },
        ],
        ..SampleBank::default()
    }
}

/// `staccato` sets controller 32 to 100 in [`controller_layers`]; no other choice has a target.
fn controller_target() -> SamplerMapping {
    let mut targets = vec![None; 5];
    if let Some(slot) = targets.get_mut(3) {
        *slot = Some(ArticulationTarget {
            member: 0,
            selector: Some(Selector::Controller {
                number: 32,
                value: 100,
            }),
            legato: false,
        });
    }
    SamplerMapping::new(targets, DynamicsDrive::Velocity, None, 0)
}

#[test]
fn a_controller_target_sets_what_the_regions_read() {
    let mut processor = mapped(
        controller_layers(),
        controller_target(),
        &articulation("staccato"),
    );
    // The value opens the upper layer, on every channel, and starts no controller region.
    assert_eq!(voices(&*processor), 0.0);
    close(note(&mut *processor), 0.25);
    let other_channel = run(&mut *processor, &[press(62, 127, 9)]);
    close(level(&other_channel), 0.25);
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    run(&mut *processor, &[]);
    // An event of the controller from a clip wins after the target.
    run(&mut *processor, &[cc(32, 10, 0)]);
    close(note(&mut *processor), 0.5);
    // All notes off applies the target again; a chased value after it wins.
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    close(note(&mut *processor), 0.25);
    // Back at `auto`, the clip's value applies again.
    set(&mut *processor, &articulation("auto"));
    close(note(&mut *processor), 0.5);
}

#[test]
fn a_controller_target_reaches_the_crossfades_of_sounding_notes() {
    // The forced value is what the controller crossfades read at each block start too: a held
    // note faded in by controller 32 over 0-127 sounds at the target's level, and goes back to
    // the controller's own value (0: silent) at `auto`.
    let bank = SampleBank {
        samples: vec![constant(0.5)],
        regions: vec![Region {
            cc_crossfades: Box::new([CcCrossfade {
                controller: 32,
                fade: Crossfade {
                    in_lo: 0,
                    in_hi: 127,
                    curve: CrossfadeCurve::Gain,
                    ..Crossfade::NONE
                },
            }]),
            amp_veltrack: 0.0,
            ..region(0)
        }],
        ..SampleBank::default()
    };
    let mut processor = mapped(bank, controller_target(), &articulation("staccato"));
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    let held = run(&mut *processor, &[]);
    close(level(&held), 0.5 * 100.0 / 127.0);
    set(&mut *processor, &articulation("auto"));
    let faded = run(&mut *processor, &[]);
    close(level(&faded), 0.0);
}

/// A key press at full velocity that plays the `articulation` choice with index `choice` as the
/// note's own articulation: 2 `sustain`, 3 `staccato`, 4 `pizzicato` (see [`descriptor`]).
fn marked(key: u8, choice: u16) -> InstrumentEvent {
    InstrumentEvent {
        articulation: Some(choice),
        ..event(EventKind::NoteOn, key, 127)
    }
}

/// `event` at frame `offset` of its block.
fn at_offset(offset: usize, event: InstrumentEvent) -> InstrumentEvent {
    InstrumentEvent { offset, ..event }
}

/// The level of a note on key 60 at full velocity with the note's own articulation `choice`,
/// played alone as in [`note`].
fn marked_note(processor: &mut dyn Processor, choice: u16) -> f64 {
    let quiet = run(processor, &[]);
    assert!(
        quiet.iter().flatten().all(|v| *v == 0.0),
        "nothing sounds before the note"
    );
    let sounded = level(&run(processor, &[marked(60, choice)]));
    run(processor, &[event(EventKind::NoteOff, 60, 0)]);
    sounded
}

#[test]
fn a_note_plays_its_own_articulation_from_its_frame_over_the_parameter() {
    // The parameter plays `pizzicato` (switch 27, 0.0625). In one block, an unmarked note at
    // frame 0, a `staccato` note (switch 26, 0.125) at frame 700 and an unmarked note at frame
    // 1400: the marked note sounds from its own frame, and the note after it plays the
    // parameter's articulation again.
    let mut processor = mapped(
        four_articulations(),
        four_targets(0),
        &articulation("pizzicato"),
    );
    let out = run(
        &mut *processor,
        &[
            event(EventKind::NoteOn, 60, 127),
            at_offset(700, marked(62, 3)),
            at_offset(1_400, event(EventKind::NoteOn, 64, 127)),
        ],
    );
    let left = |frame: usize| out[frame][0];
    close(left(699), 0.0625);
    assert!(left(700) > 0.0625, "the marked note starts at its frame");
    close(left(1_399), 0.0625 + 0.125);
    close(level(&out), 0.0625 + 0.125 + 0.0625);
    assert_eq!(voices(&*processor), 3.0, "no voice of a switch key");
    run(
        &mut *processor,
        &[
            event(EventKind::NoteOff, 60, 0),
            event(EventKind::NoteOff, 62, 0),
            event(EventKind::NoteOff, 64, 0),
        ],
    );
    // Later notes play the parameter's articulation; another marked note its own again.
    close(note(&mut *processor), 0.0625);
    close(marked_note(&mut *processor, 3), 0.125);
    close(note(&mut *processor), 0.0625);
}

#[test]
fn two_notes_at_one_frame_play_their_own_articulations() {
    let mut processor = mapped(four_articulations(), four_targets(0), &articulation("auto"));
    let out = run(&mut *processor, &[marked(60, 3), marked(62, 4)]);
    close(level(&out), 0.125 + 0.0625);
}

#[test]
fn a_note_articulation_leaves_the_clip_keyswitch_and_the_parameter_in_place() {
    // In `auto`, the clip selects switch 25 (0.25); a `staccato` note and an `ordinario` note
    // (the default switch 24, 0.5) play their own switches, and the clip's switch plays after
    // them. All notes off still restores the patch's default.
    let mut processor = mapped(four_articulations(), four_targets(0), &articulation("auto"));
    run(&mut *processor, &[event(EventKind::KeySwitch, 25, 0)]);
    close(marked_note(&mut *processor, 3), 0.125);
    close(note(&mut *processor), 0.25);
    close(marked_note(&mut *processor, SamplerMapping::ORDINARIO), 0.5);
    close(note(&mut *processor), 0.25);
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    close(note(&mut *processor), 0.5);
    // Under `staccato` a clip keyswitch after it wins for unmarked notes; a `pizzicato` note
    // plays its own switch over both, and the clip's switch stays.
    set(&mut *processor, &articulation("staccato"));
    run(&mut *processor, &[event(EventKind::KeySwitch, 25, 0)]);
    close(marked_note(&mut *processor, 4), 0.0625);
    close(note(&mut *processor), 0.25);
    // All notes off applies the parameter's target again, not the marked note's.
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    close(note(&mut *processor), 0.125);
}

#[test]
fn a_note_articulation_without_a_target_plays_the_parameter() {
    // A choice the mapping has no target for (`auto`, or an index beyond the choices) plays
    // the parameter's `staccato` (0.125); a target without a selector (`sustain`) plays the
    // patch as it is, its default switch 24 (0.5), in place of the parameter's.
    let mut processor = mapped(
        four_articulations(),
        four_targets(0),
        &articulation("staccato"),
    );
    close(marked_note(&mut *processor, SamplerMapping::AUTO), 0.125);
    close(marked_note(&mut *processor, u16::MAX), 0.125);
    close(marked_note(&mut *processor, 2), 0.5);
    close(note(&mut *processor), 0.125);
    // A preset about which nothing is claimed has no target: every note plays the patch.
    let mut unclaimed = sampler_with(four_articulations(), &articulation("staccato"));
    for choice in 0..=4 {
        close(marked_note(&mut *unclaimed, choice), 0.5);
    }
}

#[test]
fn a_marked_note_releases_with_its_own_switch() {
    // Release regions of the switches 24 (0.25) and 26 (0.125): in `auto` (switch 24), a
    // `staccato` note plays the tail of switch 26 and an unmarked note that of switch 24, also
    // when the sustain pedal holds the release until it lifts.
    let tail = |sample, switch| Region {
        sw_last: Some(switch),
        trigger: Trigger::Release,
        key_lo: 36,
        ..region(sample)
    };
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.25), constant(0.125)],
        regions: vec![
            Region {
                key_lo: 36,
                release: 0.001,
                ..region(0)
            },
            tail(1, 24),
            tail(2, 26),
        ],
        keyswitches: switches(&[(24, 27)], Some(24)),
        ..SampleBank::default()
    };
    for held in [false, true] {
        for (press, tail) in [
            (marked(60, 3), 0.125),
            (event(EventKind::NoteOn, 60, 127), 0.25),
        ] {
            let mut processor = mapped(bank.clone(), four_targets(0), &articulation("auto"));
            run(&mut *processor, &[pedal(held), press]);
            let mut events = vec![event(EventKind::NoteOff, 60, 0)];
            if held {
                events.push(at_offset(BLOCK / 2, pedal(false)));
            }
            close(level(&run(&mut *processor, &events)), tail);
        }
    }
}

#[test]
fn a_note_plays_in_the_velocity_zone_of_its_own_articulation() {
    // `pizzicato` plays the zone 1-40 of the two layers: a marked note at 127 plays the soft
    // layer at velocity 40, and its release region too; the next note its own velocity.
    let mut processor = mapped(two_layers(), zone_target(), &articulation("auto"));
    let attack = run(&mut *processor, &[marked(60, 4)]);
    close(level(&attack), 0.5 * gain(40));
    let release = run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
    close(level(&release), 0.125 * gain(40));
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    run(&mut *processor, &[]);
    let attack = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    close(level(&attack), 0.25);
}

#[test]
fn a_note_reads_the_controller_value_of_its_own_articulation_while_it_sounds() {
    // `staccato` sets controller 32 to 100. Gated layers: a marked note opens the upper layer
    // (0.25) and the next note the lower one again (0.5), so the marked note wrote nothing into
    // the controllers.
    let mut processor = mapped(
        controller_layers(),
        controller_target(),
        &articulation("auto"),
    );
    close(marked_note(&mut *processor, 3), 0.25);
    close(note(&mut *processor), 0.5);
    assert_eq!(voices(&*processor), 0.0, "no controller region started");
    // A crossfade over controller 32: the marked note sounds at the value 100 for as long as
    // it is held, also after a clip's event sets the controller to 10, which a note pressed
    // after the event reads.
    let bank = SampleBank {
        samples: vec![constant(0.5)],
        regions: vec![Region {
            cc_crossfades: Box::new([CcCrossfade {
                controller: 32,
                fade: Crossfade {
                    in_lo: 0,
                    in_hi: 127,
                    curve: CrossfadeCurve::Gain,
                    ..Crossfade::NONE
                },
            }]),
            amp_veltrack: 0.0,
            ..region(0)
        }],
        ..SampleBank::default()
    };
    let mut processor = mapped(bank, controller_target(), &articulation("auto"));
    run(&mut *processor, &[marked(60, 3)]);
    close(level(&run(&mut *processor, &[])), 0.5 * 100.0 / 127.0);
    run(&mut *processor, &[cc(32, 10, 0)]);
    let both = run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)]);
    // The values 100 and 10 sum to 110.
    close(level(&both), 0.5 * 110.0 / 127.0);
}

#[test]
fn the_key_offset_moves_notes_as_their_unmoved_keys_play() {
    // The oracle: the same patch without an offset, played on the moved keys.
    let ramp = Sample {
        rate: RATE,
        channels: 1,
        data: (0..100_u8).map(|i| f32::from(i) / 100.0).collect(),
    };
    let bank = SampleBank {
        samples: vec![ramp],
        regions: vec![Region {
            loop_mode: LoopMode::Continuous,
            loop_start: 0,
            loop_end: 100,
            end: 100,
            ..region(0)
        }],
        ..SampleBank::default()
    };
    let offset =
        |key_offset| SamplerMapping::new(Vec::new(), DynamicsDrive::Velocity, None, key_offset);
    for (key_offset, played, sounding) in [(-12, 72, 60), (12, 60, 72), (-24, 100, 76)] {
        let mut moved = mapped(bank.clone(), offset(key_offset), &Params::new());
        let mut oracle = sampler(bank.clone());
        let blocks = |moved_key: u8| -> [Vec<InstrumentEvent>; 2] {
            [
                vec![event(EventKind::NoteOn, moved_key, 90)],
                vec![InstrumentEvent {
                    offset: BLOCK / 2,
                    ..event(EventKind::NoteOff, moved_key, 0)
                }],
            ]
        };
        for (got, want) in blocks(played).iter().zip(blocks(sounding).iter()) {
            assert_eq!(
                run(&mut *moved, got),
                run(&mut *oracle, want),
                "{key_offset}"
            );
        }
        assert_eq!(
            voices(&*moved),
            0.0,
            "the moved release ends the moved note"
        );
    }
}

#[test]
fn a_note_the_key_offset_moves_out_of_the_keys_is_dropped() {
    let below = mapped(
        two_layers(),
        SamplerMapping::new(Vec::new(), DynamicsDrive::Velocity, None, -12),
        &Params::new(),
    );
    let above = mapped(
        two_layers(),
        SamplerMapping::new(Vec::new(), DynamicsDrive::Velocity, None, 12),
        &Params::new(),
    );
    for (mut processor, dropped, kept) in [(below, 11, 12), (above, 116, 115)] {
        let out = run(&mut *processor, &[event(EventKind::NoteOn, dropped, 127)]);
        assert!(out.iter().flatten().all(|v| *v == 0.0), "{dropped}");
        assert_eq!(voices(&*processor), 0.0);
        // Its release is dropped with it, and starts no release region.
        let out = run(&mut *processor, &[event(EventKind::NoteOff, dropped, 0)]);
        assert!(out.iter().flatten().all(|v| *v == 0.0), "{dropped}");
        // The last key the offset keeps plays (key 127 runs through the sample within the
        // block, so the loudest frame is the oracle).
        let out = run(&mut *processor, &[event(EventKind::NoteOn, kept, 127)]);
        close(
            out.iter().fold(0.0, |peak, frame| f64::max(peak, frame[0])),
            0.25,
        );
    }
}

#[test]
fn the_key_offset_moves_keyswitch_notes_and_events_too() {
    // A clip writes its keyswitch notes where the offset moves them onto the patch's keyswitch
    // keys, as it writes every other note: with an offset of -12, key 37 presses the switch
    // key 25 (and sounds the region on the switch keys), a keyswitch event of key 38 selects
    // 26 without sounding, and a press of key 25 plays key 13, which neither switches nor
    // sounds. A note from key 48 up plays the articulations' keys from 36 up.
    let mut moved = mapped(four_articulations(), four_targets(-12), &Params::new());
    let switched = run(&mut *moved, &[event(EventKind::NoteOn, 37, 127)]);
    close(level(&switched), 0.031_25);
    run(&mut *moved, &[event(EventKind::NoteOff, 37, 0)]);
    run(&mut *moved, &[]);
    let unswitched = run(&mut *moved, &[event(EventKind::NoteOn, 25, 127)]);
    assert!(unswitched.iter().flatten().all(|v| *v == 0.0));
    assert_eq!(voices(&*moved), 0.0);
    close(note(&mut *moved), 0.25);
    let mut sounding = run(&mut *moved, &[event(EventKind::NoteOn, 72, 127)]);
    close(level(&sounding), 0.25);
    run(&mut *moved, &[event(EventKind::KeySwitch, 38, 0)]);
    sounding = run(&mut *moved, &[event(EventKind::NoteOn, 74, 127)]);
    close(level(&sounding), 0.25 + 0.125);
    assert_eq!(voices(&*moved), 2.0, "the keyswitch event sounded nothing");
    // An articulation's keyswitch names the patch's key and is not moved.
    let mut forced = mapped(
        four_articulations(),
        four_targets(-12),
        &articulation("pizzicato"),
    );
    let target = run(&mut *forced, &[event(EventKind::NoteOn, 72, 127)]);
    close(level(&target), 0.0625);
}

#[test]
fn the_switch_notes_are_the_clip_keys_the_offset_moves_onto_switch_keys() {
    let setup = |key_offset| SamplerSetup {
        bank: Arc::new(four_articulations()),
        mapping: Arc::new(SamplerMapping::new(
            Vec::new(),
            DynamicsDrive::Velocity,
            None,
            key_offset,
        )),
    };
    let notes = |key_offset| -> Vec<u8> {
        (0..=127)
            .filter(|key| setup(key_offset).switch_notes().contains(*key))
            .collect()
    };
    assert_eq!(notes(0), [24, 25, 26, 27]);
    assert_eq!(notes(-12), [36, 37, 38, 39]);
    assert_eq!(notes(12), [12, 13, 14, 15]);
    // Switch keys that no clip key reaches are no switch notes.
    assert_eq!(notes(-110), [] as [u8; 0]);
    assert_eq!(notes(i8::MIN), [] as [u8; 0]);
    let mapping =
        |key_offset| SamplerMapping::new(Vec::new(), DynamicsDrive::Velocity, None, key_offset);
    let (down, up, none) = (mapping(-12), mapping(12), mapping(0));
    // The sampler moves every note on the render thread (RT-02).
    let keys = assert_no_alloc(|| {
        [
            down.patch_key(12),
            down.patch_key(11),
            up.patch_key(115),
            up.patch_key(116),
            none.patch_key(127),
            none.patch_key(128),
        ]
    });
    assert_eq!(keys, [Some(0), None, Some(127), None, Some(127), None]);
}

// --- The `dynamics` parameter: velocity, fixed velocity and controller drives ---

/// The parameters with `dynamics` at `value`.
fn dynamics(value: f64) -> Params {
    let mut params = Params::new();
    params.insert("dynamics".to_owned(), ParamValue::Float(value));
    params
}

/// The mapping of a preset whose dynamics are driven by `drive`, with nothing else claimed.
fn driven(drive: DynamicsDrive) -> SamplerMapping {
    SamplerMapping::new(Vec::new(), drive, None, 0)
}

/// Every sample of `frames` as its bits, so that a comparison is bit for bit.
fn bits(frames: &[[f64; 2]]) -> Vec<u64> {
    frames.iter().flatten().map(|x| x.to_bits()).collect()
}

/// Velocity layers with a crossfade, a release tail, a layer that controller 1 fades in and a
/// layer that the sustain pedal gates: what the velocity, the controllers and the release give
/// a note.
fn expressive_bank() -> SampleBank {
    SampleBank {
        samples: vec![
            constant(0.5),
            constant(0.25),
            constant(0.125),
            constant(0.0625),
        ],
        regions: vec![
            Region {
                velocity_crossfade: fade_out(70, 90, CrossfadeCurve::Power),
                release: 0.01,
                ..region(0)
            },
            Region {
                velocity_crossfade: fade_in(70, 90, CrossfadeCurve::Power),
                cc_crossfades: cc_fade(true, CrossfadeCurve::Gain),
                amp_veltrack: 50.0,
                release: 0.01,
                ..region(1)
            },
            Region {
                trigger: Trigger::Release,
                ..region(2)
            },
            Region {
                cc_gates: range(64, 64, 127),
                ..region(3)
            },
        ],
        ..SampleBank::default()
    }
}

/// A phrase on [`expressive_bank`]: notes inside and outside the crossfade, a controller swell,
/// the pedal, releases with tails, all notes off and a loud note after it.
fn expressive_phrase(processor: &mut dyn Processor) -> Vec<[f64; 2]> {
    let at = |offset, event| InstrumentEvent { offset, ..event };
    let blocks: [&[InstrumentEvent]; 8] = [
        &[
            event(EventKind::NoteOn, 60, 100),
            at(300, event(EventKind::NoteOn, 64, 75)),
        ],
        &[cc(1, 90, 0)],
        &[
            pedal(true),
            at(10, event(EventKind::NoteOff, 60, 0)),
            at(500, event(EventKind::NoteOn, 67, 40)),
        ],
        &[],
        &[pedal(false), event(EventKind::NoteOff, 64, 0)],
        &[
            event(EventKind::AllNotesOff, 0, 0),
            event(EventKind::NoteOn, 72, 127),
        ],
        &[at(100, event(EventKind::NoteOff, 72, 0))],
        &[],
    ];
    blocks
        .iter()
        .flat_map(|events| run(processor, events))
        .collect()
}

#[test]
fn dynamics_one_is_identical_to_today() {
    // The default 1 plays every note exactly as the sampler did before the parameter acted:
    // every other test of this file runs at it against values computed from the bank, and here
    // the same phrase renders bit for bit alike with the parameter left out, set to 1, moved
    // to 0.5 and back before the phrase, and with a drive that changes neither velocities nor
    // levels (a controller that no region reads).
    let oracle = expressive_phrase(&mut *sampler(expressive_bank()));
    assert!(
        oracle.iter().all(|frame| frame[0].is_finite())
            && oracle.iter().filter(|frame| frame[0] != 0.0).count() > 6 * BLOCK,
        "the phrase sounds"
    );
    let explicit = expressive_phrase(&mut *sampler_with(expressive_bank(), &dynamics(1.0)));
    assert_eq!(bits(&explicit), bits(&oracle), "dynamics set to 1");
    let mut moved = sampler_with(expressive_bank(), &dynamics(1.0));
    set(&mut *moved, &dynamics(0.5));
    set(&mut *moved, &dynamics(1.0));
    assert_eq!(
        bits(&expressive_phrase(&mut *moved)),
        bits(&oracle),
        "moved and back"
    );
    let velocity = mapped(
        expressive_bank(),
        driven(DynamicsDrive::Velocity),
        &dynamics(1.0),
    );
    let controller = mapped(
        expressive_bank(),
        driven(DynamicsDrive::Controller(100)),
        &dynamics(1.0),
    );
    for (mut processor, what) in [(velocity, "velocity drive"), (controller, "controller")] {
        assert_eq!(
            bits(&expressive_phrase(&mut *processor)),
            bits(&oracle),
            "{what}"
        );
    }
}

#[test]
fn dynamics_scale_the_velocity_of_new_notes() {
    // `max(1, round(v · d))` picks the layer (1-40: 0.5, 41 up: 0.25) and the gain, and the
    // note's release region (1-40: 0.125) plays at it too. 0 plays the softest note, not none.
    for (pressed, value, played) in [
        (100, 0.5, 50),
        (127, 0.5, 64),
        (100, 0.4, 40),
        (127, 0.3, 38),
        (64, 0.75, 48),
        (1, 0.5, 1),
        (100, 0.0, 1),
        (127, 1.0, 127),
    ] {
        let mut processor = sampler_with(two_layers(), &dynamics(value));
        let attack = run(&mut *processor, &[event(EventKind::NoteOn, 60, pressed)]);
        let layer = if played <= 40 { 0.5 } else { 0.25 };
        close(level(&attack), layer * gain(played));
        assert_eq!(voices(&*processor), 1.0, "{pressed} at {value}");
        let release = run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
        let tail = if played <= 40 {
            0.125 * gain(played)
        } else {
            0.0
        };
        close(level(&release), tail);
    }
}

/// A note whose level ignores its velocity (`amp_veltrack=0`): a constant 0.5.
fn untracked() -> SampleBank {
    SampleBank {
        samples: vec![constant(0.5)],
        regions: vec![Region {
            amp_veltrack: 0.0,
            ..region(0)
        }],
        ..SampleBank::default()
    }
}

#[test]
fn dynamics_scales_held_notes() {
    // A held note follows a change as the gain `(d / d₀)²`, ramped linearly over the next block
    // and exact on its last frame, as the controller gains are.
    let mut processor = sampler(untracked());
    let held = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    close(level(&held), 0.5);
    set(&mut *processor, &dynamics(0.5));
    let ramp = run(&mut *processor, &[]);
    for (i, frame) in (1..=BLOCK).zip(&ramp) {
        let at = 1.0 - 0.75 * f64::from(u32::try_from(i).expect("a block")) / 2_048.0;
        close(frame[0], 0.5 * at);
    }
    close(level(&ramp), 0.125);
    for frame in &run(&mut *processor, &[]) {
        close(frame[0], 0.125);
    }
    // A note pressed now starts at its own level; the held one keeps the level of the change.
    let both = run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)]);
    close(level(&both), 0.125 + 0.5);
    set(&mut *processor, &dynamics(1.0));
    run(&mut *processor, &[]);
    let louder = run(&mut *processor, &[]);
    close(level(&louder), 0.5 + 0.5 * 4.0);
}

#[test]
fn a_held_note_rises_at_most_twelve_db() {
    // From 0.1 a rise to 0.15 gives (1.5)² = 2.25; a rise to 1 would give 100, capped at 4.
    for (to, gain) in [(0.15, 2.25), (1.0, 4.0)] {
        let mut processor = sampler_with(untracked(), &dynamics(0.1));
        let held = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
        close(level(&held), 0.5);
        set(&mut *processor, &dynamics(to));
        close(level(&run(&mut *processor, &[])), 0.5 * gain);
    }
    // Below 1/127 the parameter counts as 1/127: a note pressed at 0 starts at its own level,
    // rises by (2/127 · 127)² = 4 at 2/127, and a note pressed at 0.5 falls to
    // ((1/127) / 0.5)² at 0, all but silent.
    let mut processor = sampler_with(untracked(), &dynamics(0.0));
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)])),
        0.5,
    );
    set(&mut *processor, &dynamics(2.0 / 127.0));
    close(level(&run(&mut *processor, &[])), 2.0);
    let mut processor = sampler_with(untracked(), &dynamics(0.5));
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    set(&mut *processor, &dynamics(0.0));
    let faded = level(&run(&mut *processor, &[]));
    let expected = 0.5 * (2.0_f64 / 127.0).powi(2);
    assert!((faded - expected).abs() < 1e-12, "{faded} != {expected}");
}

#[test]
fn a_release_tail_follows_the_dynamics_its_note_was_pressed_with() {
    // Pressed at 0.5 (velocity 64), raised to 1 while held: the tail plays at the note's
    // velocity and at the note's gain (1 / 0.5)² = 4, at the release and at the pedal's lift.
    let mut processor = sampler_with(note_with_tail(0.125), &dynamics(0.5));
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    set(&mut *processor, &dynamics(1.0));
    close(level(&run(&mut *processor, &[])), 4.0 * 0.5 * gain(64));
    let tail = run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
    close(level(&tail), 4.0 * 0.125 * gain(64));

    let mut processor = sampler_with(note_with_tail(0.125), &dynamics(0.5));
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    run(
        &mut *processor,
        &[pedal(true), event(EventKind::NoteOff, 60, 0)],
    );
    set(&mut *processor, &dynamics(0.25));
    run(&mut *processor, &[]);
    let lifted = run(&mut *processor, &[pedal(false)]);
    close(level(&lifted), 0.25 * 0.125 * gain(64));
}

#[test]
fn a_controller_voice_follows_the_dynamics_from_its_start() {
    // A pedal noise has no velocity: it starts at its own level and follows later changes.
    let mut processor = sampler_with(pedal_noises(), &dynamics(0.5));
    let down = run(&mut *processor, &[pedal(true)]);
    let start = level(&down);
    assert!(start > 0.0, "the pedal noise sounds");
    set(&mut *processor, &dynamics(1.0));
    close(level(&run(&mut *processor, &[])), 4.0 * start);
}

#[test]
fn a_fixed_velocity_replaces_the_notes_own() {
    // Every note plays at 100 (the loud layer), whatever its velocity; the dynamics scale the
    // fixed velocity: at 0.4 every note plays at 40, the soft layer, with its tail.
    for pressed in [1, 64, 127] {
        let mut processor = mapped(
            two_layers(),
            driven(DynamicsDrive::Fixed(100)),
            &dynamics(1.0),
        );
        let attack = run(&mut *processor, &[event(EventKind::NoteOn, 60, pressed)]);
        close(level(&attack), 0.25 * gain(100));
        let mut processor = mapped(
            two_layers(),
            driven(DynamicsDrive::Fixed(100)),
            &dynamics(0.4),
        );
        let attack = run(&mut *processor, &[event(EventKind::NoteOn, 60, pressed)]);
        close(level(&attack), 0.5 * gain(40));
        let release = run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
        close(level(&release), 0.125 * gain(40));
    }
    // A held note follows a change as with the velocity drive.
    let mut processor = mapped(
        untracked(),
        driven(DynamicsDrive::Fixed(100)),
        &dynamics(0.5),
    );
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 60, 10)])),
        0.5,
    );
    set(&mut *processor, &dynamics(0.25));
    close(level(&run(&mut *processor, &[])), 0.5 * 0.25);
}

#[test]
fn the_dynamics_scale_a_note_before_an_articulation_moves_it_into_its_zone() {
    // `pizzicato` plays the zone 41-127, the loud layer. At 0.25 a press at 127 plays at 32,
    // which the zone moves to 41 + 31 · 86 / 126 = 62: still the loud layer. Moved first and
    // scaled after, it would play at 32, the soft layer outside the zone.
    let mut targets = vec![None; 5];
    if let Some(slot) = targets.get_mut(4) {
        *slot = Some(ArticulationTarget {
            member: 0,
            selector: Some(Selector::Velocity { lo: 41, hi: 127 }),
            legato: false,
        });
    }
    let mapping = SamplerMapping::new(targets, DynamicsDrive::Velocity, None, 0);
    let mut params = articulation("pizzicato");
    params.insert("dynamics".to_owned(), ParamValue::Float(0.25));
    let mut processor = mapped(two_layers(), mapping, &params);
    let attack = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    close(level(&attack), 0.25 * gain(62));
}

/// Two layers that controller 1 crossfades (equal power): the soft one (0.5) fades out and the
/// loud one (0.25) in over 0 to 127. Velocity does not change their level.
fn controller_dynamics_bank() -> SampleBank {
    SampleBank {
        samples: vec![constant(0.5), constant(0.25)],
        regions: vec![
            Region {
                cc_crossfades: cc_fade(false, CrossfadeCurve::Power),
                amp_veltrack: 0.0,
                ..region(0)
            },
            Region {
                cc_crossfades: cc_fade(true, CrossfadeCurve::Power),
                amp_veltrack: 0.0,
                ..region(1)
            },
        ],
        ..SampleBank::default()
    }
}

/// The level of [`controller_dynamics_bank`] at controller 1 = `value`.
fn crossfaded(value: u8) -> f64 {
    let x = f64::from(value) / 127.0;
    0.5 * (1.0 - x).sqrt() + 0.25 * x.sqrt()
}

/// A prepared sampler playing [`controller_dynamics_bank`] with its dynamics on controller 1.
fn on_controller_one(params: &Params) -> Box<dyn Processor> {
    mapped(
        controller_dynamics_bank(),
        driven(DynamicsDrive::Controller(1)),
        params,
    )
}

#[test]
fn controller_dynamics_drive_the_crossfade() {
    // The default 1 sets controller 1 to 127: the loudest layer alone.
    let mut processor = on_controller_one(&Params::new());
    close(note(&mut *processor), crossfaded(127));
    // `round(d · 127)`: 0.5 sets 64, and a held chord follows from the next block.
    let held = run(
        &mut *processor,
        &[event(EventKind::NoteOn, 60, 127), press(64, 1, 9)],
    );
    close(level(&held), 2.0 * crossfaded(127));
    set(&mut *processor, &dynamics(0.5));
    close(level(&run(&mut *processor, &[])), 2.0 * crossfaded(64));
    set(&mut *processor, &dynamics(0.0));
    close(level(&run(&mut *processor, &[])), 2.0 * crossfaded(0));
}

#[test]
fn controller_dynamics_leave_velocities_and_held_levels_alone() {
    // On the velocity layers of [`two_layers`], with the dynamics on a controller no region
    // reads: a press at 100 plays the loud layer at its own velocity at 0.3 (the velocity drive
    // would play it at 30, the soft layer), and a held note keeps its level when the value
    // changes.
    let mut processor = mapped(
        two_layers(),
        driven(DynamicsDrive::Controller(1)),
        &dynamics(0.3),
    );
    let attack = run(&mut *processor, &[event(EventKind::NoteOn, 60, 100)]);
    close(level(&attack), 0.25 * gain(100));
    set(&mut *processor, &dynamics(1.0));
    for frame in &run(&mut *processor, &[]) {
        close(frame[0], 0.25 * gain(100));
    }
}

#[test]
fn the_last_writer_of_the_controller_wins() {
    let mut processor = on_controller_one(&dynamics(0.5));
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    // A clip's event after the parameter wins.
    run(&mut *processor, &[cc(1, 10, 0)]);
    close(level(&run(&mut *processor, &[])), crossfaded(10));
    // A change of another parameter does not write the dynamics again.
    let mut louder = dynamics(0.5);
    louder.insert("volume".to_owned(), ParamValue::Float(0.0));
    louder.insert("pan".to_owned(), ParamValue::Float(0.0));
    louder.insert("release_scale".to_owned(), ParamValue::Float(2.0));
    set(&mut *processor, &louder);
    close(level(&run(&mut *processor, &[])), crossfaded(10));
    // A change of the parameter after the clip's event wins.
    set(&mut *processor, &dynamics(0.75));
    close(level(&run(&mut *processor, &[])), crossfaded(95));
}

#[test]
fn controller_dynamics_are_applied_again_at_all_notes_off_and_on_a_reset() {
    let mut processor = on_controller_one(&dynamics(0.5));
    run(&mut *processor, &[cc(1, 10, 0)]);
    close(note(&mut *processor), crossfaded(10));
    // All notes off, as at a jump, writes the parameter's value again.
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    run(&mut *processor, &[]);
    close(note(&mut *processor), crossfaded(64));
    // The chase's restored value of the controller follows the all notes off and wins.
    run(
        &mut *processor,
        &[event(EventKind::AllNotesOff, 0, 0), cc(1, 20, 0)],
    );
    run(&mut *processor, &[]);
    close(note(&mut *processor), crossfaded(20));
    // A reset, as on play from stop, and a new instrument write it again.
    assert_no_alloc(|| processor.reset());
    close(note(&mut *processor), crossfaded(64));
    run(&mut *processor, &[cc(1, 20, 0)]);
    processor.set_state(&DeviceState {
        sampler: Some(Arc::new(SamplerSetup {
            bank: Arc::new(controller_dynamics_bank()),
            mapping: Arc::new(driven(DynamicsDrive::Controller(1))),
        })),
        ..DeviceState::default()
    });
    close(note(&mut *processor), crossfaded(64));
}

#[test]
fn controller_dynamics_reach_every_channel_but_start_no_region() {
    // A region that an event of controller 1 from 64 up starts: the parameter's value is no
    // event, so it starts nothing, while a clip's event does. The value reaches every channel.
    let bank = SampleBank {
        samples: vec![constant(0.5), constant(0.125)],
        regions: vec![
            Region {
                cc_gates: range(1, 64, 127),
                amp_veltrack: 0.0,
                ..region(0)
            },
            Region {
                on_key: false,
                cc_triggers: range(1, 64, 127),
                ..region(1)
            },
        ],
        ..SampleBank::default()
    };
    let mut processor = mapped(bank, driven(DynamicsDrive::Controller(1)), &dynamics(0.9));
    run(&mut *processor, &[]);
    assert_eq!(voices(&*processor), 0.0);
    set(&mut *processor, &dynamics(0.8));
    run(&mut *processor, &[]);
    assert_eq!(voices(&*processor), 0.0);
    let gated = run(&mut *processor, &[press(60, 127, 15)]);
    close(level(&gated), 0.5);
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    run(&mut *processor, &[]);
    run(&mut *processor, &[cc(1, 100, 3)]);
    assert_eq!(voices(&*processor), 1.0, "a clip's event starts the region");
}

#[test]
fn controller_dynamics_on_expression_set_the_level() {
    // Volume (7) and expression (11) set the sampler's level: the parameter does too, and a
    // clip's event replaces it.
    for number in [7, 11] {
        let mut processor = mapped(
            untracked(),
            driven(DynamicsDrive::Controller(number)),
            &dynamics(0.5),
        );
        close(note(&mut *processor), 0.5 * 64.0 / 127.0);
        run(&mut *processor, &[cc(number, 127, 0)]);
        close(note(&mut *processor), 0.5);
        set(&mut *processor, &dynamics(0.25));
        close(note(&mut *processor), 0.5 * 32.0 / 127.0);
    }
}

#[test]
fn a_dynamics_value_outside_its_range_plays_as_the_nearest_end() {
    // The descriptor keeps the value between 0 and 1; slots built by hand are clamped, and a
    // value that is not a number plays as the default 1.
    let defaults = param_slots(&descriptor(), &Params::new()).expect("valid parameters");
    for (value, played) in [(f64::NAN, 127), (2.0, 127), (-1.0, 1), (0.5, 64)] {
        let mut slots = defaults;
        slots.set(6, ParamSlot::Float(value));
        let mut processor = sampler(two_layers());
        assert_no_alloc(|| processor.set_params(&slots));
        let attack = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
        let layer = if played <= 40 { 0.5 } else { 0.25 };
        close(level(&attack), layer * gain(played));
    }
}

// --- The overlap legato ---

/// The `articulation` choice `sustain` of [`descriptor`].
const SUSTAIN_CHOICE: u16 = 2;
/// The `articulation` choice `legato` of [`descriptor`].
const LEGATO_CHOICE: u16 = 5;

/// Overlap legato times in milliseconds: the crossfade, the start offset and the gap.
fn overlap(crossfade_ms: u16, start_offset_ms: u16, gap_ms: u16) -> LegatoSpec {
    LegatoSpec {
        crossfade_ms,
        start_offset_ms,
        gap_ms,
    }
}

/// The mapping of a preset whose library says it plays legato by overlapping its notes with
/// the times `spec` (`None`: a preset that claims no overlap legato): `sustain` and `legato`
/// both play the patch as it is, and only `legato` is the legato target.
fn legato_mapping(spec: Option<LegatoSpec>) -> SamplerMapping {
    let target = |legato| {
        Some(ArticulationTarget {
            member: 0,
            selector: None,
            legato,
        })
    };
    SamplerMapping::new(
        vec![None, target(false), target(false), None, None, target(true)],
        DynamicsDrive::Velocity,
        spec,
        0,
    )
}

/// One region per key, each at its own key centre: key 60 plays 0.5, key 62 plays 0.25 and key
/// 64 plays 0.125.
fn line() -> SampleBank {
    let note = |key, sample| Region {
        key_lo: key,
        key_hi: key,
        pitch_keycenter: key,
        ..region(sample)
    };
    SampleBank {
        samples: vec![constant(0.5), constant(0.25), constant(0.125)],
        regions: vec![note(60, 0), note(62, 1), note(64, 2)],
        ..SampleBank::default()
    }
}

/// A prepared sampler playing `bank` with the [`legato_mapping`] of `spec` and the
/// `articulation` choice `choice`.
fn legato_sampler(bank: SampleBank, spec: Option<LegatoSpec>, choice: &str) -> Box<dyn Processor> {
    mapped(bank, legato_mapping(spec), &articulation(choice))
}

/// The frames of `ms` milliseconds at the tests' rate.
fn frames_of(ms: u16) -> f64 {
    f64::from(ms) / 1000.0 * f64::from(RATE)
}

/// The gains of an equal-power crossfade over `frames` frames on its frame `index` (from 0):
/// the note it leaves is heard at the cosine and the note it joins at the sine of the part done
/// by the end of that frame, as an angle from 0 to π/2.
fn crossfade(index: u32, frames: f64) -> (f64, f64) {
    let angle = (f64::from(index + 1) / frames).min(1.0) * FRAC_PI_2;
    (angle.cos(), angle.sin())
}

/// The level of a voice on frame `index` (from 0) of a linear fade from `from` to 0 over
/// `frames` frames.
fn faded_from(from: f64, index: u32, frames: f64) -> f64 {
    from * (1.0 - f64::from(index + 1) / frames).max(0.0)
}

/// Checks every frame of `block` against `expected` of the frame's index in it, on both
/// channels, within the rounding of the per-frame ramps.
fn assert_frames(block: &[[f64; 2]], expected: impl Fn(u32) -> f64) {
    for (frame, index) in block.iter().zip(0_u32..) {
        let want = expected(index);
        assert!(
            (frame[0] - want).abs() < 1e-9 && (frame[1] - want).abs() < 1e-9,
            "frame {index}: {frame:?} != {want}"
        );
    }
}

/// Plays `events` on `processor` in one block of `frames` frames (at most [`BLOCK`]), inside
/// `assert_no_alloc` (RT-02).
fn run_frames(
    processor: &mut dyn Processor,
    events: &[InstrumentEvent],
    frames: usize,
) -> Vec<[f64; 2]> {
    let mut buffer = vec![[0.0; 2]; frames];
    let ctx = BlockContext {
        frame: 0,
        sample_rate: RATE,
        beat: 0.0,
        bpm: 120.0,
        playing: true,
        sidechain: None,
        events,
    };
    assert_no_alloc(|| processor.process(&mut buffer, &ctx));
    buffer
}

#[test]
fn legato_overlap_crossfades_over_the_profile_time() {
    // 62 starts while 60 is held: 60 (0.5) fades out and 62 (0.25) fades in over the preset's
    // 20 ms crossfade (960 frames), along the equal-power curves, and then 60 stops.
    let mut processor = legato_sampler(line(), Some(overlap(20, 0, 0)), "legato");
    let first = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    close(level(&first), 0.5);
    let joined = run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)]);
    let frames = frames_of(20);
    assert_frames(&joined, |index| {
        let (left, entered) = crossfade(index, frames);
        0.5 * left + 0.25 * entered
    });
    assert!(
        (0..960).all(|index| {
            let (left, entered) = crossfade(index, frames);
            (left * left + entered * entered - 1.0).abs() < 1e-12
        }),
        "the two notes keep their power through the crossfade"
    );
    assert_eq!(voices(&*processor), 1.0, "the left note has stopped");
}

/// A sample in three parts: 0.25 for its first 60 ms (2880 frames, the attack), 0.75 for the
/// next 1000 frames and 1.0 after.
fn attack_then_sustain() -> Sample {
    Sample {
        rate: RATE,
        channels: 1,
        data: (0..SAMPLE_FRAMES)
            .map(|frame| match frame {
                0..2880 => 0.25,
                2880..3880 => 0.75,
                _ => 1.0,
            })
            .collect(),
    }
}

#[test]
fn a_joined_note_starts_past_its_samples_attack() {
    // With a start offset of 60 ms, a joined 62 reads its sample from frame 2880 on: 0.75 from
    // its first frame, rising with the 10 ms crossfade, and 1.0 from 1000 frames later. Played
    // with nothing to join, it starts at the sample's start, its attack part, 0.25.
    let mut bank = line();
    bank.samples[1] = attack_then_sustain();
    let spec = Some(overlap(10, 60, 0));
    let mut alone = legato_sampler(bank.clone(), spec, "legato");
    let attack = run(&mut *alone, &[event(EventKind::NoteOn, 62, 127)]);
    // After the region's shortest attack, 0.5 ms (24 frames).
    assert_frames(&attack[30..], |_| 0.25);

    let mut joined = legato_sampler(bank, spec, "legato");
    run(&mut *joined, &[event(EventKind::NoteOn, 60, 127)]);
    let block = run(&mut *joined, &[event(EventKind::NoteOn, 62, 127)]);
    let frames = frames_of(10);
    assert_frames(&block, |index| {
        let (left, entered) = crossfade(index, frames);
        let read = if index < 1000 { 0.75 } else { 1.0 };
        0.5 * left + read * entered
    });
}

/// A sample that reads 0.25 before frame 1000, 0.75 from 1000 to 1249 and 1.0 from 1250 on.
fn steps(frame: u32) -> f32 {
    match frame {
        0..1000 => 0.25,
        1000..1250 => 0.75,
        _ => 1.0,
    }
}

#[test]
fn a_start_offset_past_the_loop_or_the_end_stays_in_the_sample() {
    // The 60 ms start offset (frame 2880) lies past the loop end and past the region end
    // (2000). A looping region starts at its loop start (1000) and loops on from there; a
    // region that does not loop starts at its start, without the skip, and ends at frame 2000.
    let data = (0..u32::try_from(SAMPLE_FRAMES).unwrap())
        .map(steps)
        .collect();
    let mut bank = line();
    bank.samples[1] = Sample {
        rate: RATE,
        channels: 1,
        data,
    };
    let frames = frames_of(10);
    let looping = Region {
        loop_mode: LoopMode::Continuous,
        loop_start: 1000,
        loop_end: 2000,
        ..bank.regions[1].clone()
    };
    let ending = Region {
        end: 2000,
        ..bank.regions[1].clone()
    };
    for (region, read) in [
        (
            looping,
            (|index| steps(1000 + index % 1000)) as fn(u32) -> f32,
        ),
        (
            ending,
            |index| if index < 2000 { steps(index) } else { 0.0 },
        ),
    ] {
        let mut bank = bank.clone();
        bank.regions[1] = region;
        let mut processor = legato_sampler(bank, Some(overlap(10, 60, 0)), "legato");
        run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
        let block = run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)]);
        assert_frames(&block, |index| {
            let (left, entered) = crossfade(index, frames);
            0.5 * left + f64::from(read(index)) * entered
        });
    }
}

/// [`line`] as patch 0 of a set whose patch 1 has the keyswitches 24 (its default) and 25:
/// keys 24 and 25 are keyswitch keys of the device, of another patch than the line's.
fn line_beside_a_keyswitch_patch() -> SampleBank {
    SampleBank {
        keyswitches: switches(&[(24, 25)], None),
        member_keyswitches: vec![Keyswitches::NONE, switches(&[(24, 25)], Some(24))],
        ..line()
    }
}

#[test]
fn a_keyswitch_of_another_patch_of_a_set_leaves_a_legato_line_alone() {
    // The line plays legato in patch 0; a press of 25, a keyswitch of patch 1, while 60 is held
    // selects patch 1's switch and plays no part in the legato, as a keyswitch of the line's own
    // patch does: 60 sounds on, and 62 still joins it. The oracle is the same line without the
    // keyswitch, bit for bit (TST-06: the keyswitch used to leave 60, which faded out with
    // nothing in its place, and 62 then started a new phrase).
    let spec = Some(overlap(20, 0, 0));
    let mut plain = legato_sampler(line_beside_a_keyswitch_patch(), spec, "legato");
    let mut switched = legato_sampler(line_beside_a_keyswitch_patch(), spec, "legato");
    let on = |key| event(EventKind::NoteOn, key, 127);
    let pressed = run(&mut *plain, &[on(60)]);
    assert_eq!(run(&mut *switched, &[on(60)]), pressed);
    let held = run(&mut *plain, &[]);
    close(level(&held), 0.5);
    assert_eq!(run(&mut *switched, &[on(25)]), held);
    let joined = run(&mut *plain, &[on(62)]);
    assert_eq!(run(&mut *switched, &[on(62)]), joined);
    assert_eq!(voices(&*switched), voices(&*plain));
}

#[test]
fn a_held_keyswitch_of_another_patch_of_a_set_does_not_make_a_note_legato() {
    // [`first_and_legato`] as patch 0 of a set whose patch 1 has the keyswitches 26 and 27:
    // 27 held down while 60 is pressed is a keyswitch of the device, so 60 still starts the
    // phrase with its `first` region, as with a held keyswitch of its own patch (TST-06: it
    // used to count as another key down, and 60 played its `legato` region).
    let set = SampleBank {
        keyswitches: switches(&[(24, 27)], None),
        member_keyswitches: vec![
            switches(&[(24, 25)], Some(24)),
            switches(&[(26, 27)], Some(26)),
        ],
        ..first_and_legato()
    };
    let mut processor = sampler(set);
    let out = run(
        &mut *processor,
        &[
            event(EventKind::NoteOn, 27, 127),
            event(EventKind::NoteOn, 60, 127),
        ],
    );
    close(level(&out), 0.5);
    assert_eq!(processor.report().entries(), [("voices", 1.0)]);
}

#[test]
fn legato_same_pitch_retriggers() {
    // A press of the legato note's own key plays its attack beside it, as without the legato:
    // the oracle is the same phrase in `sustain`.
    let phrase = |choice| -> Vec<[f64; 2]> {
        let mut processor = legato_sampler(line(), Some(overlap(20, 60, 0)), choice);
        let blocks: [&[InstrumentEvent]; 4] = [
            &[event(EventKind::NoteOn, 60, 127)],
            &[event(EventKind::NoteOn, 60, 127)],
            &[event(EventKind::NoteOff, 60, 0)],
            &[],
        ];
        blocks
            .iter()
            .flat_map(|events| run(&mut *processor, events))
            .collect()
    };
    let oracle = phrase("sustain");
    close(oracle[2 * BLOCK - 1][0], 1.0);
    assert_eq!(phrase("legato"), oracle);
}

#[test]
fn chord_in_legato_mode_is_monophonic() {
    // Three keys on one frame: each joins the one before, which has not sounded yet, so only
    // the last sounds, fading in over the crossfade. In `sustain` the whole chord sounds.
    let chord = [
        event(EventKind::NoteOn, 60, 127),
        event(EventKind::NoteOn, 62, 127),
        event(EventKind::NoteOn, 64, 127),
    ];
    let spec = Some(overlap(20, 0, 0));
    let mut legato = legato_sampler(line(), spec, "legato");
    let block = run(&mut *legato, &chord);
    let frames = frames_of(20);
    assert_frames(&block, |index| 0.125 * crossfade(index, frames).1);
    assert_eq!(voices(&*legato), 1.0);

    let mut sustain = legato_sampler(line(), spec, "sustain");
    close(level(&run(&mut *sustain, &chord)), 0.875);
    assert_eq!(voices(&*sustain), 3.0);
}

#[test]
fn a_note_that_joins_nothing_plays_its_attack() {
    // The first note of a phrase, a note after the legato note's release beyond the gap, and
    // the first note after all notes off (a seek or a loop wrap), which ends the held legato
    // note, play exactly as in `sustain`.
    let phrase = |choice| -> Vec<[f64; 2]> {
        let mut processor = legato_sampler(line(), Some(overlap(20, 60, 0)), choice);
        let blocks: [&[InstrumentEvent]; 6] = [
            &[event(EventKind::NoteOn, 60, 127)],
            &[event(EventKind::NoteOff, 60, 0)],
            &[],
            &[event(EventKind::NoteOn, 62, 127)],
            &[event(EventKind::AllNotesOff, 0, 0)],
            &[event(EventKind::NoteOn, 64, 127)],
        ];
        blocks
            .iter()
            .flat_map(|events| run(&mut *processor, events))
            .collect()
    };
    let oracle = phrase("sustain");
    for (block, sounding) in [(0, 0.5), (3, 0.25), (5, 0.125)] {
        close(oracle[(block + 1) * BLOCK - 1][0], sounding);
    }
    assert_eq!(phrase("legato"), oracle);
}

#[test]
fn a_note_that_starts_where_the_last_one_ends_is_joined() {
    // The engine sends the note-offs of a frame before its note-ons. With a gap of 0, a note
    // that starts on the frame of the legato note's release joins it exactly as one that
    // starts while it is held; a frame later it plays its attack, as in `sustain`.
    let spec = Some(overlap(20, 0, 0));
    let mut held = legato_sampler(line(), spec, "legato");
    run(&mut *held, &[event(EventKind::NoteOn, 60, 127)]);
    let overlapped = run(&mut *held, &[event(EventKind::NoteOn, 62, 127)]);
    let mut abutting = legato_sampler(line(), spec, "legato");
    run(&mut *abutting, &[event(EventKind::NoteOn, 60, 127)]);
    let joined = run(
        &mut *abutting,
        &[
            event(EventKind::NoteOff, 60, 0),
            event(EventKind::NoteOn, 62, 127),
        ],
    );
    assert_eq!(joined, overlapped);

    let later = |choice| {
        let mut processor = legato_sampler(line(), spec, choice);
        run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
        run(
            &mut *processor,
            &[
                event(EventKind::NoteOff, 60, 0),
                at_offset(1, event(EventKind::NoteOn, 62, 127)),
            ],
        )
    };
    assert_eq!(later("legato"), later("sustain"));
}

#[test]
fn a_note_within_the_gap_joins_the_released_note_where_its_release_has_brought_it() {
    // 60 releases over 1 s. 62 starts 10 ms (480 frames) after the release, within the 10 ms
    // gap: from there 60 fades over the crossfade instead of its release, from the level its
    // release has reached, 1 - 480/48000, along the curve that sounds at that level, and 62
    // fades in. One frame later, beyond the gap, 62 plays its attack beside 60's release, as
    // in `sustain`.
    let mut bank = line();
    bank.regions[0].release = 1.0;
    let spec = Some(overlap(20, 0, 10));
    let mut processor = legato_sampler(bank.clone(), spec, "legato");
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    let block = run(
        &mut *processor,
        &[
            event(EventKind::NoteOff, 60, 0),
            at_offset(480, event(EventKind::NoteOn, 62, 127)),
        ],
    );
    let release = f64::from(RATE);
    assert_frames(&block[..480], |index| 0.5 * faded_from(1.0, index, release));
    let reached = 1.0 - 480.0 / release;
    let on_the_curve = reached.asin() / FRAC_PI_2;
    let frames = frames_of(20);
    assert_frames(&block[480..], |index| {
        let left = faded_from(on_the_curve, index, frames);
        0.5 * (left * FRAC_PI_2).sin() + 0.25 * crossfade(index, frames).1
    });

    let later = |choice| {
        let mut processor = legato_sampler(bank.clone(), spec, choice);
        run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
        run(
            &mut *processor,
            &[
                event(EventKind::NoteOff, 60, 0),
                at_offset(481, event(EventKind::NoteOn, 62, 127)),
            ],
        )
    };
    assert_eq!(later("legato"), later("sustain"));
}

#[test]
fn a_crossfade_from_a_note_in_its_attack_starts_where_it_is_heard() {
    // 60 rises over a 100 ms attack (4800 frames); 62 joins it 20 ms in, where 60 sounds at a
    // fifth of its level. 60 fades from there along the curve that sounds at that level, so
    // its level does not jump.
    let mut bank = line();
    bank.regions[0].attack = 0.1;
    let mut processor = legato_sampler(bank, Some(overlap(20, 0, 0)), "legato");
    let block = run(
        &mut *processor,
        &[
            event(EventKind::NoteOn, 60, 127),
            at_offset(960, event(EventKind::NoteOn, 62, 127)),
        ],
    );
    assert_frames(&block[..960], |index| 0.5 * f64::from(index + 1) / 4800.0);
    let on_the_curve = 0.2_f64.asin() / FRAC_PI_2;
    let frames = frames_of(20);
    assert_frames(&block[960..], |index| {
        let left = faded_from(on_the_curve, index, frames);
        0.5 * (left * FRAC_PI_2).sin() + 0.25 * crossfade(index, frames).1
    });
}

/// [`line`] with a release sample of key 60 that plays 0.0625.
fn line_with_tail() -> SampleBank {
    let mut bank = line();
    bank.samples.push(constant(0.0625));
    bank.regions.push(Region {
        key_lo: 60,
        key_hi: 60,
        trigger: Trigger::Release,
        ..region(3)
    });
    bank
}

#[test]
fn a_note_the_legato_leaves_starts_no_release_samples() {
    // 62 joins the held 60, which is released after the crossfade. In `legato`, 60's release
    // sample does not start, at the release or when the pedal lifts, and the pedal does not
    // hold 60 through the crossfade: 62 sounds alone. In `sustain` the release sample starts
    // beside 62.
    for pedal_down in [false, true] {
        let play = |choice| {
            let mut processor = legato_sampler(line_with_tail(), Some(overlap(20, 0, 0)), choice);
            if pedal_down {
                run(&mut *processor, &[pedal(true)]);
            }
            run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
            run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)]);
            let joined = voices(&*processor);
            run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
            let lifted = run(&mut *processor, &[pedal(false)]);
            (joined, level(&lifted), voices(&*processor))
        };
        let (joined, alone, sounding) = play("legato");
        assert_eq!((joined, sounding), (1.0, 1.0), "pedal down: {pedal_down}");
        close(alone, 0.25);
        let (_, sustained, _) = play("sustain");
        close(sustained, 0.25 + 0.0625);
    }
}

#[test]
fn legato_run_keeps_two_voices_per_layer() {
    // A run faster than the 90 ms crossfade: a note every 500 frames (10.4 ms), each released
    // 250 frames after the next starts. Each transition shortens the fade of the note before
    // the one it leaves to 10 ms (480 frames), so 500 frames after each start only that note
    // and the one it left sound.
    let bank = SampleBank {
        samples: vec![constant(0.5)],
        regions: vec![Region {
            key_lo: 48,
            key_hi: 72,
            ..region(0)
        }],
        ..SampleBank::default()
    };
    let mut processor = legato_sampler(bank, Some(overlap(90, 0, 0)), "legato");
    let keys = [60, 62, 64, 65, 67, 69, 71, 72, 71, 69, 67, 65];
    let mut previous = None;
    for (index, key) in keys.into_iter().enumerate() {
        let mut events = vec![event(EventKind::NoteOn, key, 127)];
        if let Some(previous) = previous {
            events.push(at_offset(250, event(EventKind::NoteOff, previous, 0)));
        }
        run_frames(&mut *processor, &events, 500);
        let expected = if index == 0 { 1.0 } else { 2.0 };
        assert_eq!(voices(&*processor), expected, "after note {index}");
        previous = Some(key);
    }
}

#[test]
fn a_note_joins_by_its_own_legato_articulation() {
    // On a device that plays `sustain`, a note marked `legato` joins the held note before it,
    // as every note does under the `legato` choice: the new note's legato decides. On a device
    // that plays `legato`, a note marked `sustain` joins nothing: it sounds beside the held
    // note, as in `sustain`; and the next note, which plays `legato`, joins it.
    let spec = Some(overlap(20, 0, 0));
    let mut by_note = legato_sampler(line(), spec, "sustain");
    run(&mut *by_note, &[event(EventKind::NoteOn, 60, 127)]);
    let joined = run(&mut *by_note, &[marked(62, LEGATO_CHOICE)]);
    let mut by_device = legato_sampler(line(), spec, "legato");
    run(&mut *by_device, &[event(EventKind::NoteOn, 60, 127)]);
    assert_eq!(
        joined,
        run(&mut *by_device, &[event(EventKind::NoteOn, 62, 127)])
    );

    let mut beside = legato_sampler(line(), spec, "legato");
    run(&mut *beside, &[event(EventKind::NoteOn, 60, 127)]);
    let both = run(&mut *beside, &[marked(62, SUSTAIN_CHOICE)]);
    close(level(&both), 0.75);
    assert_eq!(voices(&*beside), 2.0);
    let block = run(&mut *beside, &[event(EventKind::NoteOn, 64, 127)]);
    let frames = frames_of(20);
    assert_frames(&block, |index| {
        let (left, entered) = crossfade(index, frames);
        0.5 + 0.25 * left + 0.125 * entered
    });
}

#[test]
fn the_legato_target_without_an_overlap_claim_joins_nothing() {
    // TST-03: a legato target of a preset that claims no overlap legato (its own regions play
    // legato, or nothing is claimed) has no times to join with: the overlapping phrase plays
    // as in `sustain`, both notes sounding.
    let phrase = |choice| -> Vec<[f64; 2]> {
        let mut processor = legato_sampler(line(), None, choice);
        let blocks: [&[InstrumentEvent]; 4] = [
            &[event(EventKind::NoteOn, 60, 127)],
            &[event(EventKind::NoteOn, 62, 127)],
            &[event(EventKind::NoteOff, 60, 0)],
            &[event(EventKind::NoteOff, 62, 0)],
        ];
        blocks
            .iter()
            .flat_map(|events| run(&mut *processor, events))
            .collect()
    };
    let oracle = phrase("sustain");
    close(oracle[2 * BLOCK - 1][0], 0.75);
    assert_eq!(phrase("legato"), oracle);
}

#[test]
fn a_joined_note_releases_as_any_note() {
    // Once its crossfade is done, a joined note plays at full level and fades out over its
    // release linearly, as any note: the same frames as in `sustain`, where the note it
    // overlapped has ended too.
    let release = |choice| {
        let mut processor = legato_sampler(line(), Some(overlap(20, 0, 0)), choice);
        run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
        run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)]);
        run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
        run(&mut *processor, &[event(EventKind::NoteOff, 62, 0)])
    };
    let legato = release("legato");
    // The region's shortest release, 5 ms (240 frames).
    assert_frames(&legato, |index| 0.25 * faded_from(1.0, index, 240.0));
    assert_eq!(legato, release("sustain"));
}

#[test]
fn an_exclusive_group_keeps_the_crossfade_of_the_left_note() {
    // In a monophonic patch (every region in group 1, which switches them off), the joined
    // note's group would switch the held one off in 10 ms; the transition keeps its crossfade.
    // Without the legato the group cuts it.
    let mut bank = line();
    for region in &mut bank.regions {
        region.group = 1;
        region.off_by = 1;
    }
    let spec = Some(overlap(20, 0, 0));
    let mut legato = legato_sampler(bank.clone(), spec, "legato");
    run(&mut *legato, &[event(EventKind::NoteOn, 60, 127)]);
    let block = run(&mut *legato, &[event(EventKind::NoteOn, 62, 127)]);
    let frames = frames_of(20);
    assert_frames(&block, |index| {
        let (left, entered) = crossfade(index, frames);
        0.5 * left + 0.25 * entered
    });

    let mut sustain = legato_sampler(bank, spec, "sustain");
    run(&mut *sustain, &[event(EventKind::NoteOn, 60, 127)]);
    let cut = run(&mut *sustain, &[event(EventKind::NoteOn, 62, 127)]);
    assert_frames(&cut, |index| {
        0.5 * faded_from(1.0, index, 480.0) + 0.25 * (f64::from(index + 1) / 24.0).min(1.0)
    });
}

#[test]
fn a_crossfade_lasts_at_least_five_ms() {
    // TST-03: a mapping built with a crossfade of 0 ms (the manifest allows 10 to 500) still
    // crossfades, over the shortest fade-out, 5 ms (240 frames), so the transition never clicks.
    let mut processor = legato_sampler(line(), Some(overlap(0, 0, 0)), "legato");
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    let joined = run(&mut *processor, &[event(EventKind::NoteOn, 62, 127)]);
    assert_frames(&joined, |index| {
        let (left, entered) = crossfade(index, 240.0);
        0.5 * left + 0.25 * entered
    });
}

// --- Multi-patch sets ---

/// Three patches of one instrument in one bank, each on the keys from 36 up: patch 0 plays
/// 0.5, patch 1 plays 0.25, and patch 2 is a keyswitch patch whose switch 24 (its default)
/// plays 0.125 and switch 25 plays 0.0625. Keys 24 and 25 switch in patch 2 only.
fn three_patches() -> SampleBank {
    let on = |member, sample| Region {
        member,
        key_lo: 36,
        ..region(sample)
    };
    let switched = |sample, switch| Region {
        sw_last: Some(switch),
        ..on(2, sample)
    };
    SampleBank {
        samples: vec![
            constant(0.5),
            constant(0.25),
            constant(0.125),
            constant(0.0625),
        ],
        regions: vec![on(0, 0), on(1, 1), switched(2, 24), switched(3, 25)],
        keyswitches: switches(&[(24, 25)], None),
        member_keyswitches: vec![
            Keyswitches::NONE,
            Keyswitches::NONE,
            switches(&[(24, 25)], Some(24)),
        ],
        ..SampleBank::default()
    }
}

/// The target played by patch `member`, reached there with `selector`.
fn in_patch(member: u8, selector: Option<Selector>) -> ArticulationTarget {
    ArticulationTarget {
        member,
        ..ArticulationTarget::select(selector)
    }
}

/// The mapping of the tests' choices onto [`three_patches`]: `ordinario` patch `ordinario` as
/// it is, `sustain` patch 2 as it is (with the clips' keyswitch), `staccato` patch 1 and
/// `pizzicato` patch 2 with its switch 25.
fn set_targets(ordinario: u8) -> SamplerMapping {
    SamplerMapping::new(
        vec![
            None,
            Some(in_patch(ordinario, None)),
            Some(in_patch(2, None)),
            Some(in_patch(1, None)),
            Some(in_patch(2, Some(Selector::Keyswitch(25)))),
        ],
        DynamicsDrive::Velocity,
        None,
        0,
    )
}

#[test]
fn each_patch_of_a_bank_keeps_its_own_keyswitches() {
    let set = three_patches();
    assert_eq!(set.member_count(), 3);
    assert_eq!(*set.keyswitches_of(2), switches(&[(24, 25)], Some(24)));
    assert_eq!(*set.keyswitches_of(0), Keyswitches::NONE);
    assert_eq!(
        *set.keyswitches_of(3),
        Keyswitches::NONE,
        "no patch has that number"
    );
    // A bank of one patch is patch 0, with the bank's keyswitches.
    let one = four_articulations();
    assert_eq!(one.member_count(), 1);
    assert_eq!(*one.keyswitches_of(0), one.keyswitches);
    assert_eq!(*one.keyswitches_of(1), Keyswitches::NONE);
    // `auto` plays the patch of the ordinary sound, else the first.
    assert_eq!(set_targets(1).default_member(), 1);
    assert_eq!(SamplerMapping::default().default_member(), 0);
}

#[test]
fn a_set_plays_the_patch_each_articulation_names() {
    let mut processor = mapped(three_patches(), set_targets(0), &Params::new());
    // `auto` plays the patch of the ordinary sound alone, not every patch of the set.
    close(note(&mut *processor), 0.5);
    for (choice, expected) in [
        ("staccato", 0.25),
        ("pizzicato", 0.0625),
        // Patch 2 as it is: its default switch.
        ("sustain", 0.125),
        ("ordinario", 0.5),
        ("auto", 0.5),
    ] {
        // The switch is a parameter change at a block start, then the notes, both without
        // allocating (RT-02).
        set(&mut *processor, &articulation(choice));
        let quiet = run(&mut *processor, &[]);
        assert!(quiet.iter().flatten().all(|v| *v == 0.0), "{choice}");
        let sounded = run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
        close(level(&sounded), expected);
        assert_eq!(voices(&*processor), 1.0, "one patch sounds for {choice}");
        run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
    }
}

#[test]
fn auto_on_a_set_plays_the_patch_of_the_ordinary_sound() {
    let mut processor = mapped(three_patches(), set_targets(1), &Params::new());
    close(note(&mut *processor), 0.25);
    assert_eq!(voices(&*processor), 0.0, "the note has faded");
    // A set about which nothing is claimed plays its first patch.
    let mut unmapped = sampler(three_patches());
    let sounded = run(&mut *unmapped, &[event(EventKind::NoteOn, 60, 127)]);
    close(level(&sounded), 0.5);
    assert_eq!(voices(&*unmapped), 1.0);
}

#[test]
fn an_automated_articulation_switches_patches_from_its_frame() {
    // In one block under `auto` (patch 0, 0.5): a note at frame 0, a change to `staccato`
    // (patch 1, 0.25) at frame 700 and a note at frame 1400. The first note rings on in its
    // patch while the second plays the new one; the whole block runs without allocating.
    let mut processor = mapped(three_patches(), set_targets(0), &Params::new());
    let block = run(
        &mut *processor,
        &[
            event(EventKind::NoteOn, 60, 127),
            choice_event(700, "articulation", 3),
            at_offset(1400, event(EventKind::NoteOn, 64, 127)),
        ],
    );
    let at = |frame: usize| block.get(frame).expect("a frame")[0];
    close(at(1399), 0.5);
    close(level(&block), 0.5 + 0.25);
    assert_eq!(voices(&*processor), 2.0);
}

#[test]
fn a_note_plays_the_patch_of_its_own_articulation() {
    let mut processor = mapped(three_patches(), set_targets(0), &Params::new());
    // `staccato` (3) on this note alone: patch 1; `pizzicato` (4): patch 2 with switch 25.
    close(marked_note(&mut *processor, 3), 0.25);
    close(marked_note(&mut *processor, 4), 0.0625);
    // The next note without one plays the parameter's patch again.
    close(note(&mut *processor), 0.5);
}

#[test]
fn each_patch_keeps_the_last_keyswitch_the_clips_selected() {
    let mut processor = mapped(three_patches(), set_targets(0), &Params::new());
    // Under `auto` (patch 0), a clip's keyswitch 25 is no switch of patch 0, which plays on,
    // but it selects switch 25 in patch 2.
    run(&mut *processor, &[event(EventKind::KeySwitch, 25, 0)]);
    close(note(&mut *processor), 0.5);
    set(&mut *processor, &articulation("sustain"));
    close(note(&mut *processor), 0.0625);
    // A press of key 24 is a note in patch 0 (silent there, below its keys) and a switch in
    // patch 2.
    set(&mut *processor, &articulation("auto"));
    run(&mut *processor, &[event(EventKind::NoteOn, 24, 127)]);
    run(&mut *processor, &[event(EventKind::NoteOff, 24, 0)]);
    set(&mut *processor, &articulation("sustain"));
    close(note(&mut *processor), 0.125);
    // `pizzicato` forces switch 25 in patch 2; back on `sustain` the forced switch is gone, and
    // patch 2 plays the clips' last switch, 24.
    set(&mut *processor, &articulation("pizzicato"));
    close(note(&mut *processor), 0.0625);
    set(&mut *processor, &articulation("sustain"));
    close(note(&mut *processor), 0.125);
    // All notes off brings every patch back to its default switch.
    run(&mut *processor, &[event(EventKind::KeySwitch, 25, 0)]);
    run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    close(note(&mut *processor), 0.125);
}

/// Two patches with a release tail each: patch 0 plays 0.5 and its tail 0.031 25, patch 1
/// plays 0.25 and its tail 0.015 625.
fn two_patches_with_tails() -> SampleBank {
    let on = |member, sample| Region {
        member,
        ..region(sample)
    };
    let tail = |member, sample| Region {
        trigger: Trigger::Release,
        ..on(member, sample)
    };
    SampleBank {
        samples: vec![
            constant(0.5),
            constant(0.031_25),
            constant(0.25),
            constant(0.015_625),
        ],
        regions: vec![on(0, 0), tail(0, 1), on(1, 2), tail(1, 3)],
        member_keyswitches: vec![Keyswitches::NONE; 2],
        ..SampleBank::default()
    }
}

/// `staccato` plays patch 1; `ordinario` is patch 0.
fn tail_targets() -> SamplerMapping {
    SamplerMapping::new(
        vec![None, Some(in_patch(0, None)), None, Some(in_patch(1, None))],
        DynamicsDrive::Velocity,
        None,
        0,
    )
}

#[test]
fn a_release_plays_the_tail_of_the_patch_its_note_played() {
    // A note under `auto` (patch 0) is released after the articulation moved to `staccato`
    // (patch 1): its tail is patch 0's.
    let mut processor = mapped(two_patches_with_tails(), tail_targets(), &Params::new());
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)])),
        0.5,
    );
    set(&mut *processor, &articulation("staccato"));
    let released = run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
    close(level(&released), 0.031_25);
    assert_eq!(voices(&*processor), 1.0, "the tail alone sounds");
    // The same under the sustain pedal: the tail waits for the pedal and keeps the note's
    // patch, whatever the articulation is when the pedal lifts.
    let mut processor = mapped(two_patches_with_tails(), tail_targets(), &Params::new());
    run(
        &mut *processor,
        &[pedal(true), event(EventKind::NoteOn, 60, 127)],
    );
    set(&mut *processor, &articulation("staccato"));
    run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
    let lifted = run(&mut *processor, &[pedal(false)]);
    close(level(&lifted), 0.031_25);
    // A note pressed under `staccato` releases with patch 1's tail.
    let mut processor = mapped(
        two_patches_with_tails(),
        tail_targets(),
        &articulation("staccato"),
    );
    run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)]);
    let released = run(&mut *processor, &[event(EventKind::NoteOff, 60, 0)]);
    close(level(&released), 0.015_625);
}

// --- A dense passage: the microbenchmark's work under the real-time rules ---

#[path = "support/dense_sampler.rs"]
mod dense_sampler;

use dense_sampler::{DensePassage, Load, PERIOD, WARM_UP};

/// The voices `processor` reports, read as the render thread reads a report: inside
/// `assert_no_alloc` (RT-02).
fn reported_voices(processor: &dyn Processor) -> f64 {
    let report = assert_no_alloc(|| processor.report());
    report
        .entries()
        .iter()
        .find(|(name, _)| *name == "voices")
        .map(|(_, value)| *value)
        .expect("the sampler reports its voices")
}

/// Plays the blocks `blocks` of `passage`, each inside `assert_no_alloc` (RT-02), and returns
/// their output.
fn play_passage(passage: &mut DensePassage, blocks: std::ops::Range<usize>) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    for block in blocks {
        assert_no_alloc(|| passage.play(block));
        out.extend_from_slice(passage.output());
    }
    out
}

#[test]
fn a_dense_passage_allocates_nothing_and_holds_its_voices() {
    // Each case of the sampler's microbenchmark (`benches/sampler.rs`), the controller table
    // and the overlap legato on and off, with the `dynamics`, `articulation`, `mute` and
    // `vibrato` changing as automation changes them: every parameter set, block and report
    // runs inside `assert_no_alloc` (RT-02). The voices follow from the passage's timing
    // (TST-07): the chord and the melody hold 62. At the end of a block whose melody note
    // joins the note before, that note still fades under the 20 ms crossfade (960 frames,
    // more than the block), so all 64 sound; a melody note that attacks instead releases the
    // note before at frame 64 over 5 ms (240 frames), which has ended by then. A block later
    // the crossfade has ended too. No voice is ever stolen, and none is lost.
    for load in Load::ALL {
        let name = load.name();
        let mut passage = DensePassage::new(load);
        for block in 0..WARM_UP + 3 * PERIOD {
            assert_no_alloc(|| passage.play(block));
            let joins = block
                .checked_sub(WARM_UP)
                .is_some_and(|beat| beat % 2 == 0 && load.legato);
            let expected = if joins { 64.0 } else { 62.0 };
            assert_eq!(
                reported_voices(&*passage.processor),
                expected,
                "{name}: block {block}"
            );
            assert!(
                passage.output().iter().flatten().all(|x| x.is_finite()),
                "{name}: block {block} is finite"
            );
        }
    }
}

#[test]
fn a_reset_plays_the_dense_passage_again_as_a_new_sampler() {
    // TST-07: a reset, as on play from stop, brings back every part of the sampler's state
    // that the passage changes: the voices, the keys, the legato's last note and frame clock,
    // the controller table and the parameters' targets. Reset in the middle of a period (the
    // `articulation` at `staccato`, the `mute` at `con_sordino`, `dynamics` at 0.8), it plays
    // the passage again, from its first parameter set, bit for bit as it played from a new
    // sampler. The reset runs inside `assert_no_alloc` (RT-02).
    let blocks = 0..WARM_UP + PERIOD + 4;
    let mut played: Vec<(&str, Vec<[f64; 2]>)> = Vec::new();
    for load in Load::ALL {
        let name = load.name();
        let mut passage = DensePassage::new(load);
        let first = play_passage(&mut passage, blocks.clone());
        let peak = first.iter().flatten().fold(0.0, |p: f64, x| p.max(x.abs()));
        assert!(peak > 0.1, "{name}: the passage sounds: {peak}");
        assert_no_alloc(|| passage.processor.reset());
        assert_eq!(reported_voices(&*passage.processor), 0.0, "{name}: silent");
        let again = play_passage(&mut passage, blocks.clone());
        assert!(again == first, "{name}: the passage after the reset");
        played.push((name, first));
    }
    // The four cases do different work, so each sounds different from every other: the
    // controllers move the layers' gains, and the legato crossfades the melody's notes in place
    // of their attacks.
    for (index, (name, case)) in played.iter().enumerate() {
        for (earlier, before) in played.iter().take(index) {
            assert!(case != before, "{name} sounds as {earlier}");
        }
    }
}
