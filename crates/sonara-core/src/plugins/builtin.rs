//! Descriptors of Sonara's built-in instruments and effects (`sonara.plugins.builtin`).
//!
//! The helpers mirror the prototype's `fp`, `ip`, `bp`, `ep` and `preset`, so that each
//! descriptor reads like its Python source. Keyword arguments with defaults become the chained
//! setters `log()`, `desc()` and `not_automatable()`.

use crate::model::device::{ParamValue, Params};

use super::descriptor::{
    ParamKind, ParamScale, ParamSpec, PluginDescriptor, PluginFormat, PluginKind, PluginRegistry,
    Preset,
};

/// A preset: name, optional description, then the parameter values in the prototype's keyword
/// order. Each value keeps its literal type: `true` is a bool, `3` an integer, `3.0` a float and
/// `"24"` a string.
macro_rules! preset {
    ($name:literal $(; $($key:ident = $value:expr),+ $(,)?)?) => {
        preset!($name, "" $(; $($key = $value),+)?)
    };
    ($name:literal, $desc:literal $(; $($key:ident = $value:expr),+ $(,)?)?) => {
        Preset {
            name: $name.to_owned(),
            params: Params::from([$($((stringify!($key).to_owned(), ParamValue::from($value))),+)?]),
            description: $desc.to_owned(),
        }
    };
}

const SLOPES: &[&str] = &["12", "24", "36", "48"];
const WAVES: &[&str] = &["saw", "square", "pulse", "triangle", "sine"];
/// The keys of the delay's `NOTE_VALUES`, in the prototype's order.
const NOTE_VALUES: &[&str] = &[
    "1/32", "1/16T", "1/16", "1/16D", "1/8T", "1/8", "1/8D", "1/4T", "1/4", "1/4D", "1/2", "1/2D",
    "1/1",
];

fn spec(id: &str, name: &str, kind: ParamKind, default: ParamValue) -> ParamSpec {
    ParamSpec {
        id: id.to_owned(),
        name: name.to_owned(),
        kind,
        default,
        minimum: None,
        maximum: None,
        unit: String::new(),
        choices: Vec::new(),
        scale: ParamScale::Linear,
        automatable: true,
        description: String::new(),
    }
}

/// A float parameter (the prototype's `fp`).
fn fp(id: &str, name: &str, default: f64, lo: f64, hi: f64, unit: &str) -> ParamSpec {
    ParamSpec {
        minimum: Some(lo),
        maximum: Some(hi),
        unit: unit.to_owned(),
        ..spec(id, name, ParamKind::Float, ParamValue::Float(default))
    }
}

/// An integer parameter (the prototype's `ip`).
fn ip(id: &str, name: &str, default: i32, lo: i32, hi: i32, unit: &str) -> ParamSpec {
    ParamSpec {
        minimum: Some(f64::from(lo)),
        maximum: Some(f64::from(hi)),
        unit: unit.to_owned(),
        ..spec(
            id,
            name,
            ParamKind::Int,
            ParamValue::Int(i64::from(default)),
        )
    }
}

/// A boolean parameter (the prototype's `bp`).
fn bp(id: &str, name: &str, default: bool) -> ParamSpec {
    spec(id, name, ParamKind::Bool, ParamValue::Bool(default))
}

/// An enum parameter, never automatable (the prototype's `ep`).
fn ep(id: &str, name: &str, default: &str, choices: &[&str]) -> ParamSpec {
    ParamSpec {
        choices: strings(choices),
        automatable: false,
        ..spec(id, name, ParamKind::Enum, ParamValue::from(default))
    }
}

impl ParamSpec {
    /// `log=True`: a logarithmic scale.
    fn log(self) -> Self {
        Self {
            scale: ParamScale::Log,
            ..self
        }
    }

    /// `desc=…`: help text.
    fn desc(self, description: &str) -> Self {
        Self {
            description: description.to_owned(),
            ..self
        }
    }

    /// `automatable=False`.
    fn not_automatable(self) -> Self {
        Self {
            automatable: false,
            ..self
        }
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|&item| item.to_owned()).collect()
}

/// A built-in descriptor with the prototype's defaults: vendor `Sonara`, version 1, no
/// sidechain, presets or tags.
fn descriptor(
    id: &str,
    name: &str,
    kind: PluginKind,
    category: &str,
    params: Vec<ParamSpec>,
    description: &str,
) -> PluginDescriptor {
    PluginDescriptor {
        id: id.to_owned(),
        name: name.to_owned(),
        kind,
        category: category.to_owned(),
        params,
        description: description.to_owned(),
        vendor: "Sonara".to_owned(),
        version: 1,
        format: PluginFormat::Builtin,
        sidechain: false,
        presets: Vec::new(),
        tags: Vec::new(),
    }
}

