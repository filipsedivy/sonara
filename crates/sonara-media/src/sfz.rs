//! Reads an SFZ patch and the samples it names into a [`SampleBank`] for `sonara.sampler`
//! (ADR 0010). Runs off the real-time thread: it reads files and allocates.
//!
//! **Why our own parser (QG-13).** No maintained, permissively licensed Rust crate reads the
//! full multi-file SFZ dialect that orchestral libraries use (`#include`, `#define`,
//! `default_path`, `sw_*`, `seq_*`): `sofiza` has seen no release since 2022, and the C++
//! engine sfizz would need an `unsafe` FFI module outside RT-10. The subset below is the one the
//! sampler plays, taken from the opcodes real libraries use (VSCO 2 CE, Sonatina).
//!
//! **Supported opcodes:** `sample`, `default_path`, `key`, `lokey`, `hikey`, `pitch_keycenter`,
//! `lovel`, `hivel`, `lorand`, `hirand`, `seq_length`, `seq_position`, `sw_lokey`, `sw_hikey`,
//! `sw_default`, `sw_last`, `trigger`, `group`, `off_by`, `loop_mode`, `loop_start`, `loop_end`,
//! `offset`, `end`, `tune`, `transpose`, `volume`, `ampeg_attack`, `ampeg_release`, and the
//! `#define`/`#include` directives. Every other opcode is recorded in
//! [`SampleBank::ignored_opcodes`] and has no effect.
//!
//! Samples are decoded whole, at their own rate, into `f32` (no streaming yet): memory is the
//! decoded size of the samples the patch names, capped at [`MAX_BANK_BYTES`].

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use sonara_dsp::bank::{Keyswitches, LoopMode, Region, Sample, SampleBank, Trigger};

use crate::error::MediaError;
use crate::stream::AudioStream;

/// The most decoded sample memory one patch may use, in bytes.
pub const MAX_BANK_BYTES: usize = 3 << 30;
/// The deepest `#include` nesting.
const MAX_INCLUDE_DEPTH: usize = 8;
/// Frames decoded per read.
const READ_FRAMES: usize = 16_384;

type Opcodes = HashMap<String, String>;

/// Reads the SFZ file `path` and the samples it names.
///
/// # Errors
///
/// - [`MediaError::Io`] when the patch or a sample file cannot be opened or read (a missing
///   library or sample);
/// - [`MediaError::Malformed`] when the patch has no region, a region names no sample, an
///   `#include` is too deep, or the samples exceed [`MAX_BANK_BYTES`];
/// - [`MediaError::Unsupported`] and [`MediaError::Decode`] for a sample that cannot be decoded
///   or has more than two channels.
pub fn load_sfz(path: &Path) -> Result<SampleBank, MediaError> {
    let text = read_preprocessed(path, &mut HashMap::new(), 0)?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let parsed = parse(&text);
    let mut ignored = parsed.ignored;
    let mut loader = Loader::default();
    let mut bank = SampleBank::default();
    for opcodes in &parsed.regions {
        let region = loader.region(dir, opcodes, path)?;
        bank.regions.extend(region);
    }
    if bank.regions.is_empty() {
        return Err(MediaError::malformed(
            path,
            "the patch has no playable region",
        ));
    }
    bank.samples = loader.samples;
    bank.keyswitches = parsed.keyswitches;
    bank.tail_seconds = tail_seconds(&bank);
    ignored.sort();
    ignored.dedup();
    bank.ignored_opcodes = ignored;
    Ok(bank)
}

fn tail_seconds(bank: &SampleBank) -> f64 {
    bank.regions
        .iter()
        .map(|region| {
            if region.trigger == Trigger::Release {
                bank.samples.get(region.sample).map_or(0.0, |sample| {
                    let frames = sample.frames().saturating_sub(region.offset);
                    f64::from(u32::try_from(frames).unwrap_or(u32::MAX))
                        / f64::from(sample.rate.max(1))
                })
            } else {
                region.release
            }
        })
        .fold(0.0, f64::max)
}

// --- Preprocessing: comments, #include, #define ---

