//! Recording on a manual player: the capture start of D4, the take file the
//! writer thread fills, and the frames recorded MIDI is heard at.

use serde_json::json;
use sonara_engine::player::{PlayerSettings, RecordPlan, Step, TakeTarget};
use sonara_media::export::SampleFormat;
use sonara_rt::{MidiEvent, midi_channel};
use sonara_testkit::Golden;

use crate::support::{Harness, golden_project, no_media, tracks};

/// The golden recording project: "Vocal" (an armed audio track on input 0) and "Keys" (a synth),
/// 150 BPM in 3/4 with a one-bar count-in.
fn recording_harness(edit: impl FnOnce(&mut sonara_core::model::JsonMap)) -> Harness {
    let golden = Golden::open().expect("golden data");
    let project = golden_project(&golden, "transport/record-count-in/project.json", edit);
    Harness::new(project, no_media(), PlayerSettings::default())
}

/// The input of device frame `index` on input channel 0: a ramp that `PCM_24` stores exactly as
/// `8 · index`, so a take's first sample names the device frame it started at.
#[expect(
    clippy::cast_precision_loss,
    reason = "the test's frame indices stay below 2^20"
)]
fn ramp(index: u64) -> f32 {
    index as f32 / 1_048_576.0
}

/// Steps the render thread until `blocks` more blocks are written; whenever it waits, the
/// device runs one duplex callback of 256 frames with the ramp on input 0 (input 1 silent).
/// `frames` counts the device frames so far; returns the first device frame of the current
/// generation that a callback played, if one did.
fn render_duplex(h: &mut Harness, blocks: usize, frames: &mut u64) -> Option<u64> {
    let target = h.played.len() + blocks;
    let mut first = None;
    while h.played.len() < target {
        if h.step() == Step::Wrote {
            continue;
        }
        let input: Vec<f32> = (*frames..*frames + 256)
            .flat_map(|i| [ramp(i), 0.0])
            .collect();
        let fill = h.duplex(&input, 2);
        if let Some(current) = fill.first_current() {
            first.get_or_insert(*frames + current.offset as u64);
        }
        *frames += 256;
        h.player.service();
    }
    first
}

/// The 24-bit samples of a mono WAV take written by the take writer (after its 44-byte header).
fn take_samples(path: &std::path::Path) -> Vec<i32> {
    let bytes = std::fs::read(path).expect("the take file");
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(&bytes[36..40], b"data");
    let size: [u8; 4] = bytes[40..44].try_into().expect("a data size");
    let data = usize::try_from(u32::from_le_bytes(size)).expect("a short take");
    bytes[44..44 + data]
        .as_chunks::<3>()
        .0
        .iter()
        .map(|b| i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8)
        .collect()
}

/// A take starts at the first frame of the recording's generation that the device plays,
/// with that frame's timeline position, even after an earlier play left the device's position
/// far away; the input before it is not in the take. The prototype stamped the take with the
/// position at the end of the previous callback: the old playhead (here beat 12 and later), so
/// its trim cut real audio or misplaced the clip.
#[test]
fn capture_start_alignment() {
    let mut h = recording_harness(|_| {});
    let snapshot = h.snapshot.clone();
    let project = snapshot.project.clone();
    assert_eq!(h.player.open_stream(&project, Some(2)).expect("duplex"), 2);
    let mut frames = 0;
    h.player
        .play(&snapshot, Some(12.0), None, false)
        .expect("play");
    render_duplex(&mut h, 20, &mut frames);
    let _ = h.player.stop(Some(&project));
    let old_playhead = h.control().position().expect("a position").get();
    assert!(
        old_playhead > 200_000,
        "the play left the playhead at {old_playhead}"
    );

    let path = h.scratch.path().join("vocal-take.wav");
    let vocal = project.tracks[0].id.clone();
    let position = h
        .player
        .record(
            &snapshot,
            RecordPlan {
                start_beat: Some(3.0),
                takes: vec![TakeTarget {
                    track_id: vocal.clone(),
                    path: path.clone(),
                    channels: vec![0],
                    format: SampleFormat::Pcm24,
                }],
                midi_tracks: Vec::new(),
            },
        )
        .expect("record");
    // The one-bar count-in in 3/4 starts the transport at beat 0.
    assert_eq!(position.frame(), 0);
    let recording_from = frames;
    let first = render_duplex(&mut h, 60, &mut frames).expect("the take started");
    assert!(first >= recording_from);
    let stopped = h.player.stop(Some(&project));
    assert!(stopped.was_playing);
    let done = stopped.recording.expect("the takes");
    assert_eq!(done.record_from, 57_600, "beat 3 at 150 BPM");
    assert_eq!(done.capture_start, Some(0), "the count-in's first frame");
    assert_eq!(done.latency_frames + done.input_latency_frames, 1_536);
    assert_eq!(done.capture_overruns, 0);
    assert_eq!(done.takes.len(), 1);
    let take = &done.takes[0];
    assert_eq!(
        (take.track_id.as_str(), take.error.as_deref()),
        (vocal.as_str(), None)
    );
    // Every device frame from the take's start on is in the file, beginning with the frame
    // that played the count-in's first frame.
    assert_eq!(take.frames, frames - first);
    let samples = take_samples(&path);
    assert_eq!(samples.len() as u64, take.frames);
    assert_eq!(i64::from(samples[0]), 8 * i64::try_from(first).unwrap());
    assert!(
        samples.windows(2).all(|pair| pair[1] - pair[0] == 8),
        "the take is the ramp without gaps"
    );
    assert!(done.writer.drains > 0 && done.writer.frames == take.frames);
    assert!(!h.player.is_recording());
}

