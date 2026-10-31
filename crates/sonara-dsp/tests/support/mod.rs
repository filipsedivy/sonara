//! The shared golden runner of the plugin area (GLD-01).
//!
//! It replays `scripts/golden/area_plugins.py` and `area_plugins_rates.py` exactly and compares
//! every recorded render of one plugin. Everything is driven by the recording, so a newly
//! ported processor needs only its entry in `sonara_dsp::processors::create` and a test that
//! calls [`check_plugin`]:
//!
//! - **Areas:** `plugins/` (48 kHz, the default configuration and every preset) and
//!   `plugins-44100/` (default configurations only); each area's `index.json` gives its rate,
//!   block size and tempo and lists the configurations.
//! - **`_make`:** a new processor from the factory, `prepare(ProcessSpec(sr, 512))`, then
//!   `set_params` with the resolved, validated preset values. The presets come from the Rust
//!   descriptors (`builtin_registry`), and the resolved set is checked against the recorded
//!   `params` first.
//! - **`_run`:** fixed 512-frame blocks, zero-padded; each block is a fresh copy of the input,
//!   with `BlockContext(frame a, sr, beat a/sr·bpm/60, bpm)`, the sidechain only for the
//!   `sine-1k-with-sidechain` render of a plugin that declares one, and for instruments the
//!   events of `_event_source` (the render's recorded `seconds` and `notes`). The render graph
//!   sends the full parameter set every block, and the prototype's dict comparison makes the
//!   repeats no-ops, so the runner sends it before every block too. `set_params` and `process`
//!   run inside `assert_no_alloc` (the test binary installs `AllocDisabler`), so they are shown
//!   to allocate and free nothing (RT-02).
//! - **Comparison:** every render's WAV (float32) against the output rounded to f32, by the null
//!   test's peak residual in dBFS; `report()`, `latency` and `tail_seconds` against
//!   `config.json` wherever the recorder stored them.
//!
//! The convolver's configurations with an impulse response (`plugins/convolver-ir.json`) have
//! their own entry point, [`check_convolver_ir`]: the device setup in the recorded call order
//! (`prepare`, `set_params` and `set_state`, either order of the last two) with the recorded
//! fixture as the device state, then the same `_run` and comparisons. The configuration that
//! records prototype defect D2 is compared with the correct result instead.
//!
//! Device setup runs outside `assert_no_alloc`, because the render graph does it off the
//! real-time thread and loading an impulse response allocates; so does the first `set_params`
//! of a plugin in [`SETUP_ON_FIRST_PARAMS`]. Every per-block call stays inside.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::sync::Arc;

use assert_no_alloc::assert_no_alloc;
use serde_json::Value;
use sonara_core::model::device::{ParamValue, Params};
use sonara_core::plugins::{PluginDescriptor, PluginKind, builtin_registry};
use sonara_dsp::processor::{
    BlockContext, DeviceState, EventKind, ImpulseResponse, InstrumentEvent, ParamSlots,
    ProcessSpec, Processor,
};
use sonara_dsp::processors::{create, param_slots};
use sonara_testkit::{Golden, peak_residual_dbfs};

/// The golden null-test threshold: the peak residual of a render, in dBFS.
pub const NULL_TEST_DBFS: f64 = -120.0;

/// Largest relative deviation of a `report()` value or `tail_seconds` from the recording
/// (against `max(|recorded|, 1)`). The values are exact where this platform's libm agrees with
/// macOS libm; elsewhere one ulp in a `pow` or `log10` may carry into them.
const REPORT_TOLERANCE: f64 = 1e-9;

/// The areas: the main plugin area and the 44.1 kHz default configurations.
const AREAS: [&str; 2] = ["plugins", "plugins-44100"];
/// The recorder's block size (`BLOCK`).
const BLOCK: usize = 512;
/// The render of a sidechain-capable plugin with the sidechain connected.
const SIDECHAIN_RENDER: &str = "sine-1k-with-sidechain";
/// Its input and its sidechain signal (`_sidechain`), both in `plugins/signals/`.
const SIDECHAIN_INPUT: &str = "sine-1k";
const SIDECHAIN_SIGNAL: &str = "sidechain-for-sine-1k";
/// Plugins whose first `set_params` after `prepare` is setup work: the reverb synthesizes its
/// impulse response in the call that sets its key (in the render graph, the device setup).
/// That call runs outside `assert_no_alloc`; the per-block calls of `_run` stay inside.
const SETUP_ON_FIRST_PARAMS: [&str; 1] = ["sonara.reverb"];
/// The listing of the convolver's configurations with an impulse response.
const CONVOLVER_IR: &str = "plugins/convolver-ir.json";
/// The configuration whose renders are the correct result of the one marked
/// `prototype_defect = "D2"` (same parameters, the recorder's call order).
const D2_REFERENCE: &str = "ir-fixture";

