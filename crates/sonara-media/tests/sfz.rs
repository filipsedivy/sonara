//! The SFZ loader against a generated library (a tempdir: no licensed samples are needed).
//! The fixture is a two-layer, two-round-robin instrument with a release sample and keyswitches,
//! written with a macro-free `#include`, `#define` and `default_path` the way real libraries do.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use sonara_core::model::device::Params;
use sonara_core::plugins::{SamplerChoices, sampler_descriptor};
use sonara_dsp::bank::{
    CrossfadeCurve, Keyswitches, LoopMode, SampleBank, Trigger, VelocityCrossfade,
};
use sonara_dsp::processor::{BlockContext, DeviceState, EventKind, InstrumentEvent, ProcessSpec};
use sonara_dsp::processors::{create, param_slots};
use sonara_media::MediaError;
use sonara_media::sfz::{LayerZone, MAX_BANK_BYTES, PatchReport, VelocityLayer, inspect, load_sfz};

/// A 16-bit mono PCM WAV of `frames` frames of the constant `value`.
fn wav(path: &Path, rate: u32, frames: u32, value: i16) {
    let data_len = frames * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for _ in 0..frames {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(path, bytes).expect("write the wav");
}

fn library() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a tempdir");
    let samples = dir.path().join("Samples");
    fs::create_dir(&samples).expect("mkdir");
    for (name, value) in [
        ("soft1", 3_000),
        ("soft2", 4_000),
        ("loud1", 20_000),
        ("loud2", 24_000),
        ("tail", 1_000),
    ] {
        wav(&samples.join(format!("{name}.wav")), 44_100, 4_410, value);
    }
    fs::write(
        dir.path().join("common.sfz"),
        "#define $SOFT 63\n<control> default_path=Samples\\\n",
    )
    .expect("write");
    fs::write(
        dir.path().join("violin.sfz"),
        "#include \"common.sfz\"\n\
         // keyswitches and defaults\n\
         <global> sw_lokey=c1 sw_hikey=d1 sw_default=c1 ampeg_release=0.3 cutoff=800 locc1=20\n\
         <group> lokey=c3 hikey=c5 pitch_keycenter=c4 sw_last=c1 hivel=$SOFT seq_length=2\n\
         <region> sample=soft1.wav seq_position=1\n\
         <region> sample=soft2.wav seq_position=2\n\
         <group> lokey=c3 hikey=c5 pitch_keycenter=c4 sw_last=c1 lovel=64 loop_mode=loop_continuous loop_start=100 loop_end=999\n\
         <region> sample=loud1.wav\n\
         <region> sample=loud2.wav tune=-12 volume=-3 ampeg_attack=0.02\n\
         <group> lokey=c3 hikey=c5 trigger=release\n\
         <region> sample=tail.wav pitch_keycenter=60\n",
    )
    .expect("write");
    dir
}

#[test]
fn reads_regions_with_their_inherited_opcodes() {
    let dir = library();
    let bank = load_sfz(&dir.path().join("violin.sfz")).expect("loads");
    assert_eq!(bank.regions.len(), 5);
    assert_eq!(bank.samples.len(), 5, "each file is decoded once");
    let soft = &bank.regions[0];
    assert_eq!(
        (soft.key_lo, soft.key_hi, soft.pitch_keycenter),
        (48, 72, 60)
    );
    assert_eq!((soft.vel_lo, soft.vel_hi), (0, 63), "$SOFT is substituted");
    assert_eq!((soft.seq_length, soft.seq_position), (2, 1));
    assert_eq!(bank.regions[1].seq_position, 2);
    assert_eq!(soft.sw_last, Some(24), "c1 is key 24");
    assert_eq!(soft.release, 0.3, "the global opcode is inherited");
    let loud = &bank.regions[3];
    assert_eq!(loud.vel_lo, 64);
    assert_eq!(loud.loop_mode, LoopMode::Continuous);
    assert_eq!(
        (loud.loop_start, loud.loop_end),
        (100, 1_000),
        "loop_end is inclusive"
    );
    assert_eq!(
        (loud.tune_cents, loud.volume_db, loud.attack),
        (-12.0, -3.0, 0.02)
    );
    assert_eq!(bank.regions[4].trigger, Trigger::Release);
    let switches = bank.keyswitches;
    assert_eq!(switches.keys().collect::<Vec<_>>(), [24, 25, 26]);
    assert_eq!(switches.default, Some(24));
}

/// Loads `text` as a patch beside the fixture's `Samples` folder.
fn load_patch(dir: &tempfile::TempDir, text: &str) -> SampleBank {
    load_sfz_text(dir, text).expect("loads")
}

/// Writes `text` as a patch beside the fixture's `Samples` folder and loads it.
fn load_sfz_text(dir: &tempfile::TempDir, text: &str) -> Result<SampleBank, MediaError> {
    let path = dir.path().join("patch.sfz");
    fs::write(&path, text).expect("write");
    load_sfz(&path)
}

#[test]
fn sw_default_in_the_first_group_survives_later_groups() {
    // VSCO 2 CE's key-switch layout (TST-06): the default sits only in the first group, and a
    // later `<control>` and `<group>` repeat the range without it.
    let dir = library();
    let bank = load_patch(
        &dir,
        "<control> default_path=Samples/\n\
         <group> sw_lokey=c2 sw_hikey=d#2 sw_default=c2 sw_last=c2 lokey=c3 hikey=c5\n\
         <region> sample=soft1.wav\n\
         <control> default_path=Samples/\n\
         <group> sw_lokey=c2 sw_hikey=d#2 sw_last=c#2 lokey=c3 hikey=c5\n\
         <region> sample=loud1.wav\n",
    );
    assert_eq!(bank.keyswitches.default, Some(36), "c2 is key 36");
    assert_eq!(
        bank.keyswitches.keys().collect::<Vec<_>>(),
        [36, 37, 38, 39]
    );
    assert_eq!(
        bank.regions.iter().map(|r| r.sw_last).collect::<Vec<_>>(),
        [Some(36), Some(37)]
    );
}

#[test]
fn control_header_keeps_the_enclosing_global_switch() {
    // Sonatina's key-switch layout (TST-06): each articulation is a `<global>` with the switch
    // range and its `sw_last`, followed by an `#include` of a file that may start with its own
    // `<control>`. The control header must not clear the global's switch opcodes.
    let dir = library();
    fs::write(
        dir.path().join("sustain.sfz"),
        "<control> default_path=Samples/\n\
         <group> lokey=c3 hikey=c5\n\
         <region> sample=soft1.wav\n",
    )
    .expect("write");
    fs::write(
        dir.path().join("staccato.sfz"),
        "<group> lokey=c3 hikey=c5\n\
         <region> sample=loud1.wav\n",
    )
    .expect("write");
    let bank = load_patch(
        &dir,
        "<global> sw_lokey=c2 sw_hikey=a2 sw_default=c2 sw_last=c2\n\
         #include \"sustain.sfz\"\n\
         <global> sw_lokey=c2 sw_hikey=a2 sw_default=c2 sw_last=c#2\n\
         #include \"staccato.sfz\"\n",
    );
    assert_eq!(
        bank.regions.iter().map(|r| r.sw_last).collect::<Vec<_>>(),
        [Some(36), Some(37)],
        "each articulation keeps its own switch"
    );
    assert_eq!(
        bank.keyswitches.keys().collect::<Vec<_>>(),
        (36..=45).collect::<Vec<_>>(),
        "c2..a2 are keys 36..45"
    );
    assert_eq!(bank.keyswitches.default, Some(36));
}

