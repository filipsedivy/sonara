//! Standard MIDI File export and import (ports of the SMF tests of `tests/unit/test_midi.py`,
//! plus the edge cases of `sonara.midi.smf`).

use std::path::PathBuf;
use std::sync::Arc;

use midly::num::{u4, u7, u15, u24, u28};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
use sonara_core::error::ErrorCode;
use sonara_core::ids::SequentialIds;
use sonara_core::midi::{ControlKind, MidiControlEvent, MidiNote};
use sonara_core::model::clip::{Clip, MidiClip, MidiClipData};
use sonara_core::model::device::{Device, Params};
use sonara_core::model::markers::{Marker, Section};
use sonara_core::model::project::{Project, ProjectData};
use sonara_core::model::track::{Track, TrackData, TrackKind};
use sonara_core::model::{JsonMap, Valid};
use sonara_core::time::{TempoMap, TempoPoint, TimeSignatureMap, TimeSignaturePoint};
use sonara_midi::smf::{
    ImportedMidi, PPQ, clip_from_imported, encode_smf, export_smf, import_smf, parse_smf,
};

// -- fixtures -----------------------------------------------------------------------------

fn note(id: &str, pitch: i64, start: f64, length: f64, velocity: i64) -> MidiNote {
    MidiNote::new(id, pitch, start, length, velocity, 0).expect("valid note")
}

fn midi_clip(
    id: &str,
    name: &str,
    start: f64,
    length: f64,
    notes: Vec<MidiNote>,
    controls: Vec<MidiControlEvent>,
) -> MidiClipData {
    MidiClipData {
        id: id.to_owned(),
        name: name.to_owned(),
        start,
        length,
        notes: Arc::new(notes),
        controls: Arc::new(controls),
        content_offset: 0.0,
        loop_length: None,
        muted: false,
        color: None,
        extra: JsonMap::new(),
    }
}

fn instrument(id: &str, name: &str, plugin: &str, clips: Vec<MidiClipData>) -> Track {
    let mut ids = SequentialIds::new(0);
    let mut data = TrackData::new(id, name, TrackKind::Instrument);
    data.clips = clips
        .into_iter()
        .map(|clip| Clip::Midi(MidiClip::new(clip).expect("valid clip")))
        .collect();
    data.instrument = Some(
        Device::create(&mut ids, plugin, "Instrument", Params::default(), None).expect("device"),
    );
    Valid::new(data).expect("valid track")
}

fn project_with(tracks: Vec<Track>) -> ProjectData {
    let mut data = ProjectData::new("prj_1", "SMF");
    data.tracks = tracks;
    data
}

/// The prototype test's `_smf_project`.
fn smf_project() -> Project {
    let keys = midi_clip(
        "clp_k",
        "Keys",
        4.0,
        8.0,
        vec![
            note("n_1", 60, 0.0, 1.0, 90),
            note("n_2", 64, 1.0 / 3.0, 0.5, 80),
            note("n_3", 67, 1.0 / 7.0, 2.25, 70),
        ],
        vec![
            MidiControlEvent::new(0.5, ControlKind::Cc, 64, 1, 0).expect("valid"),
            MidiControlEvent::new(1.0, ControlKind::PitchBend, -4096, 0, 0).expect("valid"),
        ],
    );
    let mut beat = midi_clip(
        "clp_d",
        "Beat",
        0.0,
        4.0,
        vec![
            note("n_4", 36, 0.0, 0.25, 120),
            note("n_5", 38, 1.0, 0.25, 110),
        ],
        vec![],
    );
    beat.loop_length = Some(2.0);
    let mut data = project_with(vec![
        instrument("trk_k", "Keys", "sonara.synth.poly", vec![keys]),
        instrument("trk_d", "Drums", "sonara.drums", vec![beat]),
        Valid::new(TrackData::new("trk_a", "Audio", TrackKind::Audio)).expect("audio track"),
    ]);
    data.tempo_map = TempoMap::new(vec![
        TempoPoint::new(0.0, 128.0, false).expect("tempo"),
        TempoPoint::new(8.0, 96.0, false).expect("tempo"),
    ])
    .expect("tempo map");
    data.time_signatures = TimeSignatureMap::new(vec![
        TimeSignaturePoint::new(0, 4, 4).expect("signature"),
        TimeSignaturePoint::new(2, 3, 4).expect("signature"),
    ])
    .expect("signature map");
    data.markers = vec![Marker::new("mrk_1", 4.0, "Verse", None).expect("marker")];
    data.sections = vec![Section::new("sec_1", "Intro", 0.0, 4.0, None).expect("section")];
    Valid::new(data).expect("valid project")
}

