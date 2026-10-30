//! `sonara.sampler` against synthetic banks: layers, round robin, release, release triggers,
//! the sustain pedal, keyswitches, exclusive groups, polyphony and the real-time rules (TST-10:
//! every check compares against a value that follows from the bank, not "it ran").
//!
//! The samples are constants, so a voice's steady-state output is its sample value times the
//! velocity gain `(velocity / 127)²`, which the tests compute independently of the engine.

use std::f64::consts::FRAC_1_SQRT_2;
use std::sync::Arc;

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use sonara_core::model::device::Params;
use sonara_core::plugins::{SamplerChoices, sampler_descriptor};
use sonara_dsp::bank::{
    CrossfadeCurve, Keyswitches, LoopMode, Region, Sample, SampleBank, Trigger, VelocityCrossfade,
};
use sonara_dsp::processor::{
    BlockContext, DeviceState, EventKind, InstrumentEvent, ProcessSpec, Processor,
};
use sonara_dsp::processors::{create, param_slots};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

fn descriptor() -> sonara_core::plugins::PluginDescriptor {
    sampler_descriptor(&SamplerChoices {
        presets: vec!["/library/violin.sfz".to_owned()],
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
    let mut processor = create("sonara.sampler").expect("the sampler exists");
    processor.prepare(ProcessSpec {
        sample_rate: RATE,
        block_size: BLOCK,
        channels: 2,
    });
    processor.set_state(&DeviceState {
        sample_bank: Some(Arc::new(bank)),
        ..DeviceState::default()
    });
    let slots = param_slots(&descriptor(), &Params::new()).expect("defaults");
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
fn fade_in(lo: u8, hi: u8, curve: CrossfadeCurve) -> VelocityCrossfade {
    VelocityCrossfade {
        in_lo: lo,
        in_hi: hi,
        curve,
        ..VelocityCrossfade::NONE
    }
}

/// A fade-out over `lo..hi` with `curve`.
fn fade_out(lo: u8, hi: u8, curve: CrossfadeCurve) -> VelocityCrossfade {
    VelocityCrossfade {
        out_lo: lo,
        out_hi: hi,
        curve,
        ..VelocityCrossfade::NONE
    }
}

/// Checks `crossfade` at each velocity from `first` against `expected`, to the five digits
/// sfizz's tests give.
fn assert_fade(crossfade: VelocityCrossfade, first: u8, expected: &[f64]) {
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
        assert_eq!(VelocityCrossfade::NONE.gain(velocity), 1.0, "{velocity}");
    }
    assert_eq!(VelocityCrossfade::default(), VelocityCrossfade::NONE);
    assert_eq!(
        Region::default().velocity_crossfade,
        VelocityCrossfade::NONE
    );
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
    // A fade-out of no width, or one whose bounds cross, is full up to its low bound.
    assert_fade(fade_out(60, 60, power), 59, &[1.0, 1.0, 0.0, 0.0]);
    assert_fade(fade_out(90, 70, power), 89, &[1.0, 1.0, 0.0]);
}

#[test]
fn complementary_crossfades_keep_power_or_amplitude() {
    // TST-07: a layer that fades out over the velocities another fades in over keeps the summed
    // power with the equal-power curve, and the summed amplitude with the gain curve, at every
    // velocity.
    for velocity in 0..=127 {
        let power = |fade: VelocityCrossfade| fade.gain(velocity).powi(2);
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

#[test]
fn all_notes_off_keeps_the_switch() {
    // Pins the current behaviour: all notes off (also sent on a seek or a loop wrap while
    // playing) silences the voices but keeps the last keyswitch, so the articulation chosen
    // before the jump stays active after it.
    let articulation = |sample, switch| Region {
        sw_last: Some(switch),
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
    // The default articulation sounds; the switch press changes only the next notes.
    let before = run(
        &mut *processor,
        &[
            event(EventKind::NoteOn, 60, 127),
            event(EventKind::NoteOn, 25, 127),
        ],
    );
    close(level(&before), 0.5);
    let silenced = run(&mut *processor, &[event(EventKind::AllNotesOff, 0, 0)]);
    close(level(&silenced), 0.0);
    assert_eq!(processor.report().entries(), [("voices", 0.0)]);
    close(
        level(&run(&mut *processor, &[event(EventKind::NoteOn, 60, 127)])),
        0.25,
    );
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