#[test]
fn control_header_keeps_master_and_group_opcodes() {
    let dir = library();
    let bank = load_patch(
        &dir,
        "<master> volume=-6\n\
         <group> lokey=c3 hikey=c5 sw_last=c1 sw_lokey=c1 sw_hikey=d1\n\
         <control> default_path=Samples/\n\
         <region> sample=soft1.wav\n",
    );
    let region = &bank.regions[0];
    assert_eq!((region.key_lo, region.key_hi), (48, 72));
    assert_eq!(region.volume_db, -6.0);
    assert_eq!(region.sw_last, Some(24));
    assert_eq!(bank.keyswitches.keys().collect::<Vec<_>>(), [24, 25, 26]);
}

#[test]
fn a_new_control_header_resets_default_path() {
    // The second region names its sample from the patch folder: a `default_path` left over from
    // the first `<control>` would look for `Samples/Samples/loud1.wav`.
    let dir = library();
    let bank = load_patch(
        &dir,
        "<control> default_path=Samples/\n\
         <region> sample=soft1.wav\n\
         <control>\n\
         <region> sample=Samples/loud1.wav\n",
    );
    assert_eq!(bank.regions.len(), 2);
    assert_eq!(bank.samples.len(), 2, "two distinct files are decoded");
    let loud = &bank.samples[bank.regions[1].sample];
    assert!(
        (f64::from(loud.data[0]) - 20_000.0 / 32_768.0).abs() < 1e-6,
        "the second region plays loud1.wav"
    );
}

#[test]
fn region_level_switch_range_is_read() {
    let dir = library();
    let bank = load_patch(
        &dir,
        "<group> lokey=c3 hikey=c5\n\
         <region> sw_lokey=c1 sw_hikey=c#1 sw_last=c1 sample=Samples/soft1.wav\n\
         <region> sw_lokey=c1 sw_hikey=c#1 sw_last=c#1 sw_default=c#1 sample=Samples/loud1.wav\n",
    );
    assert_eq!(bank.keyswitches.keys().collect::<Vec<_>>(), [24, 25]);
    assert_eq!(
        bank.keyswitches.default,
        Some(25),
        "a region-level sw_default counts"
    );
}

#[test]
fn disjoint_switch_ranges_leave_the_keys_between_them_playable() {
    let dir = library();
    let bank = load_patch(
        &dir,
        "<group> sw_lokey=c1 sw_hikey=c#1 sw_last=c1 lokey=c3 hikey=c5\n\
         <region> sample=Samples/soft1.wav\n\
         <group> sw_lokey=g1 sw_hikey=g#1 sw_last=g1 lokey=c3 hikey=c5\n\
         <region> sample=Samples/loud1.wav\n",
    );
    assert_eq!(
        bank.keyswitches.keys().collect::<Vec<_>>(),
        [24, 25, 31, 32]
    );
    assert!(
        (26..=30).all(|key| !bank.keyswitches.contains(key)),
        "the keys between the ranges are notes"
    );
    assert_eq!(bank.keyswitches.default, None, "no sw_default anywhere");
}

#[test]
fn the_last_valid_sw_default_in_the_file_wins() {
    let dir = library();
    let bank = load_patch(
        &dir,
        "<global> sw_lokey=c1 sw_hikey=d1 sw_default=c1\n\
         <group> sw_default=d1 sw_last=c1 lokey=c3 hikey=c5\n\
         <region> sample=Samples/soft1.wav\n\
         <group> sw_last=d1 lokey=c3 hikey=c5\n\
         <region> sample=Samples/loud1.wav sw_default=c#1\n\
         <group> sw_default=x9\n",
    );
    assert_eq!(
        bank.keyswitches.default,
        Some(25),
        "the region's c#1 is the last valid one; x9 is no key"
    );
}

/// Writes Salamander's layout (TST-06): note regions, then a third `<global>` and an include
/// whose first line, before any header, sets `lokey=-1 hikey=-1` for that global's scope, so
/// that every pedal region inherits it. The pedal-down and pedal-up noises are each split by
/// `lorand`/`hirand` and started by CC 64.
fn salamander_pedals(dir: &tempfile::TempDir) -> SampleBank {
    fs::write(
        dir.path().join("pedal.txt"),
        "lokey=-1 hikey=-1 group=1\n\
         <group> on_locc64=127 on_hicc64=127 locc64=127 hicc64=127\n\
         <region> sample=Samples/loud1.wav lorand=0 hirand=0.5\n\
         <region> sample=Samples/loud2.wav lorand=0.5 hirand=1\n\
         <group> on_locc64=0 on_hicc64=0 locc64=0 hicc64=0\n\
         <region> sample=Samples/tail.wav\n",
    )
    .expect("write");
    load_patch(
        dir,
        "<global> volume=0\n\
         <group> lovel=1 hivel=127\n\
         <region> sample=Samples/soft1.wav key=60\n\
         <global> volume=0\n\
         #include \"pedal.txt\"\n",
    )
}

#[test]
fn minus_one_keys_disable_key_triggering() {
    let dir = library();
    let bank = salamander_pedals(&dir);
    assert_eq!(
        bank.regions.iter().map(|r| r.on_key).collect::<Vec<_>>(),
        [true, false, false, false],
        "only the note region plays on a key"
    );
    assert_eq!((bank.regions[0].key_lo, bank.regions[0].key_hi), (60, 60));
    // Each of the three key opcodes alone, and a region's own range over an inherited -1.
    let bank = load_patch(
        &dir,
        "<global> lokey=-1 hikey=-1\n\
         <region> sample=Samples/soft1.wav lokey=c3 hikey=c5\n\
         <global>\n\
         <region> sample=Samples/soft1.wav key=-1\n\
         <region> sample=Samples/soft1.wav lokey=-1\n\
         <region> sample=Samples/soft1.wav hikey=-1\n\
         <region> sample=Samples/soft1.wav lokey=c3 hikey=c5\n",
    );
    assert_eq!(
        bank.regions.iter().map(|r| r.on_key).collect::<Vec<_>>(),
        [true, false, false, false, true]
    );
    assert_eq!((bank.regions[0].key_lo, bank.regions[0].key_hi), (48, 72));
    assert_eq!(
        bank.regions[1].pitch_keycenter, 60,
        "key=-1 leaves the default key centre"
    );
    assert!(
        bank.ignored_opcodes.is_empty(),
        "{:?}",
        bank.ignored_opcodes
    );
}

#[test]
fn the_deepest_scope_that_writes_a_key_decides() {
    // The last key opcode decides, as the reference player sfizz reads it: a region's own
    // `key=60` under an inherited -1 plays on key 60, as it did before -1 was read, and a -1
    // written deeper than a valid key still disables the region.
    let dir = library();
    let bank = load_patch(
        &dir,
        "<global> lokey=-1 hikey=-1\n\
         <region> sample=Samples/soft1.wav key=60\n\
         <group> key=-1\n\
         <region> sample=Samples/soft1.wav\n\
         <global> key=60\n\
         <group> hikey=-1\n\
         <region> sample=Samples/soft1.wav\n\
         <region> sample=Samples/soft1.wav hikey=c5\n",
    );
    assert_eq!(
        bank.regions.iter().map(|r| r.on_key).collect::<Vec<_>>(),
        [true, false, false, true]
    );
    assert_eq!(
        (
            bank.regions[0].key_lo,
            bank.regions[0].key_hi,
            bank.regions[0].pitch_keycenter
        ),
        (60, 60, 60)
    );
    assert_eq!((bank.regions[3].key_lo, bank.regions[3].key_hi), (60, 72));
}

