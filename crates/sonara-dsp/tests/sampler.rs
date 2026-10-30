//! `sonara.sampler` against synthetic banks: layers, round robin, release, release triggers,
//! the sustain pedal, keyswitches, exclusive groups, polyphony and the real-time rules (TST-10: every check
//! compares against a value that follows from the bank, not "it ran").
//!
//! The samples are constants, so a voice's steady-state output is its sample value times the
//! velocity gain `(velocity / 127)²`, which the tests compute independently of the engine.

use std::sync::Arc;

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use sonara_core::model::device::Params;
use sonara_core::plugins::sampler_descriptor;
use sonara_dsp::bank::{Keyswitches, LoopMode, Region, Sample, SampleBank, Trigger};
use sonara_dsp::processor::{
    BlockContext, DeviceState, EventKind, InstrumentEvent, ProcessSpec, Processor,
};
use sonara_dsp::processors::{create, param_slots};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

fn descriptor() -> sonara_core::plugins::PluginDescriptor {
    sampler_descriptor(&["/library/violin.sfz".to_owned()]).expect("one patch")
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