fn read_preprocessed(
    path: &Path,
    defines: &mut HashMap<String, String>,
    depth: usize,
) -> Result<String, MediaError> {
    if depth > MAX_INCLUDE_DEPTH {
        return Err(MediaError::malformed(path, "#include nesting is too deep"));
    }
    let raw = fs::read_to_string(path).map_err(|e| MediaError::io("read", path, e))?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut out = String::new();
    for line in strip_block_comments(&raw).lines() {
        let line = line.split("//").next().unwrap_or("").trim();
        if let Some(rest) = line.strip_prefix("#include") {
            let name = rest.trim().trim_matches('"').replace('\\', "/");
            out.push_str(&read_preprocessed(&dir.join(name), defines, depth + 1)?);
        } else if let Some(rest) = line.strip_prefix("#define") {
            let mut words = rest.split_whitespace();
            if let (Some(name), Some(value)) = (words.next(), words.next()) {
                defines.insert(name.to_owned(), value.to_owned());
            }
        } else {
            let mut line = line.to_owned();
            // Longest names first, so `$KEY2` is not eaten by `$KEY`.
            let mut names: Vec<_> = defines.iter().collect();
            names.sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));
            for (name, value) in names {
                if line.contains(name.as_str()) {
                    line = line.replace(name.as_str(), value);
                }
            }
            out.push_str(&line);
            out.push('\n');
        }
    }
    Ok(out)
}