fn utility() -> PluginDescriptor {
    PluginDescriptor {
        tags: strings(&["gain", "trim", "width", "polarity", "mono"]),
        ..descriptor(
            "sonara.utility",
            "Utility",
            PluginKind::Effect,
            "utility",
            vec![
                fp("gain_db", "Gain", 0.0, -60.0, 24.0, "dB"),
                fp("width", "Stereo Width", 1.0, 0.0, 2.0, "x")
                    .desc("0 = mono, 1 = unchanged, 2 = doubled side"),
                bp("mono", "Mono", false),
                bp("swap", "Swap L/R", false),
                bp("invert_left", "Invert Left Polarity", false),
                bp("invert_right", "Invert Right Polarity", false),
                bp("dc_block", "DC Block", false),
            ],
            "Trim gain, stereo width (mid/side), mono, channel swap, polarity and DC removal.",
        )
    }
}

fn eq() -> PluginDescriptor {
    PluginDescriptor {
        presets: vec![
            preset!("Low Cut", "Remove rumble below 100 Hz";
                hp_on = true, hp_freq = 100.0, hp_slope = "24"),
            preset!("Vocal Presence";
                hp_on = true, hp_freq = 90.0, mid1_freq = 300.0, mid1_gain = -2.5,
                mid2_freq = 4000.0, mid2_gain = 3.0, high_gain = 2.0),
            preset!("Bass Tight";
                hp_on = true, hp_freq = 35.0, low_freq = 80.0, low_gain = 2.0,
                mid1_freq = 250.0, mid1_gain = -3.0, mid1_q = 1.4),
            preset!("Air"; high_freq = 12000.0, high_gain = 3.0),
            preset!("Telephone";
                hp_on = true, hp_freq = 400.0, hp_slope = "24", lp_on = true, lp_freq = 3400.0,
                lp_slope = "24", mid1_freq = 1500.0, mid1_gain = 4.0),
        ],
        tags: strings(&["equalizer", "tone", "filter"]),
        ..descriptor(
            "sonara.eq.parametric",
            "Parametric EQ",
            PluginKind::Effect,
            "eq",
            vec![
                bp("hp_on", "High-Pass On", false),
                fp("hp_freq", "High-Pass Frequency", 80.0, 20.0, 2000.0, "Hz").log(),
                ep("hp_slope", "High-Pass Slope", "12", SLOPES).desc("dB/octave"),
                bp("low_on", "Low Band On", true),
                ep("low_type", "Low Band Type", "shelf", &["shelf", "peak"]),
                fp("low_freq", "Low Frequency", 100.0, 20.0, 1000.0, "Hz").log(),
                fp("low_gain", "Low Gain", 0.0, -24.0, 24.0, "dB"),
                fp("low_q", "Low Q", 0.71, 0.1, 10.0, "").log(),
                bp("mid1_on", "Mid 1 On", true),
                fp("mid1_freq", "Mid 1 Frequency", 500.0, 20.0, 20000.0, "Hz").log(),
                fp("mid1_gain", "Mid 1 Gain", 0.0, -24.0, 24.0, "dB"),
                fp("mid1_q", "Mid 1 Q", 1.0, 0.1, 18.0, "").log(),
                bp("mid2_on", "Mid 2 On", true),
                fp("mid2_freq", "Mid 2 Frequency", 2500.0, 20.0, 20000.0, "Hz").log(),
                fp("mid2_gain", "Mid 2 Gain", 0.0, -24.0, 24.0, "dB"),
                fp("mid2_q", "Mid 2 Q", 1.0, 0.1, 18.0, "").log(),
                bp("high_on", "High Band On", true),
                ep("high_type", "High Band Type", "shelf", &["shelf", "peak"]),
                fp("high_freq", "High Frequency", 8000.0, 1000.0, 20000.0, "Hz").log(),
                fp("high_gain", "High Gain", 0.0, -24.0, 24.0, "dB"),
                fp("high_q", "High Q", 0.71, 0.1, 10.0, "").log(),
                bp("lp_on", "Low-Pass On", false),
                fp(
                    "lp_freq",
                    "Low-Pass Frequency",
                    18000.0,
                    1000.0,
                    20000.0,
                    "Hz",
                )
                .log(),
                ep("lp_slope", "Low-Pass Slope", "12", SLOPES).desc("dB/octave"),
                fp("output_db", "Output", 0.0, -24.0, 24.0, "dB"),
            ],
            concat!(
                "Six-band parametric equalizer: Butterworth high/low-pass (12-48 dB/oct), low and ",
                "high shelf/peak bands and two fully parametric mid bands (RBJ biquads)."
            ),
        )
    }
}

fn filter() -> PluginDescriptor {
    PluginDescriptor {
        presets: vec![
            preset!("Slow Sweep";
                lfo_rate_hz = 0.1, lfo_depth_oct = 2.5, cutoff_hz = 1200.0, resonance = 0.35),
            preset!("Wah";
                mode = "bandpass", lfo_rate_hz = 2.0, lfo_depth_oct = 1.5, cutoff_hz = 900.0,
                resonance = 0.6),
        ],
        tags: strings(&["filter", "sweep", "lfo"]),
        ..descriptor(
            "sonara.filter",
            "Filter",
            PluginKind::Effect,
            "filter",
            vec![
                ep(
                    "mode",
                    "Mode",
                    "lowpass",
                    &["lowpass", "highpass", "bandpass", "notch"],
                ),
                fp("cutoff_hz", "Cutoff", 1000.0, 20.0, 20000.0, "Hz").log(),
                fp("resonance", "Resonance", 0.2, 0.0, 1.0, ""),
                fp("lfo_rate_hz", "LFO Rate", 0.5, 0.01, 20.0, "Hz").log(),
                fp("lfo_depth_oct", "LFO Depth", 0.0, 0.0, 4.0, "oct"),
                fp("mix", "Mix", 1.0, 0.0, 1.0, ""),
            ],
            "Resonant multimode state-variable filter with cutoff LFO; ideal for automated sweeps.",
        )
    }
}