#[test]
fn other_keys_that_name_no_key_keep_the_full_keyboard() {
    // TST-03: only -1 means "no key"; another invalid key is ignored as before.
    let dir = library();
    let bank = load_patch(
        &dir,
        "<region> sample=Samples/soft1.wav lokey=x9 hikey=200\n\
         <region> sample=Samples/soft1.wav lokey=-2 hikey=-1.0\n",
    );
    for region in &bank.regions {
        assert!(region.on_key);
        assert_eq!((region.key_lo, region.key_hi), (0, 127));
    }
}

#[test]
fn controller_triggered_regions_are_not_key_triggered() {
    // A controller trigger replaces the key's. A -1 is the format's "no trigger", and a
    // `locc`/`hicc` range is a condition, not a trigger.
    let dir = library();
    let bank = load_patch(
        &dir,
        "<group> lokey=c3 hikey=c5\n\
         <region> sample=Samples/soft1.wav on_locc64=127 on_hicc64=127\n\
         <region> sample=Samples/soft1.wav on_hicc1=0\n\
         <region> sample=Samples/soft1.wav on_lohdcc11=0.5\n\
         <region> sample=Samples/soft1.wav on_locc64=-1\n\
         <region> sample=Samples/soft1.wav locc64=0 hicc64=63\n",
    );
    assert_eq!(
        bank.regions.iter().map(|r| r.on_key).collect::<Vec<_>>(),
        [false, false, false, true, true]
    );
    assert_eq!(
        bank.ignored_opcodes,
        ["hicc*", "locc*", "on_hicc*", "on_locc*", "on_lohdcc*"],
        "the sampler starts no region from a controller yet"
    );
}

#[test]
fn salamander_pedal_noises_do_not_sound_on_a_note() {
    // The loaded patch in the player: a note used to start the note, one of the two pedal-down
    // noises and the pedal-up noise, each transposed to the key. Now only the note sounds.
    let dir = library();
    let bank = salamander_pedals(&dir);
    let mut sampler = create("sonara.sampler").expect("the sampler exists");
    sampler.prepare(ProcessSpec {
        sample_rate: 44_100,
        block_size: 2_048,
        channels: 2,
    });
    sampler.set_state(&DeviceState {
        sample_bank: Some(Arc::new(bank)),
        ..DeviceState::default()
    });
    let descriptor = sampler_descriptor(&SamplerChoices {
        presets: vec!["/library/piano.sfz".to_owned()],
        ..SamplerChoices::default()
    })
    .expect("a patch");
    sampler.set_params(&param_slots(&descriptor, &Params::new()).expect("defaults"));
    let mut buffer = vec![[0.0; 2]; 2_048];
    let note_on = InstrumentEvent {
        offset: 0,
        kind: EventKind::NoteOn,
        pitch: 60,
        velocity: 127,
        channel: 0,
        value: 0,
        number: 0,
    };
    sampler.process(
        &mut buffer,
        &BlockContext {
            frame: 0,
            sample_rate: 44_100,
            beat: 0.0,
            bpm: 120.0,
            playing: true,
            sidechain: None,
            events: &[note_on],
        },
    );
    assert_eq!(sampler.report().entries(), [("voices", 1.0)]);
    // soft1.wav at full velocity, unity volume and centre pan.
    let last = buffer.last().expect("a block")[0];
    assert!((last - 3_000.0 / 32_768.0).abs() < 1e-6, "{last}");
}

#[test]
fn a_patch_without_switch_opcodes_has_no_keyswitches() {
    let dir = library();
    let bank = load_patch(&dir, "<region> sample=Samples/soft1.wav sw_last=c1\n");
    assert_eq!(bank.keyswitches, Keyswitches::NONE);
}

#[test]
fn decodes_the_samples_and_reports_what_it_ignores() {
    let dir = library();
    let bank = load_sfz(&dir.path().join("violin.sfz")).expect("loads");
    let soft = &bank.samples[bank.regions[0].sample];
    assert_eq!(
        (soft.rate, soft.channels, soft.frames()),
        (44_100, 1, 4_410)
    );
    assert!((f64::from(soft.data[0]) - 3_000.0 / 32_768.0).abs() < 1e-6);
    assert_eq!(bank.ignored_opcodes, ["cutoff", "locc*"]);
    // The release sample's whole length is the tail: 4410 frames at 44.1 kHz = 0.1 s,
    // the release time of the attack regions is 0.3 s.
    assert!((bank.tail_seconds - 0.3).abs() < 1e-9);
}

#[test]
fn unsupported_trigger_values_are_reported() {
    // Sonatina's legato and timpani roll patches (TST-06): `first`, `legato` and `release_key`
    // regions play on every key press until the sampler plays those triggers, and the load
    // warning used to say nothing about it.
    let dir = library();
    let bank = load_patch(
        &dir,
        "<group> lokey=c3 hikey=c5\n\
         <region> sample=Samples/soft1.wav trigger=first\n\
         <region> sample=Samples/soft1.wav trigger=legato\n\
         <region> sample=Samples/soft1.wav trigger=release_key\n\
         <region> sample=Samples/soft1.wav trigger=release\n\
         <region> sample=Samples/soft1.wav trigger=attack\n\
         <region> sample=Samples/soft1.wav trigger=legato\n",
    );
    assert_eq!(
        bank.ignored_opcodes,
        ["trigger=first", "trigger=legato", "trigger=release_key"]
    );
    assert_eq!(
        bank.regions.iter().map(|r| r.trigger).collect::<Vec<_>>(),
        [
            Trigger::Attack,
            Trigger::Attack,
            Trigger::Attack,
            Trigger::Release,
            Trigger::Attack,
            Trigger::Attack
        ],
        "an unsupported trigger plays as the default, attack"
    );
}

#[test]
fn unsupported_values_and_headers_are_reported() {
    // Salamander's stand-alone `<curve>` and an unknown loop mode (TST-06) used to be dropped
    // without a word. The opcodes under a skipped header are not reported one by one.
    let dir = library();
    let bank = load_patch(
        &dir,
        "<control> default_path=Samples/\n\
         <curve> curve_index=7 v000=0 v127=1\n\
         <effect> type=lofi bus=main\n\
         <midi> polyphony=8\n\
         <sample> name=embedded.wav\n\
         <group> lokey=c3 hikey=c5\n\
         <region> sample=soft1.wav loop_mode=loop_forever\n\
         <region> sample=loud1.wav loopmode=sustain\n\
         <region> sample=tail.wav loop_mode=no_loop\n\
         <region> sample=soft2.wav loop_mode=loop_sustain\n",
    );
    assert_eq!(
        bank.ignored_opcodes,
        [
            "<curve>",
            "<effect>",
            "<midi>",
            "<sample>",
            "loop_mode=loop_forever",
            "loopmode=sustain"
        ]
    );
    assert_eq!(
        bank.regions.iter().map(|r| r.loop_mode).collect::<Vec<_>>(),
        [
            LoopMode::NoLoop,
            LoopMode::NoLoop,
            LoopMode::NoLoop,
            LoopMode::Sustain
        ],
        "an unsupported loop mode plays as the default, no_loop"
    );
    assert!(
        bank.regions
            .iter()
            .all(|r| (r.key_lo, r.key_hi) == (48, 72)),
        "the group after the skipped headers is read"
    );
}