/// A second recording on the same stream takes over its capture, and an earlier take's frames
/// do not leak into it.
#[test]
fn a_second_take_starts_clean_on_the_same_stream() {
    let mut h = recording_harness(|_| {});
    let snapshot = h.snapshot.clone();
    let project = snapshot.project.clone();
    h.player.open_stream(&project, Some(2)).expect("duplex");
    let mut frames = 0;
    let vocal = project.tracks[0].id.clone();
    let plan = |name: &str, h: &Harness| RecordPlan {
        start_beat: Some(6.0),
        takes: vec![TakeTarget {
            track_id: vocal.clone(),
            path: h.scratch.path().join(name),
            channels: vec![0],
            format: SampleFormat::Pcm24,
        }],
        midi_tracks: Vec::new(),
    };
    let first_plan = plan("one.wav", &h);
    h.player.record(&snapshot, first_plan).expect("record");
    render_duplex(&mut h, 20, &mut frames);
    let one = h.player.stop(Some(&project)).recording.expect("take one");
    let second_plan = plan("two.wav", &h);
    let two_path = second_plan.takes[0].path.clone();
    h.player
        .record(&snapshot, second_plan)
        .expect("record again");
    let first = render_duplex(&mut h, 20, &mut frames).expect("take two started");
    let two = h.player.stop(Some(&project)).recording.expect("take two");
    assert!(one.takes[0].frames > 0);
    assert_eq!(two.takes[0].frames, frames - first);
    assert_eq!(
        i64::from(take_samples(&two_path)[0]),
        8 * i64::try_from(first).unwrap()
    );
    // Beat 6 with one bar of 3/4 before it: the transport starts at beat 3.
    assert_eq!(two.capture_start, Some(57_600));
}

/// `_poll_midi`'s frames: each input's newest message of a drain is heard at the device's
/// software position minus the output latency, its earlier messages by their difference in
/// arrival time, rounded half to even. Inputs are anchored separately (their clocks need not
/// agree), and only messages with a target are recorded.
#[test]
fn recorded_midi_is_anchored_like_the_prototype() {
    let mut h = recording_harness(|data| {
        // Keys becomes the armed instrument track: the MIDI take's target.
        tracks(data)[1]["input"]["armed"] = json!(true);
        tracks(data)[0]["input"]["armed"] = json!(false);
    });
    let snapshot = h.snapshot.clone();
    let project = snapshot.project.clone();
    let keys = project.tracks[1].id.clone();
    let (mut keyboard, keyboard_in) = midi_channel(64);
    let (mut pads, pads_in) = midi_channel(64);
    h.player
        .add_midi_inputs(vec![(0, keyboard_in), (1, pads_in)], Some(&snapshot))
        .expect("live input");
    h.player
        .record(
            &snapshot,
            RecordPlan {
                start_beat: Some(3.0),
                takes: Vec::new(),
                midi_tracks: vec![keys],
            },
        )
        .expect("record");
    let mut frames = 0;
    render_duplex(&mut h, 30, &mut frames);
    let note = |time, bytes: &[u8], port| MidiEvent::new(time, port, bytes).expect("a message");
    // 0.5 s and 0.25 s before the keyboard's newest message, and 1 µs before that.
    assert!(keyboard.push(note(1_000_000, &[0x90, 60, 100], 0)));
    assert!(keyboard.push(note(1_250_000, &[0xB0, 64, 127], 0)));
    assert!(keyboard.push(note(1_499_999, &[0xE0, 0x00, 0x50], 0)));
    assert!(keyboard.push(note(1_500_000, &[0x80, 60, 0], 0)));
    // The pads' clock starts elsewhere: its newest message is anchored on its own.
    assert!(pads.push(note(7, &[0x99, 36, 90], 1)));
    let heard_now = h.control().position().expect("a position").get() - 768;
    // One step drains both inputs at this position (no callback in between).
    while h.step() == Step::Wrote {}
    let done = h
        .player
        .stop(Some(&project))
        .recording
        .expect("the MIDI take");
    let heard: Vec<(i64, u8)> = done
        .midi_events
        .iter()
        .map(|(frame, event)| (*frame, event.status()))
        .collect();
    assert_eq!(
        heard,
        [
            // Merged by arrival time: the pads' message first.
            (heard_now, 0x99),
            (heard_now - 24_000, 0x90),
            (heard_now - 12_000, 0xB0),
            // 1 µs is 0.048 frames: rounds to 0.
            (heard_now, 0xE0),
            (heard_now, 0x80),
        ]
    );
    assert!(done.takes.is_empty() && done.capture_start.is_none());
}
