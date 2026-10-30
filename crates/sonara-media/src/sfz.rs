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
//! **Regions no key starts.** A `key`, `lokey` or `hikey` of -1 means "no key" (SFZ 2), and a
//! region with a controller trigger (`on_loccN`/`on_hiccN`, or their `hdcc` forms, other than
//! -1) is started by that controller instead of a key, as the SFZ format reads it. Such a region
//! is loaded with [`Region::on_key`] false and never plays on a key: Salamander's pedal noises
//! are written this way and used to sound on every note. The deepest scope that writes a key
//! opcode decides, so a region's own `key=60` under a `<global> lokey=-1` plays on key 60. The
//! sampler starts no region from a controller yet, so `on_locc*`/`on_hicc*` stay in the
//! ignored opcodes. Any other key value that names no key is ignored, as before.
//!
//! **Directives.** `#include` and `#define` are read wherever a token starts, not only at the
//! start of a line, as the reference player sfizz reads them: Salamander's
//! `<group> #include "vel.txt" lovel=1 hivel=26 #include "region.txt"` reads both files in
//! place. An `#include` takes one path, quoted (spaces allowed) or a single word, and the line
//! goes on after it. A `#define` takes the rest of its line, and its value is the first word
//! after the name. The `$defines` known so far are substituted in an include path too
//! (Sonatina's `#include "$DIR/$DYN.txt"`), and every include, however deeply nested, resolves
//! against the main patch file's directory, not the including file's, as the SFZ format and
//! sfizz read it: a fragment in `Data/` names its siblings `Data/…` (the MTG saxophones).
//!
//! **Header scopes.** A region inherits the opcodes of the open `<control>`, `<global>`,
//! `<master>` and `<group>` headers. `<global>`, `<master>` and `<group>` each start their own
//! scope afresh and clear the scopes inside it; `<control>` clears only the control scope, so a
//! file included under a `<global>` may start with its own `<control>` and keep that global's
//! opcodes.
//!
//! **Keyswitches.** The switch keys are the union of the `sw_lokey`..`sw_hikey` ranges of every
//! region, each with the opcodes it inherits from its headers or writes itself, so disjoint
//! ranges switch only on their own keys. The default switch is the last valid `sw_default` in
//! the file, at any level, as the reference player sfizz reads it: a later header that does not
//! repeat it (VSCO 2 CE's key-switch patches start a new `<control>` and `<group>`) keeps it.
//!
//! **Samples a region cannot play.** `sample=*silence` is a region that plays nothing; its note
//! still switches off the exclusive group it belongs to, which is what patches write it for. The
//! other generators the SFZ format names with a leading `*` (`*sine`, `*saw`, `*square`,
//! `*triangle`, `*noise`) are not played, and a sample file that does not exist cannot be. Either
//! leaves out only the regions that name it, with a note in [`SampleBank::load_notes`], so one
//! missing file no longer silences a whole instrument. The patch fails only when no region is
//! left. A sample file that exists but cannot be decoded still fails the patch.
//!
//! Samples are decoded whole, at their own rate, into `f32` (no streaming yet): memory is the
//! decoded size of the samples the patch names, capped at [`MAX_BANK_BYTES`].

use std::collections::HashMap;
use std::fs;
use std::io;
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
/// The generator of a region that plays nothing.
const SILENCE: &str = "*silence";
/// The rate given to the silent sample; a sample without frames never plays, so it is not used.
const SILENCE_RATE: u32 = 44_100;

type Opcodes = HashMap<String, String>;

/// Reads the SFZ file `path` and the samples it names.
///
/// A region whose sample file does not exist, or that names a generator the sampler does not
/// play, is left out and noted in [`SampleBank::load_notes`] (see the module documentation). A
/// region that names no sample, or is disabled with a negative `end`, is left out silently.
///
/// # Errors
///
/// - [`MediaError::Io`] when the patch, an included file or an existing sample file cannot be
///   opened or read (a missing library or include);
/// - [`MediaError::Malformed`] when no region is left (the message lists the notes of the
///   regions left out, such as a missing sample), an `#include` is too deep, or the samples
///   exceed [`MAX_BANK_BYTES`];
/// - [`MediaError::NotRecognised`], [`MediaError::Rejected`], [`MediaError::Unsupported`],
///   [`MediaError::Malformed`] and [`MediaError::Decode`] for a sample file that cannot be
///   decoded or has more than two channels.
pub fn load_sfz(path: &Path) -> Result<SampleBank, MediaError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let text = read_preprocessed(path, dir, &mut HashMap::new(), 0)?;
    let parsed = parse(&text);
    let mut ignored = parsed.ignored;
    let mut loader = Loader::default();
    let mut bank = SampleBank::default();
    for opcodes in &parsed.regions {
        let region = loader.region(dir, opcodes, path)?;
        bank.regions.extend(region);
    }
    let notes = loader.notes();
    if bank.regions.is_empty() {
        let message = if notes.is_empty() {
            "the patch has no playable region".to_owned()
        } else {
            format!("the patch has no playable region: {}", notes.join("; "))
        };
        return Err(MediaError::malformed(path, message));
    }
    bank.load_notes = notes;
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