fn import(bytes: &[u8]) -> ImportedMidi {
    parse_smf(&mut SequentialIds::new(0), bytes, "test.mid").expect("valid file")
}

fn import_err(bytes: &[u8]) -> (ErrorCode, String) {
    let err = parse_smf(&mut SequentialIds::new(0), bytes, "test.mid").expect_err("invalid file");
    (err.code, err.message)
}

/// A file with `events` per track: `(delta, kind)` pairs; the end-of-track events are added.
fn file(format: Format, ppq: u16, tracks: &[Vec<(u32, TrackEventKind<'_>)>]) -> Vec<u8> {
    let tracks: Vec<Vec<TrackEvent<'_>>> = tracks
        .iter()
        .map(|events| {
            events
                .iter()
                .map(|(delta, kind)| TrackEvent {
                    delta: u28::new(*delta),
                    kind: *kind,
                })
                .chain([TrackEvent {
                    delta: u28::new(0),
                    kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
                }])
                .collect()
        })
        .collect();
    let smf = Smf {
        header: Header::new(format, Timing::Metrical(u15::new(ppq))),
        tracks,
    };
    let mut bytes = Vec::new();
    smf.write_std(&mut bytes).expect("writable");
    bytes
}

fn on(channel: u8, key: u8, vel: u8) -> TrackEventKind<'static> {
    TrackEventKind::Midi {
        channel: u4::new(channel),
        message: MidiMessage::NoteOn {
            key: u7::new(key),
            vel: u7::new(vel),
        },
    }
}

fn off(channel: u8, key: u8) -> TrackEventKind<'static> {
    TrackEventKind::Midi {
        channel: u4::new(channel),
        message: MidiMessage::NoteOff {
            key: u7::new(key),
            vel: u7::new(0),
        },
    }
}

fn name(text: &[u8]) -> TrackEventKind<'_> {
    TrackEventKind::Meta(MetaMessage::TrackName(text))
}

fn temp_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sonara-midi-{}-{test}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

// -- export -------------------------------------------------------------------------------

#[test]
fn round_trip_keeps_notes_tempo_signatures_and_markers() {
    let project = smf_project();
    let encoded = encode_smf(&project, None).expect("export");
    // The looped drum clip plays twice.
    assert_eq!((encoded.tracks, encoded.notes), (3, 3 + 4));
    assert!(encoded.max_rounding_error_beats <= 0.5 / f64::from(PPQ) + 1e-12);
    let raw = Smf::parse(&encoded.bytes).expect("a standard file");
    assert_eq!(raw.header.format, Format::Parallel);
    assert_eq!(raw.header.timing, Timing::Metrical(u15::new(PPQ)));

    let imported = import(&encoded.bytes);
    let tempos: Vec<(f64, f64)> = imported
        .tempo_map
        .points()
        .iter()
        .map(|p| (p.beat, (p.bpm * 1e6).round() / 1e6))
        .collect();
    assert_eq!(tempos, [(0.0, 128.0), (8.0, 96.0)]);
    let signatures: Vec<(i64, u32, u32)> = imported
        .time_signatures
        .points()
        .iter()
        .map(|p| (p.bar, p.numerator, p.denominator))
        .collect();
    assert_eq!(signatures, [(0, 4, 4), (2, 3, 4)]);
    let mut markers = imported.markers.clone();
    markers.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(
        markers,
        [(0.0, "Intro".to_owned()), (4.0, "Verse".to_owned())]
    );

    let keys = imported
        .tracks
        .iter()
        .find(|t| t.name == "Keys")
        .expect("keys");
    let drums = imported
        .tracks
        .iter()
        .find(|t| t.is_drums())
        .expect("drums");
    assert_ne!(keys.channel, 9);
    assert_eq!(drums.channel, 9);
    let ppq = f64::from(PPQ);
    let mut expected: Vec<(u8, f64, f64, u8)> = project
        .track("trk_k")
        .expect("track")
        .midi_clips()
        .flat_map(|c| c.notes.iter())
        .map(|n| (n.pitch, 4.0 + n.start, n.length, n.velocity))
        .collect();
    expected.sort_by_key(|a| a.0);
    let mut got: Vec<(u8, f64, f64, u8)> = keys
        .notes
        .iter()
        .map(|n| (n.pitch, n.start, n.length, n.velocity))
        .collect();
    got.sort_by_key(|a| a.0);
    assert_eq!(expected.len(), got.len());
    for (want, have) in expected.iter().zip(&got) {
        assert_eq!((want.0, want.3), (have.0, have.3));
        assert!(
            (want.1 - have.1).abs() <= 1.0 / ppq,
            "start {want:?} {have:?}"
        );
        assert!(
            (want.2 - have.2).abs() <= 2.0 / ppq,
            "length {want:?} {have:?}"
        );
    }
    let mut starts: Vec<f64> = drums.notes.iter().map(|n| n.start).collect();
    starts.sort_by(f64::total_cmp);
    assert_eq!(starts, [0.0, 1.0, 2.0, 3.0]);
    assert!(
        keys.controls
            .iter()
            .any(|c| c.kind == ControlKind::PitchBend && c.value == -4096)
    );
    assert!(
        keys.controls
            .iter()
            .any(|c| c.kind == ControlKind::Cc && c.value == 64 && c.number == 1)
    );

    let clip = clip_from_imported(&mut SequentialIds::new(0), keys, 0.0, None, None).expect("clip");
    assert_eq!(clip.notes.len(), 3);
    assert_eq!(clip.controls.len(), 2);
    assert!((clip.length % 4.0).abs() < f64::EPSILON && clip.length >= 8.0);
    assert_eq!(clip.name, "Keys");
}