fn compressor() -> PluginDescriptor {
    PluginDescriptor {
        sidechain: true,
        presets: vec![
            preset!("Gentle Bus Glue";
                threshold_db = -12.0, ratio = 2.0, attack_ms = 30.0, release_ms = 200.0,
                knee_db = 10.0),
            preset!("Vocal Leveler";
                threshold_db = -20.0, ratio = 3.5, attack_ms = 5.0, release_ms = 100.0),
            preset!("Drum Punch";
                threshold_db = -16.0, ratio = 4.0, attack_ms = 25.0, release_ms = 80.0,
                makeup_db = 3.0),
            preset!("Sidechain Pump", "Duck against the kick of the sidechain source (low-passed key)";
                threshold_db = -30.0, ratio = 8.0, attack_ms = 0.5, release_ms = 160.0,
                knee_db = 3.0, sidechain = true, sidechain_lpf_hz = 150.0),
            preset!("Parallel Smash";
                threshold_db = -35.0, ratio = 12.0, attack_ms = 1.0, release_ms = 60.0,
                mix = 0.35, makeup_db = 10.0),
        ],
        tags: strings(&["compression", "dynamics", "sidechain", "ducking"]),
        ..descriptor(
            "sonara.dynamics.compressor",
            "Compressor",
            PluginKind::Effect,
            "dynamics",
            vec![
                fp("threshold_db", "Threshold", -18.0, -60.0, 0.0, "dB"),
                fp("ratio", "Ratio", 4.0, 1.0, 20.0, ":1").log(),
                fp("attack_ms", "Attack", 10.0, 0.05, 200.0, "ms").log(),
                fp("release_ms", "Release", 120.0, 5.0, 2000.0, "ms").log(),
                fp("knee_db", "Knee", 6.0, 0.0, 24.0, "dB"),
                fp("makeup_db", "Makeup Gain", 0.0, -12.0, 24.0, "dB"),
                bp("auto_makeup", "Auto Makeup", false),
                fp("mix", "Mix", 1.0, 0.0, 1.0, "")
                    .desc("Parallel compression: 0 = dry, 1 = fully compressed"),
                ep("detector", "Detector", "peak", &["peak", "rms"]),
                bp("sidechain", "External Sidechain", false)
                    .desc("Use the sidechain source track as the key signal"),
                fp(
                    "sidechain_hpf_hz",
                    "Sidechain High-Pass",
                    20.0,
                    20.0,
                    500.0,
                    "Hz",
                )
                .log(),
                fp(
                    "sidechain_lpf_hz",
                    "Sidechain Low-Pass",
                    20000.0,
                    40.0,
                    20000.0,
                    "Hz",
                )
                .log()
                .desc("Key on the kick of a full drum bus (e.g. 120 Hz)"),
            ],
            concat!(
                "Feed-forward soft-knee compressor with stereo-linked detection, parallel mix and ",
                "external sidechain input (for ducking, e.g. bass keyed by kick)."
            ),
        )
    }
}

fn limiter() -> PluginDescriptor {
    PluginDescriptor {
        presets: vec![
            preset!("Master -1 dBTP"; ceiling_db = -1.0, true_peak = true),
            preset!("Streaming Loud"; ceiling_db = -1.0, input_gain_db = 4.0, release_ms = 80.0),
        ],
        tags: strings(&["limiting", "mastering", "true peak"]),
        ..descriptor(
            "sonara.dynamics.limiter",
            "Limiter",
            PluginKind::Effect,
            "dynamics",
            vec![
                fp("ceiling_db", "Ceiling", -1.0, -24.0, 0.0, "dBFS"),
                fp("input_gain_db", "Input Gain", 0.0, 0.0, 24.0, "dB"),
                fp("release_ms", "Release", 60.0, 1.0, 1000.0, "ms").log(),
                fp("lookahead_ms", "Lookahead", 2.0, 0.1, 10.0, "ms"),
                bp("true_peak", "True-Peak Detection", true)
                    .desc("Detect inter-sample peaks with 4x oversampling"),
            ],
            concat!(
                "Lookahead brickwall limiter with optional 4x oversampled true-peak detection. ",
                "Introduces latency equal to the lookahead (compensated automatically)."
            ),
        )
    }
}

