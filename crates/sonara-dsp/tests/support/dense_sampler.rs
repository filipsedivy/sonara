//! A dense passage on one `sonara.sampler`: the work the sampler's microbenchmark
//! (`benches/sampler.rs`) measures, and that `tests/sampler.rs` plays under `assert_no_alloc`
//! (RT-02), so the block that is measured is the block that is shown to allocate nothing.
//!
//! **The instrument** is a set of two patches, as the application loads a preset whose
//! articulations lie in separate patches: patch 0 plays `sustain` and `legato`, patch 1
//! `staccato` (in a velocity zone), and `sustain` too while the `mute` is `con_sordino`. Each
//! patch has two layers on every pair of keys from C2 to B6 (36 to 95), at the lower key's
//! pitch: 120 regions over 120 looping stereo samples of half a second at 48 kHz, deterministic
//! noise (23 MB). The preset joins its `legato` notes by the overlap legato, over 20 ms from
//! 50 ms into the sample.
//!
//! **The passage.** A chord of 30 keys (C2 to F4) holds 60 voices on two MIDI channels, and a
//! melody on a third channel alternates two keys, one note every second block, each note
//! released 64 frames after the next starts. Every note starts both layers of its keys, so the
//! chord and the melody hold 62 voices, and 64 while a melody note joins the one before: the
//! sampler's whole polyphony, and none is stolen. Two choices make the four cases:
//!
//! - **controllers**: the layers crossfade on the modulation wheel (CC 1) and their level
//!   follows it, as libraries play dynamics, and a pedal gate selects them; CC 1 moves every
//!   block on each channel, so every voice ramps to a new gain. Without, the layers play at full
//!   gain and no controller moves.
//! - **legato**: the melody's notes are marked `legato` and each joins the note before over the
//!   crossfade, along the equal-power curve; without, they are marked `sustain`, each attacks,
//!   and the note before releases over 5 ms.
//!
//! In every case the `dynamics` parameter steps twice in each period of eight blocks, and the
//! sounding voices follow it; the `articulation`, `mute` and `vibrato` choices change inside
//! blocks as choice events, as the render graph plays a sampler's automation lanes, and each
//! change looks its target up again. While the `mute` is `con_sordino`, the melody's `sustain`
//! notes play patch 1.

use std::sync::Arc;

use sonara_core::model::device::{ParamValue, Params};
use sonara_core::plugins::{PluginDescriptor, SamplerChoices, sampler_descriptor};
use sonara_dsp::bank::{
    ArticulationTarget, CcCrossfade, CcDepth, CcRange, Crossfade, DynamicsDrive, Keyswitches,
    LegatoSpec, LoopMode, Region, Sample, SampleBank, SamplerMapping, SamplerSetup, Selector,
    TargetTable,
};
use sonara_dsp::processor::{
    BlockContext, DeviceState, EventKind, InstrumentEvent, ParamSlots, ProcessSpec, Processor,
};
use sonara_dsp::processors::{create, param_slots};

/// The frames of a block: the player's (`sonara_rt::BLOCK_FRAMES`).
pub const BLOCK: usize = 512;
/// The blocks that press the chord and the melody's first note, and let their attacks end.
pub const WARM_UP: usize = 2;
/// The blocks after which the passage repeats, once warmed up.
pub const PERIOD: usize = 8;

const RATE: u32 = 48_000;
/// The frames of every sample, and the frame its loop starts at; it loops to its end.
const SAMPLE_FRAMES: usize = 24_000;
const LOOP_START: usize = 4_800;
/// The lowest key of the patches, the number of key pairs they map, and the chord's keys from
/// the lowest up.
const LOW_KEY: u8 = 36;
const ZONES: u8 = 30;
const CHORD_KEYS: u8 = 30;
/// The melody's two keys, its channel, and the frame of each block on which the note before
/// the new one is released.
const MELODY: [u8; 2] = [72, 76];
const MELODY_CHANNEL: u8 = 2;
const MELODY_RELEASE: usize = 64;
/// The modulation wheel's value in each block of the period.
const MODULATION: [i32; PERIOD] = [0, 32, 64, 96, 127, 96, 64, 32];