#[test]
fn exporting_a_subset_and_a_ramp() {
    let project = smf_project();
    let ramped = project
        .edit(|d| {
            d.tempo_map = TempoMap::new(vec![
                TempoPoint::new(0.0, 100.0, true).expect("tempo"),
                TempoPoint::new(4.0, 140.0, false).expect("tempo"),
            ])
            .expect("tempo map");
        })
        .expect("valid project");
    let encoded = encode_smf(&ramped, Some(&["trk_k"])).expect("export");
    assert_eq!((encoded.tracks, encoded.notes), (2, 3));
    let imported = import(&encoded.bytes);
    let bpms: Vec<f64> = imported.tempo_map.points().iter().map(|p| p.bpm).collect();
    assert_eq!(bpms.len(), 17);
    assert!((bpms[16] / 140.0 - 1.0).abs() < 1e-5);
    assert!(bpms.windows(2).all(|w| w[0] < w[1]));
    // The stepped approximation keeps beats on time (within 1 ms) through and after the ramp.
    for beat in [1.0, 2.0, 4.0, 8.0] {
        let diff =
            imported.tempo_map.beat_to_seconds(beat) - ramped.tempo_map.beat_to_seconds(beat);
        assert!(diff.abs() < 1e-3, "beat {beat}: {diff}");
    }

    // A ramp on the last point has nothing to ramp to: it exports as a constant tempo.
    let last = project
        .edit(|d| {
            d.tempo_map = TempoMap::new(vec![
                TempoPoint::new(0.0, 90.0, false).expect("tempo"),
                TempoPoint::new(4.0, 110.0, true).expect("tempo"),
            ])
            .expect("tempo map");
        })
        .expect("valid project");
    let imported = import(&encode_smf(&last, None).expect("export").bytes);
    let bpms: Vec<f64> = imported
        .tempo_map
        .points()
        .iter()
        .map(|p| (p.bpm * 1000.0).round() / 1000.0)
        .collect();
    assert_eq!(bpms, [90.0, 110.0]);
}

#[test]
fn channels_skip_the_drum_channel_and_drums_keep_it() {
    let tracks: Vec<Track> = (0..11)
        .map(|i| {
            let plugin = if i == 3 {
                "sonara.drums"
            } else {
                "sonara.synth.poly"
            };
            instrument(
                &format!("trk_{i}"),
                &format!("T{i}"),
                plugin,
                vec![midi_clip(
                    &format!("clp_{i}"),
                    "c",
                    0.0,
                    4.0,
                    vec![note("n", 60, 0.0, 1.0, 100)],
                    vec![],
                )],
            )
        })
        .collect();
    let project = Valid::new(project_with(tracks)).expect("valid project");
    let bytes = encode_smf(&project, None).expect("export").bytes;
    let smf = Smf::parse(&bytes).expect("standard file");
    let channels: Vec<u8> = smf.tracks[1..]
        .iter()
        .map(|events| {
            events
                .iter()
                .find_map(|e| match e.kind {
                    TrackEventKind::Midi { channel, .. } => Some(channel.as_int()),
                    _ => None,
                })
                .expect("a note")
        })
        .collect();
    // The drum track (index 3) uses channel 10 without consuming a melodic channel; the
    // melodic tracks take 0..8, then skip 9.
    assert_eq!(channels, [0, 1, 2, 9, 3, 4, 5, 6, 7, 8, 10]);
}