#[test]
fn amp_veltrack_is_read_with_its_scope_and_kept_in_range() {
    // Organ and harpsichord patches set `amp_veltrack=0` so velocity leaves the level alone; the
    // opcode used to be ignored. Values outside -100 to 100 are read as the nearest end, and one
    // that is not a number as the default. The velocity curve opcodes stay unsupported.
    let dir = library();
    let bank = load_patch(
        &dir,
        "<control> default_path=Samples/\n\
         <group> amp_veltrack=0 lokey=c3 hikey=c5\n\
         <region> sample=soft1.wav\n\
         <region> sample=soft1.wav amp_veltrack=-40.5\n\
         <region> sample=soft1.wav amp_veltrack=-150\n\
         <region> sample=soft1.wav amp_veltrack=250\n\
         <region> sample=soft1.wav amp_veltrack=nan\n\
         <group> amp_velcurve_064=1 amp_veltrack_oncc1=20\n\
         <region> sample=soft1.wav\n",
    );
    assert_eq!(
        bank.regions
            .iter()
            .map(|r| r.amp_veltrack)
            .collect::<Vec<_>>(),
        [0.0, -40.5, -100.0, 100.0, 100.0, 100.0]
    );
    assert_eq!(
        bank.ignored_opcodes,
        ["amp_velcurve_*", "amp_veltrack_oncc*"]
    );
}

/// Sonatina's timpani layout (TST-06): the f group fades in and the p group fades out over
/// velocities 70 to 90, neither with a velocity range, so both used to sound on every hit.
const TIMPANI: &str = "<control> default_path=Samples/\n\
     <group> xfin_lovel=70 xfin_hivel=90 lokey=c3 hikey=c5\n\
     <region> sample=loud1.wav\n\
     <group> xfout_lovel=70 xfout_hivel=90 lokey=c3 hikey=c5\n\
     <region> sample=soft1.wav\n";

#[test]
fn velocity_crossfades_are_read_with_their_scope() {
    let dir = library();
    let bank = load_patch(&dir, TIMPANI);
    let crossfades: Vec<_> = bank.regions.iter().map(|r| r.velocity_crossfade).collect();
    assert_eq!(
        crossfades,
        [
            VelocityCrossfade {
                in_lo: 70,
                in_hi: 90,
                ..VelocityCrossfade::NONE
            },
            VelocityCrossfade {
                out_lo: 70,
                out_hi: 90,
                ..VelocityCrossfade::NONE
            },
        ]
    );
    assert!(
        bank.ignored_opcodes.is_empty(),
        "{:?}",
        bank.ignored_opcodes
    );
    assert!(
        bank.regions
            .iter()
            .all(|r| (r.vel_lo, r.vel_hi) == (0, 127)),
        "a crossfade does not narrow the velocity range"
    );
}

#[test]
fn velocity_crossfade_values_and_curves() {
    // A region's own value overrides its group's; a value above 127 is read as 127; one bound
    // alone keeps the other's default; an unknown curve is reported and plays as power. The key
    // and controller crossfades stay unsupported and reported.
    let dir = library();
    let bank = load_patch(
        &dir,
        "<control> default_path=Samples/\n\
         <group> xfin_lovel=10 xfin_hivel=30 xf_velcurve=gain\n\
         <region> sample=soft1.wav\n\
         <region> sample=soft1.wav xfin_hivel=200 xf_velcurve=power\n\
         <group>\n\
         <region> sample=soft1.wav xfout_lovel=100\n\
         <region> sample=soft1.wav xfin_lovel=70 xf_velcurve=sine\n\
         <region> sample=soft1.wav xfin_locc1=0 xfin_hicc1=127 xf_cccurve=gain xfin_lokey=60\n",
    );
    let crossfade = |in_lo, in_hi, out_lo, curve| VelocityCrossfade {
        in_lo,
        in_hi,
        out_lo,
        out_hi: 127,
        curve,
    };
    let (power, gain) = (CrossfadeCurve::Power, CrossfadeCurve::Gain);
    assert_eq!(
        bank.regions
            .iter()
            .map(|r| r.velocity_crossfade)
            .collect::<Vec<_>>(),
        [
            crossfade(10, 30, 127, gain),
            crossfade(10, 127, 127, power),
            crossfade(0, 0, 100, power),
            crossfade(70, 0, 127, power),
            VelocityCrossfade::NONE,
        ]
    );
    assert_eq!(
        bank.ignored_opcodes,
        [
            "xf_cccurve",
            "xf_velcurve=sine",
            "xfin_hicc*",
            "xfin_locc*",
            "xfin_lokey"
        ]
    );
}

#[test]
fn a_misspelt_header_is_reported_and_its_opcodes_skipped() {
    // TST-03: headers are matched as written, so `<Group>` is not a group: its opcodes do not
    // reach the region, and the header is reported.
    let dir = library();
    let bank = load_patch(
        &dir,
        "<Group> lokey=c3 hikey=c5 cutoff=500\n<region> sample=Samples/soft1.wav\n",
    );
    assert_eq!(bank.ignored_opcodes, ["<Group>"]);
    assert_eq!((bank.regions[0].key_lo, bank.regions[0].key_hi), (0, 127));
}

#[test]
fn a_missing_patch_or_sample_is_a_typed_error() {
    let dir = library();
    let missing = load_sfz(&dir.path().join("nothing.sfz"));
    assert!(matches!(
        missing,
        Err(MediaError::Io {
            operation: "read",
            ..
        })
    ));
    // A missing sample no longer fails the patch by itself, only its regions; with no region
    // left, the patch is malformed, and the message names the missing file. It was the
    // sample's `Io` error before.
    fs::write(
        dir.path().join("broken.sfz"),
        "<region> sample=Samples/absent.wav\n",
    )
    .expect("write");
    let result = load_sfz(&dir.path().join("broken.sfz"));
    let absent = dir.path().join("Samples/absent.wav");
    assert!(
        matches!(&result, Err(MediaError::Malformed { message, .. })
        if *message == format!(
            "the patch has no playable region: sample {} is missing (1 region left out)",
            absent.display()
        )),
        "{result:?}"
    );
}

#[test]
fn a_generator_sample_does_not_fail_the_patch() {
    // The SFZ format's generators (TST-06): `*silence` is a region that plays nothing, and a
    // generator the sampler does not play leaves out its own region, as a missing file does.
    // Any of them used to fail the whole patch.
    let dir = library();
    let bank = load_patch(
        &dir,
        "<region> sample=Samples/soft1.wav lokey=c3 hikey=c5\n\
         <region> sample=*sine key=c6\n\
         <region> sample=*silence key=d6 group=1\n\
         <region> sample=Samples/absent.wav key=e6\n\
         <region> sample=*silence key=f6 trigger=release\n",
    );
    assert_eq!(bank.regions.len(), 3, "the note and the two silent regions");
    assert!((first_value(&bank, 0) - 3_000.0 / 32_768.0).abs() < 1e-6);
    let silent: Vec<_> = bank.regions[1..]
        .iter()
        .map(|r| (r.key_lo, r.group, r.trigger, r.sample))
        .collect();
    let silence = bank.regions[1].sample;
    assert_eq!(
        silent,
        [
            (86, 1, Trigger::Attack, silence),
            (89, 0, Trigger::Release, silence)
        ],
        "both silent regions share one sample"
    );
    assert_eq!(bank.samples.len(), 2);
    assert_eq!(bank.samples[silence].frames(), 0);
    assert_eq!(
        bank.load_notes,
        [
            "the sampler does not play the generator *sine (1 region left out)".to_owned(),
            format!(
                "sample {} is missing (1 region left out)",
                dir.path().join("Samples/absent.wav").display()
            ),
        ]
    );
    assert!(
        bank.ignored_opcodes.is_empty(),
        "{:?}",
        bank.ignored_opcodes
    );
    assert!(
        (bank.tail_seconds - 0.0).abs() < f64::EPSILON,
        "a silent release region adds no tail"
    );
}

