//! SON-P03: a seek, a pause with a seek, and a stop end the notes that sound, observed as
//! the player's output through the manual device (TST-04, TST-10).
//!
//! The project is one synth track holding a single note, 16 beats at 120 BPM from beat 1 to
//! beat 15 (the golden `play-position` project with its notes replaced; the golden data is
//! not touched, GLD-01). The oracle is the level of the blocks written after the move:
//! a position inside the note sounds (a retriggered chase), a position after it is silent,
//! which only an all-notes-off at the discontinuity achieves for a voice that is still held.

use serde_json::json;
use sonara_engine::player::{PlayerSettings, PlayerState};
use sonara_testkit::Golden;

use crate::support::{Harness, Played, golden_project, no_media, tracks};

/// The synth track that holds the note.
const SYNTH: usize = 1;
/// After the note: beat 15 is its end.
const AFTER_THE_NOTE: f64 = 15.5;
/// Inside the note.
const INSIDE_THE_NOTE: f64 = 8.0;
/// −100 dBFS, the level the renderer's `auto` tail treats as silence (docs/audio-engine.md).
const SILENCE: f32 = 1.0e-5;
/// Blocks of 512 frames that a released voice and the 128-frame flush fade may take.
const SETTLE_BLOCKS: usize = 8;

fn harness() -> Harness {
    let golden = Golden::open().expect("golden data");
    let project = golden_project(&golden, "transport/play-position/project.json", |data| {
        let tracks = tracks(data);
        for (index, track) in tracks.iter_mut().enumerate() {
            let notes = if index == SYNTH {
                json!([{"id": "n_900000000001", "pitch": 60, "start": 1.0, "length": 14.0,
                        "velocity": 110, "channel": 0}])
            } else {
                json!([])
            };
            track["clips"][0]["notes"] = notes;
        }
    });
    Harness::new(project, no_media(), PlayerSettings::default())
}

fn peak(blocks: &[Played]) -> f32 {
    blocks
        .iter()
        .flat_map(|block| block.samples.iter())
        .fold(0.0, |peak, sample| peak.max(sample.abs()))
}

/// Renders until `blocks` more blocks of `generation` exist; returns them.
fn render_generation(h: &mut Harness, generation: u32, blocks: usize) -> Vec<Played> {
    let have = |h: &Harness| {
        h.played
            .iter()
            .filter(|p| p.generation == generation)
            .count()
    };
    while have(h) < blocks {
        h.render(1);
    }
    h.played
        .iter()
        .filter(|p| p.generation == generation)
        .cloned()
        .collect()
}

/// The blocks of `generation` after the settling time: the steady level of the new position.
fn settled(h: &mut Harness, generation: u32) -> Vec<Played> {
    let blocks = render_generation(h, generation, SETTLE_BLOCKS + 16);
    blocks[SETTLE_BLOCKS..].to_vec()
}

fn generation(h: &Harness) -> u32 {
    h.played.last().expect("a block").generation
}

/// A note is held while playing; a seek past its end releases it, and a seek inside it keeps
/// it sounding (the control, so that the silence is not just the flush).
#[test]
fn a_seek_past_the_note_ends_the_held_voice() {
    let mut h = harness();
    let snapshot = h.snapshot.clone();
    h.player
        .play(&snapshot, Some(4.0), None, false)
        .expect("play");
    h.render(1);
    let started = generation(&h);
    let playing = settled(&mut h, started);
    assert!(peak(&playing) > 0.01, "the note sounds before the seek");

    let _ = h.player.seek(&snapshot.project, AFTER_THE_NOTE);
    h.render(1);
    let moved = generation(&h);
    let after = settled(&mut h, moved);
    assert!(
        peak(&after) < SILENCE,
        "the voice stuck after the seek: peak {}",
        peak(&after)
    );
    h.render(32);
    assert_eq!(h.control().stats().underruns, 0);
}

#[test]
fn a_seek_inside_the_note_keeps_it_sounding() {
    let mut h = harness();
    let snapshot = h.snapshot.clone();
    h.player
        .play(&snapshot, Some(4.0), None, false)
        .expect("play");
    h.render(4);
    let _ = h.player.seek(&snapshot.project, INSIDE_THE_NOTE);
    h.render(1);
    let moved = generation(&h);
    let after = settled(&mut h, moved);
    assert!(peak(&after) > 0.01, "the chase retriggers the held note");
}

/// A pause halts the stream and a resume is a play, which builds a fresh graph (a seek alone
/// keeps it and chases); a seek while paused and the resume past the note's end leave no
/// voice of the note paused in.
#[test]
fn a_resume_past_the_note_leaves_no_voice() {
    let mut h = harness();
    let snapshot = h.snapshot.clone();
    h.player
        .play(&snapshot, Some(4.0), None, false)
        .expect("play");
    h.render(16);
    let paused = h.player.pause(Some(&snapshot.project));
    assert!(paused.paused);
    assert_eq!(h.player.state(), PlayerState::Paused);
    let _ = h.player.seek(&snapshot.project, AFTER_THE_NOTE);
    h.player.play(&snapshot, None, None, false).expect("resume");
    h.render(1);
    let resumed = generation(&h);
    let after = settled(&mut h, resumed);
    assert!(
        peak(&after) < SILENCE,
        "the voice stuck after the resume: peak {}",
        peak(&after)
    );
}

/// A stop silences the device at once and leaves no voice for the next play.
#[test]
fn a_stop_ends_the_note_and_the_next_play_starts_clean() {
    let mut h = harness();
    let snapshot = h.snapshot.clone();
    h.player
        .play(&snapshot, Some(4.0), None, false)
        .expect("play");
    h.render(16);
    let stopped = h.player.stop(Some(&snapshot.project));
    assert!(stopped.was_playing);
    assert_eq!(h.player.state(), PlayerState::Stopped);
    // The device drains only the 128-frame fade, then nothing: the ring was flushed.
    let _ = h.callback(256);
    let drained = h.callback(1024);
    assert!(
        drained.iter().all(|s| s.abs() < SILENCE),
        "the device keeps playing after the stop"
    );

    h.player
        .play(&snapshot, Some(AFTER_THE_NOTE), None, false)
        .expect("play");
    h.render(1);
    let started = generation(&h);
    let after = settled(&mut h, started);
    assert!(
        peak(&after) < SILENCE,
        "a voice survived the stop: peak {}",
        peak(&after)
    );
}