fn gate() -> PluginDescriptor {
    PluginDescriptor {
        sidechain: true,
        tags: strings(&["gate", "expander", "noise"]),
        ..descriptor(
            "sonara.dynamics.gate",
            "Gate / Expander",
            PluginKind::Effect,
            "dynamics",
            vec![
                ep("mode", "Mode", "gate", &["gate", "expander"]),
                fp("threshold_db", "Threshold", -40.0, -80.0, 0.0, "dB"),
                fp("range_db", "Range", -80.0, -80.0, 0.0, "dB").desc("Maximum attenuation"),
                fp("ratio", "Expander Ratio", 2.0, 1.0, 10.0, ":1"),
                fp("hysteresis_db", "Hysteresis", 3.0, 0.0, 12.0, "dB"),
                fp("attack_ms", "Attack", 0.5, 0.05, 50.0, "ms").log(),
                fp("hold_ms", "Hold", 20.0, 0.0, 500.0, "ms"),
                fp("release_ms", "Release", 150.0, 5.0, 2000.0, "ms").log(),
                bp("sidechain", "External Sidechain", false),
            ],
            "Noise gate with hysteresis and hold, or downward expander.",
        )
    }
}

fn delay() -> PluginDescriptor {
    PluginDescriptor {
        presets: vec![
            preset!("Dotted Eighth"; sync = true, note = "1/8D", feedback = 0.4, mix = 0.25),
            preset!("Ping-Pong Quarter";
                sync = true, note = "1/4", feedback = 0.45, mix = 0.3, ping_pong = true),
            preset!("Slapback";
                sync = false, time_ms = 110.0, feedback = 0.1, mix = 0.3, highcut_hz = 5000.0),
            preset!("Dub Echo";
                sync = true, note = "1/4D", feedback = 0.7, mix = 0.35, lowcut_hz = 300.0,
                highcut_hz = 2500.0),
        ],
        tags: strings(&["echo", "delay", "tempo"]),
        ..descriptor(
            "sonara.delay",
            "Delay",
            PluginKind::Effect,
            "delay",
            vec![
                fp("time_ms", "Time", 375.0, 1.0, 4000.0, "ms").log(),
                bp("sync", "Tempo Sync", true),
                ep("note", "Note Value", "1/8D", NOTE_VALUES),
                fp("feedback", "Feedback", 0.35, 0.0, 0.98, ""),
                fp("mix", "Mix", 0.25, 0.0, 1.0, ""),
                bp("ping_pong", "Ping-Pong", false),
                fp("lowcut_hz", "Feedback Low Cut", 120.0, 20.0, 2000.0, "Hz").log(),
                fp(
                    "highcut_hz",
                    "Feedback High Cut",
                    7000.0,
                    1000.0,
                    20000.0,
                    "Hz",
                )
                .log(),
            ],
            "Stereo feedback delay with tempo sync, ping-pong and band-limited repeats.",
        )
    }
}

fn reverb() -> PluginDescriptor {
    PluginDescriptor {
        presets: vec![
            preset!("Small Room"; character = "room", decay_s = 0.6, predelay_ms = 5.0, mix = 0.2),
            preset!("Vocal Plate";
                character = "plate", decay_s = 1.8, predelay_ms = 25.0, damping = 0.4,
                mix = 0.22),
            preset!("Concert Hall";
                character = "hall", decay_s = 2.8, predelay_ms = 30.0, damping = 0.55, mix = 0.3),
            preset!("Cathedral";
                character = "hall", decay_s = 7.0, predelay_ms = 45.0, damping = 0.6, mix = 0.35),
            preset!("Ambient Wash";
                character = "hall", decay_s = 12.0, predelay_ms = 60.0, damping = 0.3, mix = 0.5,
                lowcut_hz = 250.0),
        ],
        tags: strings(&["reverb", "space", "ambience"]),
        ..descriptor(
            "sonara.reverb",
            "Reverb",
            PluginKind::Effect,
            "reverb",
            vec![
                ep(
                    "character",
                    "Character",
                    "hall",
                    &["room", "chamber", "hall", "plate"],
                ),
                fp("decay_s", "Decay (RT60)", 2.0, 0.1, 20.0, "s").log(),
                fp("predelay_ms", "Pre-Delay", 15.0, 0.0, 250.0, "ms"),
                fp("damping", "Damping", 0.5, 0.0, 1.0, "")
                    .desc("High-frequency decay relative to the RT60"),
                fp("lowcut_hz", "Low Cut", 120.0, 20.0, 1000.0, "Hz").log(),
                fp("highcut_hz", "High Cut", 12000.0, 1000.0, 20000.0, "Hz").log(),
                fp("width", "Width", 1.0, 0.0, 1.0, ""),
                fp("mix", "Mix", 0.25, 0.0, 1.0, ""),
            ],
            concat!(
                "Algorithmic reverb via a synthesized, frequency-dependent decaying impulse ",
                "response and partitioned FFT convolution."
            ),
        )
    }
}