#[test]
fn a_missing_sample_leaves_out_only_the_regions_that_name_it() {
    // One missing file in a large patch used to silence the whole instrument. Its regions are
    // counted in one note, whichever header path names the file.
    let dir = library();
    let bank = load_patch(
        &dir,
        "<control> default_path=Samples/\n\
         <group> lokey=c3 hikey=c5\n\
         <region> sample=absent.wav lovel=1 hivel=63\n\
         <region> sample=soft1.wav lovel=1 hivel=63\n\
         <region> sample=absent.wav lovel=64\n\
         <region> sample=loud1.wav lovel=64\n\
         <region> sample=gone.wav trigger=release\n",
    );
    assert_eq!(
        bank.regions
            .iter()
            .map(|r| (r.vel_lo, r.vel_hi))
            .collect::<Vec<_>>(),
        [(1, 63), (64, 127)]
    );
    assert!((first_value(&bank, 0) - 3_000.0 / 32_768.0).abs() < 1e-6);
    assert!((first_value(&bank, 1) - 20_000.0 / 32_768.0).abs() < 1e-6);
    let samples = dir.path().join("Samples");
    assert_eq!(
        bank.load_notes,
        [
            format!(
                "sample {} is missing (2 regions left out)",
                samples.join("absent.wav").display()
            ),
            format!(
                "sample {} is missing (1 region left out)",
                samples.join("gone.wav").display()
            ),
        ]
    );
}

#[test]
fn a_patch_of_unplayable_samples_names_them_all() {
    // TST-03: with no region left, the patch fails, and the message gives every reason.
    let dir = library();
    let result = load_sfz_text(
        &dir,
        "<region> sample=*saw\n<region> sample=*saw key=61\n<region> sample=Samples/x.wav\n",
    );
    let expected = format!(
        "the patch has no playable region: the sampler does not play the generator *saw \
         (2 regions left out); sample {} is missing (1 region left out)",
        dir.path().join("Samples/x.wav").display()
    );
    assert!(
        matches!(&result, Err(MediaError::Malformed { message, .. }) if *message == expected),
        "{result:?}"
    );
}

#[test]
fn a_sample_that_cannot_be_decoded_still_fails_the_patch() {
    // TST-03: only a sample file that does not exist leaves out its regions; a file that is
    // there but broken is an error of the patch, as before.
    let dir = library();
    fs::write(
        dir.path().join("Samples/bad.wav"),
        b"RIFF not really a wave file",
    )
    .expect("write");
    let result = load_sfz_text(
        &dir,
        "<region> sample=Samples/soft1.wav\n<region> sample=Samples/bad.wav key=61\n",
    );
    let error = result.expect_err("a broken sample fails the patch");
    let message = error.to_string();
    assert!(message.contains("bad.wav"), "{message}");
    assert!(!message.contains("no playable region"), "{message}");
}

/// Plays `bank` in the sampler at 44.1 kHz with the default parameters, one 2048-frame block per
/// entry of `blocks`, each with its note-ons at frame 0. Returns the last block and the voices
/// still sounding at its end.
fn play(bank: SampleBank, blocks: &[&[u8]]) -> (Vec<[f64; 2]>, f64) {
    let mut sampler = create("sonara.sampler").expect("the sampler exists");
    sampler.prepare(ProcessSpec {
        sample_rate: 44_100,
        block_size: 2_048,
        channels: 2,
    });
    sampler.set_state(&DeviceState {
        sample_bank: Some(Arc::new(bank)),
        ..DeviceState::default()
    });
    let descriptor = sampler_descriptor(&SamplerChoices {
        presets: vec!["/library/patch.sfz".to_owned()],
        ..SamplerChoices::default()
    })
    .expect("a patch");
    sampler.set_params(&param_slots(&descriptor, &Params::new()).expect("defaults"));
    let mut buffer = vec![[0.0; 2]; 2_048];
    for (block, keys) in blocks.iter().enumerate() {
        let events: Vec<_> = keys
            .iter()
            .map(|key| InstrumentEvent {
                offset: 0,
                kind: EventKind::NoteOn,
                pitch: *key,
                velocity: 127,
                channel: 0,
                value: 0,
                number: 0,
            })
            .collect();
        buffer.fill([0.0; 2]);
        sampler.process(
            &mut buffer,
            &BlockContext {
                frame: i64::try_from(block * 2_048).expect("a small frame"),
                sample_rate: 44_100,
                beat: 0.0,
                bpm: 120.0,
                playing: true,
                sidechain: None,
                events: &events,
            },
        );
    }
    let voices = sampler.report().entries()[0].1;
    (buffer, voices)
}

#[test]
fn a_silence_region_plays_nothing_and_chokes_its_group() {
    // A `*silence` region is how a patch writes a note that only cuts others off: its note
    // starts no voice, and its group switches off the regions that name it in `off_by`.
    let dir = library();
    let text = |group: &str| {
        format!(
            "<region> sample=Samples/loud1.wav key=60 off_by=1 loop_mode=one_shot\n\
             <region> sample=*silence key=62 {group}\n"
        )
    };
    let (alone, voices) = play(load_patch(&dir, &text("group=1")), &[&[62]]);
    assert_eq!(voices, 0.0, "the silent note starts no voice");
    assert!(alone.iter().all(|frame| *frame == [0.0, 0.0]));

    // loud1.wav is 4410 frames long, so the note still sounds at the end of the second block,
    // unless the silent note's group cut it off with the 10 ms choke.
    let (choked, voices) = play(load_patch(&dir, &text("group=1")), &[&[60], &[62]]);
    assert_eq!(voices, 0.0, "the note is choked");
    assert!(choked[512..].iter().all(|frame| *frame == [0.0, 0.0]));
    let (kept, voices) = play(load_patch(&dir, &text("")), &[&[60], &[62]]);
    assert_eq!(voices, 1.0, "without a group nothing is choked");
    assert!(
        (kept[2_000][0] - 20_000.0 / 32_768.0).abs() < 1e-6,
        "{}",
        kept[2_000][0]
    );
}

#[test]
fn a_patch_without_regions_is_malformed() {
    let dir = library();
    fs::write(dir.path().join("empty.sfz"), "<global> volume=1\n").expect("write");
    assert!(matches!(
        load_sfz(&dir.path().join("empty.sfz")),
        Err(MediaError::Malformed { .. })
    ));
}

/// The first sample value of `region`'s sample, to tell the fixture's files apart.
fn first_value(bank: &SampleBank, region: usize) -> f64 {
    f64::from(bank.samples[bank.regions[region].sample].data[0])
}

#[test]
fn an_include_after_a_header_on_the_same_line_is_read() {
    // Salamander's layout (TST-06): the velocity layer's opcodes and its regions are included
    // in the middle of a `<group>` line. Read only at the start of a line, the first include
    // was dropped and the second was appended to `hivel`, which then fell back to 127, and the
    // note regions were never loaded.
    let dir = library();
    fs::write(dir.path().join("v.txt"), "lokey=c3 hikey=c5 volume=-3\n").expect("write");
    fs::write(
        dir.path().join("r.txt"),
        "<region> sample=Samples/soft1.wav\n<region> sample=Samples/loud1.wav\n",
    )
    .expect("write");
    let bank = load_patch(
        &dir,
        "<group> #include \"v.txt\" lovel=1 hivel=26 #include \"r.txt\"\n",
    );
    assert_eq!(bank.regions.len(), 2, "the included regions are read");
    for region in &bank.regions {
        assert_eq!((region.vel_lo, region.vel_hi), (1, 26));
        assert_eq!((region.key_lo, region.key_hi), (48, 72), "v.txt is read");
        assert_eq!(region.volume_db, -3.0);
    }
    assert!((first_value(&bank, 0) - 3_000.0 / 32_768.0).abs() < 1e-6);
    assert!((first_value(&bank, 1) - 20_000.0 / 32_768.0).abs() < 1e-6);
    assert_eq!(
        bank.ignored_opcodes,
        Vec::<String>::new(),
        "every opcode in v.txt is supported"
    );
}

