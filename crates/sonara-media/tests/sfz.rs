//! The SFZ loader against a generated library (a tempdir: no licensed samples are needed).
//! The fixture is a two-layer, two-round-robin instrument with a release sample and keyswitches,
//! written with a macro-free `#include`, `#define` and `default_path` the way real libraries do.

use std::fs;
use std::path::Path;

use sonara_dsp::bank::{Keyswitches, LoopMode, SampleBank, Trigger};
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
    let path = dir.path().join("patch.sfz");
    fs::write(&path, text).expect("write");
    load_sfz(&path).expect("loads")
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
    fs::write(
        dir.path().join("broken.sfz"),
        "<region> sample=Samples/absent.wav\n",
    )
    .expect("write");
    assert!(matches!(
        load_sfz(&dir.path().join("broken.sfz")),
        Err(MediaError::Io { .. })
    ));
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