fn convolver() -> PluginDescriptor {
    PluginDescriptor {
        tags: strings(&["reverb", "convolution", "impulse response"]),
        ..descriptor(
            "sonara.convolver",
            "Convolution Reverb",
            PluginKind::Effect,
            "reverb",
            vec![
                fp("mix", "Mix", 0.3, 0.0, 1.0, ""),
                fp("gain_db", "Wet Gain", 0.0, -24.0, 24.0, "dB"),
                ep("input", "Input", "mono_sum", &["mono_sum", "stereo"])
                    .desc("mono_sum: L+R into a stereo IR"),
                bp("normalize", "Normalize IR", true).not_automatable(),
            ],
            concat!(
                "Convolution with an impulse response loaded from the project's media ",
                "(device state key 'ir_asset_id')."
            ),
        )
    }
}

fn saturation() -> PluginDescriptor {
    PluginDescriptor {
        presets: vec![
            preset!("Warm Tape";
                mode = "tanh", drive_db = 4.0, tone_hz = 12000.0, output_db = -2.0),
            preset!("Tube Drive"; mode = "tube", drive_db = 10.0, bias = 0.15, output_db = -6.0),
            preset!("Hard Crunch";
                mode = "hard_clip", drive_db = 18.0, tone_hz = 6000.0, output_db = -10.0,
                oversampling = "4x"),
        ],
        tags: strings(&["saturation", "distortion", "drive", "clipping"]),
        ..descriptor(
            "sonara.saturation",
            "Saturation",
            PluginKind::Effect,
            "distortion",
            vec![
                ep(
                    "mode",
                    "Mode",
                    "tanh",
                    &["tanh", "soft_clip", "hard_clip", "tube", "foldback"],
                ),
                fp("drive_db", "Drive", 6.0, 0.0, 36.0, "dB"),
                fp("bias", "Bias", 0.0, -0.5, 0.5, "").desc("Asymmetry (tube mode)"),
                fp("tone_hz", "Tone", 20000.0, 1000.0, 20000.0, "Hz").log(),
                fp("mix", "Mix", 1.0, 0.0, 1.0, ""),
                fp("output_db", "Output", -3.0, -24.0, 12.0, "dB"),
                ep("oversampling", "Oversampling", "2x", &["1x", "2x", "4x"]),
            ],
            "Waveshaping saturation and distortion with IIR oversampling against aliasing.",
        )
    }
}

fn synth_params() -> Vec<ParamSpec> {
    vec![
        ep(
            "voice_mode",
            "Voice Mode",
            "poly",
            &["poly", "mono", "legato"],
        ),
        ip("polyphony", "Polyphony", 16, 1, 32, "voices"),
        fp("glide", "Glide", 0.0, 0.0, 2.0, "s"),
        ep("osc1_wave", "Osc 1 Wave", "saw", WAVES),
        fp("osc1_level", "Osc 1 Level", 0.8, 0.0, 1.0, ""),
        ip("osc1_octave", "Osc 1 Octave", 0, -3, 3, ""),
        fp("osc1_detune", "Osc 1 Detune", 0.0, -100.0, 100.0, "cents"),
        ep("osc2_wave", "Osc 2 Wave", "square", WAVES),
        fp("osc2_level", "Osc 2 Level", 0.0, 0.0, 1.0, ""),
        ip("osc2_octave", "Osc 2 Octave", 0, -3, 3, ""),
        ip("osc2_semitone", "Osc 2 Semitone", 0, -12, 12, ""),
        fp("osc2_detune", "Osc 2 Detune", 7.0, -100.0, 100.0, "cents"),
        fp("pulse_width", "Pulse Width", 0.5, 0.05, 0.95, ""),
        fp("noise_level", "Noise", 0.0, 0.0, 1.0, ""),
        ip("unison", "Unison Voices", 1, 1, 7, ""),
        fp("unison_detune", "Unison Detune", 15.0, 0.0, 100.0, "cents"),
        fp("unison_spread", "Unison Spread", 0.7, 0.0, 1.0, ""),
        ep(
            "filter_type",
            "Filter Type",
            "lowpass",
            &["lowpass", "highpass", "bandpass"],
        ),
        fp("filter_cutoff", "Cutoff", 2500.0, 20.0, 20000.0, "Hz").log(),
        fp("filter_resonance", "Resonance", 0.15, 0.0, 1.0, ""),
        fp(
            "filter_env_amount",
            "Filter Env Amount",
            2.0,
            -8.0,
            8.0,
            "oct",
        ),
        fp("filter_keytrack", "Key Tracking", 0.5, 0.0, 1.0, ""),
        fp("filter_attack", "Filter Attack", 0.005, 0.0005, 10.0, "s").log(),
        fp("filter_decay", "Filter Decay", 0.4, 0.001, 10.0, "s").log(),
        fp("filter_sustain", "Filter Sustain", 0.3, 0.0, 1.0, ""),
        fp("filter_release", "Filter Release", 0.3, 0.001, 10.0, "s").log(),
        fp("amp_attack", "Amp Attack", 0.005, 0.0005, 10.0, "s").log(),
        fp("amp_decay", "Amp Decay", 0.3, 0.001, 10.0, "s").log(),
        fp("amp_sustain", "Amp Sustain", 0.8, 0.0, 1.0, ""),
        fp("amp_release", "Amp Release", 0.25, 0.001, 10.0, "s").log(),
        fp("lfo_rate", "LFO Rate", 5.0, 0.01, 20.0, "Hz").log(),
        fp("lfo_pitch_cents", "Vibrato", 0.0, 0.0, 100.0, "cents"),
        fp("lfo_cutoff_oct", "LFO to Cutoff", 0.0, 0.0, 4.0, "oct"),
        fp(
            "velocity_sensitivity",
            "Velocity Sensitivity",
            0.7,
            0.0,
            1.0,
            "",
        ),
        fp(
            "velocity_to_filter",
            "Velocity to Filter",
            0.3,
            0.0,
            1.0,
            "",
        ),
        ip("bend_range", "Pitch-Bend Range", 2, 0, 24, "semitones"),
        fp("gain_db", "Output", -9.0, -48.0, 12.0, "dB"),
    ]
}