/// The text of the patch file `path` without comments, with each `#include` replaced by the
/// file it names and the `$defines` known so far substituted. The directives are read wherever
/// a token starts, so the line goes on after an `#include` (see the module documentation).
/// Every include path, at any depth, is joined to `root`, the main patch file's directory.
fn read_preprocessed(
    path: &Path,
    root: &Path,
    defines: &mut HashMap<String, String>,
    depth: usize,
) -> Result<String, MediaError> {
    if depth > MAX_INCLUDE_DEPTH {
        return Err(MediaError::malformed(path, "#include nesting is too deep"));
    }
    let raw = fs::read_to_string(path).map_err(|e| MediaError::io("read", path, e))?;
    let mut out = String::new();
    for line in strip_block_comments(&raw).lines() {
        let mut rest = line.split("//").next().unwrap_or("");
        loop {
            let (text, directive) = split_directive(rest);
            push_expanded(&mut out, text, defines);
            match directive {
                Some(Directive::Include { file, after }) => {
                    let file = root.join(expand_defines(file, defines).replace('\\', "/"));
                    out.push_str(&read_preprocessed(&file, root, defines, depth + 1)?);
                    rest = after;
                }
                Some(Directive::Define(definition)) => {
                    let mut words = definition.split_whitespace();
                    if let (Some(name), Some(value)) = (words.next(), words.next()) {
                        defines.insert(name.to_owned(), value.to_owned());
                    }
                    break;
                }
                None => break,
            }
        }
    }
    Ok(out)
}

/// A preprocessor directive found in a line.
enum Directive<'a> {
    /// `#include`: the file it names, and the rest of the line after it.
    Include { file: &'a str, after: &'a str },
    /// `#define`: the rest of its line, which all belongs to the define.
    Define(&'a str),
}

/// Splits `line` at its first directive: the text before it and the directive. A directive
/// counts only where a token starts, at the start of the line or after whitespace or a header's
/// `>`, so a `#` inside a value (`key=c#4`) is never one.
fn split_directive(line: &str) -> (&str, Option<Directive<'_>>) {
    for (at, _) in line.match_indices('#') {
        let Some((text, from)) = line.split_at_checked(at) else {
            continue;
        };
        let token_start = text
            .chars()
            .next_back()
            .is_none_or(|c| c.is_whitespace() || c == '>');
        if token_start && let Some(directive) = directive(from) {
            return (text, Some(directive));
        }
    }
    (line, None)
}

/// The directive `text` starts with: `#include` or `#define` as a whole word.
fn directive(text: &str) -> Option<Directive<'_>> {
    if let Some(rest) = strip_keyword(text, "#include") {
        let (file, after) = include_file(rest);
        Some(Directive::Include { file, after })
    } else {
        strip_keyword(text, "#define").map(Directive::Define)
    }
}

/// `text` after `keyword`, when `text` starts with it and no identifier character follows.
fn strip_keyword<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = text.strip_prefix(keyword)?;
    let whole_word = !rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_');
    whole_word.then_some(rest)
}

/// The file an `#include` names and the text after it. A quoted path runs to its closing quote,
/// spaces included, or to the end of the line when the quote is not closed; an unquoted one is
/// one word.
fn include_file(rest: &str) -> (&str, &str) {
    let rest = rest.trim_start();
    if let Some(quoted) = rest.strip_prefix('"') {
        quoted.split_once('"').unwrap_or((quoted.trim_end(), ""))
    } else {
        rest.split_once(char::is_whitespace).unwrap_or((rest, ""))
    }
}

/// Appends `text` with its `$defines` substituted, on a line of its own, so that its tokens stay
/// apart from an included file's.
fn push_expanded(out: &mut String, text: &str, defines: &HashMap<String, String>) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    out.push_str(&expand_defines(text, defines));
    out.push('\n');
}