/// Checks every recorded configuration of `plugin_id` in both areas and panics with every
/// mismatch. `max_residual_dbfs` is the null-test threshold of the renders ([`NULL_TEST_DBFS`]
/// unless the spec documents another for this plugin). Prints one line per render.
pub fn check_plugin(plugin_id: &str, max_residual_dbfs: f64) {
    let golden = Golden::open().expect("golden data is available (set SONARA_GOLDEN)");
    let registry = builtin_registry();
    let descriptor = registry
        .get(plugin_id)
        .unwrap_or_else(|e| panic!("{plugin_id} is not a built-in plugin: {e}"));
    let mut failures = Vec::new();
    let mut renders = 0;
    for area in AREAS {
        let area = Area::open(&golden, area);
        for (config, values) in area.configs(descriptor) {
            renders += check_config(
                &golden,
                &area,
                descriptor,
                &config,
                &values,
                max_residual_dbfs,
                &mut failures,
            );
        }
    }
    assert!(renders > 0, "{plugin_id}: no renders were checked");
    assert!(
        failures.is_empty(),
        "{plugin_id}: {} mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// One area of the recording.
struct Area {
    dir: &'static str,
    sample_rate: u32,
    index: Value,
}

impl Area {
    fn open(golden: &Golden, dir: &'static str) -> Self {
        let index: Value = golden
            .json(&format!("{dir}/index.json"))
            .unwrap_or_else(|e| panic!("cannot read {dir}/index.json: {e}"));
        let sample_rate = u32::try_from(int(&index["sample_rate"])).expect("a sample rate");
        assert_eq!(
            int(&index["block_size"]).try_into(),
            Ok(BLOCK),
            "{dir}: block size"
        );
        assert_eq!(
            number(&index["bpm"]).to_bits(),
            BPM.to_bits(),
            "{dir}: tempo"
        );
        Self {
            dir,
            sample_rate,
            index,
        }
    }

    /// The configurations of `descriptor` this area records, with their preset values: the
    /// default (`{}`) and, in the main area, every preset as `preset-<slug>`. They must be
    /// exactly the ones `index.json` lists for the plugin, in its order.
    fn configs(&self, descriptor: &PluginDescriptor) -> Vec<(String, Params)> {
        let mut configs = vec![("default".to_owned(), Params::new())];
        if self.dir == "plugins" {
            configs.extend(
                descriptor
                    .presets
                    .iter()
                    .map(|p| (format!("preset-{}", slug(&p.name)), p.params.clone())),
            );
        }
        let listed: Vec<&str> = self.index["configs"]
            .as_array()
            .expect("a configs list")
            .iter()
            .filter(|c| c["plugin"] == descriptor.id.as_str())
            .map(|c| {
                assert_eq!(c["kind"], descriptor.kind.as_str(), "{}", descriptor.id);
                c["config"].as_str().expect("a config name")
            })
            .collect();
        let names: Vec<&str> = configs.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names, listed,
            "{}: configurations in {}",
            descriptor.id, self.dir
        );
        configs
    }
}

/// The recorder's tempo (`BPM`).
const BPM: f64 = 120.0;

/// `_slug`: lower case, runs of other characters than `a-z0-9` as one `-`, trimmed.
fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_owned()
}