#[test]
fn muted_clips_and_clip_ends_are_respected() {
    let mut muted = midi_clip(
        "clp_m",
        "m",
        0.0,
        4.0,
        vec![note("n_m", 60, 0.0, 1.0, 100)],
        vec![],
    );
    muted.muted = true;
    let cut = midi_clip(
        "clp_c",
        "cut",
        4.0,
        2.0,
        vec![
            note("n_c", 62, 1.5, 2.0, 100),
            note("n_o", 64, 3.0, 1.0, 100),
        ],
        vec![MidiControlEvent::new(3.0, ControlKind::ChannelPressure, 50, 0, 0).expect("valid")],
    );
    let project = Valid::new(project_with(vec![instrument(
        "trk_1",
        "T",
        "sonara.synth.poly",
        vec![muted, cut],
    )]))
    .expect("valid project");
    let encoded = encode_smf(&project, None).expect("export");
    // The note starting after the clip end is not played; the long one is cut at the clip end.
    assert_eq!(encoded.notes, 1);
    let smf = Smf::parse(&encoded.bytes).expect("standard file");
    let mut now = 0_u32;
    let mut seen = Vec::new();
    for event in &smf.tracks[1] {
        now += event.delta.as_int();
        if let TrackEventKind::Midi { message, .. } = event.kind {
            seen.push((now, message));
        }
    }
    let ppq = u32::from(PPQ);
    assert_eq!(
        seen.len(),
        2,
        "the note on and off, and no pressure event outside the clip: {seen:?}"
    );
    assert_eq!(seen[0].0, 5 * ppq + ppq / 2);
    assert_eq!(seen[1].0, 6 * ppq);
}

#[test]
fn very_short_notes_last_at_least_one_tick() {
    let clip = midi_clip(
        "clp_1",
        "c",
        0.0,
        4.0,
        vec![note("n", 60, 1.0, 1e-6, 100)],
        vec![],
    );
    let project = Valid::new(project_with(vec![instrument(
        "trk_1",
        "T",
        "sonara.synth.poly",
        vec![clip],
    )]))
    .expect("valid project");
    let imported = import(&encode_smf(&project, None).expect("export").bytes);
    let length = imported.tracks[0].notes[0].length;
    assert!((length - 1.0 / f64::from(PPQ)).abs() < 1e-12, "{length}");
}

#[test]
fn names_are_written_as_utf8_and_read_back() {
    let clip = midi_clip(
        "clp_1",
        "c",
        0.0,
        4.0,
        vec![note("n", 60, 0.0, 1.0, 100)],
        vec![],
    );
    let mut data = project_with(vec![instrument(
        "trk_1",
        "Ječící smyčce",
        "sonara.synth.poly",
        vec![clip],
    )]);
    data.name = "Strašidelný dům".to_owned();
    data.markers = vec![Marker::new("mrk_1", 1.0, "Žně – část", None).expect("marker")];
    let bytes = encode_smf(&Valid::new(data).expect("valid project"), None)
        .expect("export")
        .bytes;
    let utf8 = "Ječící smyčce".as_bytes();
    assert!(bytes.windows(utf8.len()).any(|w| w == utf8));
    let imported = import(&bytes);
    assert_eq!(imported.tracks[0].name, "Ječící smyčce");
    assert_eq!(imported.markers, [(1.0, "Žně – část".to_owned())]);
}

