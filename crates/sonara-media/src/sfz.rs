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
//! `sw_default`, `sw_last`, `trigger`, `group`, `off_by`, `off_mode`, `off_time`, `loop_mode`,
//! `loop_start`, `loop_end`, `offset`, `end`, `tune`, `transpose`, `volume`, `amplitude`,
//! `amp_veltrack`, `xfin_lovel`, `xfin_hivel`, `xfout_lovel`, `xfout_hivel`, `xf_velcurve`,
//! `ampeg_attack`, `ampeg_release`, the controller opcodes `loccN`, `hiccN`, `on_loccN`,
//! `on_hiccN`, `xfin_loccN`, `xfin_hiccN`, `xfout_loccN`, `xfout_hiccN`, `xf_cccurve`,
//! `volume_onccN` (also written `gain_ccN` and `gain_onccN`), `amplitude_onccN` (also
//! `amplitude_ccN`) and `set_ccN` for the controllers 0 to 127, and the `#define`/`#include`
//! directives. Every other opcode is recorded in
//! [`SampleBank::ignored_opcodes`] and has no effect. So is a value of `trigger` other than
//! `attack`, `release`, `first`, `legato` and `release_key`, of `loop_mode` other than
//! `no_loop`, `one_shot`, `loop_continuous` and `loop_sustain`, or of `off_mode` other than
//! `fast`, `normal` and `time`, as `loop_mode=loop_forever`: the region plays as if the opcode
//! were absent, so an unknown trigger plays on every key press. A header other than `<control>`,
//! `<global>`, `<master>`, `<group>` and `<region>` (`<curve>`, `<effect>`, `<midi>`, `<sample>`)
//! is recorded as written, and the opcodes after it are skipped up to the next of those headers.
//!
//! **Triggers.** `trigger` selects what starts a region ([`Trigger`]), as the SFZ format and the
//! reference player sfizz read it: `attack` (the default) a key press, `release` a key release
//! (under the sustain pedal, the pedal's lift), `first` a press while no other key is down,
//! `legato` a press while one is, and `release_key` a key release whatever the pedal does.
//! Sonatina's legato patches pair a `first` and a `legato` group, and its timpani end their rolls
//! with `release_key` regions.
//!
//! **Regions no key starts.** A `key`, `lokey` or `hikey` of -1 means "no key" (SFZ 2), and a
//! region with a controller trigger (`on_loccN`/`on_hiccN`, or their `hdcc` forms, other than
//! -1) is started by that controller instead of a key, as the SFZ format reads it. Such a region
//! is loaded with [`Region::on_key`] false and never plays on a key: Salamander's pedal noises
//! are written this way and used to sound on every note. The deepest scope that writes a key
//! opcode decides, so a region's own `key=60` under a `<global> lokey=-1` plays on key 60. Any
//! other key value that names no key is ignored, as before.
//!
//! **Controllers.** `on_loccN`/`on_hiccN` give the range of controller `N` whose events start
//! the region ([`Region::cc_triggers`]); a bound of -1 or none is the end of the range 0 to
//! 127, and a region whose bounds are both -1 has no trigger. `loccN`/`hiccN` are its gates
//! ([`Region::cc_gates`], default 0 and 127): the region starts only while controller `N` lies
//! in the range. `xfin_loccN`/`xfin_hiccN` and `xfout_loccN`/`xfout_hiccN` fade it in and out
//! with controller `N` ([`Region::cc_crossfades`]), shaped by `xf_cccurve` (`power`, the
//! default, or `gain`; any other value is reported and plays as `power`); a region may fade with
//! several controllers. `volume_onccN` gives the change in dB at value 127 and
//! `amplitude_onccN` the percentage of the amplitude at value 127 ([`Region::cc_volume`],
//! [`Region::cc_amplitude`]); where a scope writes more than one spelling of one of them, they
//! are one opcode and the last written wins. Controller values above 127 are read as 127 and
//! below 0 as 0; a volume change is kept within ±144 dB, an amplitude within ±100 %, and a value
//! that is not a number is ignored. `set_ccN` in any scope gives where controller `N` starts,
//! the last one in the file winning ([`SampleBank::controllers`]). The high-resolution forms
//! (`lohdccN`, `on_lohdccN`, `set_hdccN` and the like) and controllers above 127 are not
//! supported and are reported.
//!
//! **Directives.** `#include` and `#define` are read wherever a token starts, not only at the
//! start of a line, as the reference player sfizz reads them: Salamander's
//! `<group> #include "vel.txt" lovel=1 hivel=26 #include "region.txt"` reads both files in
//! place. An `#include` takes one path, quoted (spaces allowed) or a single word, and the line
//! goes on after it. A `#define` takes the rest of its line, and its value is all of it after
//! the name, trimmed, spaces included, so a define may name a folder with a space in it or stand
//! for several opcodes (`#define $LAYER lovel=64 hivel=127`). The format leaves this open: by
//! the reference player sfizz's own account Cakewalk's players read a value this way, while
//! ARIA and sfizz keep its first word and read the rest as opcodes. The `$defines` known so far
//! are substituted in an include path too (Sonatina's `#include "$DIR/$DYN.txt"`), and every
//! include, however deeply nested, resolves against the main patch file's directory, not the
//! including file's, as the SFZ format and sfizz read it: a fragment in `Data/` names its
//! siblings `Data/…` (the MTG saxophones). The names are substituted one after another, the
//! longest first and names of the same length in name order, so `$KEY2` is not read as `$KEY`
//! followed by `2`, and a value that names another define expands the same way on every load.
//!
//! **Comments** are read left to right, as sfizz reads them: a `//` comment runs to the end of
//! its line and a `/* … */` comment to its close, so a `/*` written in a line comment opens
//! nothing. A block comment keeps the text on either side apart, as a space or the line breaks
//! it spans.
//!
//! **Header scopes.** A region inherits the opcodes of the open `<control>`, `<global>`,
//! `<master>` and `<group>` headers. `<global>`, `<master>` and `<group>` each start their own
//! scope afresh and clear the scopes inside it; `<control>` clears only the control scope, so a
//! file included under a `<global>` may start with its own `<control>` and keep that global's
//! opcodes. A header need not stand apart from its opcodes: an opcode's value ends where a
//! header starts, as the reference player sfizz reads it, so `<region>key=36 sample=kick.wav`
//! (the SFZ format site's own example) is a region and `sample=a.wav<region>` ends one.
//!
//! **Keyswitches.** The switch keys are the union of the `sw_lokey`..`sw_hikey` ranges of every
//! region, each with the opcodes it inherits from its headers or writes itself, so disjoint
//! ranges switch only on their own keys. The default switch is the last valid `sw_default` in
//! the file, at any level, as the reference player sfizz reads it: a later header that does not
//! repeat it (VSCO 2 CE's key-switch patches start a new `<control>` and `<group>`) keeps it.
//!
//! **Velocity.** `amp_veltrack` sets how much a region's gain follows the velocity
//! ([`Region::velocity_gain`]); a value outside -100 to 100 is read as the nearest end, and one
//! that is not a number as the default 100. `amp_velcurve_N` and the `amp_veltrack_onccN`
//! modulation are not supported. The velocity crossfade opcodes fade a region in and out over a
//! velocity range ([`Crossfade`]), so dynamic layers written as crossfades blend rather
//! than both sounding (Sonatina's timpani fade their p layer out and their f layer in over
//! velocities 70 to 90); a velocity above 127 is read as 127, and an `xf_velcurve` other than
//! `power` and `gain` is reported and plays as `power`. The key crossfades (`xfin_lokey` and its
//! kin) and their curve are not supported. `amplitude` (0 to 100 %, default 100) scales the
//! region's level; a value outside that range is read as the nearest end.
//!
//! **Exclusive groups.** A region of `group` switches off the voices whose region names that
//! group in `off_by`, and `off_mode` says how they fade ([`OffMode`]): `fast` (the default) is
//! the sampler's quick choke, `normal` the region's release, and `time` a fade over `off_time`
//! seconds (6 ms when the patch writes none), as the SFZ format describes them. A region that
//! writes `off_time` without `off_mode` fades over it too, as ARIA, whose default mode is a timed
//! fade underneath, and the reference player sfizz read it; an explicit `fast` or `normal` keeps
//! its own fade. An `off_time` below 0 is read as 0, and one that is not a number as absent.
//! Sonatina's legato patches write `off_mode=time off_time=1`, so a new note overlaps the old one
//! for a second. `off_curve` and `off_shape` are not supported.
//!
//! **Loops a sample file embeds.** A WAV file may carry its loop in a sampler chunk (`smpl`); its
//! first loop is read. Its points are the defaults of `loop_start` and `loop_end` (the chunk's
//! last frame is played, as an SFZ `loop_end` is), and it makes `loop_continuous` the default
//! `loop_mode`, as the SFZ format and sfizz read it: a region that writes none of these loops
//! between the file's points while it sounds, and `loop_mode=no_loop` turns the loop off. A
//! region that a key release or a controller starts keeps `no_loop` as its default, since no
//! release would ever stop its loop. A loop that starts after its end or past the sample's last
//! frame is ignored, and every loop plays forward, whatever type the file gives it. Loops in
//! other formats (AIFF markers, FLAC metadata) are not read.
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
//!
//! **Inspection without decoding.** [`inspect`] reads a patch as [`load_sfz`] does, with the same
//! regions, but opens each sample file only for its header (format, channels and length) and
//! decodes no audio, so it is cheap enough to run over whole libraries. Its [`PatchReport`] gives
//! the regions, the keys and velocity layers a key press sounds, the keyswitches, what the
//! sampler ignores, the samples it cannot play and the memory the decoded samples would take.
//! Where `load_sfz` fails the patch for a sample file that cannot be read, or for the memory
//! cap, `inspect` reports it and says that the patch does not load.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use sonara_dsp::bank::{
    CcCrossfade, CcDepth, CcRange, Controllers, Crossfade, CrossfadeCurve, Keyswitches, LoopMode,
    OffMode, Region, Sample, SampleBank, Trigger,
};

