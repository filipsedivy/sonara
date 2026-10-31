//! Deferred setup through the public processor API.
//!
//! A live graph reuses processors across rebuilds and sends them changed parameters on the
//! render thread, from a rebuild (`set_params` with the edited values) and from automation
//! (`set_params` every block). These tests check that this is real-time safe for every built-in
//! processor in deferred mode (RT-02): each parameter in turn jumps to the value farthest from
//! its default and back, with a block processed after each jump, all inside `assert_no_alloc`,
//! which aborts on an allocation or a deallocation. The reverb's impulse response, the one
//! change that needs setup, is prepared outside and installed inside it.

use std::sync::Arc;

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use sonara_core::model::device::{ParamValue, Params};
use sonara_core::plugins::{ParamKind, ParamSpec, PluginDescriptor, builtin_registry};
use sonara_dsp::processor::{
    BlockContext, DeviceState, EventKind, ImpulseResponse, InstrumentEvent, ParamSlots,
    ProcessSpec, Processor,
};
use sonara_dsp::processors::{PORTED, create, param_slots, reusable};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const BLOCK: usize = 512;
const SPEC: ProcessSpec = ProcessSpec {
    sample_rate: 48_000,
    block_size: BLOCK,
    channels: 2,
};

const NOTES: [InstrumentEvent; 2] = [
    InstrumentEvent {
        offset: 0,
        kind: EventKind::NoteOn,
        pitch: 36,
        velocity: 100,
        channel: 0,
        value: 0,
        number: 0,
        articulation: None,
    },
    InstrumentEvent {
        offset: 7,
        kind: EventKind::NoteOn,
        pitch: 60,
        velocity: 90,
        channel: 0,
        value: 0,
        number: 0,
        articulation: None,
    },
];

/// The value farthest from the default: the far bound of a number, the other switch position,
/// another choice.
fn alternative(spec: &ParamSpec) -> ParamValue {
    match spec.kind {
        ParamKind::Float | ParamKind::Int => {
            let default = spec.default.as_f64().expect("a numeric default");
            let (low, high) = (
                spec.minimum.expect("a bound"),
                spec.maximum.expect("a bound"),
            );
            let far = if (default - low).abs() > (high - default).abs() {
                low
            } else {
                high
            };
            if spec.kind == ParamKind::Int {
                #[expect(clippy::cast_possible_truncation, reason = "integer bounds")]
                ParamValue::Int(far as i64)
            } else {
                ParamValue::Float(far)
            }
        }
        ParamKind::Bool => ParamValue::Bool(spec.default != ParamValue::Bool(true)),
        ParamKind::Enum => {
            let other = spec
                .choices
                .iter()
                .rev()
                .find(|c| spec.default != ParamValue::Str((*c).clone()))
                .expect("another choice");
            ParamValue::Str(other.clone())
        }
    }
}

fn slots(descriptor: &PluginDescriptor, values: &[(&str, ParamValue)]) -> ParamSlots {
    let values: Params = values
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect();
    param_slots(descriptor, &values).expect("valid values")
}

/// A one-second decaying stereo response: what a convolver loads from the project's media.
fn impulse_response() -> DeviceState {
    let samples = (0..48_000_u32)
        .flat_map(|i| {
            let v = (-f64::from(i) / 4_800.0).exp() * if i % 2 == 0 { 0.5 } else { -0.5 };
            [v, v * 0.8]
        })
        .collect();
    DeviceState {
        impulse_response: Some(Arc::new(ImpulseResponse {
            channels: 2,
            samples,
        })),
        ..DeviceState::default()
    }
}

/// A processor set up as a live graph's builder sets it up (prepare, state, parameters), then
/// switched to deferred setup.
fn deferred(id: &str, state: &DeviceState, params: &ParamSlots) -> Box<dyn Processor> {
    let mut processor = create(id).expect("a built-in");
    processor.prepare(SPEC);
    processor.set_state(state);
    processor.set_params(params);
    processor.defer_setup();
    processor
}