#[test]
fn a_define_after_a_header_on_the_same_line_is_read() {
    let dir = library();
    let bank = load_patch(
        &dir,
        "<group> lokey=c3 hikey=c5 #define $TOP 26\n\
         <region> sample=Samples/soft1.wav hivel=$TOP\n",
    );
    let region = &bank.regions[0];
    assert_eq!(
        (region.key_lo, region.key_hi),
        (48, 72),
        "the text before the define is read"
    );
    assert_eq!(region.vel_hi, 26, "$TOP is defined");
}

#[test]
fn a_define_takes_the_rest_of_its_line() {
    // `hivel=2` belongs to the define, as the reference player sfizz reads a define's value to
    // the end of its line; it never reaches the open `<group>`.
    let dir = library();
    let bank = load_patch(
        &dir,
        "<group> lovel=1 #define $TOP 26 hivel=2\n\
         <region> sample=Samples/soft1.wav\n",
    );
    let region = &bank.regions[0];
    assert_eq!((region.vel_lo, region.vel_hi), (1, 127));
}

#[test]
fn include_paths_are_quoted_or_one_word() {
    // A quoted path keeps its spaces; an unquoted one ends at the next space, and the line goes
    // on after either. A header's `>` also ends a token, so an include may follow it directly.
    let dir = library();
    fs::write(
        dir.path().join("my part.sfz"),
        "<region> sample=Samples/soft1.wav\n",
    )
    .expect("write");
    fs::write(
        dir.path().join("loud.sfz"),
        "<region> sample=Samples/loud1.wav\n",
    )
    .expect("write");
    let bank = load_patch(
        &dir,
        "<group> lokey=c3 hikey=c5 #include \"my part.sfz\" lovel=10\n\
         <group>#include loud.sfz lovel=64\n",
    );
    assert_eq!(bank.regions.len(), 2);
    assert!((first_value(&bank, 0) - 3_000.0 / 32_768.0).abs() < 1e-6);
    assert_eq!((bank.regions[0].key_lo, bank.regions[0].key_hi), (48, 72));
    assert_eq!(
        bank.regions[0].vel_lo, 10,
        "the line goes on after the path"
    );
    assert!((first_value(&bank, 1) - 20_000.0 / 32_768.0).abs() < 1e-6);
    assert_eq!(bank.regions[1].vel_lo, 64);
}

#[test]
fn a_hash_inside_a_value_is_not_a_directive() {
    // Only `#include` and `#define` are directives: a sample path with `#` after a space and
    // a note name with a sharp stay opcode values.
    let dir = library();
    wav(&dir.path().join("Samples/Hit #1.wav"), 44_100, 441, 7_000);
    let bank = load_patch(&dir, "<region> sample=Samples/Hit #1.wav key=c#4\n");
    assert_eq!(bank.regions.len(), 1);
    assert!((first_value(&bank, 0) - 7_000.0 / 32_768.0).abs() < 1e-6);
    assert_eq!((bank.regions[0].key_lo, bank.regions[0].key_hi), (61, 61));
}

#[test]
fn a_directive_in_a_comment_is_ignored() {
    let dir = library();
    let bank = load_patch(
        &dir,
        "// #include \"absent.sfz\"\n\
         <region> sample=Samples/soft1.wav // #define $X 1 #include \"absent.sfz\"\n\
         /* <region> #include \"absent.sfz\" */\n",
    );
    assert_eq!(bank.regions.len(), 1, "no absent file is read");
}

#[test]
fn an_include_cycle_is_malformed() {
    let dir = library();
    let path = dir.path().join("self.sfz");
    fs::write(
        &path,
        "<region> sample=Samples/soft1.wav #include \"self.sfz\"\n",
    )
    .expect("write");
    let result = load_sfz(&path);
    assert!(
        matches!(&result, Err(MediaError::Malformed { message, .. }) if message.contains("too deep")),
        "{result:?}"
    );
}

#[test]
fn defines_expand_in_include_paths() {
    // Sonatina Grand Piano's layout (TST-06): the fragment folder and each dynamic layer are
    // defines, and the layer define is redefined between includes. Taken literally, the path
    // `$DIR/$DYN.txt` named no file and the whole patch failed to load.
    let dir = library();
    let data = dir.path().join("Data");
    fs::create_dir(&data).expect("mkdir");
    fs::write(
        data.join("p.txt"),
        "<region> sample=Samples/soft1.wav hivel=63\n",
    )
    .expect("write");
    fs::write(
        data.join("f.txt"),
        "<region> sample=Samples/loud1.wav lovel=64\n",
    )
    .expect("write");
    let bank = load_patch(
        &dir,
        "#define $DIR Data\n\
         #define $DYN p\n\
         <group> lokey=c3 hikey=c5 #include \"$DIR/$DYN.txt\"\n\
         #define $DYN f\n\
         <group> lokey=c3 hikey=c5 #include $DIR/$DYN.txt\n",
    );
    assert_eq!(bank.regions.len(), 2, "both layers are read");
    assert!((first_value(&bank, 0) - 3_000.0 / 32_768.0).abs() < 1e-6);
    assert_eq!((bank.regions[0].vel_lo, bank.regions[0].vel_hi), (0, 63));
    assert!(
        (first_value(&bank, 1) - 20_000.0 / 32_768.0).abs() < 1e-6,
        "the redefined $DYN names the second layer"
    );
    assert_eq!((bank.regions[1].vel_lo, bank.regions[1].vel_hi), (64, 127));
}

#[test]
fn an_undefined_name_in_an_include_path_stays_as_written() {
    // TST-03: a name that was never defined is not dropped from the path, so the include names
    // a file that does not exist and the load fails with the path it tried.
    let dir = library();
    let result = load_sfz_text(&dir, "#include \"$NOPE/x.txt\"\n");
    assert!(
        matches!(&result, Err(MediaError::Io { operation: "read", path, .. })
            if path == &dir.path().join("$NOPE/x.txt")),
        "{result:?}"
    );
}

#[test]
fn nested_includes_resolve_against_the_main_file() {
    // The MTG saxophones' layout (TST-06): the main patch includes `Data/a.txt`, which names its
    // sibling as `Data/b.txt`, relative to the main file. Resolved against the including file,
    // the path became `Data/Data/b.txt`, which does not exist, and the patch failed to load. The
    // decoy at that path proves the include is not read from there.
    let dir = library();
    let data = dir.path().join("Data");
    fs::create_dir_all(data.join("Data")).expect("mkdir");
    fs::write(
        data.join("a.txt"),
        "<group> lokey=c3 hikey=c5\n#include \"Data/b.txt\"\n",
    )
    .expect("write");
    fs::write(data.join("b.txt"), "<region> sample=Samples/soft1.wav\n").expect("write");
    fs::write(
        data.join("Data/b.txt"),
        "<region> sample=Samples/loud1.wav\n",
    )
    .expect("write");
    let bank = load_patch(&dir, "#include \"Data/a.txt\"\n");
    assert_eq!(bank.regions.len(), 1);
    assert!(
        (first_value(&bank, 0) - 3_000.0 / 32_768.0).abs() < 1e-6,
        "Data/b.txt beside the main file is read, not the decoy"
    );
    assert_eq!((bank.regions[0].key_lo, bank.regions[0].key_hi), (48, 72));
}