/// The `articulation` choices of [`descriptor`]: `auto`, `ordinario`, then these.
const SUSTAIN: u16 = 2;
const STACCATO: u16 = 3;
const LEGATO: u16 = 4;
/// The `mute` choice `con_sordino` and the `vibrato` choice `non_vibrato`; 0 is `auto`.
const CON_SORDINO: u16 = 1;
const NON_VIBRATO: u16 = 1;

/// One case of the passage.
#[derive(Debug, Clone, Copy)]
pub struct Load {
    /// Whether the layers follow the controller table: they crossfade and change level on the
    /// modulation wheel, which moves every block, and a pedal gate selects them.
    pub controllers: bool,
    /// Whether the melody's notes play `legato`, each joining the note before, or `sustain`.
    pub legato: bool,
}

impl Load {
    /// The four cases.
    pub const ALL: [Self; 4] = [
        Self {
            controllers: false,
            legato: false,
        },
        Self {
            controllers: true,
            legato: false,
        },
        Self {
            controllers: false,
            legato: true,
        },
        Self {
            controllers: true,
            legato: true,
        },
    ];

    /// The case's name in the microbenchmark and in test messages.
    pub fn name(self) -> &'static str {
        match (self.controllers, self.legato) {
            (false, false) => "plain",
            (true, false) => "controllers",
            (false, true) => "legato",
            (true, true) => "controllers_legato",
        }
    }
}

/// The parameters and events of one block of the passage.
struct Step {
    params: ParamSlots,
    events: Vec<InstrumentEvent>,
}

/// A prepared sampler and the passage it plays.
pub struct DensePassage {
    /// The sampler: its report, and a reset back to the start of the passage.
    pub processor: Box<dyn Processor>,
    /// The warm-up's blocks, then one period's.
    steps: Vec<Step>,
    output: Vec<[f64; 2]>,
}

impl DensePassage {
    /// Setup: a prepared sampler with the passage's instrument and its starting parameters, and
    /// the passage of `load`, which [`DensePassage::play`] starts with block 0.
    pub fn new(load: Load) -> Self {
        let descriptor = descriptor();
        let steps = script(load, &descriptor);
        let mut processor = create("sonara.sampler").expect("the sampler is built in");
        processor.prepare(ProcessSpec {
            sample_rate: RATE,
            block_size: BLOCK,
            channels: 2,
        });
        processor.set_state(&DeviceState {
            sampler: Some(Arc::new(SamplerSetup {
                bank: Arc::new(bank(load.controllers)),
                mapping: Arc::new(mapping()),
            })),
            ..DeviceState::default()
        });
        let start = steps.first().expect("the passage has blocks");
        processor.set_params(&start.params);
        Self {
            processor,
            steps,
            output: vec![[0.0; 2]; BLOCK],
        }
    }

    /// Plays block `block` of the passage, counted from its start, as the render graph plays a
    /// block: its parameters, then its events. Real-time: it allocates nothing.
    pub fn play(&mut self, block: usize) {
        let index = if block < WARM_UP {
            block
        } else {
            WARM_UP + (block - WARM_UP) % PERIOD
        };
        let step = self.steps.get(index).expect("an index within the steps");
        self.processor.set_params(&step.params);
        let ctx = BlockContext {
            frame: 0,
            sample_rate: RATE,
            beat: 0.0,
            bpm: 120.0,
            playing: true,
            sidechain: None,
            events: &step.events,
        };
        self.processor.process(&mut self.output, &ctx);
    }

    /// The output of the last block played.
    pub fn output(&self) -> &[[f64; 2]] {
        &self.output
    }
}

/// The sampler's descriptor: one preset, the articulations `sustain`, `staccato` and `legato`,
/// the mute `con_sordino` and the vibrato `non_vibrato`.
fn descriptor() -> PluginDescriptor {
    let owned = |id: &str| vec![id.to_owned()];
    sampler_descriptor(&SamplerChoices {
        presets: owned("lib/dense"),
        articulations: ["sustain", "staccato", "legato"]
            .map(str::to_owned)
            .to_vec(),
        mutes: owned("con_sordino"),
        vibratos: owned("non_vibrato"),
        ..SamplerChoices::default()
    })
    .expect("one preset")
}