#[test]
fn export_and_import_work_on_files() {
    let dir = temp_dir("files");
    let path = dir.join("nested").join("song.mid");
    let exported = export_smf(&smf_project(), &path, None).expect("export");
    assert_eq!(exported.path, path.display().to_string());
    assert_eq!((exported.tracks, exported.notes), (3, 7));
    let imported = import_smf(&mut SequentialIds::new(0), &path).expect("import");
    assert_eq!(imported.tracks.len(), 2);
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

/// SON-P14: the exported bytes read back with `midly` as plain events, without the importer, and
/// agree with values known from the fixture: the header, the tempo of the first point
/// (128 BPM is 468 750 µs per quarter note) and the note-on count.
#[test]
fn exported_bytes_parse_as_a_type_1_file_with_the_fixtures_tempo_and_notes() {
    let encoded = encode_smf(&smf_project(), None).expect("encode");
    let smf = Smf::parse(&encoded.bytes).expect("a standard MIDI file");
    assert_eq!(smf.header.format, Format::Parallel);
    assert_eq!(smf.header.timing, Timing::Metrical(u15::new(PPQ)));
    assert_eq!(smf.tracks.len(), encoded.tracks);
    let first_tempo = smf
        .tracks
        .iter()
        .flatten()
        .find_map(|event| match event.kind {
            TrackEventKind::Meta(MetaMessage::Tempo(us)) => Some(us.as_int()),
            _ => None,
        })
        .expect("a tempo event");
    assert_eq!(first_tempo, 468_750);
    let note_ons = smf
        .tracks
        .iter()
        .flatten()
        .filter(|event| {
            matches!(
                event.kind,
                TrackEventKind::Midi { message: MidiMessage::NoteOn { vel, .. }, .. } if vel.as_int() > 0
            )
        })
        .count();
    // The looped drum clip (2 beats of 4) repeats its two notes, so 3 + 4 notes sound.
    assert_eq!(note_ons, encoded.notes);
    assert_eq!(note_ons, 7);
}

/// SON-P14: export failures are explicit errors with the documented codes, never a short file.
#[test]
fn export_failures_are_explicit() {
    // A position beyond the 28-bit delta time of the file format.
    let far = midi_clip(
        "clp_far",
        "Far",
        400_000.0,
        4.0,
        vec![note("n_f", 60, 0.0, 1.0, 90)],
        vec![],
    );
    let project = Valid::new(project_with(vec![instrument(
        "trk_f",
        "Far",
        "sonara.synth.poly",
        vec![far],
    )]))
    .expect("project");
    let err = encode_smf(&project, None).expect_err("a delta time that does not fit");
    assert_eq!(err.code, ErrorCode::LimitExceeded);

    // The target's parent is a file, so the directory cannot be created.
    let dir = temp_dir("export-io");
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, b"x").expect("a file");
    let err = export_smf(&smf_project(), &blocker.join("song.mid"), None).expect_err("io error");
    assert_eq!(err.code, ErrorCode::IoError);
    assert!(!blocker.join("song.mid").exists());
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

// -- import -------------------------------------------------------------------------------

#[test]
fn import_errors_use_the_prototype_codes_and_messages() {
    let missing = std::path::Path::new("/nonexistent/missing.mid");
    let err = import_smf(&mut SequentialIds::new(0), missing).expect_err("missing");
    assert_eq!(err.code, ErrorCode::NotFound);
    assert_eq!(
        err.message,
        "MIDI file /nonexistent/missing.mid does not exist"
    );

    assert_eq!(
        import_err(b"not a midi file"),
        (
            ErrorCode::FormatError,
            "test.mid is not a valid Standard MIDI File: MThd not found. Probably not a MIDI file"
                .to_owned()
        )
    );
    // mido's EOFError has no message.
    let good = encode_smf(&smf_project(), None).expect("export").bytes;
    for cut in [0, 7, 10, 20, good.len() - 1] {
        assert_eq!(
            import_err(&good[..cut]),
            (
                ErrorCode::FormatError,
                "test.mid is not a valid Standard MIDI File: ".to_owned()
            ),
            "cut at {cut}"
        );
    }
    let mut wrong_chunk = good.clone();
    wrong_chunk[14..18].copy_from_slice(b"XXXX");
    assert_eq!(
        import_err(&wrong_chunk).1,
        "test.mid is not a valid Standard MIDI File: no MTrk header at start of track"
    );
}

#[test]
fn type_2_files_and_smpte_division_are_unsupported() {
    let sequential = file(Format::Sequential, 480, &[vec![(0, on(0, 60, 100))]]);
    assert_eq!(
        import_err(&sequential),
        (
            ErrorCode::Unsupported,
            "Type 2 (sequential) MIDI files are not supported".to_owned()
        )
    );
    let mut smpte = file(Format::Parallel, 480, &[vec![(0, on(0, 60, 100))]]);
    smpte[12..14].copy_from_slice(&[0xE7, 0x28]);
    assert_eq!(import_err(&smpte).0, ErrorCode::Unsupported);
}

#[test]
fn dangling_notes_are_dropped_with_a_warning() {
    let bytes = file(
        Format::Parallel,
        480,
        &[vec![
            (0, on(0, 60, 100)),
            (480, off(0, 60)),
            (0, on(0, 62, 100)),
        ]],
    );
    let imported = import(&bytes);
    assert_eq!(imported.tracks.len(), 1);
    assert_eq!(imported.tracks[0].notes.len(), 1);
    assert!((imported.tracks[0].notes[0].length - 1.0).abs() < 1e-12);
    assert_eq!(
        imported.warnings,
        ["Track 1: 1 note(s) without note-off were dropped"]
    );
    assert_eq!(
        imported.tempo_map.points()[0].bpm.to_bits(),
        120.0_f64.to_bits()
    );
    // A file with a single track names the channel tracks after their channel.
    assert_eq!(imported.tracks[0].name, "Channel 1");
}

#[test]
fn overlapping_notes_pair_first_in_first_out() {
    let bytes = file(
        Format::Parallel,
        480,
        &[vec![
            (0, on(0, 60, 100)),
            (240, on(0, 60, 50)),
            (240, off(0, 60)),
            (480, on(0, 60, 0)),
        ]],
    );
    let notes = &import(&bytes).tracks[0].notes;
    assert_eq!(notes.len(), 2);
    assert_eq!(
        (notes[0].start, notes[0].length, notes[0].velocity),
        (0.0, 1.0, 100)
    );
    assert_eq!(
        (notes[1].start, notes[1].length, notes[1].velocity),
        (0.5, 1.5, 50)
    );
    // IDs follow the order of the note-offs.
    assert_eq!(notes[0].id, "n_000000000001");
    assert_eq!(notes[1].id, "n_000000000002");
}

#[test]
fn tracks_are_named_and_split_by_channel() {
    let drums = b"Kit";
    let bytes = file(
        Format::Parallel,
        480,
        &[
            vec![
                (0, name(b"Lead")),
                (0, on(0, 60, 100)),
                (0, on(9, 36, 100)),
                (240, off(0, 60)),
                (0, off(9, 36)),
            ],
            vec![(0, name(drums)), (0, on(9, 38, 90)), (240, off(9, 38))],
            vec![(0, name(b"  ")), (0, on(2, 40, 90)), (240, off(2, 40))],
        ],
    );
    let imported = import(&bytes);
    let summary: Vec<(&str, u8)> = imported
        .tracks
        .iter()
        .map(|t| (t.name.as_str(), t.channel))
        .collect();
    assert_eq!(
        summary,
        [
            ("Lead", 0),
            ("Lead (Drums)", 9),
            ("Kit (Drums)", 9),
            ("Track 3", 2)
        ]
    );
}

#[test]
fn tempos_are_clamped_replaced_and_completed() {
    let tempo = |micros: u32| TrackEventKind::Meta(MetaMessage::Tempo(u24::new(micros)));
    let bytes = file(
        Format::Parallel,
        480,
        &[vec![
            (480, tempo(500_000)),
            (0, tempo(250_000)),
            (480, tempo(60_000_000)),
            (480, tempo(0)),
        ]],
    );
    let imported = import(&bytes);
    let points: Vec<(f64, f64)> = imported
        .tempo_map
        .points()
        .iter()
        .map(|p| (p.beat, p.bpm))
        .collect();
    // No tempo at beat 0: the first one is repeated there. The second event at beat 1 replaces
    // the first. 1 BPM clamps to 10, and a tempo of 0 reads as the maximum.
    assert_eq!(
        points,
        [(0.0, 240.0), (1.0, 240.0), (2.0, 10.0), (3.0, 999.0)]
    );
}

#[test]
fn time_signatures_are_placed_on_bars_and_unsupported_ones_ignored() {
    let signature = |numerator: u8, power: u8| {
        TrackEventKind::Meta(MetaMessage::TimeSignature(numerator, power, 24, 8))
    };
    let bytes = file(
        Format::Parallel,
        480,
        &[vec![
            (0, signature(3, 2)),
            (1920, signature(7, 3)),
            (0, signature(4, 7)),
            (0, signature(4, 70)),
        ]],
    );
    let imported = import(&bytes);
    let points: Vec<(i64, u32, u32)> = imported
        .time_signatures
        .points()
        .iter()
        .map(|p| (p.bar, p.numerator, p.denominator))
        .collect();
    assert_eq!(points, [(0, 3, 4), (1, 7, 8)]);
    assert_eq!(
        imported.warnings,
        [
            "time signature 4/128 not supported; ignored",
            "time signature 4/1180591620717411303424 not supported; ignored"
        ]
    );
}

#[test]
fn markers_cue_points_and_latin1_names_decode() {
    let marker = |text: &'static [u8]| TrackEventKind::Meta(MetaMessage::Marker(text));
    let cue = |text: &'static [u8]| TrackEventKind::Meta(MetaMessage::CuePoint(text));
    let bytes = file(
        Format::Parallel,
        480,
        &[vec![
            (0, marker(b" Intro ")),
            (480, cue(b"Drop")),
            (0, marker(b"  ")),
            // "Caf\xe9" is Latin-1 and not valid UTF-8; the Czech text is UTF-8.
            (0, name(b"Caf\xe9")),
            (0, marker("Zvěř".as_bytes())),
            (0, on(0, 60, 100)),
            (480, off(0, 60)),
        ]],
    );
    let imported = import(&bytes);
    assert_eq!(
        imported.markers,
        [
            (0.0, "Intro".to_owned()),
            (1.0, "Drop".to_owned()),
            (1.0, "Zvěř".to_owned())
        ]
    );
    // The name 'Café' is Latin-1 in the file (not valid UTF-8).
    assert_eq!(imported.tracks[0].name, "Café");
}

#[test]
fn controllers_program_and_pressure_are_read() {
    let message = |message| TrackEventKind::Midi {
        channel: u4::new(1),
        message,
    };
    let bytes = file(
        Format::Parallel,
        480,
        &[vec![
            (
                0,
                message(MidiMessage::ProgramChange {
                    program: u7::new(40),
                }),
            ),
            (
                240,
                message(MidiMessage::Controller {
                    controller: u7::new(7),
                    value: u7::new(99),
                }),
            ),
            (
                240,
                message(MidiMessage::PitchBend {
                    bend: midly::PitchBend::from_int(-2048),
                }),
            ),
            (
                240,
                message(MidiMessage::ChannelAftertouch { vel: u7::new(12) }),
            ),
            (
                0,
                message(MidiMessage::Aftertouch {
                    key: u7::new(60),
                    vel: u7::new(5),
                }),
            ),
        ]],
    );
    let imported = import(&bytes);
    let track = &imported.tracks[0];
    assert_eq!((track.channel, track.program), (1, Some(40)));
    let controls: Vec<(f64, ControlKind, i64, i64)> = track
        .controls
        .iter()
        .map(|c| (c.beat, c.kind, c.value, c.number))
        .collect();
    assert_eq!(
        controls,
        [
            (0.5, ControlKind::Cc, 99, 7),
            (1.0, ControlKind::PitchBend, -2048, 0),
            (1.5, ControlKind::ChannelPressure, 12, 0)
        ]
    );
}

#[test]
fn tracks_without_notes_or_controllers_are_dropped() {
    let bytes = file(
        Format::Parallel,
        480,
        &[vec![(0, name(b"Meta"))], vec![(0, on(0, 60, 100))]],
    );
    let imported = import(&bytes);
    assert_eq!(imported.tracks.len(), 0);
    assert_eq!(
        imported.warnings,
        ["Track 2: 1 note(s) without note-off were dropped"]
    );
}

#[test]
fn clip_from_imported_rounds_up_to_whole_bars() {
    let bytes = file(
        Format::Parallel,
        480,
        &[vec![(0, on(0, 60, 100)), (2400, off(0, 60))]],
    );
    let imported = import(&bytes);
    let mut ids = SequentialIds::new(0);
    let clip = clip_from_imported(&mut ids, &imported.tracks[0], 8.0, None, None).expect("clip");
    // The note ends at beat 5: two bars.
    assert_eq!((clip.start, clip.length), (8.0, 8.0));
    let clip = clip_from_imported(&mut ids, &imported.tracks[0], 0.0, Some("Mine"), Some(3.0))
        .expect("clip");
    assert_eq!((clip.name.as_str(), clip.length), ("Mine", 3.0));
}