/// Checks one configuration; returns the number of renders checked.
fn check_config(
    golden: &Golden,
    area: &Area,
    descriptor: &PluginDescriptor,
    config: &str,
    values: &Params,
    max_residual_dbfs: f64,
    failures: &mut Vec<String>,
) -> usize {
    let base = format!("{}/{}/{config}", area.dir, descriptor.id);
    let record: Value = golden
        .json(&format!("{base}/config.json"))
        .unwrap_or_else(|e| panic!("cannot read {base}/config.json: {e}"));
    let validated = descriptor
        .validate_param_values(values)
        .unwrap_or_else(|e| panic!("{base}: preset values: {e}"));
    check_params(
        &base,
        &descriptor.resolve(&validated),
        &record["params"],
        failures,
    );
    let renders = record["renders"].as_object().expect("a renders object");
    for (name, meta) in renders {
        let label = format!("{base}/{name}");
        let input = render_input(golden, area, descriptor, name, meta);
        let (mut processor, slots) = make(descriptor, &validated, area.sample_rate);
        let output = run(
            processor.as_mut(),
            &slots,
            area.sample_rate,
            &input.signal,
            input.sidechain.as_deref(),
            &input.events,
        );
        let expected = golden
            .wav(&format!("{base}/{name}.wav"))
            .unwrap_or_else(|e| panic!("cannot read {label}.wav: {e}"));
        let mut line = compare_audio(&label, &output, &expected, max_residual_dbfs, failures);
        compare_meta(&label, processor.as_ref(), meta, &mut line, failures);
        println!("{line}");
    }
    renders.len()
}

/// What one render is processed from.
struct RenderInput {
    signal: Vec<[f64; 2]>,
    sidechain: Option<Vec<[f64; 2]>>,
    events: EventSource,
}

/// The input, sidechain and events of render `name`.
fn render_input(
    golden: &Golden,
    area: &Area,
    descriptor: &PluginDescriptor,
    name: &str,
    meta: &Value,
) -> RenderInput {
    let effect = |signal, sidechain| RenderInput {
        signal,
        sidechain,
        events: EventSource::default(),
    };
    match descriptor.kind {
        PluginKind::Instrument => {
            // `np.zeros((int(seconds * SR), 2))` and `_event_source(notes)`.
            let seconds = number(&meta["seconds"]);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "Python's int() truncates a positive length"
            )]
            let frames = (seconds * f64::from(area.sample_rate)) as usize;
            let notes = meta["notes"].as_array().expect("a notes list");
            let events = EventSource::new(notes, area.sample_rate);
            RenderInput {
                signal: vec![[0.0; 2]; frames],
                sidechain: None,
                events,
            }
        }
        PluginKind::Effect if name == SIDECHAIN_RENDER => {
            assert!(
                descriptor.sidechain,
                "{name} for a plugin without sidechain"
            );
            let input = signal(golden, area.dir, SIDECHAIN_INPUT);
            let sidechain = signal(golden, area.dir, SIDECHAIN_SIGNAL);
            effect(input, Some(sidechain))
        }
        PluginKind::Effect => effect(signal(golden, area.dir, name), None),
    }
}

/// A recorded `(frames, 2)` signal of `<dir>/signals/`.
fn signal(golden: &Golden, dir: &str, name: &str) -> Vec<[f64; 2]> {
    let rel = format!("{dir}/signals/{name}");
    let array = golden
        .f64_array(&rel)
        .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"));
    assert_eq!(array.shape.get(1), Some(&2), "{rel}: stereo");
    array.data.as_chunks::<2>().0.to_vec()
}

/// `_make`: a prepared processor with the configuration's parameters applied, and the slots it
/// was given.
fn make(
    descriptor: &PluginDescriptor,
    validated: &Params,
    sample_rate: u32,
) -> (Box<dyn Processor>, ParamSlots) {
    let mut processor =
        create(&descriptor.id).unwrap_or_else(|| panic!("no processor for {}", descriptor.id));
    processor.prepare(ProcessSpec {
        sample_rate,
        block_size: BLOCK,
        channels: 2,
    });
    let slots = param_slots(descriptor, validated)
        .unwrap_or_else(|e| panic!("{}: parameter slots: {e}", descriptor.id));
    if SETUP_ON_FIRST_PARAMS.contains(&descriptor.id.as_str()) {
        processor.set_params(&slots);
    } else {
        assert_no_alloc(|| processor.set_params(&slots));
    }
    (processor, slots)
}

