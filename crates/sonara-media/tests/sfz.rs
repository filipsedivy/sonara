//! The SFZ loader against a generated library (a tempdir: no licensed samples are needed).
//! The fixture is a two-layer, two-round-robin instrument with a release sample and keyswitches,
//! written with a macro-free `#include`, `#define` and `default_path` the way real libraries do.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use sonara_core::model::device::Params;
use sonara_core::plugins::sampler_descriptor;
use sonara_dsp::bank::{Keyswitches, LoopMode, SampleBank, Trigger};
use sonara_dsp::processor::{BlockContext, DeviceState, EventKind, InstrumentEvent, ProcessSpec};
use sonara_dsp::processors::{create, param_slots};
use sonara_media::{MediaError, sfz::load_sfz};

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
    let descriptor = sampler_descriptor(&["/library/piano.sfz".to_owned()]).expect("a patch");
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
    let descriptor = sampler_descriptor(&["/library/patch.sfz".to_owned()]).expect("a patch");
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