/// The preset's mapping (see the module docs). No vibrato changes a target.
fn mapping() -> SamplerMapping {
    let targets = TargetTable::tabulate(5, 2, 2, |choice, passage| {
        let muted = passage.mute == CON_SORDINO;
        let target = |member, selector, legato| ArticulationTarget {
            member,
            selector,
            legato,
        };
        match choice {
            SUSTAIN | 1 => Some(target(u8::from(muted), None, false)),
            STACCATO => {
                let hi = if muted { 60 } else { 90 };
                Some(target(1, Some(Selector::Velocity { lo: 1, hi }), false))
            }
            LEGATO => Some(target(0, None, true)),
            _ => None,
        }
    });
    SamplerMapping::new(
        targets,
        DynamicsDrive::Velocity,
        Some(LegatoSpec {
            crossfade_ms: 20,
            start_offset_ms: 50,
            gap_ms: 0,
        }),
        0,
    )
}

/// Half a second of deterministic stereo noise (a 64-bit LCG), uniform in [−0.1, 0.1).
fn noise(seed: u64) -> Sample {
    let mut state = seed;
    let data = (0..2 * SAMPLE_FRAMES)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let top = u16::try_from(state >> 48).expect("16 bits fit u16");
            (f32::from(top) / 65_536.0 - 0.5) * 0.2
        })
        .collect();
    Sample {
        rate: RATE,
        channels: 2,
        data,
    }
}

/// The two patches of the set (see the module docs), each layer with its own sample; with
/// `controllers`, the soft layer fades out and the loud one in on the modulation wheel, whose
/// value also raises both by up to 6 dB, and both start only while the sustain pedal is up.
fn bank(controllers: bool) -> SampleBank {
    let mut samples = Vec::new();
    let mut regions = Vec::new();
    for member in 0..2_u8 {
        for zone in 0..ZONES {
            let key = LOW_KEY + 2 * zone;
            for loud in [false, true] {
                let fade = if loud {
                    Crossfade {
                        in_lo: 0,
                        in_hi: 127,
                        ..Crossfade::NONE
                    }
                } else {
                    Crossfade {
                        out_lo: 0,
                        out_hi: 127,
                        ..Crossfade::NONE
                    }
                };
                let mut region = Region {
                    sample: samples.len(),
                    member,
                    key_lo: key,
                    key_hi: key + 1,
                    pitch_keycenter: key,
                    loop_mode: LoopMode::Continuous,
                    loop_start: LOOP_START,
                    loop_end: SAMPLE_FRAMES,
                    end: SAMPLE_FRAMES,
                    release: 0.005,
                    ..Region::default()
                };
                if controllers {
                    region.cc_crossfades = Box::new([CcCrossfade {
                        controller: 1,
                        fade,
                    }]);
                    region.cc_volume = Box::new([CcDepth {
                        controller: 1,
                        depth: 6.0,
                    }]);
                    region.cc_gates = Box::new([CcRange {
                        controller: 64,
                        lo: 0,
                        hi: 63,
                    }]);
                }
                let seed = u64::try_from(samples.len()).expect("a sample index fits u64");
                samples.push(noise(seed.wrapping_add(1)));
                regions.push(region);
            }
        }
    }
    let bytes = samples.iter().map(|sample| sample.data.len() * 4).sum();
    SampleBank {
        samples,
        regions,
        member_keyswitches: vec![Keyswitches::NONE; 2],
        bytes,
        tail_seconds: 0.005,
        ..SampleBank::default()
    }
}

/// An event of `kind` at frame `offset` of its block, on channel 0.
fn event(offset: usize, kind: EventKind) -> InstrumentEvent {
    InstrumentEvent {
        offset,
        kind,
        pitch: 0,
        velocity: 0,
        channel: 0,
        value: 0,
        number: 0,
        articulation: None,
    }
}

/// A key press of `pitch` at `velocity` on `channel` at frame `offset`, with the note's own
/// `articulation` choice, if any.
fn press(
    offset: usize,
    pitch: u8,
    velocity: u8,
    channel: u8,
    articulation: Option<u16>,
) -> InstrumentEvent {
    InstrumentEvent {
        pitch,
        velocity,
        channel,
        articulation,
        ..event(offset, EventKind::NoteOn)
    }
}

/// The choices and the dynamics in force.
#[derive(Clone, Copy)]
struct State {
    articulation: u16,
    mute: u16,
    vibrato: u16,
    dynamics: f64,
}

