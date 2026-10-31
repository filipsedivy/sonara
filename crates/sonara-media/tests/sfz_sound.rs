//! What `sfz::measure` finds in the decoded samples of a patch: the onset of each region a key
//! press starts and the longest a held note lasts. The samples are generated WAV files of known
//! onsets and lengths (a tempdir: no licensed samples are needed); at 44.1 kHz, 441 frames are
//! 10 ms.

use std::fs;
use std::path::Path;
use std::time::Duration;

use sonara_dsp::bank::{Region, Sample, SampleBank};
use sonara_media::sfz::{ONSET_LEVEL_DB, RegionOnset, SoundReport, load_sfz, measure};

const RATE: u32 = 44_100;

/// Writes a 16-bit PCM WAV at 44.1 kHz whose frames are given as runs of `(frames, level)`, the
/// same level on every channel unless `right_only` puts it on the right channel of a stereo file.
fn wav(path: &Path, channels: u16, runs: &[(u32, i16)], right_only: bool) {
    let frames: u32 = runs.iter().map(|(count, _)| count).sum();
    let block = u32::from(channels) * 2;
    let data_len = frames * block;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&RATE.to_le_bytes());
    bytes.extend_from_slice(&(RATE * block).to_le_bytes());
    bytes.extend_from_slice(&u16::try_from(block).expect("a small block").to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for &(count, level) in runs {
        for _ in 0..count {
            for channel in 0..channels {
                let value = if right_only && channel == 0 { 0 } else { level };
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    fs::write(path, bytes).expect("write the wav");
}

/// A mono sample of `silent` frames of silence, then `loud` frames at full scale.
fn step(path: &Path, silent: u32, loud: u32) {
    wav(path, 1, &[(silent, 0), (loud, i16::MAX)], false);
}

/// Writes `text` as `patch.sfz` in `dir`, loads it and measures it.
fn measure_text(dir: &Path, text: &str) -> SoundReport {
    let path = dir.join("patch.sfz");
    fs::write(&path, text).expect("write the patch");
    measure(&load_sfz(&path).expect("the patch loads"))
}

/// The onsets of `report` in milliseconds, to the microsecond.
fn onsets_ms(report: &SoundReport) -> Vec<f64> {
    report.onsets.iter().map(|onset| ms(onset.onset)).collect()
}

fn ms(duration: Duration) -> f64 {
    (duration.as_secs_f64() * 1e6).round() / 1e3
}

/// The time `frames` frames take at 44.1 kHz, in milliseconds, to the microsecond.
fn frames_ms(frames: u32) -> f64 {
    (f64::from(frames) / f64::from(RATE) * 1e6).round() / 1e3
}

/// The onset is the first frame of the played part that reaches the level, counted from the
/// region's `offset`, plus its `ampeg_attack`; each region keeps the switch and velocities that
/// select it (TST-10: generated samples of known onset).
#[test]
fn measures_each_onset_from_the_offset_with_the_attack() {
    let dir = tempfile::tempdir().expect("tempdir");
    step(&dir.path().join("step.wav"), 441, 4_410);
    let report = measure_text(
        dir.path(),
        "<global> sw_lokey=24 sw_hikey=25 sw_default=24\n\
         <region> sample=step.wav key=60 sw_last=24 lovel=1 hivel=63\n\
         <region> sample=step.wav key=60 sw_last=24 lovel=64 offset=220\n\
         <region> sample=step.wav key=61 sw_last=25 ampeg_attack=0.02\n\
         <region> sample=step.wav key=62 offset=1000\n",
    );
    assert_eq!(
        onsets_ms(&report),
        [10.0, frames_ms(221), 30.0, 0.0],
        "a step after 441 frames is heard 10 ms in; an offset past it is heard at once"
    );
    let selectors: Vec<_> = report
        .onsets
        .iter()
        .map(|onset| (onset.switch, onset.vel_lo, onset.vel_hi))
        .collect();
    assert_eq!(
        selectors,
        [
            (Some(24), 1, 63),
            (Some(24), 64, 127),
            (Some(25), 0, 127),
            (None, 0, 127)
        ]
    );
}

/// The level is relative to the region's peak: a frame 19.2 dB below it starts the note, one
/// 20.9 dB below it does not, and a sample a hundred times quieter has the same onset, as a soft
/// velocity layer recorded quietly must (TST-10).
#[test]
fn the_onset_level_is_twenty_db_below_the_peak() {
    assert!((ONSET_LEVEL_DB + 20.0).abs() < f64::EPSILON);
    let dir = tempfile::tempdir().expect("tempdir");
    // 900 and 1100 lie 20.9 and 19.2 dB below the peak of 10000; 9 and 11 below 100 alike.
    wav(
        &dir.path().join("loud.wav"),
        1,
        &[(441, 900), (441, 1_100), (441, 10_000)],
        false,
    );
    wav(
        &dir.path().join("quiet.wav"),
        1,
        &[(441, 9), (441, 11), (441, 100)],
        false,
    );
    let report = measure_text(
        dir.path(),
        "<region> sample=loud.wav key=60\n<region> sample=quiet.wav key=61\n",
    );
    assert_eq!(onsets_ms(&report), [10.0, 10.0]);
}

/// The onset is timed at the speed the sampler plays the region: at its key centre with its
/// `tune`, or at the key of its range nearest to the centre (TST-10).
#[test]
fn the_onset_follows_the_transposition_of_the_region() {
    let dir = tempfile::tempdir().expect("tempdir");
    step(&dir.path().join("step.wav"), 882, 441);
    let report = measure_text(
        dir.path(),
        "<region> sample=step.wav key=60 tune=1200\n\
         <region> sample=step.wav lokey=72 hikey=84 pitch_keycenter=60\n\
         <region> sample=step.wav lokey=40 hikey=50 pitch_keycenter=62\n",
    );
    // An octave up plays twice as fast: 20 ms of sample in 10 ms. Key 50 lies 12 semitones
    // below the centre 62: twice as slow.
    assert_eq!(onsets_ms(&report), [10.0, 10.0, 40.0]);
}

/// A stereo sample starts with whichever channel first reaches the level (TST-10).
#[test]
fn a_stereo_onset_is_heard_on_either_channel() {
    let dir = tempfile::tempdir().expect("tempdir");
    wav(
        &dir.path().join("right.wav"),
        2,
        &[(441, 0), (441, i16::MAX)],
        true,
    );
    let report = measure_text(dir.path(), "<region> sample=right.wav key=60\n");
    assert_eq!(onsets_ms(&report), [10.0]);
}

/// Only the regions a press starts with no other key down have an onset: releases, legato
/// transitions, regions no key starts, regions under a switch the sampler cannot reach, silent
/// samples and `*silence` have none (TST-03).
#[test]
fn only_the_attacks_a_press_sounds_have_an_onset() {
    let dir = tempfile::tempdir().expect("tempdir");
    step(&dir.path().join("step.wav"), 441, 441);
    wav(&dir.path().join("silent.wav"), 1, &[(441, 0)], false);
    let report = measure_text(
        dir.path(),
        "<global> sw_lokey=24 sw_hikey=24\n\
         <region> sample=step.wav key=60 trigger=first\n\
         <region> sample=step.wav key=60 trigger=legato\n\
         <region> sample=step.wav key=60 trigger=release\n\
         <region> sample=step.wav key=60 trigger=release_key\n\
         <region> sample=step.wav key=-1 on_locc1=0 on_hicc1=127\n\
         <region> sample=step.wav key=61 sw_last=30\n\
         <region> sample=step.wav key=61 lovel=90 hivel=10\n\
         <region> sample=silent.wav key=62\n\
         <region> sample=*silence key=63 group=1\n",
    );
    assert_eq!(
        report.onsets,
        [RegionOnset {
            switch: None,
            vel_lo: 0,
            vel_hi: 127,
            onset: report.onsets[0].onset,
        }]
    );
    assert_eq!(onsets_ms(&report), [10.0]);
}

/// The longest hold is the longest region a press starts, from its offset to its end, at its
/// lowest key; releases do not count, legato regions do, and a one-shot plays to its end
/// whatever the key does (TST-10: generated samples of known length).
#[test]
fn the_longest_hold_is_the_longest_unlooped_region() {
    let dir = tempfile::tempdir().expect("tempdir");
    step(&dir.path().join("short.wav"), 0, 4_410);
    step(&dir.path().join("long.wav"), 0, 44_100);
    let hold = |text: &str| measure_text(dir.path(), text).max_hold.map(ms);

    assert_eq!(hold("<region> sample=short.wav key=60\n"), Some(100.0));
    // Offset and end shorten it; `end` names the last frame played.
    assert_eq!(
        hold("<region> sample=short.wav key=60 offset=441 end=2204\n"),
        Some(frames_ms(1_764))
    );
    // Twelve semitones below the key centre the sample plays at half speed.
    assert_eq!(
        hold("<region> sample=short.wav lokey=48 hikey=72 pitch_keycenter=60\n"),
        Some(200.0)
    );
    assert_eq!(
        hold(
            "<region> sample=short.wav key=60\n\
             <region> sample=long.wav key=60 trigger=release\n\
             <region> sample=long.wav key=-1 on_locc1=0 on_hicc1=127\n"
        ),
        Some(100.0),
        "a release or a controller-started region is no held note"
    );
    assert_eq!(
        hold(
            "<region> sample=short.wav key=60\n\
             <region> sample=long.wav key=61 trigger=legato loop_mode=one_shot\n"
        ),
        Some(1_000.0)
    );
}

/// A region that loops while its key is held sounds as long as the key is held, so the patch has
/// no longest hold; a loop the sampler does not play leaves the region unlooped (TST-07).
#[test]
fn a_loop_while_held_lifts_the_limit() {
    let dir = tempfile::tempdir().expect("tempdir");
    step(&dir.path().join("short.wav"), 0, 4_410);
    let hold = |text: &str| measure_text(dir.path(), text).max_hold;

    for mode in ["loop_continuous", "loop_sustain"] {
        assert_eq!(
            hold(&format!(
                "<region> sample=short.wav key=60\n\
                 <region> sample=short.wav key=61 loop_mode={mode} loop_start=100 loop_end=999\n"
            )),
            None,
            "{mode}"
        );
    }
    assert_eq!(
        hold(
            "<region> sample=short.wav key=60 loop_mode=loop_continuous loop_start=900 \
             loop_end=100\n"
        )
        .map(ms),
        Some(100.0),
        "a loop that ends before it starts does not play"
    );
    assert_eq!(
        hold("<region> sample=short.wav key=60 trigger=release loop_mode=loop_continuous\n"),
        None,
        "no region sounds on a press"
    );
    assert_eq!(
        hold("<region> sample=short.wav key=60 offset=5000\n"),
        None,
        "a region that starts past its end plays no frame"
    );
}

/// A sample the sampler cannot time gives no onset and no hold, and nothing panics (TST-03).
#[test]
fn a_sample_that_cannot_be_timed_gives_nothing() {
    let region = Region {
        key_lo: 60,
        key_hi: 60,
        ..Region::default()
    };
    let loud = vec![1.0_f32; 64];
    for sample in [
        Sample {
            rate: 0,
            channels: 1,
            data: loud.clone(),
        },
        Sample {
            rate: RATE,
            channels: 0,
            data: loud.clone(),
        },
    ] {
        let bank = SampleBank {
            samples: vec![sample],
            regions: vec![region.clone()],
            ..SampleBank::default()
        };
        assert_eq!(
            measure(&bank),
            SoundReport {
                onsets: Vec::new(),
                max_hold: None,
            }
        );
    }
    let bank = SampleBank {
        samples: vec![Sample {
            rate: RATE,
            channels: 1,
            data: loud,
        }],
        regions: vec![Region {
            tune_cents: f64::NAN,
            ..region
        }],
        ..SampleBank::default()
    };
    assert_eq!(measure(&bank).onsets, []);
    assert_eq!(measure(&bank).max_hold, None);
}