use crate::container::WaveHeader;
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
    let content = read_patch(path, Reading::Decode)?;
    let notes = content.loader.notes();
    if content.regions.is_empty() {
        let message = if notes.is_empty() {
            "the patch has no playable region".to_owned()
        } else {
            format!("the patch has no playable region: {}", notes.join("; "))
        };
        return Err(MediaError::malformed(path, message));
    }
    let mut bank = SampleBank::default();
    bank.regions = content.regions;
    bank.load_notes = notes;
    bank.samples = content.loader.samples;
    bank.keyswitches = content.keyswitches;
    bank.controllers = content.controllers;
    bank.tail_seconds = tail_seconds(&bank);
    bank.ignored_opcodes = content.ignored;
    Ok(bank)
}

/// What [`inspect`] finds in a patch without decoding its samples.
///
/// The regions are the ones [`load_sfz`] builds, so the report describes what the sampler plays
/// when the patch loads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchReport {
    /// The `<region>` headers after the includes are read, whether they can play or not. A
    /// patch without any is not an instrument.
    pub regions: usize,
    /// The regions the sampler would load: those that name a sample file that is there and
    /// can be read, or `*silence`, and are not disabled. [`load_sfz`] loads exactly these when
    /// the patch loads.
    pub playable_regions: usize,
    /// The keys a press sounds on, ascending, as runs of neighbouring keys with the same
    /// velocity layers. A key sounds when a loaded region that a press starts covers it (an
    /// attack region, or a `first` or `legato` region, which a press starts with no other key
    /// down or with one down) whose sample has frames and whose `sw_last`, if it writes one, is
    /// a switch key or the default switch. Release regions (`release`, `release_key`), regions no
    /// key starts and `*silence` do not count. A region with controller gates or crossfades
    /// counts, since some controller values play it.
    pub sampled: Vec<LayerZone>,
    /// The switch keys and the default switch, as [`load_sfz`] gives them to the sampler. A
    /// switch key that is also in [`PatchReport::sampled`] both switches and sounds.
    pub keyswitches: Keyswitches,
    /// What the patch writes that the sampler does not play, as
    /// [`SampleBank::ignored_opcodes`] holds it: unsupported opcodes, unsupported `trigger` and
    /// `loop_mode` values, and skipped headers.
    pub ignored_opcodes: Vec<String>,
    /// The sample files the regions name that do not exist, in the order the patch first
    /// names them. Their regions are left out.
    pub missing_samples: Vec<PathBuf>,
    /// The sample files that exist but whose header cannot be read, or that are not mono or
    /// stereo, in the order the patch first names them. Their regions are left out here;
    /// [`load_sfz`] fails the whole patch on them.
    pub unreadable_samples: Vec<PathBuf>,
    /// One note per sample the regions left out name, with the number of regions: the notes of
    /// [`SampleBank::load_notes`], and a note for each unreadable sample.
    pub notes: Vec<String>,
    /// The memory the decoded samples would take, in bytes: frames × channels × 4 for each
    /// sample file read, counted once per file, as [`load_sfz`] counts it against
    /// [`MAX_BANK_BYTES`].
    pub decoded_bytes: usize,
}

impl PatchReport {
    /// Whether [`load_sfz`] is expected to load the patch: at least one region can play, no
    /// sample file is unreadable, and the decoded samples fit in [`MAX_BANK_BYTES`]. Loading can
    /// still fail on a sample whose header reads but whose audio does not decode.
    #[must_use]
    pub fn is_loadable(&self) -> bool {
        self.playable_regions > 0
            && self.unreadable_samples.is_empty()
            && self.decoded_bytes <= MAX_BANK_BYTES
    }

    /// The keys a press sounds on, ascending.
    pub fn sampled_keys(&self) -> impl Iterator<Item = u8> + '_ {
        self.sampled.iter().flat_map(|zone| zone.keys.clone())
    }
}

/// A run of neighbouring keys that sound with the same velocity layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerZone {
    /// The keys, lowest to highest.
    pub keys: RangeInclusive<u8>,
    /// The velocity layers, ascending: the velocity ranges within which the same regions play,
    /// across every switch and round-robin position. A region plays at the velocities its
    /// velocity range covers, less those its velocity crossfade silences. A new layer starts
    /// wherever a region starts or stops playing, so overlapping ranges give a layer for each
    /// part, and the zone where two crossfaded layers blend is a layer of its own; a velocity no
    /// region plays at lies in no layer. Velocity tracking (`amp_veltrack`) changes a region's
    /// gain, not the layers.
    pub layers: Vec<VelocityLayer>,
}

/// The velocities `lo..=hi`, within which the same regions of a key play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VelocityLayer {
    /// The lowest velocity of the layer.
    pub lo: u8,
    /// The highest velocity of the layer.
    pub hi: u8,
}

/// Reads the SFZ file `path` and the headers of the samples it names, without decoding them.
///
/// A region whose sample file is missing or cannot be read, or that names a generator the
/// sampler does not play, is left out and noted in the report, as is the memory the samples
/// would take; none of these fails the call (see [`PatchReport::is_loadable`]).
///
/// # Errors
///
/// - [`MediaError::Io`] when the patch or an included file cannot be opened or read;
/// - [`MediaError::Malformed`] when an `#include` is too deep.
pub fn inspect(path: &Path) -> Result<PatchReport, MediaError> {
    let content = read_patch(path, Reading::Header)?;
    let keyswitches = content.keyswitches;
    let sounding: Vec<&Region> = content
        .regions
        .iter()
        .filter(|region| {
            sounds_on_press(
                region,
                content.loader.frames_of(region.sample),
                &keyswitches,
            )
        })
        .collect();
    Ok(PatchReport {
        regions: content.written,
        playable_regions: content.regions.len(),
        sampled: layer_zones(&sounding),
        keyswitches,
        ignored_opcodes: content.ignored,
        missing_samples: content.loader.skipped_files(|cause| match cause {
            Unplayable::Missing(file) => Some(file),
            _ => None,
        }),
        unreadable_samples: content.loader.skipped_files(|cause| match cause {
            Unplayable::Unreadable { file, .. } => Some(file),
            _ => None,
        }),
        notes: content.loader.notes(),
        decoded_bytes: content.loader.bytes,
    })
}