/// Checks every configuration of `plugins/convolver-ir.json` and panics with every mismatch:
/// the device setup in the recorded call order with the recorded impulse response, `_run`,
/// and the comparisons of [`check_plugin`].
///
/// The configuration marked `prototype_defect = "D2"` is the render graph's order (`set_state`
/// before `set_params`), in which the prototype ignores "Normalize IR". Rust fixes the defect,
/// so its renders must match those of
/// [`D2_REFERENCE`], which records the normalized result, within `max_residual_dbfs`, and they
/// must differ from the defect's own recording wherever that recording differs from the
/// correct one (every render but silence), so that a regression to the defect cannot pass.
pub fn check_convolver_ir(max_residual_dbfs: f64) {
    let golden = Golden::open().expect("golden data is available (set SONARA_GOLDEN)");
    let registry = builtin_registry();
    let listing: Value = golden
        .json(CONVOLVER_IR)
        .unwrap_or_else(|e| panic!("cannot read {CONVOLVER_IR}: {e}"));
    let sample_rate = u32::try_from(int(&listing["sample_rate"])).expect("a sample rate");
    assert_eq!(
        int(&listing["block_size"]).try_into(),
        Ok(BLOCK),
        "block size"
    );
    assert_eq!(number(&listing["bpm"]).to_bits(), BPM.to_bits(), "tempo");
    let state = impulse_response_state(&golden, &listing, sample_rate);
    let mut failures = Vec::new();
    let mut renders = 0;
    for entry in listing["configs"].as_array().expect("a configs list") {
        let plugin = entry["plugin"].as_str().expect("a plugin");
        let config = entry["config"].as_str().expect("a config name");
        let descriptor = registry
            .get(plugin)
            .unwrap_or_else(|e| panic!("{plugin} is not a built-in plugin: {e}"));
        assert_eq!(entry["impulse_response"], listing["impulse_response"]);
        let base = format!("plugins/{plugin}/{config}");
        let record: Value = golden
            .json(&format!("{base}/config.json"))
            .unwrap_or_else(|e| panic!("cannot read {base}/config.json: {e}"));
        assert_eq!(record["call_order"], entry["call_order"], "{base}");
        let order: Vec<&str> = record["call_order"]
            .as_array()
            .expect("a call order")
            .iter()
            .map(|step| step.as_str().expect("a call"))
            .collect();
        let slots = recorded_slots(&base, descriptor, &record["params"], &mut failures);
        let defect = record.get("prototype_defect").and_then(Value::as_str);
        let expected_base = match defect {
            None => base.clone(),
            Some("D2") => format!("plugins/{plugin}/{D2_REFERENCE}"),
            Some(other) => panic!("{base}: unknown prototype defect {other}"),
        };
        let renders_meta = record["renders"].as_object().expect("a renders object");
        for (name, meta) in renders_meta {
            let label = format!("{base}/{name}");
            let input = signal(&golden, "plugins", name);
            let mut processor =
                create(plugin).unwrap_or_else(|| panic!("no processor for {plugin}"));
            device_setup(processor.as_mut(), &order, sample_rate, &slots, &state);
            let output = run(
                processor.as_mut(),
                &slots,
                sample_rate,
                &input,
                None,
                &EventSource::default(),
            );
            let expected = golden
                .wav(&format!("{expected_base}/{name}.wav"))
                .unwrap_or_else(|e| panic!("cannot read {expected_base}/{name}.wav: {e}"));
            let mut line =
                compare_audio(&label, &output, &expected, max_residual_dbfs, &mut failures);
            if expected_base != base {
                let recorded = golden
                    .wav(&format!("{label}.wav"))
                    .unwrap_or_else(|e| panic!("cannot read {label}.wav: {e}"));
                let defect = Defect {
                    label: &label,
                    recorded: &recorded,
                    correct: &expected,
                };
                defect.check_fixed(&output, max_residual_dbfs, &mut line, &mut failures);
            }
            compare_meta(&label, processor.as_ref(), meta, &mut line, &mut failures);
            println!("{line}");
        }
        renders += renders_meta.len();
    }
    assert!(renders > 0, "{CONVOLVER_IR}: no renders were checked");
    assert!(
        failures.is_empty(),
        "{CONVOLVER_IR}: {} mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The parameter slots of a recorded `params` object, which is not a preset but the resolved
/// set the recorder applied: validated, checked against its own resolution and converted.
fn recorded_slots(
    base: &str,
    descriptor: &PluginDescriptor,
    recorded: &Value,
    failures: &mut Vec<String>,
) -> ParamSlots {
    let values: Params = recorded
        .as_object()
        .expect("a params object")
        .iter()
        .map(|(key, value)| (key.clone(), param_value(value)))
        .collect();
    let validated = descriptor
        .validate_param_values(&values)
        .unwrap_or_else(|e| panic!("{base}: recorded values: {e}"));
    check_params(base, &descriptor.resolve(&validated), recorded, failures);
    param_slots(descriptor, &validated).unwrap_or_else(|e| panic!("{base}: parameter slots: {e}"))
}

/// One render of a configuration that records a prototype defect, with the recording of the
/// correct behaviour.
struct Defect<'a> {
    label: &'a str,
    recorded: &'a sonara_testkit::Audio,
    correct: &'a sonara_testkit::Audio,
}

impl Defect<'_> {
    /// Fails when `output` is within `max_residual_dbfs` of the defect's recording although that
    /// recording differs audibly from the correct one: the defect would have come back.
    fn check_fixed(
        &self,
        output: &[[f64; 2]],
        max_residual_dbfs: f64,
        line: &mut String,
        failures: &mut Vec<String>,
    ) {
        let audible =
            peak_residual_dbfs(&self.correct.samples, &self.recorded.samples) > max_residual_dbfs;
        let from_defect = peak_residual_dbfs(&self.recorded.samples, &rounded(output));
        write!(
            line,
            " (against {D2_REFERENCE}); {from_defect:.1} dBFS from the defect's recording"
        )
        .expect("writing to a String cannot fail");
        if audible && from_defect <= max_residual_dbfs {
            failures.push(format!("{}: reproduces the prototype defect", self.label));
        }
    }
}