fn block(processor: &mut dyn Processor, buffer: &mut [[f64; 2]], events: &[InstrumentEvent]) {
    for (i, frame) in buffer.iter_mut().enumerate() {
        let x = (f64::from(u32::try_from(i).expect("a block index")) * 0.05).sin() * 0.3;
        *frame = [x, 0.5 * x];
    }
    let ctx = BlockContext {
        frame: 0,
        sample_rate: SPEC.sample_rate,
        beat: 0.0,
        bpm: 120.0,
        playing: true,
        sidechain: None,
        events,
    };
    processor.process(buffer, &ctx);
}

#[test]
fn deferred_parameter_changes_allocate_nothing() {
    let registry = builtin_registry();
    let mut buffer = vec![[0.0; 2]; BLOCK];
    let mut requests = 0;
    for &id in PORTED {
        let descriptor = registry.get(id).expect("a descriptor");
        let defaults = slots(descriptor, &[]);
        let state = if id == "sonara.convolver" {
            impulse_response()
        } else {
            DeviceState::default()
        };
        let mut processor = deferred(id, &state, &defaults);
        for spec in &descriptor.params {
            let changed = slots(descriptor, &[(spec.id.as_str(), alternative(spec))]);
            let processor = processor.as_mut();
            let request = assert_no_alloc(|| {
                processor.set_params(&changed);
                block(processor, &mut buffer, &NOTES);
                let request = processor.setup_request();
                processor.set_params(&defaults);
                block(processor, &mut buffer, &[]);
                request
            });
            assert!(
                buffer.iter().flatten().all(|x| x.is_finite()),
                "{id}.{}",
                spec.id
            );
            if let Some(request) = request {
                requests += 1;
                assert_eq!(id, "sonara.reverb", "only the reverb defers setup");
                // The builder thread's work, then the install on the render thread.
                let mut prepared = request.prepare();
                processor.set_params(&changed);
                let installed = assert_no_alloc(|| {
                    let installed = processor.install_setup(&mut prepared);
                    block(processor, &mut buffer, &[]);
                    installed
                });
                assert!(installed, "{id}.{}", spec.id);
                assert!(!prepared.is_pending());
                processor.set_params(&defaults);
                let mut back = processor
                    .setup_request()
                    .expect("the default key is no longer installed")
                    .prepare();
                assert!(assert_no_alloc(|| processor.install_setup(&mut back)));
            }
            assert!(processor.setup_request().is_none(), "{id}.{}", spec.id);
        }
    }
    // Decay, pre-delay, damping, both cuts, width and character shape the response.
    assert_eq!(requests, 7, "the reverb's response parameters");
}

/// A deferred convolver keeps its loaded response when "Normalize IR" changes: that change
/// makes it not reusable, so a live graph replaces it instead.
#[test]
fn a_deferred_convolver_keeps_its_response() {
    let registry = builtin_registry();
    let descriptor = registry.get("sonara.convolver").expect("a descriptor");
    let normalized = slots(descriptor, &[("mix", ParamValue::Float(1.0))]);
    let raw = slots(
        descriptor,
        &[
            ("mix", ParamValue::Float(1.0)),
            ("normalize", ParamValue::Bool(false)),
        ],
    );
    let other_mix = slots(descriptor, &[("mix", ParamValue::Float(0.4))]);
    assert!(!reusable("sonara.convolver", &normalized, &raw));
    assert!(reusable("sonara.convolver", &normalized, &other_mix));
    assert!(reusable("sonara.reverb", &ParamSlots::EMPTY, &other_mix));

    let state = impulse_response();
    let mut live = deferred("sonara.convolver", &state, &normalized);
    let mut reference = deferred("sonara.convolver", &state, &normalized);
    let (mut a, mut b) = (vec![[0.0; 2]; BLOCK], vec![[0.0; 2]; BLOCK]);
    assert_no_alloc(|| live.set_params(&raw));
    block(live.as_mut(), &mut a, &[]);
    block(reference.as_mut(), &mut b, &[]);
    assert_eq!(a, b, "still the normalized response");
    assert!(a.iter().any(|f| f[0] != 0.0));
    assert!(live.setup_request().is_none(), "it never waits for setup");
}