impl State {
    /// The parameter set of the state, as the engine passes it at a block start.
    fn params(self, descriptor: &PluginDescriptor) -> ParamSlots {
        let name = |param: &str, index: u16| {
            let spec = descriptor
                .params
                .iter()
                .find(|spec| spec.id == param)
                .expect("the sampler has the parameter");
            let choice = spec
                .choices
                .get(usize::from(index))
                .expect("a choice of the parameter");
            ParamValue::Str(choice.clone())
        };
        let mut params = Params::new();
        params.insert(
            "articulation".to_owned(),
            name("articulation", self.articulation),
        );
        params.insert("mute".to_owned(), name("mute", self.mute));
        params.insert("vibrato".to_owned(), name("vibrato", self.vibrato));
        params.insert("dynamics".to_owned(), ParamValue::Float(self.dynamics));
        param_slots(descriptor, &params).expect("valid parameters")
    }
}

/// A choice event at frame `offset`: the parameter `param` of `descriptor` takes the choice
/// `value`.
fn choice(
    descriptor: &PluginDescriptor,
    offset: usize,
    param: &str,
    value: u16,
) -> InstrumentEvent {
    let index = descriptor
        .params
        .iter()
        .position(|spec| spec.id == param)
        .expect("the sampler has the parameter");
    InstrumentEvent {
        number: u8::try_from(index).expect("a parameter index fits a byte"),
        value: i32::from(value),
        ..event(offset, EventKind::ParamChoice)
    }
}

/// The warm-up's blocks and one period's (see the module docs). Each block's parameters carry
/// the choices that the choice events of the blocks before left in force, as the engine's do,
/// and the period ends in the state it starts in.
fn script(load: Load, descriptor: &PluginDescriptor) -> Vec<Step> {
    let mut state = State {
        articulation: SUSTAIN,
        mute: 0,
        vibrato: 0,
        dynamics: 1.0,
    };
    let melody_choice = if load.legato { LEGATO } else { SUSTAIN };
    let mut steps = Vec::with_capacity(WARM_UP + PERIOD);
    for block in 0..WARM_UP + PERIOD {
        let mut events = Vec::new();
        if block == 0 {
            for key in LOW_KEY..LOW_KEY + CHORD_KEYS {
                events.push(press(0, key, 100, key % 2, None));
            }
            // Marked `sustain`, so it joins no chord note.
            events.push(press(0, MELODY[0], 90, MELODY_CHANNEL, Some(SUSTAIN)));
        }
        let mut choices = Vec::new();
        if let Some(beat) = block.checked_sub(WARM_UP) {
            if load.controllers {
                let value = *MODULATION.get(beat).expect("a block of the period");
                for channel in [0, 1, MELODY_CHANNEL] {
                    events.push(InstrumentEvent {
                        channel,
                        number: 1,
                        value,
                        ..event(0, EventKind::Cc)
                    });
                }
            }
            if beat % 2 == 0 {
                let new = MELODY[(beat / 2 + 1) % 2];
                let old = MELODY[(beat / 2) % 2];
                events.push(press(0, new, 90, MELODY_CHANNEL, Some(melody_choice)));
                events.push(InstrumentEvent {
                    pitch: old,
                    channel: MELODY_CHANNEL,
                    ..event(MELODY_RELEASE, EventKind::NoteOff)
                });
            }
            match beat {
                1 => state.dynamics = 0.8,
                5 => state.dynamics = 1.0,
                _ => {}
            }
            choices = match beat {
                2 => vec![(128, "articulation", STACCATO)],
                3 => vec![(256, "mute", CON_SORDINO)],
                4 => vec![(300, "vibrato", NON_VIBRATO)],
                6 => vec![(128, "articulation", SUSTAIN)],
                7 => vec![(256, "mute", 0), (300, "vibrato", 0)],
                _ => Vec::new(),
            };
        }
        let params = state.params(descriptor);
        for &(offset, param, value) in &choices {
            events.push(choice(descriptor, offset, param, value));
            match param {
                "articulation" => state.articulation = value,
                "mute" => state.mute = value,
                _ => state.vibrato = value,
            }
        }
        steps.push(Step { params, events });
    }
    steps
}