#[test]
fn includes_of_a_patch_in_a_subfolder_resolve_against_that_folder() {
    // The main file's folder is the root of every include, wherever the patch sits.
    let dir = library();
    let patches = dir.path().join("Patches");
    fs::create_dir_all(patches.join("Data")).expect("mkdir");
    fs::write(patches.join("Data/a.txt"), "#include \"Data/b.txt\"\n").expect("write");
    fs::write(
        patches.join("Data/b.txt"),
        "<region> sample=../Samples/tail.wav\n",
    )
    .expect("write");
    let path = patches.join("main.sfz");
    fs::write(&path, "#include \"Data/a.txt\"\n").expect("write");
    let bank = load_sfz(&path).expect("loads");
    assert_eq!(bank.regions.len(), 1);
    assert!((first_value(&bank, 0) - 1_000.0 / 32_768.0).abs() < 1e-6);
}

// --- Inspection without decoding ---

/// The velocity layers `lo..=hi` of `ranges`.
fn layers(ranges: &[(u8, u8)]) -> Vec<VelocityLayer> {
    ranges
        .iter()
        .map(|&(lo, hi)| VelocityLayer { lo, hi })
        .collect()
}

/// Writes `text` as a patch beside the fixture's `Samples` folder and inspects it.
fn inspect_text(dir: &tempfile::TempDir, text: &str) -> PatchReport {
    let path = dir.path().join("patch.sfz");
    fs::write(&path, text).expect("write");
    inspect(&path).expect("inspects")
}

/// Checks `report` against the bank the loader builds from the same patch: the same regions,
/// keyswitches, ignored opcodes and notes, and the decoded size of the samples it decoded.
fn assert_agrees_with_the_loader(report: &PatchReport, bank: &SampleBank) {
    assert_eq!(report.playable_regions, bank.regions.len());
    assert_eq!(report.keyswitches, bank.keyswitches);
    assert_eq!(report.ignored_opcodes, bank.ignored_opcodes);
    assert_eq!(report.notes, bank.load_notes);
    let decoded: usize = bank.samples.iter().map(|s| s.data.len() * 4).sum();
    assert_eq!(report.decoded_bytes, decoded);
    assert!(report.is_loadable());
}

/// The header of a PCM WAV file of `frames` frames, `channels` channels and `bytes` bytes per
/// sample; its data chunk follows.
fn wav_header(channels: u16, bytes: u16, frames: u32) -> Vec<u8> {
    let rate = 44_100_u32;
    let block = u32::from(channels) * u32::from(bytes);
    let data_len = frames * block;
    let mut header = Vec::new();
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(36 + data_len).to_le_bytes());
    header.extend_from_slice(b"WAVEfmt ");
    header.extend_from_slice(&16_u32.to_le_bytes());
    header.extend_from_slice(&1_u16.to_le_bytes());
    header.extend_from_slice(&channels.to_le_bytes());
    header.extend_from_slice(&rate.to_le_bytes());
    header.extend_from_slice(&(rate * block).to_le_bytes());
    header.extend_from_slice(&u16::try_from(block).expect("a small block").to_le_bytes());
    header.extend_from_slice(&(bytes * 8).to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_len.to_le_bytes());
    header
}

#[test]
fn inspect_reports_the_structure_the_loader_builds() {
    // The fixture: two velocity layers with round robin on c3-c5, a release group, three switch
    // keys from c1 with c1 as the default, and two opcodes the sampler ignores.
    let dir = library();
    let path = dir.path().join("violin.sfz");
    let report = inspect(&path).expect("inspects");
    assert_eq!(
        report,
        PatchReport {
            regions: 5,
            playable_regions: 5,
            sampled: vec![LayerZone {
                keys: 48..=72,
                layers: layers(&[(0, 63), (64, 127)]),
            }],
            keyswitches: report.keyswitches,
            ignored_opcodes: vec!["cutoff".to_owned(), "locc*".to_owned()],
            missing_samples: Vec::new(),
            unreadable_samples: Vec::new(),
            notes: Vec::new(),
            // Five mono files of 4410 frames, 4 bytes per decoded sample.
            decoded_bytes: 5 * 4_410 * 4,
        }
    );
    assert_eq!(report.keyswitches.keys().collect::<Vec<_>>(), [24, 25, 26]);
    assert_eq!(report.keyswitches.default, Some(24));
    assert_eq!(
        report.sampled_keys().collect::<Vec<_>>(),
        (48..=72).collect::<Vec<_>>()
    );
    assert_agrees_with_the_loader(&report, &load_sfz(&path).expect("loads"));
}

#[test]
fn inspect_agrees_with_the_loader_on_skipped_and_ignored_parts() {
    // Generators, a missing sample, unsupported values and skipped headers: the report and the
    // bank come from the same regions.
    let dir = library();
    for text in [
        "<region> sample=Samples/soft1.wav lokey=c3 hikey=c5\n\
         <region> sample=*sine key=c6\n\
         <region> sample=*silence key=d6 group=1\n\
         <region> sample=Samples/absent.wav key=e6\n",
        "<control> default_path=Samples/\n\
         <curve> curve_index=7 v000=0 v127=1\n\
         <group> lokey=c3 hikey=c5 cutoff=500\n\
         <region> sample=soft1.wav loop_mode=loop_forever trigger=legato\n\
         <region> sample=soft1.wav trigger=release\n\
         <region> sample=gone.wav trigger=first\n",
    ] {
        let report = inspect_text(&dir, text);
        let bank = load_patch(&dir, text);
        assert_agrees_with_the_loader(&report, &bank);
    }
}

#[test]
fn inspect_reports_the_velocity_layers_of_each_key() {
    let dir = library();
    let report = inspect_text(
        &dir,
        "<control> default_path=Samples/\n\
         <region> sample=soft1.wav lokey=48 hikey=59 lovel=1 hivel=62\n\
         <region> sample=loud1.wav lokey=48 hikey=59 lovel=63 hivel=127\n\
         <group> lokey=60 hikey=64 seq_length=2\n\
         <region> sample=soft2.wav seq_position=1\n\
         <region> sample=loud2.wav seq_position=2\n\
         <group>\n\
         <region> sample=loud2.wav key=65 lovel=64\n\
         <region> sample=soft1.wav key=67 hivel=80\n\
         <region> sample=loud1.wav key=67 lovel=40\n\
         <region> sample=soft1.wav key=69 lovel=1 hivel=62\n\
         <region> sample=loud1.wav key=69 lovel=63\n",
    );
    assert_eq!(
        report.sampled,
        [
            // Two layers.
            LayerZone {
                keys: 48..=59,
                layers: layers(&[(1, 62), (63, 127)]),
            },
            // Round-robin positions share one layer.
            LayerZone {
                keys: 60..=64,
                layers: layers(&[(0, 127)]),
            },
            // Velocities below 64 sound nothing.
            LayerZone {
                keys: 65..=65,
                layers: layers(&[(64, 127)]),
            },
            // Overlapping ranges: one layer for each part.
            LayerZone {
                keys: 67..=67,
                layers: layers(&[(0, 39), (40, 80), (81, 127)]),
            },
            // The layers of 48-59 again, but not next to them.
            LayerZone {
                keys: 69..=69,
                layers: layers(&[(1, 62), (63, 127)]),
            },
        ]
    );
    assert_eq!(
        report.sampled_keys().collect::<Vec<_>>(),
        (48..=65).chain([67, 69]).collect::<Vec<_>>()
    );
}