/// Whether a press can sound `region`, whose sample has `frames` frames: a region a key press
/// starts (`attack`, `first` or `legato`, whether or not another key is down), with some
/// velocity, a sample that plays, and a `sw_last` the sampler can reach (a switch key, or the
/// default switch it starts with).
fn sounds_on_press(region: &Region, frames: usize, keyswitches: &Keyswitches) -> bool {
    (region.trigger.starts_on_press(false) || region.trigger.starts_on_press(true))
        && region.on_key
        && region.vel_lo <= region.vel_hi
        && frames > 0
        && region.sw_last.is_none_or(|switch| {
            keyswitches.contains(switch) || keyswitches.default == Some(switch)
        })
}

/// The keys `regions` sound on, as runs of neighbouring keys with the same velocity layers.
fn layer_zones(regions: &[&Region]) -> Vec<LayerZone> {
    let mut zones: Vec<LayerZone> = Vec::new();
    for key in 0..=127_u8 {
        let on_key: Vec<&Region> = regions
            .iter()
            .copied()
            .filter(|region| (region.key_lo..=region.key_hi).contains(&key))
            .collect();
        let layers = velocity_layers(&on_key);
        if layers.is_empty() {
            continue;
        }
        match zones.last_mut() {
            Some(zone) if zone.layers == layers && zone.keys.end().checked_add(1) == Some(key) => {
                zone.keys = *zone.keys.start()..=key;
            }
            _ => zones.push(LayerZone {
                keys: key..=key,
                layers,
            }),
        }
    }
    zones
}

/// The velocity layers of the regions of one key: the ranges within which the same regions
/// play. A layer ends where a region starts or stops playing, or where no region plays.
fn velocity_layers(regions: &[&Region]) -> Vec<VelocityLayer> {
    // Bit `v` of a mask: whether the region plays at velocity `v`.
    let masks: Vec<u128> = regions.iter().map(|region| plays_at(region)).collect();
    let sounding = masks.iter().fold(0, |all, mask| all | mask);
    // Bit `v`: whether some region plays at `v` but not at `v - 1`, or the other way round.
    let edges = masks.iter().fold(0, |all, mask| all | (mask ^ (mask << 1)));
    let mut layers = Vec::new();
    let mut start = None;
    for velocity in 0..=127_u8 {
        let bit = 1_u128 << velocity;
        let sounds = sounding & bit != 0;
        if let Some(lo) = start
            && (edges & bit != 0 || !sounds)
        {
            // The layer started at a lower velocity, so `velocity` is at least 1.
            layers.push(VelocityLayer {
                lo,
                hi: velocity.saturating_sub(1),
            });
            start = None;
        }
        if sounds && start.is_none() {
            start = Some(velocity);
        }
    }
    if let Some(lo) = start {
        layers.push(VelocityLayer { lo, hi: 127 });
    }
    layers
}

/// The velocities at which `region` plays, as bit `v` for velocity `v`: those its velocity range
/// covers, less those its velocity crossfade silences.
fn plays_at(region: &Region) -> u128 {
    (0..=127_u8)
        .filter(|velocity| {
            (region.vel_lo..=region.vel_hi).contains(velocity)
                && region.velocity_crossfade.gain(*velocity) > 0.0
        })
        .fold(0, |mask, velocity| mask | (1 << velocity))
}

/// A patch read with its regions built: what [`load_sfz`] and [`inspect`] share.
struct ReadPatch {
    /// The `<region>` headers after preprocessing.
    written: usize,
    /// The regions the loader kept, in patch order.
    regions: Vec<Region>,
    keyswitches: Keyswitches,
    /// Where the controllers start ([`SampleBank::controllers`]).
    controllers: Controllers,
    /// The ignored opcodes, values and headers, sorted, without duplicates.
    ignored: Vec<String>,
    loader: Loader,
}

/// Reads the patch `path` and builds its regions, reading their samples as `reading` says.
///
/// # Errors
///
/// The errors of [`read_preprocessed`] and of [`Loader::region`].
fn read_patch(path: &Path, reading: Reading) -> Result<ReadPatch, MediaError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let text = read_preprocessed(path, dir, &mut Defines::new(), 0)?;
    let parsed = parse(&text);
    let mut loader = Loader::new(reading);
    let mut regions = Vec::new();
    for opcodes in &parsed.regions {
        regions.extend(loader.region(dir, opcodes, path)?);
    }
    let mut ignored = parsed.ignored;
    ignored.append(&mut loader.ignored);
    ignored.sort();
    ignored.dedup();
    Ok(ReadPatch {
        written: parsed.regions.len(),
        regions,
        keyswitches: parsed.keyswitches,
        controllers: parsed.controllers,
        ignored,
        loader,
    })
}

fn tail_seconds(bank: &SampleBank) -> f64 {
    bank.regions
        .iter()
        .map(|region| {
            // A key release or a controller can start the sample after the last release.
            if region.trigger.is_release() || !region.cc_triggers.is_empty() {
                bank.samples.get(region.sample).map_or(0.0, |sample| {
                    let frames = sample.frames().saturating_sub(region.offset);
                    f64::from(u32::try_from(frames).unwrap_or(u32::MAX))
                        / f64::from(sample.rate.max(1))
                })
            } else {
                match region.off_mode {
                    // A press just before the last release may have started this fade.
                    OffMode::Time(seconds) if region.off_by != 0 => region.release.max(seconds),
                    _ => region.release,
                }
            }
        })
        .fold(0.0, f64::max)
}

// --- Preprocessing: comments, #include, #define ---

/// The text of the patch file `path` preprocessed ([`preprocess`]), each `#include` replaced by
/// the file it names, preprocessed in turn. Every include path, at any depth, is joined to
/// `root`, the main patch file's directory.
///
/// # Errors
///
/// [`MediaError::Io`] when the file or an included one cannot be read, and
/// [`MediaError::Malformed`] when the includes nest deeper than [`MAX_INCLUDE_DEPTH`].
fn read_preprocessed(
    path: &Path,
    root: &Path,
    defines: &mut Defines,
    depth: usize,
) -> Result<String, MediaError> {
    if depth > MAX_INCLUDE_DEPTH {
        return Err(MediaError::malformed(path, "#include nesting is too deep"));
    }
    let raw = fs::read_to_string(path).map_err(|e| MediaError::io("read", path, e))?;
    preprocess(&raw, defines, &mut |file, defines| {
        read_preprocessed(&root.join(file), root, defines, depth + 1)
    })
}

/// The `$defines` of a patch: each name, with its `$`, and its value.
type Defines = HashMap<String, String>;