fn synth_presets() -> Vec<Preset> {
    vec![
        preset!("Init"),
        preset!("Analog Bass", "Saw plus sub square, mono with slight glide";
            voice_mode = "mono", glide = 0.03, osc1_wave = "saw", osc2_wave = "square",
            osc2_level = 0.6, osc2_octave = -1, osc2_detune = 0.0, filter_cutoff = 420.0,
            filter_resonance = 0.3, filter_env_amount = 2.8, filter_decay = 0.22,
            filter_sustain = 0.15, amp_attack = 0.002, amp_decay = 0.3, amp_sustain = 0.85,
            amp_release = 0.08, filter_keytrack = 0.3, gain_db = -6.0),
        preset!("Sub Bass";
            voice_mode = "mono", osc1_wave = "sine", osc1_level = 1.0, osc2_wave = "triangle",
            osc2_level = 0.25, osc2_detune = 0.0, filter_cutoff = 600.0, filter_env_amount = 0.0,
            amp_attack = 0.003, amp_release = 0.1, gain_db = -4.0),
        preset!("Acid Bass";
            voice_mode = "legato", glide = 0.06, osc1_wave = "saw", filter_cutoff = 300.0,
            filter_resonance = 0.78, filter_env_amount = 3.5, filter_decay = 0.18,
            filter_sustain = 0.05, amp_sustain = 0.9, amp_release = 0.05, gain_db = -8.0),
        preset!("Warm Pad";
            osc1_wave = "saw", osc2_wave = "saw", osc2_level = 0.7, osc2_detune = 9.0, unison = 3,
            unison_detune = 12.0, filter_cutoff = 1400.0, filter_resonance = 0.12,
            filter_env_amount = 1.2, filter_attack = 0.8, filter_decay = 1.5,
            filter_sustain = 0.5, amp_attack = 0.6, amp_decay = 1.0, amp_sustain = 0.85,
            amp_release = 1.6, lfo_rate = 0.3, lfo_cutoff_oct = 0.3, velocity_sensitivity = 0.3,
            gain_db = -14.0),
        preset!("Strings", "Bowed ensemble approximation";
            osc1_wave = "saw", osc2_wave = "saw", osc2_level = 0.6, osc2_detune = -6.0,
            unison = 5, unison_detune = 10.0, unison_spread = 0.9, filter_cutoff = 3200.0,
            filter_env_amount = 0.6, filter_attack = 0.3, filter_decay = 1.0,
            filter_sustain = 0.7, amp_attack = 0.35, amp_decay = 0.5, amp_sustain = 0.9,
            amp_release = 0.8, lfo_rate = 5.2, lfo_pitch_cents = 6.0,
            velocity_sensitivity = 0.5, gain_db = -15.0),
        preset!("Brass", "Brass section approximation";
            osc1_wave = "saw", osc2_wave = "saw", osc2_level = 0.8, osc2_detune = 5.0,
            unison = 2, unison_detune = 6.0, filter_cutoff = 700.0, filter_resonance = 0.1,
            filter_env_amount = 2.6, filter_attack = 0.07, filter_decay = 0.45,
            filter_sustain = 0.55, amp_attack = 0.04, amp_decay = 0.3, amp_sustain = 0.85,
            amp_release = 0.25, velocity_to_filter = 0.7, lfo_rate = 5.5, lfo_pitch_cents = 4.0,
            gain_db = -12.0),
        preset!("Pluck";
            osc1_wave = "saw", osc2_wave = "square", osc2_level = 0.3, osc2_octave = 1,
            filter_cutoff = 600.0, filter_env_amount = 4.0, filter_decay = 0.18,
            filter_sustain = 0.0, amp_attack = 0.001, amp_decay = 0.45, amp_sustain = 0.0,
            amp_release = 0.25, gain_db = -8.0),
        preset!("Supersaw Lead";
            osc1_wave = "saw", osc2_wave = "saw", osc2_level = 0.5, osc2_octave = 1, unison = 7,
            unison_detune = 28.0, unison_spread = 0.9, filter_cutoff = 5000.0,
            filter_env_amount = 1.0, amp_attack = 0.01, amp_sustain = 0.9, amp_release = 0.3,
            gain_db = -16.0),
        preset!("Square Lead";
            voice_mode = "legato", glide = 0.04, osc1_wave = "square", osc2_wave = "saw",
            osc2_level = 0.4, osc2_octave = 1, filter_cutoff = 3000.0, filter_env_amount = 1.5,
            amp_sustain = 0.9, lfo_rate = 5.5, lfo_pitch_cents = 10.0, gain_db = -10.0),
        preset!("Electric Keys";
            osc1_wave = "triangle", osc2_wave = "sine", osc2_level = 0.5, osc2_octave = 1,
            osc2_detune = 0.0, filter_cutoff = 2600.0, filter_env_amount = 1.5,
            filter_decay = 0.6, amp_attack = 0.002, amp_decay = 1.4, amp_sustain = 0.25,
            amp_release = 0.4, velocity_to_filter = 0.8, gain_db = -8.0),
        preset!("Glass Bells";
            osc1_wave = "sine", osc2_wave = "triangle", osc2_level = 0.45, osc2_octave = 2,
            osc2_semitone = 7, osc2_detune = 0.0, filter_cutoff = 9000.0,
            filter_env_amount = 0.0, amp_attack = 0.002, amp_decay = 1.8, amp_sustain = 0.0,
            amp_release = 1.2, gain_db = -8.0),
        preset!("Choir Pad";
            osc1_wave = "pulse", pulse_width = 0.3, osc2_wave = "saw", osc2_level = 0.4,
            unison = 4, unison_detune = 9.0, filter_type = "bandpass", filter_cutoff = 1100.0,
            filter_resonance = 0.25, filter_env_amount = 0.0, amp_attack = 0.5,
            amp_release = 1.4, lfo_rate = 4.8, lfo_pitch_cents = 5.0, gain_db = -10.0),
    ]
}