#[test]
fn inspect_splits_the_velocity_layers_at_a_crossfade() {
    // The timpani's p layer plays up to velocity 89 and the f layer from 71: below the
    // crossfade only p plays, inside it both, above it only f.
    let dir = library();
    let report = inspect_text(&dir, TIMPANI);
    assert_eq!(
        report.sampled,
        [LayerZone {
            keys: 48..=72,
            layers: layers(&[(0, 70), (71, 89), (90, 127)]),
        }]
    );
    assert_agrees_with_the_loader(&report, &load_patch(&dir, TIMPANI));
}

#[test]
fn inspect_counts_only_the_keys_a_press_sounds() {
    // Release regions, regions no key starts, silent regions, empty samples or velocity
    // ranges, and a `sw_last` that no switch reaches sound nothing on a press. A switch key
    // with a region both switches and sounds (VSCO's CelloEns-KS has such keys).
    let dir = library();
    wav(&dir.path().join("Samples/empty.wav"), 44_100, 0, 0);
    let report = inspect_text(
        &dir,
        "<control> default_path=Samples/\n\
         <global> sw_lokey=c1 sw_hikey=d1 sw_default=c1\n\
         <region> sample=soft1.wav key=25\n\
         <region> sample=soft1.wav key=70 trigger=release\n\
         <region> sample=soft1.wav key=-1\n\
         <region> sample=soft1.wav key=71 on_locc64=127 on_hicc64=127\n\
         <region> sample=*silence key=72\n\
         <region> sample=soft1.wav key=73 sw_last=c0\n\
         <region> sample=soft1.wav key=74 sw_last=d1\n\
         <region> sample=soft1.wav key=75 lovel=100 hivel=20\n\
         <region> sample=*sine key=76\n\
         <region> sample=soft1.wav key=77\n\
         <region> sample=empty.wav key=78\n",
    );
    assert_eq!(report.sampled_keys().collect::<Vec<_>>(), [25, 74, 77]);
    assert_eq!(report.keyswitches.keys().collect::<Vec<_>>(), [24, 25, 26]);
    assert_eq!((report.regions, report.playable_regions), (11, 10));
    // A key of -1 from the scope is overridden by nothing: the same patch, loaded, agrees.
    let bank = load_patch(
        &dir,
        &fs::read_to_string(dir.path().join("patch.sfz")).expect("read"),
    );
    assert_eq!(report.playable_regions, bank.regions.len());
}

#[test]
fn inspect_lists_the_samples_the_sampler_cannot_play() {
    // TST-03: a missing file and a generator leave out their regions, as when loading; a file
    // that is there but cannot be read fails the load, but is a note in the report, so a library
    // scan still sees the rest of the patch.
    let dir = library();
    let samples = dir.path().join("Samples");
    fs::write(samples.join("bad.wav"), b"RIFF not really a wave file").expect("write");
    let mut three = wav_header(3, 2, 10);
    three.extend_from_slice(&[0; 60]);
    fs::write(samples.join("three.wav"), three).expect("write");
    let text = "<control> default_path=Samples/\n\
                <region> sample=soft1.wav lokey=c3 hikey=c5\n\
                <region> sample=bad.wav key=c6\n\
                <region> sample=absent.wav key=d6\n\
                <region> sample=*saw key=e6\n\
                <region> sample=three.wav key=f6\n\
                <region> sample=bad.wav key=g6\n";
    let report = inspect_text(&dir, text);
    assert_eq!((report.regions, report.playable_regions), (6, 1));
    assert_eq!(report.missing_samples, [samples.join("absent.wav")]);
    assert_eq!(
        report.unreadable_samples,
        [samples.join("bad.wav"), samples.join("three.wav")]
    );
    assert_eq!(report.notes.len(), 4, "{:?}", report.notes);
    let bad = format!(
        "sample {} cannot be read: ",
        samples.join("bad.wav").display()
    );
    assert!(
        report.notes[0].starts_with(&bad) && report.notes[0].ends_with("(2 regions left out)"),
        "{}",
        report.notes[0]
    );
    assert_eq!(
        report.notes[1],
        format!(
            "sample {} is missing (1 region left out)",
            samples.join("absent.wav").display()
        )
    );
    assert_eq!(
        report.notes[2],
        "the sampler does not play the generator *saw (1 region left out)"
    );
    assert!(
        report.notes[3].contains("mono or stereo samples, this has 3 channels"),
        "{}",
        report.notes[3]
    );
    assert_eq!(
        report.decoded_bytes,
        4_410 * 4,
        "only the file that was read"
    );
    assert_eq!(
        report.sampled_keys().collect::<Vec<_>>(),
        (48..=72).collect::<Vec<_>>()
    );
    assert!(
        !report.is_loadable(),
        "the loader fails on the unreadable files"
    );
    assert!(load_sfz_text(&dir, text).is_err());
}

#[test]
fn inspect_reports_a_patch_without_regions() {
    // TST-03: the loader refuses a patch with no region, or none left; the report says why.
    let dir = library();
    let report = inspect_text(&dir, "<group> lokey=c3 hikey=c5\n");
    assert_eq!((report.regions, report.playable_regions), (0, 0));
    assert_eq!(report.sampled, []);
    assert!(!report.is_loadable());
    assert!(matches!(
        load_sfz_text(&dir, "<group> lokey=c3 hikey=c5\n"),
        Err(MediaError::Malformed { .. })
    ));
    let report = inspect_text(&dir, "<region> sample=Samples/absent.wav\n");
    assert_eq!((report.regions, report.playable_regions), (1, 0));
    assert_eq!(
        report.missing_samples,
        [dir.path().join("Samples/absent.wav")]
    );
    assert!(!report.is_loadable());
}

#[test]
fn inspect_counts_the_memory_of_a_patch_over_the_cap() {
    // An 8-bit mono file one frame over the cap once decoded to 4-byte samples, written sparse:
    // the report counts it from the header, where the loader refuses the patch.
    let dir = library();
    let frames = u32::try_from(MAX_BANK_BYTES / 4 + 1).expect("fits a WAV header");
    let path = dir.path().join("Samples/huge.wav");
    fs::write(&path, wav_header(1, 1, frames)).expect("write");
    let file = fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("open");
    file.set_len(44 + u64::from(frames)).expect("extend");
    let text = "<region> sample=Samples/huge.wav\n";
    let report = inspect_text(&dir, text);
    assert_eq!(report.decoded_bytes, MAX_BANK_BYTES + 4);
    assert_eq!(report.playable_regions, 1);
    assert!(!report.is_loadable());
    let result = load_sfz_text(&dir, text);
    assert!(
        matches!(&result, Err(MediaError::Malformed { message, .. })
            if message == "the samples need more than 3072 MiB of memory"),
        "{result:?}"
    );
}

#[test]
fn inspect_fails_only_when_the_patch_cannot_be_read() {
    // TST-03: the patch's own errors, as when loading.
    let dir = library();
    assert!(matches!(
        inspect(&dir.path().join("nothing.sfz")),
        Err(MediaError::Io {
            operation: "read",
            ..
        })
    ));
    let path = dir.path().join("patch.sfz");
    fs::write(&path, "#include \"missing.txt\"\n").expect("write");
    assert!(matches!(inspect(&path), Err(MediaError::Io { .. })));
    fs::write(&path, "#include \"patch.sfz\"\n").expect("write");
    assert!(matches!(
        inspect(&path),
        Err(MediaError::Malformed { message, .. }) if message == "#include nesting is too deep"
    ));
}