/// `text` with every `$define` in `defines` replaced by its value; a name that is not defined
/// stays as written.
fn expand_defines(text: &str, defines: &HashMap<String, String>) -> String {
    let mut text = text.to_owned();
    // Longest names first, so `$KEY2` is not eaten by `$KEY`.
    let mut names: Vec<_> = defines.iter().collect();
    names.sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));
    for (name, value) in names {
        if text.contains(name.as_str()) {
            text = text.replace(name.as_str(), value);
        }
    }
    text
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
    keyswitches: Keyswitches,
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
    // The last valid `sw_default` at any level; no later header clears it.
    let mut sw_default = None;
    let finish =
        |region: &mut Option<Opcodes>, scopes: &[Opcodes; 4], regions: &mut Vec<Opcodes>| {
            if let Some(own) = region.take() {
                regions.push(merge(scopes, own));
            }
        };
    for token in text.split_whitespace() {
        if token.starts_with('<') && token.ends_with('>') {
            current = None;
            in_other = false;
            finish(&mut region, &scopes, &mut regions);
            match token {
                "<control>" => {
                    // A new `<control>` resets only the control scope (`default_path` and the
                    // like), as ARIA and sfizz read it: the enclosing `<global>`, `<master>` and
                    // `<group>` keep their opcodes, so a switch range written before an
                    // `#include` that starts with `<control>` still reaches its regions.
                    if let Some(control) = scopes.first_mut() {
                        control.clear();
                    }
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
            if name == "sw_default"
                && let Some(key) = note(value)
            {
                sw_default = Some(key);
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
    let mut keyswitches = Keyswitches::NONE;
    keyswitches.default = sw_default;
    for opcodes in &regions {
        let range = |name: &str| opcodes.get(name).and_then(|v| note(v));
        if let (Some(lo), Some(hi)) = (range("sw_lokey"), range("sw_hikey")) {
            keyswitches.add_range(lo, hi);
        }
    }
    Parsed {
        regions,
        keyswitches,
        ignored,
    }
}

/// The opcodes that set a region's keys; a value of -1 means "no key".
const KEY_OPCODES: [&str; 3] = ["key", "lokey", "hikey"];

/// The opcodes a region plays with: those of its header scopes, outermost first, then its own,
/// a deeper scope overriding the same opcode. As the reference player sfizz reads it, the last
/// key opcode decides whether a key starts the region, so a key of -1 from an outer scope is
/// dropped when a deeper scope writes a key opcode and no -1: a region's `key=60` under a
/// `<global> lokey=-1 hikey=-1` plays on key 60. A dropped -1 never set a key range.
fn merge(scopes: &[Opcodes; 4], own: Opcodes) -> Opcodes {
    let is_no_key = |name: &str, value: &str| KEY_OPCODES.contains(&name) && is_minus_one(value);
    let last_keys = scopes
        .iter()
        .chain(std::iter::once(&own))
        .rev()
        .find(|scope| KEY_OPCODES.iter().any(|name| scope.contains_key(*name)));
    let keeps_no_key =
        last_keys.is_some_and(|scope| scope.iter().any(|(name, value)| is_no_key(name, value)));
    let mut merged = Opcodes::new();
    for scope in scopes {
        merged.extend(scope.clone());
    }
    merged.extend(own);
    if !keeps_no_key {
        merged.retain(|name, value| !is_no_key(name, value));
    }
    merged
}

/// Whether a key starts the region its merged `opcodes` describe. Not when `key`, `lokey` or
/// `hikey` is -1 ("no key", SFZ 2), and not when a controller trigger is set, since it replaces
/// the key's.
fn on_key(opcodes: &Opcodes) -> bool {
    let no_key = KEY_OPCODES
        .into_iter()
        .any(|name| opcodes.get(name).is_some_and(|value| is_minus_one(value)));
    let on_controller = opcodes
        .iter()
        .any(|(name, value)| is_controller_trigger(name) && !is_minus_one(value));
    !no_key && !on_controller
}

/// Whether an opcode value is -1, which the SFZ format uses for "none": no key, or no
/// controller trigger (the default of `on_loccN` and `on_hiccN`).
fn is_minus_one(value: &str) -> bool {
    value.trim().parse::<i32>() == Ok(-1)
}

/// Whether `name` sets a controller trigger: `on_locc`, `on_hicc` or their high-definition forms
/// `on_lohdcc` and `on_hihdcc`, followed by the controller number.
fn is_controller_trigger(name: &str) -> bool {
    ["on_locc", "on_hicc", "on_lohdcc", "on_hihdcc"]
        .into_iter()
        .any(|prefix| {
            name.strip_prefix(prefix).is_some_and(|number| {
                !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
            })
        })
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
    /// Each sample file named so far: its index in `samples`, or its note when it is missing.
    files: HashMap<PathBuf, FileState>,
    /// The index in `samples` of the silent sample, once a region names `*silence`.
    silence: Option<usize>,
    /// Each generator the sampler does not play, named so far: the index of its note.
    generators: HashMap<String, usize>,
    /// What the regions left out name, in the order the patch first names it.
    skipped: Vec<Skipped>,
    bytes: usize,
}

/// A sample file the loader has looked for.
#[derive(Debug, Clone, Copy)]
enum FileState {
    /// Decoded: its index in [`Loader::samples`].
    Decoded(usize),
    /// Not there: the index of its note in [`Loader::skipped`].
    Missing(usize),
}

/// A sample that regions name but the sampler cannot play, and how many regions name it.
struct Skipped {
    /// Why it cannot be played, naming the sample.
    reason: String,
    /// The regions left out because of it.
    regions: usize,
}

impl Skipped {
    /// The note for [`SampleBank::load_notes`].
    fn note(&self) -> String {
        if self.regions == 1 {
            format!("{} (1 region left out)", self.reason)
        } else {
            format!("{} ({} regions left out)", self.reason, self.regions)
        }
    }
}

impl Loader {
    /// The region `opcodes` describe, with its sample loaded; `None` for a disabled region
    /// (`end=-1`), one without a sample, and one whose sample cannot be played (noted).
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
        let sample = if name.trim().starts_with('*') {
            self.generator(name.trim())
        } else {
            let mut file = dir.to_path_buf();
            for part in [opcodes.get("default_path"), Some(name)]
                .into_iter()
                .flatten()
            {
                file.push(part.replace('\\', "/"));
            }
            self.file(&file, patch)?
        };
        let Some(sample) = sample else {
            return Ok(None);
        };
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
        region.on_key = on_key(opcodes);
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

    /// The index of the sample of the generator `name`: the shared silent sample for
    /// `*silence`. Any other generator is not played: `None`, and the region is noted.
    fn generator(&mut self, name: &str) -> Option<usize> {
        if name == SILENCE {
            if self.silence.is_none() {
                self.samples.push(Sample {
                    rate: SILENCE_RATE,
                    channels: 1,
                    data: Vec::new(),
                });
                self.silence = Some(self.samples.len() - 1);
            }
            return self.silence;
        }
        let at = if let Some(at) = self.generators.get(name) {
            *at
        } else {
            let at = self.note(format!("the sampler does not play the generator {name}"));
            self.generators.insert(name.to_owned(), at);
            at
        };
        self.count(at);
        None
    }

    /// The index of the sample in `file`, decoding it on first use. A file that does not exist
    /// gives `None`, and the region is noted.
    ///
    /// # Errors
    ///
    /// The errors of [`Loader::decode`], except that the file does not exist.
    fn file(&mut self, file: &Path, patch: &Path) -> Result<Option<usize>, MediaError> {
        match self.files.get(file).copied() {
            Some(FileState::Decoded(index)) => return Ok(Some(index)),
            Some(FileState::Missing(at)) => {
                self.count(at);
                return Ok(None);
            }
            None => {}
        }
        match self.decode(file, patch) {
            Ok(sample) => {
                let index = self.samples.len();
                self.samples.push(sample);
                self.files
                    .insert(file.to_owned(), FileState::Decoded(index));
                Ok(Some(index))
            }
            Err(MediaError::Io {
                operation: "open",
                source,
                ..
            }) if source.kind() == io::ErrorKind::NotFound => {
                let at = self.note(format!("sample {} is missing", file.display()));
                self.count(at);
                self.files.insert(file.to_owned(), FileState::Missing(at));
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    /// Starts a note about a sample that cannot be played, with no region counted yet; returns
    /// its index.
    fn note(&mut self, reason: String) -> usize {
        self.skipped.push(Skipped { reason, regions: 0 });
        self.skipped.len() - 1
    }

    /// Counts one more region left out under the note at `at`.
    fn count(&mut self, at: usize) {
        if let Some(skipped) = self.skipped.get_mut(at) {
            skipped.regions += 1;
        }
    }

    /// The notes of [`SampleBank::load_notes`].
    fn notes(&self) -> Vec<String> {
        self.skipped.iter().map(Skipped::note).collect()
    }

    /// Decodes the sample in `file`, counting its memory against [`MAX_BANK_BYTES`].
    ///
    /// # Errors
    ///
    /// [`MediaError::Io`] when the file cannot be opened or read; [`MediaError::Malformed`] when
    /// it has more than two channels or no length, or the patch's samples exceed
    /// [`MAX_BANK_BYTES`]; the reader's errors for a file it cannot decode.
    fn decode(&mut self, file: &Path, patch: &Path) -> Result<Sample, MediaError> {
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
        Ok(Sample {
            rate: stream.sample_rate(),
            channels,
            data,
        })
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