fn synth() -> PluginDescriptor {
    PluginDescriptor {
        presets: synth_presets(),
        tags: strings(&["synth", "bass", "pad", "lead", "keys", "strings", "brass"]),
        ..descriptor(
            "sonara.synth.poly",
            "Sonara Poly Synth",
            PluginKind::Instrument,
            "synth",
            synth_params(),
            concat!(
                "Polyphonic subtractive synthesizer: 2 band-limited oscillators with unison, ",
                "noise, resonant state-variable filter with envelope, amp envelope, LFO, ",
                "mono/legato glide."
            ),
        )
    }
}

fn drum_params() -> Vec<ParamSpec> {
    vec![
        fp("kick_tune", "Kick Tune", 52.0, 30.0, 120.0, "Hz"),
        fp("kick_decay", "Kick Decay", 0.45, 0.08, 2.0, "s"),
        fp("kick_punch", "Kick Punch", 0.5, 0.0, 1.0, ""),
        fp("snare_tune", "Snare Tune", 1.0, 0.5, 2.0, "x"),
        fp("snare_tone", "Snare Snappiness", 0.6, 0.0, 1.0, ""),
        fp("snare_decay", "Snare Decay", 0.18, 0.05, 1.0, "s"),
        fp("hat_tune", "Hi-Hat Tune", 1.0, 0.5, 2.0, "x"),
        fp("hat_decay", "Hi-Hat Decay", 1.0, 0.3, 3.0, "x"),
        fp("clap_decay", "Clap Decay", 0.25, 0.05, 1.0, "s"),
        fp("tom_tune", "Tom Tune", 1.0, 0.5, 2.0, "x"),
        fp("cymbal_decay", "Cymbal Decay", 1.6, 0.3, 5.0, "s"),
        fp("drive", "Drive", 0.1, 0.0, 1.0, ""),
        fp("kick_level_db", "Kick Level", 0.0, -48.0, 12.0, "dB"),
        fp("snare_level_db", "Snare Level", -2.0, -48.0, 12.0, "dB"),
        fp("hat_level_db", "Hi-Hat Level", -9.0, -48.0, 12.0, "dB"),
        fp("cymbal_level_db", "Cymbal Level", -10.0, -48.0, 12.0, "dB"),
        fp("perc_level_db", "Percussion Level", -6.0, -48.0, 12.0, "dB"),
        fp("stereo_width", "Stereo Width", 1.0, 0.0, 1.0, ""),
        // The kit peaks around -6 dBFS at velocity 100.
        fp("gain_db", "Output", -9.0, -48.0, 12.0, "dB"),
    ]
}