/// `raw` without comments ([`strip_comments`]), with the `$defines` known so far substituted
/// and each `#include` replaced by what `include` gives for the path it names, with its
/// defines substituted and `\` read as `/`. The directives are read wherever a token starts, so
/// the line goes on after an `#include` (see the module documentation).
///
/// # Errors
///
/// The errors of `include`.
fn preprocess(
    raw: &str,
    defines: &mut Defines,
    include: &mut dyn FnMut(&str, &mut Defines) -> Result<String, MediaError>,
) -> Result<String, MediaError> {
    let mut out = String::new();
    for line in strip_comments(raw).lines() {
        let mut rest = line;
        loop {
            let (text, directive) = split_directive(rest);
            push_expanded(&mut out, text, defines);
            match directive {
                Some(Directive::Include { file, after }) => {
                    let file = expand_defines(file, defines).replace('\\', "/");
                    out.push_str(&include(&file, defines)?);
                    rest = after;
                }
                Some(Directive::Define(definition)) => {
                    if let Some((name, value)) = definition.trim().split_once(char::is_whitespace) {
                        let value = value.trim();
                        if !value.is_empty() {
                            defines.insert(name.to_owned(), value.to_owned());
                        }
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
fn push_expanded(out: &mut String, text: &str, defines: &Defines) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    out.push_str(&expand_defines(text, defines));
    out.push('\n');
}

/// `text` with every `$define` in `defines` replaced by its value; a name that is not defined
/// stays as written. The names are replaced one after another, the longest first, so `$KEY2` is
/// not eaten by `$KEY`, and names of the same length in name order, so that the result does not
/// depend on the map's order when a value names another define.
fn expand_defines(text: &str, defines: &Defines) -> String {
    let mut text = text.to_owned();
    let mut names: Vec<_> = defines.iter().collect();
    names.sort_by(|(a, _), (b, _)| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    for (name, value) in names {
        if text.contains(name.as_str()) {
            text = text.replace(name.as_str(), value);
        }
    }
    text
}

/// `text` without its comments, read left to right as the reference player sfizz reads them: a
/// `//` comment runs to the end of its line, and a `/* … */` comment to its close, or to the end
/// of the text when it is not closed. So a `/*` inside a line comment opens nothing, and a `//`
/// inside a block comment ends nothing. A block comment leaves the line breaks it spans, or a
/// space when it spans none, so that the tokens and lines on either side stay apart.
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(slash) = rest.find('/') {
        // `/` is ASCII, so every split below lies on a character boundary.
        let (before, from) = rest.split_at_checked(slash).unwrap_or((rest, ""));
        out.push_str(before);
        if let Some(comment) = from.strip_prefix("//") {
            rest = comment
                .find('\n')
                .and_then(|end| comment.get(end..))
                .unwrap_or("");
        } else if let Some(comment) = from.strip_prefix("/*") {
            let Some((spanned, after)) = comment.split_once("*/") else {
                return out;
            };
            let breaks = spanned.matches('\n').count();
            if breaks == 0 {
                out.push(' ');
            }
            out.extend(std::iter::repeat_n('\n', breaks));
            rest = after;
        } else {
            out.push('/');
            rest = from.get(1..).unwrap_or("");
        }
    }
    out.push_str(rest);
    out
}

// --- Parsing: headers and opcodes ---

/// A preprocessed patch, parsed.
#[derive(Debug, PartialEq)]
struct Parsed {
    regions: Vec<Opcodes>,
    keyswitches: Keyswitches,
    controllers: Controllers,
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
    "off_mode",
    "off_time",
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
    "amplitude",
    "amp_veltrack",
    "xfin_lovel",
    "xfin_hivel",
    "xfout_lovel",
    "xfout_hivel",
    "xf_velcurve",
    "xf_cccurve",
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
    let mut patch_wide = PatchWide::default();
    let finish =
        |region: &mut Option<Opcodes>, scopes: &[Opcodes; 4], regions: &mut Vec<Opcodes>| {
            if let Some(own) = region.take() {
                regions.push(merge(scopes, own));
            }
        };
    for token in text.split_whitespace().flat_map(split_headers) {
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
                _ => {
                    // Its opcodes are skipped; the header is reported once, not each of them.
                    in_other = true;
                    ignored.push(token.to_owned());
                }
            }
            continue;
        }
        if let Some((name, value)) = token.split_once('=') {
            if in_other {
                continue;
            }
            let name = name.to_ascii_lowercase();
            let controller = cc_opcode(&name);
            if !SUPPORTED.contains(&name.as_str())
                && !HARMLESS.contains(&name.as_str())
                && controller.is_none()
            {
                ignored.push(name_without_digits(&name));
            }
            patch_wide.read(&name, controller, value);
            let name = canonical_name(name, controller);
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
    keyswitches.default = patch_wide.sw_default;
    for opcodes in &regions {
        let range = |name: &str| opcodes.get(name).and_then(|v| note(v));
        if let (Some(lo), Some(hi)) = (range("sw_lokey"), range("sw_hikey")) {
            keyswitches.add_range(lo, hi);
        }
    }
    Parsed {
        regions,
        keyswitches,
        controllers: patch_wide.controllers,
        ignored,
    }
}

/// The tokens of the whitespace-separated `word`: the word split around each header written in
/// it, so that a header glued to an opcode is a token of its own, whichever side the opcode is
/// on (`<region>sample=a.wav`, `key=60<region>`, `<group><region>`), as the reference player
/// sfizz reads it: an opcode's value ends where a header starts. A `<` with no `>` after it in
/// the word stays part of the word.
fn split_headers(word: &str) -> impl Iterator<Item = &str> {
    let mut rest = word;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let end = match rest.find('<') {
            // A header: up to its `>`, or the whole word when none closes it.
            Some(0) => rest.find('>').map_or(rest.len(), |close| close + 1),
            // Text before a header that a `>` closes.
            Some(open) if rest.get(open..).is_some_and(|tail| tail.contains('>')) => open,
            _ => rest.len(),
        };
        // `<` and `>` are ASCII, so `end` lies on a character boundary.
        let (token, after) = rest.split_at_checked(end).unwrap_or((rest, ""));
        rest = after;
        Some(token)
    })
}

/// What a patch sets for all its regions, from any level: the last valid `sw_default`, and the
/// last valid `set_ccN` of each controller. No later header clears them.
#[derive(Default)]
struct PatchWide {
    sw_default: Option<u8>,
    controllers: Controllers,
}

impl PatchWide {
    /// Reads the opcode `name` (the controller opcode `controller`, if it is one) with `value`.
    fn read(&mut self, name: &str, controller: Option<(CcOpcode, u8)>, value: &str) {
        if name == "sw_default"
            && let Some(key) = note(value)
        {
            self.sw_default = Some(key);
        }
        if let Some((CcOpcode::Initial, number)) = controller
            && let Some(value) = cc_value(value)
        {
            self.controllers.set(number, value);
        }
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

/// `pan_oncc10` and `pan_oncc64` are one opcode family: report them as `pan_oncc*`.
fn name_without_digits(name: &str) -> String {
    let trimmed = name.trim_end_matches(|c: char| c.is_ascii_digit());
    if trimmed.len() == name.len() {
        name.to_owned()
    } else {
        format!("{trimmed}*")
    }
}

/// A numbered opcode about one MIDI controller that the sampler plays: the opcode, then the
/// controller number `N` (0 to 127) that ends its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CcOpcode {
    /// `loccN`: the lowest value of the gate.
    GateLo,
    /// `hiccN`: the highest value of the gate.
    GateHi,
    /// `on_loccN`: the lowest value that starts the region.
    TriggerLo,
    /// `on_hiccN`: the highest value that starts the region.
    TriggerHi,
    /// `xfin_loccN`: where the fade-in starts.
    FadeInLo,
    /// `xfin_hiccN`: where the fade-in is full.
    FadeInHi,
    /// `xfout_loccN`: where the fade-out starts.
    FadeOutLo,
    /// `xfout_hiccN`: where the fade-out is silent.
    FadeOutHi,
    /// `volume_onccN`, `gain_ccN` and `gain_onccN`: the volume change in dB at 127.
    Volume,
    /// `amplitude_onccN` and `amplitude_ccN`: the percentage of the amplitude at 127.
    Amplitude,
    /// `set_ccN`: where the controller starts.
    Initial,
}

/// The name prefixes of the controller opcodes, each followed by the controller number.
const CC_OPCODES: [(&str, CcOpcode); 14] = [
    ("locc", CcOpcode::GateLo),
    ("hicc", CcOpcode::GateHi),
    ("on_locc", CcOpcode::TriggerLo),
    ("on_hicc", CcOpcode::TriggerHi),
    ("xfin_locc", CcOpcode::FadeInLo),
    ("xfin_hicc", CcOpcode::FadeInHi),
    ("xfout_locc", CcOpcode::FadeOutLo),
    ("xfout_hicc", CcOpcode::FadeOutHi),
    ("volume_oncc", CcOpcode::Volume),
    ("gain_cc", CcOpcode::Volume),
    ("gain_oncc", CcOpcode::Volume),
    ("amplitude_oncc", CcOpcode::Amplitude),
    ("amplitude_cc", CcOpcode::Amplitude),
    ("set_cc", CcOpcode::Initial),
];

/// The controller opcode `name` is and the controller it names; `None` for any other opcode and
/// for a controller number above 127.
fn cc_opcode(name: &str) -> Option<(CcOpcode, u8)> {
    CC_OPCODES.iter().find_map(|(prefix, opcode)| {
        let number = name.strip_prefix(prefix)?;
        if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let number = number.parse::<u8>().ok().filter(|n| *n <= 127)?;
        Some((*opcode, number))
    })
}

/// One name for the spellings of one opcode, so that a deeper scope's spelling overrides an
/// outer one's: `gain_ccN` and `gain_onccN` are `volume_onccN`, `amplitude_ccN` is
/// `amplitude_onccN`.
fn canonical_name(name: String, controller: Option<(CcOpcode, u8)>) -> String {
    match controller {
        Some((CcOpcode::Volume, number)) => format!("volume_oncc{number}"),
        Some((CcOpcode::Amplitude, number)) => format!("amplitude_oncc{number}"),
        _ => name,
    }
}

/// A controller value: a number truncated and kept within 0 to 127; `None` for a value that is
/// not a number.
fn cc_value(value: &str) -> Option<u8> {
    let number = value.trim().parse::<f64>().ok().filter(|v| v.is_finite())?;
    Some(to_u8(Some(number.trunc()), 0).min(127))
}

/// A finite number kept within `-limit..=limit`; `None` for a value that is not one.
fn bounded(value: &str, limit: f64) -> Option<f64> {
    let number = value.trim().parse::<f64>().ok().filter(|v| v.is_finite())?;
    Some(number.clamp(-limit, limit))
}

/// The largest volume change a controller gives, in dB either way.
const MAX_CC_VOLUME_DB: f64 = 144.0;
/// The largest amplitude a controller gives, in percent either way.
const MAX_CC_AMPLITUDE: f64 = 100.0;

/// The controller opcodes of a region, gathered by controller so that they come out sorted.
#[derive(Default)]
struct RegionControllers {
    gates: BTreeMap<u8, CcRange>,
    triggers: BTreeMap<u8, CcRange>,
    fades: BTreeMap<u8, Crossfade>,
    volume: BTreeMap<u8, CcDepth>,
    amplitude: BTreeMap<u8, CcDepth>,
}

impl RegionControllers {
    /// Reads the controller opcodes of a region's merged `opcodes`, its crossfades shaped by
    /// `curve`.
    fn read(opcodes: &Opcodes, curve: CrossfadeCurve) -> Self {
        let mut read = Self::default();
        for (name, value) in opcodes {
            if let Some((opcode, controller)) = cc_opcode(name) {
                read.add(opcode, controller, value, curve);
            }
        }
        read
    }

    /// Adds one opcode about `controller` with its `value`.
    fn add(&mut self, opcode: CcOpcode, controller: u8, value: &str, curve: CrossfadeCurve) {
        let full = CcRange {
            controller,
            lo: 0,
            hi: 127,
        };
        match opcode {
            CcOpcode::GateLo | CcOpcode::GateHi => {
                if let Some(value) = cc_value(value) {
                    let gate = self.gates.entry(controller).or_insert(full);
                    set_bound(gate, opcode == CcOpcode::GateLo, value);
                }
            }
            CcOpcode::TriggerLo | CcOpcode::TriggerHi => {
                // -1 is the format's "no trigger": that bound stays at its end of the range.
                if !is_minus_one(value)
                    && let Some(value) = cc_value(value)
                {
                    let trigger = self.triggers.entry(controller).or_insert(full);
                    set_bound(trigger, opcode == CcOpcode::TriggerLo, value);
                }
            }
            CcOpcode::FadeInLo | CcOpcode::FadeInHi | CcOpcode::FadeOutLo | CcOpcode::FadeOutHi => {
                if let Some(value) = cc_value(value) {
                    let fade = self.fades.entry(controller).or_insert(Crossfade {
                        curve,
                        ..Crossfade::NONE
                    });
                    match opcode {
                        CcOpcode::FadeInLo => fade.in_lo = value,
                        CcOpcode::FadeInHi => fade.in_hi = value,
                        CcOpcode::FadeOutLo => fade.out_lo = value,
                        _ => fade.out_hi = value,
                    }
                }
            }
            CcOpcode::Volume => {
                if let Some(depth) = bounded(value, MAX_CC_VOLUME_DB) {
                    self.volume
                        .insert(controller, CcDepth { controller, depth });
                }
            }
            CcOpcode::Amplitude => {
                if let Some(depth) = bounded(value, MAX_CC_AMPLITUDE) {
                    self.amplitude
                        .insert(controller, CcDepth { controller, depth });
                }
            }
            // Read for the whole patch by `parse`.
            CcOpcode::Initial => {}
        }
    }

    /// Writes the opcodes into `region`.
    fn apply(self, region: &mut Region) {
        region.cc_gates = self.gates.into_values().collect();
        region.cc_triggers = self.triggers.into_values().collect();
        region.cc_crossfades = self
            .fades
            .into_iter()
            .map(|(controller, fade)| CcCrossfade { controller, fade })
            .collect();
        region.cc_volume = self.volume.into_values().collect();
        region.cc_amplitude = self.amplitude.into_values().collect();
    }
}

/// Sets the low (`low`) or high bound of `range` to `value`.
fn set_bound(range: &mut CcRange, low: bool, value: u8) {
    if low {
        range.lo = value;
    } else {
        range.hi = value;
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
    // Checked, so an octave of any size names no key instead of overflowing.
    let key = octave
        .parse::<i32>()
        .ok()?
        .checked_add(1)?
        .checked_mul(12)?
        .checked_add(semitone + accidental)?;
    u8::try_from(key).ok().filter(|n| *n <= 127)
}

// --- Regions and samples ---

/// What the loader reads of a sample file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reading {
    /// The whole file, decoded ([`load_sfz`]): a file that cannot be decoded fails the patch,
    /// and the samples may not take more than [`MAX_BANK_BYTES`].
    Decode,
    /// Only the header ([`inspect`]): a file that cannot be read leaves out its regions with a
    /// note, and the decoded size is counted without a cap.
    Header,
}

struct Loader {
    reading: Reading,
    /// The decoded samples, by index; empty when only headers are read.
    samples: Vec<Sample>,
    /// What each sample gives its regions, by index, in both readings. The regions are built
    /// from these, so both readings build the same ones.
    infos: Vec<SampleInfo>,
    /// Each sample file named so far: its index, or its note when it cannot be played.
    files: HashMap<PathBuf, FileState>,
    /// The index of the silent sample, once a region names `*silence`.
    silence: Option<usize>,
    /// Each generator the sampler does not play, named so far: the index of its note.
    generators: HashMap<String, usize>,
    /// What the regions left out name, in the order the patch first names it.
    skipped: Vec<Skipped>,
    /// The values of supported opcodes that the sampler does not play, as `name=value`.
    ignored: Vec<String>,
    /// The decoded size of the sample files read so far, in bytes.
    bytes: usize,
}

/// What the loader knows of a sample in both readings.
#[derive(Debug, Clone, Copy, Default)]
struct SampleInfo {
    /// Its frames: the decoded frames, or the header's count.
    frames: usize,
    /// The loop the file embeds, when it lies within the sample.
    file_loop: Option<FileLoop>,
}

/// A loop a sample file embeds (a WAV `smpl` chunk), as a region plays it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileLoop {
    /// The first looped frame.
    start: usize,
    /// The frame after the last looped one.
    end: usize,
}

impl FileLoop {
    /// The loop of a file of `frames` frames whose header is `wave`: the first loop of its
    /// `smpl` chunk, `None` when it has none or when the loop starts after its end or past the
    /// last frame. Its end may lie past the last frame; the region's end bounds it.
    fn of(wave: Option<&WaveHeader>, frames: usize) -> Option<Self> {
        let written = wave?.sample_loop?;
        let start = usize::try_from(written.start).ok()?;
        let last = usize::try_from(written.end).ok()?;
        (start <= last && start < frames).then(|| Self {
            start,
            end: last.saturating_add(1),
        })
    }
}

/// A sample file the loader has looked for.
#[derive(Debug, Clone, Copy)]
enum FileState {
    /// Read: its sample's index.
    Read(usize),
    /// Not playable: the index of its note in [`Loader::skipped`].
    Skipped(usize),
}

/// Why regions name a sample the sampler cannot play.
enum Unplayable {
    /// The sample file does not exist.
    Missing(PathBuf),
    /// The sample file exists, but its header cannot be read, or it is not mono or stereo
    /// (only when headers are read; decoding fails the patch instead).
    Unreadable {
        /// The file.
        file: PathBuf,
        /// The reader's error.
        error: String,
    },
    /// A generator other than `*silence`.
    Generator(String),
}

/// A sample that regions name but the sampler cannot play, and how many regions name it.
struct Skipped {
    /// Why it cannot be played.
    cause: Unplayable,
    /// The regions left out because of it.
    regions: usize,
}

impl Skipped {
    /// The note for [`SampleBank::load_notes`].
    fn note(&self) -> String {
        let reason = match &self.cause {
            Unplayable::Missing(file) => format!("sample {} is missing", file.display()),
            Unplayable::Unreadable { file, error } => {
                format!("sample {} cannot be read: {error}", file.display())
            }
            Unplayable::Generator(name) => {
                format!("the sampler does not play the generator {name}")
            }
        };
        if self.regions == 1 {
            format!("{reason} (1 region left out)")
        } else {
            format!("{reason} ({} regions left out)", self.regions)
        }
    }
}

impl Loader {
    /// A loader that reads sample files as `reading` says.
    fn new(reading: Reading) -> Self {
        Self {
            reading,
            samples: Vec::new(),
            infos: Vec::new(),
            files: HashMap::new(),
            silence: None,
            generators: HashMap::new(),
            skipped: Vec::new(),
            ignored: Vec::new(),
            bytes: 0,
        }
    }

    /// The region `opcodes` describe, with its sample read; `None` for a disabled region
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
        let SampleInfo { frames, file_loop } = self.info_of(sample);
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
        let off_time = num("off_time")
            .filter(|v| v.is_finite())
            .map(|v| v.max(0.0));
        region.off_mode = match self.value(opcodes, &["off_mode"], parse_off_mode) {
            Some(OffMode::Time(default)) => OffMode::Time(off_time.unwrap_or(default)),
            Some(mode) => mode,
            None => off_time.map_or(OffMode::Fast, OffMode::Time),
        };
        region.volume_db = num("volume").unwrap_or(0.0);
        region.amplitude = num("amplitude")
            .filter(|v| v.is_finite())
            .map_or(100.0, |v| v.clamp(0.0, 100.0));
        region.amp_veltrack = num("amp_veltrack")
            .filter(|v| v.is_finite())
            .map_or(100.0, |v| v.clamp(-100.0, 100.0));
        let velocity = |key: &str| int(key).map(|v| to_u8(Some(v), 0).min(127));
        let none = Crossfade::NONE;
        region.velocity_crossfade = Crossfade {
            in_lo: velocity("xfin_lovel").unwrap_or(none.in_lo),
            in_hi: velocity("xfin_hivel").unwrap_or(none.in_hi),
            out_lo: velocity("xfout_lovel").unwrap_or(none.out_lo),
            out_hi: velocity("xfout_hivel").unwrap_or(none.out_hi),
            curve: self
                .value(opcodes, &["xf_velcurve"], parse_crossfade_curve)
                .unwrap_or_default(),
        };
        let cc_curve = self
            .value(opcodes, &["xf_cccurve"], parse_crossfade_curve)
            .unwrap_or_default();
        RegionControllers::read(opcodes, cc_curve).apply(&mut region);
        region.tune_cents = num("tune").unwrap_or(0.0) + num("transpose").unwrap_or(0.0) * 100.0;
        region.attack = num("ampeg_attack").unwrap_or(0.0).max(0.0);
        region.release = num("ampeg_release").unwrap_or(0.0).max(0.0);
        region.trigger = self
            .value(opcodes, &["trigger"], parse_trigger)
            .unwrap_or_default();
        region.offset = to_usize(int("offset"), 0).min(frames);
        // `end` is the last frame played; a huge one saturates instead of overflowing.
        region.end = int("end")
            .map_or(frames, |e| to_usize(Some(e), 0).saturating_add(1))
            .min(frames);
        self.read_loop(&mut region, opcodes, file_loop);
        Ok(Some(region))
    }

    /// Sets the loop of `region`, whose other fields are read, from its `opcodes`. The loop
    /// `file_loop` that its sample file embeds gives the defaults of `loop_start` and
    /// `loop_end`, and makes `loop_continuous` the default `loop_mode`, as the SFZ format and
    /// sfizz read it; but only for a region a key release ends, since nothing releases a voice
    /// that a key release or a controller started, which would then loop forever.
    fn read_loop(&mut self, region: &mut Region, opcodes: &Opcodes, file_loop: Option<FileLoop>) {
        let int = |key: &str| {
            let value = opcodes.get(key)?.trim().parse::<f64>().ok()?;
            Some(value.max(0.0).trunc())
        };
        let released_by_key = region.on_key && !region.trigger.is_release();
        let file_mode = match file_loop {
            Some(_) if released_by_key => LoopMode::Continuous,
            _ => LoopMode::NoLoop,
        };
        region.loop_mode = self
            .value(opcodes, &["loop_mode", "loopmode"], parse_loop_mode)
            .unwrap_or(file_mode);
        let loop_start = int("loop_start").or_else(|| int("loopstart"));
        let loop_end = int("loop_end").or_else(|| int("loopend"));
        region.loop_start = to_usize(loop_start, file_loop.map_or(0, |l| l.start));
        region.loop_end = loop_end
            .map_or(file_loop.map_or(region.end, |l| l.end), |e| {
                to_usize(Some(e), 0).saturating_add(1)
            })
            .min(region.end);
    }

    /// The value of the first of the opcodes `names` that the region writes, as `parse` reads
    /// it. A value that `parse` does not know is recorded as `name=value` in the ignored
    /// opcodes and gives `None`, so the region plays as if the opcode were absent.
    fn value<T>(
        &mut self,
        opcodes: &Opcodes,
        names: &[&str],
        parse: fn(&str) -> Option<T>,
    ) -> Option<T> {
        let (name, value) = names
            .iter()
            .find_map(|name| Some((*name, opcodes.get(*name)?.trim())))?;
        let parsed = parse(value);
        if parsed.is_none() {
            self.ignored.push(format!("{name}={value}"));
        }
        parsed
    }

    /// What the loader knows of the sample at `index`; no frames for an index the loader has
    /// not given out.
    fn info_of(&self, index: usize) -> SampleInfo {
        self.infos.get(index).copied().unwrap_or_default()
    }

    /// The frames of the sample at `index`; 0 for an index the loader has not given out.
    fn frames_of(&self, index: usize) -> usize {
        self.info_of(index).frames
    }

    /// Adds a sample `info` describes and returns its index. `sample` is its decoded data, kept
    /// only when decoding.
    fn push(&mut self, sample: Option<Sample>, info: SampleInfo) -> usize {
        self.samples.extend(sample);
        self.infos.push(info);
        self.infos.len() - 1
    }

    /// The index of the sample of the generator `name`: the shared silent sample for
    /// `*silence`. Any other generator is not played: `None`, and the region is noted.
    fn generator(&mut self, name: &str) -> Option<usize> {
        if name == SILENCE {
            if self.silence.is_none() {
                let sample = (self.reading == Reading::Decode).then(|| Sample {
                    rate: SILENCE_RATE,
                    channels: 1,
                    data: Vec::new(),
                });
                self.silence = Some(self.push(sample, SampleInfo::default()));
            }
            return self.silence;
        }
        let at = if let Some(at) = self.generators.get(name) {
            *at
        } else {
            let at = self.note(Unplayable::Generator(name.to_owned()));
            self.generators.insert(name.to_owned(), at);
            at
        };
        self.count(at);
        None
    }

    /// The index of the sample in `file`, read on first use. A file that does not exist, or,
    /// when only headers are read, one that cannot be read, gives `None`, and the region is
    /// noted.
    ///
    /// # Errors
    ///
    /// The errors of [`Loader::read`] when decoding, except that the file does not exist.
    fn file(&mut self, file: &Path, patch: &Path) -> Result<Option<usize>, MediaError> {
        match self.files.get(file).copied() {
            Some(FileState::Read(index)) => return Ok(Some(index)),
            Some(FileState::Skipped(at)) => {
                self.count(at);
                return Ok(None);
            }
            None => {}
        }
        let cause = match self.read(file, patch) {
            Ok(index) => {
                self.files.insert(file.to_owned(), FileState::Read(index));
                return Ok(Some(index));
            }
            Err(MediaError::Io {
                operation: "open",
                source,
                ..
            }) if source.kind() == io::ErrorKind::NotFound => Unplayable::Missing(file.to_owned()),
            Err(error) if self.reading == Reading::Header => Unplayable::Unreadable {
                file: file.to_owned(),
                error: error.to_string(),
            },
            Err(error) => return Err(error),
        };
        let at = self.note(cause);
        self.count(at);
        self.files.insert(file.to_owned(), FileState::Skipped(at));
        Ok(None)
    }

    /// Starts a note about a sample that cannot be played, with no region counted yet; returns
    /// its index.
    fn note(&mut self, cause: Unplayable) -> usize {
        self.skipped.push(Skipped { cause, regions: 0 });
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

    /// The files of the samples left out that `pick` selects, in the order the patch first
    /// names them.
    fn skipped_files(&self, pick: impl Fn(&Unplayable) -> Option<&PathBuf>) -> Vec<PathBuf> {
        self.skipped
            .iter()
            .filter_map(|skipped| pick(&skipped.cause).cloned())
            .collect()
    }

    /// Reads the sample in `file` and returns its index: its header, and, when decoding, its
    /// audio. Its decoded size is counted, against [`MAX_BANK_BYTES`] when decoding.
    ///
    /// # Errors
    ///
    /// [`MediaError::Io`] when the file cannot be opened or read; [`MediaError::Malformed`] when
    /// it has more than two channels or no length, or, when decoding, the patch's samples
    /// exceed [`MAX_BANK_BYTES`]; the reader's errors for a file it cannot decode.
    fn read(&mut self, file: &Path, patch: &Path) -> Result<usize, MediaError> {
        let opened = AudioStream::open(file)?;
        let wave = opened.wave;
        let mut stream = opened.stream;
        // Makes the decoder without decoding, so a codec the loader cannot decode is found from
        // the header too.
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
        // From the header, so that both readings give the regions the same loop.
        let file_loop = FileLoop::of(wave.as_ref(), frames);
        self.bytes = self
            .bytes
            .saturating_add(frames.saturating_mul(channels).saturating_mul(4));
        if self.reading == Reading::Header {
            return Ok(self.push(None, SampleInfo { frames, file_loop }));
        }
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
        let frames = sample.frames();
        Ok(self.push(Some(sample), SampleInfo { frames, file_loop }))
    }
}

/// The trigger a `trigger` value names; `None` for a value the sampler does not play.
fn parse_trigger(value: &str) -> Option<Trigger> {
    match value {
        "attack" => Some(Trigger::Attack),
        "release" => Some(Trigger::Release),
        "first" => Some(Trigger::First),
        "legato" => Some(Trigger::Legato),
        "release_key" => Some(Trigger::ReleaseKey),
        _ => None,
    }
}

/// The mode an `off_mode` value names, with the default `off_time` for `time`; `None` for a value
/// the sampler does not play.
fn parse_off_mode(value: &str) -> Option<OffMode> {
    match value {
        "fast" => Some(OffMode::Fast),
        "normal" => Some(OffMode::Normal),
        "time" => Some(OffMode::Time(OffMode::DEFAULT_TIME)),
        _ => None,
    }
}

/// The curve an `xf_velcurve` or `xf_cccurve` value names; `None` for a value the sampler does
/// not play.
fn parse_crossfade_curve(value: &str) -> Option<CrossfadeCurve> {
    match value {
        "power" => Some(CrossfadeCurve::Power),
        "gain" => Some(CrossfadeCurve::Gain),
        _ => None,
    }
}

/// The loop mode a `loop_mode` value names; `None` for a value the sampler does not play.
fn parse_loop_mode(value: &str) -> Option<LoopMode> {
    match value {
        "no_loop" => Some(LoopMode::NoLoop),
        "one_shot" => Some(LoopMode::OneShot),
        "loop_continuous" => Some(LoopMode::Continuous),
        "loop_sustain" => Some(LoopMode::Sustain),
        _ => None,
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use proptest::collection::vec;
    use proptest::prelude::*;
    use proptest::sample::select;
    use proptest::test_runner::RngSeed;
    use sonara_dsp::bank::LoopMode;

    use super::{Defines, Loader, Parsed, Reading, parse, preprocess, read_patch};

    /// A 16-bit mono PCM WAV of `frames` silent frames at 44.1 kHz with a `smpl` chunk after the
    /// audio whose one loop runs from `start` to `end`, both played.
    fn looped_wav(path: &Path, frames: u32, (start, end): (u32, u32)) {
        let mut chunk = Vec::new();
        for field in [0_u32, 0, 22_675, 60, 0, 0, 0, 1, 0, 1, 0, start, end, 0, 0] {
            chunk.extend_from_slice(&field.to_le_bytes());
        }
        let data_len = frames * 2;
        let mut bytes = b"RIFF".to_vec();
        let riff_len = 4 + 24 + 8 + data_len + 8 + u32::try_from(chunk.len()).unwrap();
        bytes.extend_from_slice(&riff_len.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        for field in [16_u32, 0x0001_0001, 44_100, 88_200, 0x0010_0002] {
            bytes.extend_from_slice(&field.to_le_bytes());
        }
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.resize(bytes.len() + usize::try_from(data_len).unwrap(), 0);
        bytes.extend_from_slice(b"smpl");
        bytes.extend_from_slice(&u32::try_from(chunk.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&chunk);
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn both_readings_give_a_region_the_file_loop() {
        // `inspect` reads only the header; the loop is taken from it in both readings, so the
        // regions `inspect` describes are the ones `load_sfz` plays.
        let dir = tempfile::tempdir().unwrap();
        looped_wav(&dir.path().join("a.wav"), 1_000, (100, 899));
        let patch = dir.path().join("patch.sfz");
        fs::write(
            &patch,
            "<region> key=60 sample=a.wav\n<region> key=61 sample=a.wav loop_start=300\n",
        )
        .unwrap();
        let decoded = read_patch(&patch, Reading::Decode).unwrap().regions;
        let header = read_patch(&patch, Reading::Header).unwrap().regions;
        assert_eq!(decoded, header);
        let loops: Vec<_> = decoded
            .iter()
            .map(|r| (r.loop_mode, r.loop_start, r.loop_end))
            .collect();
        assert_eq!(
            loops,
            [
                (LoopMode::Continuous, 100, 900),
                (LoopMode::Continuous, 300, 900)
            ]
        );
    }

    /// Preprocesses `text` with includes that give nothing, so that no file is read, and
    /// parses it. Returns the preprocessed text too.
    fn parse_text(text: &str) -> (String, Parsed) {
        let mut defines = Defines::new();
        let preprocessed = preprocess(text, &mut defines, &mut |_, _| Ok(String::new()))
            .expect("an include that gives nothing never fails");
        let parsed = parse(&preprocessed);
        (preprocessed, parsed)
    }

    /// Pieces of SFZ text that [`soup`] glues together: headers, directives, defines, comment
    /// marks and the characters the parser splits at.
    const FRAGMENTS: &[&str] = &[
        "<region>",
        "<group>",
        "<global>",
        "<master>",
        "<control>",
        "<curve>",
        "<",
        ">",
        "=",
        " ",
        "\n",
        "\t",
        "\r",
        "#include ",
        "#define ",
        "#",
        "$",
        "$A",
        "$B",
        "$AB",
        "\"",
        "//",
        "/*",
        "*/",
        "/",
        "\\",
        "é",
        "日",
    ];

    /// Opcode names that [`soup`] writes, each read differently.
    const NAMES: &[&str] = &[
        "sample",
        "key",
        "lokey",
        "hikey",
        "pitch_keycenter",
        "lovel",
        "hivel",
        "lorand",
        "seq_length",
        "end",
        "offset",
        "loop_mode",
        "loop_start",
        "loop_end",
        "trigger",
        "off_mode",
        "off_time",
        "amp_veltrack",
        "amplitude",
        "xfin_lovel",
        "xf_velcurve",
        "sw_lokey",
        "sw_hikey",
        "sw_default",
        "sw_last",
        "group",
        "off_by",
        "on_locc64",
        "on_hicc64",
        "locc1",
        "hicc1",
        "xfin_locc1",
        "xfout_hicc1",
        "volume_oncc7",
        "amplitude_cc7",
        "gain_cc7",
        "set_cc7",
        "set_cc300",
        "lohdcc1",
        "$A",
        "Cutoff",
    ];

    /// Values that [`soup`] writes: the edges of the ranges, numbers that overflow, words.
    const VALUES: &[&str] = &[
        "-1",
        "0",
        "127",
        "128",
        "2147483647",
        "-2147483648",
        "178956970",
        "1e300",
        "-1e300",
        "nan",
        "inf",
        "-inf",
        "64",
        "4.5",
        "*silence",
        "*sine",
        "loop_continuous",
        "release_key",
        "time",
        "gain",
        "$A",
        "$B",
        "",
    ];

    /// Arbitrary text, mostly made of [`FRAGMENTS`], headers, defines of `$A`, `$B` and `$AB`,
    /// and opcodes with a name of [`NAMES`] and a value of [`VALUES`], sometimes after a note
    /// letter, glued together with any characters between them.
    fn soup() -> impl Strategy<Value = String> {
        let headers = select(&["<region>", "<group>", "<global>", "<control>"][..]);
        let note = prop_oneof![3 => Just(""), 1 => select(&["c", "C#", "b", "db"][..])];
        let opcode = (select(NAMES), note, select(VALUES))
            .prop_map(|(name, note, value)| format!(" {name}={note}{value}"));
        let define = (select(&["$A", "$B", "$AB"][..]), select(VALUES))
            .prop_map(|(name, value)| format!("\n#define {name} {value}\n"));
        let piece = prop_oneof![
            2 => select(FRAGMENTS).prop_map(str::to_owned),
            2 => headers.prop_map(str::to_owned),
            1 => define,
            4 => opcode,
            1 => any::<char>().prop_map(String::from),
        ];
        vec(piece, 0..60).prop_map(|pieces| pieces.concat())
    }

    /// A patch line without directives, defines or comment marks: headers, opcodes and
    /// words, glued or spaced.
    fn plain_line() -> impl Strategy<Value = String> {
        let headers = select(&["<region>", "<group>", "<global>", "<control>", "<curve>"][..])
            .prop_map(str::to_owned);
        let names = select(
            &[
                "sample",
                "key",
                "lokey",
                "hivel",
                "sw_lokey",
                "sw_hikey",
                "sw_default",
                "on_locc64",
                "locc1",
                "set_cc7",
                "cutoff",
                "Lovel",
                "trigger",
            ][..],
        );
        let opcode =
            (names, "[A-Za-z0-9_.*-]{0,8}").prop_map(|(name, value)| format!("{name}={value}"));
        let item = prop_oneof![headers, opcode, "[^#$/\n]{0,6}"];
        let gap = select(&[" ", "", "\t"][..]);
        vec((item, gap), 0..8).prop_map(|items| {
            items
                .into_iter()
                .flat_map(|(item, gap)| [item, gap.to_owned()])
                .collect()
        })
    }

    /// The fixed seed of the property tests (TST-04).
    fn config() -> ProptestConfig {
        ProptestConfig {
            cases: 2_048,
            rng_seed: RngSeed::Fixed(0x5f2_2026),
            ..ProptestConfig::default()
        }
    }

    proptest! {
        #![proptest_config(config())]

        /// Any text preprocesses and parses without a panic (TST-09), the same way every time
        /// although each run builds its defines anew, and every region it gives comes from a
        /// `<region>` header. Each region then builds without a panic, read over the silent
        /// sample so that no file is opened.
        #[test]
        fn any_text_parses_the_same_way_without_panicking(
            text in prop_oneof![soup(), any::<String>()],
        ) {
            let (preprocessed, parsed) = parse_text(&text);
            prop_assert_eq!(&parse_text(&text).1, &parsed);
            prop_assert!(parsed.regions.len() <= preprocessed.matches("<region>").count());
            for opcodes in &parsed.regions {
                for name in opcodes.keys() {
                    prop_assert!(!name.contains('=') && !name.contains(char::is_whitespace));
                    prop_assert_eq!(name, &name.to_ascii_lowercase());
                }
            }
            let mut loader = Loader::new(Reading::Header);
            for opcodes in &parsed.regions {
                let mut silent = opcodes.clone();
                silent.insert("sample".to_owned(), "*silence".to_owned());
                let region = loader
                    .region(Path::new("/nonexistent"), &silent, Path::new("/nonexistent/p.sfz"))
                    .expect("the silent sample opens no file");
                if let Some(region) = region {
                    prop_assert!(region.offset <= region.end && region.loop_end <= region.end);
                    prop_assert!(region.key_lo <= 127 && region.key_hi <= 127);
                }
            }
        }

        /// A patch without directives parses the same with comments after its lines as
        /// without them, whatever the comments say: a `/*` inside a line comment opens nothing.
        #[test]
        fn a_trailing_comment_changes_nothing(
            lines in vec(
                (
                    plain_line(),
                    prop::option::of((any::<bool>(), any::<bool>(), "[^\n]{0,20}", "[^*]{0,20}")),
                ),
                0..12,
            ),
        ) {
            let mut plain = String::new();
            let mut commented = String::new();
            for (line, comment) in &lines {
                plain.push_str(line);
                commented.push_str(line);
                if let Some((spaced, block, text, block_text)) = comment {
                    if *spaced {
                        commented.push(' ');
                    }
                    if *block {
                        commented.extend(["/*", block_text.as_str(), "*/"]);
                    } else {
                        commented.extend(["//", text.as_str()]);
                    }
                }
                plain.push('\n');
                commented.push('\n');
            }
            prop_assert_eq!(parse_text(&commented).1, parse_text(&plain).1, "{:?}", commented);
        }
    }
}