/// The recorded impulse response as the render graph supplies it (`state['ir']`, float64 at the
/// project rate): the float WAV's samples widened to f64. The recorder passed
/// `fixtures.impulse_response()`, float64 values rounded through float32, which the 32-bit
/// float WAV stores exactly, so the two are equal.
fn impulse_response_state(golden: &Golden, listing: &Value, sample_rate: u32) -> DeviceState {
    let rel = listing["impulse_response"]
        .as_str()
        .expect("an impulse response");
    let audio = golden
        .wav(rel)
        .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"));
    let channels = usize::from(audio.channels);
    let shape: Vec<usize> = listing["impulse_response_shape"]
        .as_array()
        .expect("a shape")
        .iter()
        .map(|d| usize::try_from(int(d)).expect("a dimension"))
        .collect();
    assert_eq!(shape, [audio.samples.len() / channels, channels], "{rel}");
    assert_eq!(audio.sample_rate, sample_rate, "{rel}");
    DeviceState {
        impulse_response: Some(Arc::new(ImpulseResponse {
            channels,
            samples: audio.samples.iter().map(|&s| f64::from(s)).collect(),
        })),
        ..DeviceState::default()
    }
}

/// `_make_with_ir`: the device setup in the recorded `order` of `prepare`, `set_params` and
/// `set_state`. It is setup (the render graph's, off the real-time thread), so it runs outside
/// `assert_no_alloc`: loading an impulse response allocates.
fn device_setup(
    processor: &mut dyn Processor,
    order: &[&str],
    sample_rate: u32,
    slots: &ParamSlots,
    state: &DeviceState,
) {
    for &call in order {
        match call {
            "prepare" => processor.prepare(ProcessSpec {
                sample_rate,
                block_size: BLOCK,
                channels: 2,
            }),
            "set_params" => processor.set_params(slots),
            "set_state" => processor.set_state(state),
            other => panic!("unknown call {other} in a call order"),
        }
    }
}

/// The output as the float32 WAV the recorder writes (NumPy's `astype` rounds to nearest).
fn rounded(output: &[[f64; 2]]) -> Vec<f32> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the recording is float32: NumPy's astype rounds to nearest, like `as`"
    )]
    output.as_flattened().iter().map(|&v| v as f32).collect()
}