fn strip_block_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("/*") {
        out.push_str(rest.get(..start).unwrap_or(""));
        let after = rest.get(start + 2..).unwrap_or("");
        match after.find("*/") {
            Some(end) => rest = after.get(end + 2..).unwrap_or(""),
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

// --- Parsing: headers and opcodes ---

struct Parsed {
    regions: Vec<Opcodes>,
    keyswitches: Option<Keyswitches>,
    ignored: Vec<String>,
}

const SUPPORTED: &[&str] = &[
    "sample",
    "default_path",
    "key",
    "lokey",
    "hikey",
    "pitch_keycenter",
    "lovel",
    "hivel",
    "lorand",
    "hirand",
    "seq_length",
    "seq_position",
    "sw_lokey",
    "sw_hikey",
    "sw_default",
    "sw_last",
    "trigger",
    "group",
    "off_by",
    "loop_mode",
    "loopmode",
    "loop_start",
    "loopstart",
    "loop_end",
    "loopend",
    "offset",
    "end",
    "tune",
    "transpose",
    "volume",
    "ampeg_attack",
    "ampeg_release",
];

/// Opcodes that only label things; ignoring them loses nothing, so they are not reported.
const HARMLESS: &[&str] = &["sw_label", "group_label", "region_label", "sw_note"];

fn parse(text: &str) -> Parsed {
    // Opcode scopes, outermost first: control, global, master, group.
    let mut scopes: [Opcodes; 4] = Default::default();
    let mut region: Option<Opcodes> = None;
    let mut regions = Vec::new();
    let mut ignored = Vec::new();
    let mut current: Option<(usize, String)> = None; // (scope, opcode) being extended
    let mut in_other = false;
    let mut open_scope = 0;
    let finish =
        |region: &mut Option<Opcodes>, scopes: &[Opcodes; 4], regions: &mut Vec<Opcodes>| {
            if let Some(own) = region.take() {
                let mut merged = Opcodes::new();
                for scope in scopes {
                    merged.extend(scope.clone());
                }
                merged.extend(own);
                regions.push(merged);
            }
        };
    for token in text.split_whitespace() {
        if token.starts_with('<') && token.ends_with('>') {
            current = None;
            in_other = false;
            finish(&mut region, &scopes, &mut regions);
            match token {
                "<control>" => {
                    scopes = Default::default();
                    open_scope = 0;
                }
                "<global>" => {
                    scopes.iter_mut().skip(1).for_each(Opcodes::clear);
                    open_scope = 1;
                }
                "<master>" => {
                    scopes.iter_mut().skip(2).for_each(Opcodes::clear);
                    open_scope = 2;
                }
                "<group>" => {
                    scopes.iter_mut().skip(3).for_each(Opcodes::clear);
                    open_scope = 3;
                }
                "<region>" => region = Some(Opcodes::new()),
                _ => in_other = true,
            }
            continue;
        }
        if let Some((name, value)) = token.split_once('=') {
            let name = name.to_ascii_lowercase();
            if in_other {
                continue;
            }
            if !SUPPORTED.contains(&name.as_str()) && !HARMLESS.contains(&name.as_str()) {
                ignored.push(name_without_digits(&name));
            }
            let scope = if region.is_some() { 4 } else { open_scope };
            let target = match scope {
                4 => region.as_mut(),
                n => scopes.get_mut(n),
            };
            if let Some(target) = target {
                target.insert(name.clone(), value.to_owned());
            }
            current = Some((scope, name));
        } else if let Some((scope, name)) = &current {
            // A value with spaces, such as a sample path: the token continues the opcode.
            let target = match *scope {
                4 => region.as_mut(),
                n => scopes.get_mut(n),
            };
            if let Some(value) = target.and_then(|t| t.get_mut(name)) {
                value.push(' ');
                value.push_str(token);
            }
        }
    }
    finish(&mut region, &scopes, &mut regions);
    let merged_scopes: Opcodes = scopes.iter().flat_map(Clone::clone).collect();
    let keyswitches = match (
        merged_scopes.get("sw_lokey").and_then(|v| note(v)),
        merged_scopes.get("sw_hikey").and_then(|v| note(v)),
    ) {
        (Some(lo), Some(hi)) => Some(Keyswitches {
            lo,
            hi,
            default: merged_scopes.get("sw_default").and_then(|v| note(v)),
        }),
        _ => None,
    };
    Parsed {
        regions,
        keyswitches,
        ignored,
    }
}

/// `locc64` and `hicc64` are one opcode family: report them as `locc*`.
fn name_without_digits(name: &str) -> String {
    let trimmed = name.trim_end_matches(|c: char| c.is_ascii_digit());
    if trimmed.len() == name.len() {
        name.to_owned()
    } else {
        format!("{trimmed}*")
    }
}

/// A MIDI key given as a number or a note name (`c4` is 60; `#` and `b` are accidentals).
fn note(value: &str) -> Option<u8> {
    if let Ok(number) = value.trim().parse::<i32>() {
        return u8::try_from(number).ok().filter(|n| *n <= 127);
    }
    let lower = value.trim().to_ascii_lowercase();
    let mut chars = lower.chars();
    let semitone = match chars.next()? {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    let rest = chars.as_str();
    let (accidental, octave) = match rest.chars().next()? {
        '#' => (1, rest.get(1..)?),
        'b' => (-1, rest.get(1..)?),
        _ => (0, rest),
    };
    let key = (octave.parse::<i32>().ok()? + 1) * 12 + semitone + accidental;
    u8::try_from(key).ok().filter(|n| *n <= 127)
}

// --- Regions and samples ---

#[derive(Default)]
struct Loader {
    samples: Vec<Sample>,
    index: HashMap<PathBuf, usize>,
    bytes: usize,
}

impl Loader {
    /// The region `opcodes` describe, with its sample loaded; `None` for a disabled region
    /// (`end=-1`) or one without a sample.
    fn region(
        &mut self,
        dir: &Path,
        opcodes: &Opcodes,
        patch: &Path,
    ) -> Result<Option<Region>, MediaError> {
        let Some(name) = opcodes.get("sample") else {
            return Ok(None);
        };
        if opcodes
            .get("end")
            .is_some_and(|v| v.trim().starts_with('-'))
        {
            return Ok(None);
        }
        let mut file = dir.to_path_buf();
        for part in [opcodes.get("default_path"), Some(name)]
            .into_iter()
            .flatten()
        {
            file.push(part.replace('\\', "/"));
        }
        let sample = self.sample(&file, patch)?;
        let frames = self.samples.get(sample).map_or(0, Sample::frames);
        let num = |key: &str| opcodes.get(key).and_then(|v| v.trim().parse::<f64>().ok());
        let int = |key: &str| num(key).map(|v| v.max(0.0).trunc());
        let key_of = |key: &str| opcodes.get(key).and_then(|v| note(v));
        let mut region = Region {
            sample,
            ..Region::default()
        };
        if let Some(key) = key_of("key") {
            region.key_lo = key;
            region.key_hi = key;
            region.pitch_keycenter = key;
        }
        region.key_lo = key_of("lokey").unwrap_or(region.key_lo);
        region.key_hi = key_of("hikey").unwrap_or(region.key_hi);
        region.pitch_keycenter = key_of("pitch_keycenter").unwrap_or(region.pitch_keycenter);
        region.vel_lo = to_u8(int("lovel"), 0);
        region.vel_hi = to_u8(int("hivel"), 127);
        region.rand_lo = num("lorand").unwrap_or(0.0);
        region.rand_hi = num("hirand").unwrap_or(1.0);
        region.seq_length = to_u32(int("seq_length"), 1).max(1);
        region.seq_position = to_u32(int("seq_position"), 1).max(1);
        region.sw_last = key_of("sw_last");
        region.group = to_u32(int("group"), 0);
        region.off_by = to_u32(int("off_by"), 0);
        region.volume_db = num("volume").unwrap_or(0.0);
        region.tune_cents = num("tune").unwrap_or(0.0) + num("transpose").unwrap_or(0.0) * 100.0;
        region.attack = num("ampeg_attack").unwrap_or(0.0).max(0.0);
        region.release = num("ampeg_release").unwrap_or(0.0).max(0.0);
        region.trigger = match opcodes.get("trigger").map(String::as_str) {
            Some("release") => Trigger::Release,
            _ => Trigger::Attack,
        };
        region.offset = to_usize(int("offset"), 0).min(frames);
        region.end = int("end")
            .map_or(frames, |e| to_usize(Some(e), 0) + 1)
            .min(frames);
        let mode = opcodes.get("loop_mode").or_else(|| opcodes.get("loopmode"));
        region.loop_mode = match mode.map(String::as_str) {
            Some("one_shot") => LoopMode::OneShot,
            Some("loop_continuous") => LoopMode::Continuous,
            Some("loop_sustain") => LoopMode::Sustain,
            _ => LoopMode::NoLoop,
        };
        let loop_start = int("loop_start").or_else(|| int("loopstart"));
        let loop_end = int("loop_end").or_else(|| int("loopend"));
        region.loop_start = to_usize(loop_start, 0);
        region.loop_end = loop_end
            .map_or(region.end, |e| to_usize(Some(e), 0) + 1)
            .min(region.end);
        Ok(Some(region))
    }

    /// The index of the sample in `file`, decoding it on first use.
    fn sample(&mut self, file: &Path, patch: &Path) -> Result<usize, MediaError> {
        if let Some(index) = self.index.get(file) {
            return Ok(*index);
        }
        let opened = AudioStream::open(file)?;
        let mut stream = opened.stream;
        stream.check_decodable()?;
        let channels = stream.channels();
        if !(1..=2).contains(&channels) {
            return Err(MediaError::malformed(
                file,
                format!("a sampler plays mono or stereo samples, this has {channels} channels"),
            ));
        }
        let frames = stream
            .frames()
            .ok_or_else(|| MediaError::malformed(file, "the file states no length"))?;
        let frames = usize::try_from(frames).unwrap_or(usize::MAX);
        self.bytes = self
            .bytes
            .saturating_add(frames.saturating_mul(channels).saturating_mul(4));
        if self.bytes > MAX_BANK_BYTES {
            return Err(MediaError::malformed(
                patch,
                format!(
                    "the samples need more than {} MiB of memory",
                    MAX_BANK_BYTES >> 20
                ),
            ));
        }
        let mut data = Vec::with_capacity(frames * channels);
        let mut chunk = vec![0.0_f64; READ_FRAMES * channels];
        loop {
            let read = stream.read(&mut chunk)?;
            if read == 0 {
                break;
            }
            data.extend(chunk.iter().take(read * channels).copied().map(to_f32));
        }
        let sample = Sample {
            rate: stream.sample_rate(),
            channels,
            data,
        };
        self.samples.push(sample);
        let index = self.samples.len() - 1;
        self.index.insert(file.to_owned(), index);
        Ok(index)
    }
}

fn to_u8(value: Option<f64>, default: u8) -> u8 {
    value.map_or(default, |v| {
        u8::try_from(to_usize(Some(v), 0)).unwrap_or(u8::MAX)
    })
}

fn to_u32(value: Option<f64>, default: u32) -> u32 {
    value.map_or(default, |v| {
        u32::try_from(to_usize(Some(v), 0)).unwrap_or(u32::MAX)
    })
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "opcode values are non-negative whole numbers; a huge one saturates"
)]
fn to_usize(value: Option<f64>, default: usize) -> usize {
    value.map_or(default, |v| v.max(0.0) as usize)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "samples are stored as f32; the decoder's f64 holds at most 24 bits of them"
)]
fn to_f32(value: f64) -> f32 {
    value as f32
}