fn drums() -> PluginDescriptor {
    PluginDescriptor {
        presets: vec![
            preset!("Electronic 808";
                kick_tune = 45.0, kick_decay = 0.9, kick_punch = 0.3, snare_tone = 0.7,
                hat_tune = 1.0, hat_decay = 1.0, drive = 0.05),
            preset!("Punchy 909";
                kick_tune = 55.0, kick_decay = 0.35, kick_punch = 0.8, snare_tune = 1.1,
                snare_tone = 0.55, snare_decay = 0.2, hat_tune = 1.15, drive = 0.25),
            preset!("Tight House";
                kick_tune = 50.0, kick_decay = 0.4, kick_punch = 0.65, clap_decay = 0.2,
                hat_decay = 0.7, hat_level_db = -11.0),
            preset!("Cinematic Toms", "Big low toms and long cymbals for orchestral percussion";
                kick_tune = 40.0, kick_decay = 1.2, kick_punch = 0.4, tom_tune = 0.7,
                cymbal_decay = 3.5, drive = 0.0, perc_level_db = 0.0),
            preset!("Lo-Fi";
                kick_tune = 48.0, kick_decay = 0.5, snare_tone = 0.4, hat_tune = 0.8,
                hat_decay = 0.8, drive = 0.6),
        ],
        tags: strings(&["drums", "percussion", "808", "909", "beats"]),
        ..descriptor(
            "sonara.drums",
            "Sonara Drum Kit",
            PluginKind::Instrument,
            "drums",
            drum_params(),
            concat!(
                "Synthesized drum machine on the General MIDI percussion map (36 kick, 38 snare, ",
                "39 clap, 42/44/46 hats, 41-50 toms, 49 crash, 51 ride, ...)."
            ),
        )
    }
}

/// The built-in descriptors, in the prototype's registration order.
fn builtin_descriptors() -> Vec<PluginDescriptor> {
    vec![
        utility(),
        eq(),
        filter(),
        compressor(),
        limiter(),
        gate(),
        delay(),
        reverb(),
        convolver(),
        saturation(),
        synth(),
        drums(),
    ]
}

/// A registry holding the 12 built-in instruments and effects, in the prototype's registration
/// order (`create_registry()` without extra plugins). Further plugins are added with
/// [`PluginRegistry::register`].
///
/// # Panics
///
/// Never: the built-in plugin IDs are distinct, which a unit test checks.
#[must_use]
pub fn builtin_registry() -> PluginRegistry {
    let mut registry = PluginRegistry::new();
    for descriptor in builtin_descriptors() {
        registry
            .register(descriptor)
            .expect("the built-in plugin IDs are distinct (unit test `builtin_catalog_is_valid`)");
    }
    registry
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{builtin_descriptors, builtin_registry};
    use crate::model::device::ParamValue;
    use crate::plugins::PluginKind;

    /// Typed equality: `Int(1)` and `Float(1.0)` differ.
    fn same(a: &ParamValue, b: &ParamValue) -> bool {
        format!("{a:?}") == format!("{b:?}")
    }

    #[test]
    fn builtin_catalog_is_valid() {
        let descriptors = builtin_descriptors();
        assert_eq!(descriptors.len(), 12);
        let ids: HashSet<&str> = descriptors.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids.len(), descriptors.len());
        for d in &descriptors {
            // `__post_init__` of the descriptor and of every spec, plus every preset.
            d.validate().unwrap_or_else(|e| panic!("{}: {e}", d.id));
            for spec in &d.params {
                let coerced = spec.coerce_param(&spec.default).unwrap();
                assert!(same(&coerced, &spec.default), "{}/{}", d.id, spec.id);
            }
        }
    }

    #[test]
    fn builtin_registry_is_consistent() {
        let reg = builtin_registry();
        let expected: Vec<String> = builtin_descriptors().into_iter().map(|d| d.id).collect();
        let ids: Vec<&str> = reg.all(None).iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, expected);
        assert!(expected.iter().all(|id| reg.contains(id)));
        let instruments: HashSet<&str> = reg
            .all(Some(PluginKind::Instrument))
            .iter()
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(
            instruments,
            HashSet::from(["sonara.synth.poly", "sonara.drums"])
        );
        assert!(reg.get("sonara.dynamics.compressor").unwrap().sidechain);
        assert!(reg.get("sonara.dynamics.gate").unwrap().sidechain);
    }

    #[test]
    fn preset_values_keep_the_keyword_order_and_types() {
        let reg = builtin_registry();
        let eq = reg.get("sonara.eq.parametric").unwrap();
        let telephone = eq.preset("telephone").unwrap();
        assert_eq!(
            telephone.params.keys().collect::<Vec<_>>(),
            [
                "hp_on",
                "hp_freq",
                "hp_slope",
                "lp_on",
                "lp_freq",
                "lp_slope",
                "mid1_freq",
                "mid1_gain"
            ]
        );
        assert!(same(&telephone.params["hp_on"], &ParamValue::Bool(true)));
        assert!(same(
            &telephone.params["hp_slope"],
            &ParamValue::Str("24".into())
        ));
        let synth = reg.get("sonara.synth.poly").unwrap();
        assert_eq!(synth.preset("Init").unwrap().params.len(), 0);
        let bells = synth.preset("Glass Bells").unwrap();
        assert!(same(&bells.params["osc2_semitone"], &ParamValue::Int(7)));
        assert!(same(&bells.params["osc2_detune"], &ParamValue::Float(0.0)));
        let bass = synth.preset("Analog Bass").unwrap();
        assert!(same(&bass.params["osc2_octave"], &ParamValue::Int(-1)));
        assert_eq!(
            bass.description,
            "Saw plus sub square, mono with slight glide"
        );
    }
}