/// `_run`: the input in fixed, zero-padded blocks, each a fresh copy, with the full parameter
/// set sent before every block.
fn run(
    processor: &mut dyn Processor,
    slots: &ParamSlots,
    sample_rate: u32,
    input: &[[f64; 2]],
    sidechain: Option<&[[f64; 2]]>,
    events: &EventSource,
) -> Vec<[f64; 2]> {
    let frames = input.len();
    let blocks = frames.div_ceil(BLOCK);
    let padded = |x: &[[f64; 2]]| {
        let mut out = vec![[0.0; 2]; blocks * BLOCK];
        out[..x.len()].copy_from_slice(x);
        out
    };
    let input = padded(input);
    let sidechain = sidechain.map(padded);
    let block_events: Vec<Vec<InstrumentEvent>> = (0..blocks).map(|b| events.block(b)).collect();
    let mut output = vec![[0.0; 2]; blocks * BLOCK];
    let sr = f64::from(sample_rate);
    let blocks_in = input.as_chunks::<BLOCK>().0;
    let blocks_out = output.as_chunks_mut::<BLOCK>().0;
    for (b, (x, y)) in blocks_in.iter().zip(blocks_out.iter_mut()).enumerate() {
        let a = b * BLOCK;
        let ctx = BlockContext {
            frame: i64::try_from(a).expect("a frame"),
            sample_rate,
            beat: f64::from(u32::try_from(a).expect("a frame")) / sr * BPM / 60.0,
            bpm: BPM,
            playing: true,
            sidechain: sidechain.as_ref().map(|s| &s[a..a + BLOCK]),
            events: &block_events[b],
        };
        *y = *x;
        assert_no_alloc(|| {
            processor.set_params(slots);
            processor.process(y, &ctx);
        });
    }
    output.truncate(frames);
    output
}

/// `_event_source`: note-on and note-off events at the rounded note times, sorted by position
/// with note-offs first, handed out per block with block-relative offsets.
#[derive(Default)]
struct EventSource {
    events: Vec<(i64, InstrumentEvent)>,
}

impl EventSource {
    /// From the recorded notes `[start seconds, length seconds, pitch, velocity]`.
    fn new(notes: &[Value], sample_rate: u32) -> Self {
        let sr = f64::from(sample_rate);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "Python's round() of a note time in samples"
        )]
        let at = |seconds: f64| (seconds * sr).round_ties_even() as i64;
        let mut events = Vec::new();
        for note in notes {
            let note = note.as_array().expect("a note");
            let (start, length) = (number(&note[0]), number(&note[1]));
            let pitch = u8::try_from(int(&note[2])).expect("a pitch");
            let velocity = u8::try_from(int(&note[3])).expect("a velocity");
            let event = |kind, velocity| InstrumentEvent {
                offset: 0,
                kind,
                pitch,
                velocity,
                channel: 0,
                value: 0,
                number: 0,
                articulation: None,
            };
            events.push((at(start), event(EventKind::NoteOn, velocity)));
            events.push((at(start + length), event(EventKind::NoteOff, 0)));
        }
        // `sort(key=(position, kind != NOTE_OFF))`, stable.
        events.sort_by_key(|(at, e)| (*at, e.kind != EventKind::NoteOff));
        Self { events }
    }

    /// The events of block `b`.
    fn block(&self, b: usize) -> Vec<InstrumentEvent> {
        let a = i64::try_from(b * BLOCK).expect("a frame");
        let end = a + i64::try_from(BLOCK).expect("a block size");
        self.events
            .iter()
            .filter(|(at, _)| (a..end).contains(at))
            .map(|&(at, event)| InstrumentEvent {
                offset: usize::try_from(at - a).expect("an offset"),
                ..event
            })
            .collect()
    }
}

/// The null test of one render; returns the report line.
fn compare_audio(
    label: &str,
    output: &[[f64; 2]],
    expected: &sonara_testkit::Audio,
    max_residual_dbfs: f64,
    failures: &mut Vec<String>,
) -> String {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the recording is float32: NumPy's astype rounds to nearest, like `as`"
    )]
    let actual: Vec<f32> = output.as_flattened().iter().map(|&v| v as f32).collect();
    if expected.channels != 2 || actual.len() != expected.samples.len() {
        failures.push(format!(
            "{label}: {} samples in {} channels recorded, {} rendered",
            expected.samples.len(),
            expected.channels,
            actual.len()
        ));
        return format!("{label}: length mismatch");
    }
    let residual = peak_residual_dbfs(&expected.samples, &actual);
    let differing = expected
        .samples
        .iter()
        .zip(&actual)
        .filter(|(e, a)| e.to_bits() != a.to_bits())
        .count();
    if residual > max_residual_dbfs {
        failures.push(format!(
            "{label}: residual {residual:.1} dBFS above {max_residual_dbfs} dBFS"
        ));
    }
    format!(
        "{label}: residual {residual:.1} dBFS, {differing} of {} samples differ",
        actual.len()
    )
}

/// `report()`, `latency` and `tail_seconds`, where the render recorded them.
fn compare_meta(
    label: &str,
    processor: &dyn Processor,
    meta: &Value,
    line: &mut String,
    failures: &mut Vec<String>,
) {
    if let Some(latency) = meta.get("latency") {
        let expected = usize::try_from(int(latency)).expect("a latency");
        if processor.latency() != expected {
            failures.push(format!(
                "{label}: latency {} instead of {expected}",
                processor.latency()
            ));
        }
    }
    if let Some(tail) = meta.get("tail_seconds") {
        let (actual, expected) = (processor.tail_seconds(), number(tail));
        if !close(actual, expected) {
            failures.push(format!("{label}: tail {actual} s instead of {expected} s"));
        }
    }
    let Some(recorded) = meta.get("report") else {
        return;
    };
    let recorded = recorded.as_object().expect("a report object");
    let report = processor.report();
    let entries = report.entries();
    let names: BTreeSet<&str> = entries.iter().map(|(name, _)| *name).collect();
    let expected_names: BTreeSet<&str> = recorded.keys().map(String::as_str).collect();
    if names != expected_names || names.len() != entries.len() {
        failures.push(format!(
            "{label}: report keys {names:?} instead of {expected_names:?}"
        ));
        return;
    }
    for &(name, actual) in entries {
        let expected = number(&recorded[name]);
        write!(line, "; {name} {actual} (recorded {expected})")
            .expect("writing to a String cannot fail");
        if !close(actual, expected) {
            failures.push(format!(
                "{label}: report {name} {actual} instead of {expected}"
            ));
        }
    }
}

/// The resolved parameter set against the recorded `params` (keys sorted by the recorder).
fn check_params(base: &str, resolved: &Params, recorded: &Value, failures: &mut Vec<String>) {
    let recorded = recorded.as_object().expect("a params object");
    let keys: BTreeSet<&str> = resolved.keys().map(String::as_str).collect();
    let recorded_keys: BTreeSet<&str> = recorded.keys().map(String::as_str).collect();
    if keys != recorded_keys {
        failures.push(format!(
            "{base}: parameters {keys:?} instead of {recorded_keys:?}"
        ));
        return;
    }
    for (key, value) in resolved {
        let expected = param_value(&recorded[key]);
        if *value != expected {
            failures.push(format!(
                "{base}: parameter {key} is {value:?}, recorded {expected:?}"
            ));
        }
    }
}

/// A recorded parameter value in the model's type.
fn param_value(value: &Value) -> ParamValue {
    match value {
        Value::Bool(b) => ParamValue::Bool(*b),
        Value::String(s) => ParamValue::Str(s.clone()),
        Value::Number(n) => n
            .as_i64()
            .map_or_else(|| ParamValue::Float(number(value)), ParamValue::Int),
        other => panic!("not a parameter value: {other}"),
    }
}

/// Whether `actual` is within [`REPORT_TOLERANCE`] of `expected`; NaN matches NaN.
fn close(actual: f64, expected: f64) -> bool {
    if actual.is_nan() || expected.is_nan() {
        return actual.is_nan() && expected.is_nan();
    }
    actual.to_bits() == expected.to_bits()
        || (actual - expected).abs() <= REPORT_TOLERANCE * expected.abs().max(1.0)
}

/// A recorded float; the recorder writes non-finite values as `{"non_finite": "nan"}`.
fn number(value: &Value) -> f64 {
    if let Some(v) = value.as_f64() {
        return v;
    }
    match value.get("non_finite").and_then(Value::as_str) {
        Some("nan") => f64::NAN,
        Some("inf") => f64::INFINITY,
        Some("-inf") => f64::NEG_INFINITY,
        _ => panic!("not a number: {value}"),
    }
}

/// A recorded integer.
fn int(value: &Value) -> i64 {
    value
        .as_i64()
        .unwrap_or_else(|| panic!("not an integer: {value}"))
}
