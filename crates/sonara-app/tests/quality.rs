//! The quality checks (SON-P18): each known case gives its finding with a severity, an
//! explanation and a place, a clean project gives none, and the project is never changed.

use std::sync::Arc;

use sonara_app::quality::{Severity, audio_findings, check_project};
use sonara_core::ids::SequentialIds;
use sonara_core::midi::MidiNote;
use sonara_core::model::clip::{Clip, MidiClip, MidiClipData};
use sonara_core::model::device::{Device, Params};
use sonara_core::model::project::ProjectData;
use sonara_core::model::track::{Track, TrackData, TrackKind};
use sonara_core::model::{JsonMap, Valid};
use sonara_dsp::analysis::Analyzer;

fn note(id: &str, pitch: i64, start: f64, length: f64, velocity: i64) -> MidiNote {
    MidiNote::new(id, pitch, start, length, velocity, 0).expect("valid note")
}

fn clip(id: &str, length: f64, notes: Vec<MidiNote>) -> Clip {
    Clip::Midi(
        MidiClip::new(MidiClipData {
            id: id.to_owned(),
            name: id.to_owned(),
            start: 0.0,
            length,
            notes: Arc::new(notes),
            controls: Arc::new(Vec::new()),
            content_offset: 0.0,
            loop_length: None,
            muted: false,
            color: None,
            extra: JsonMap::new(),
        })
        .expect("valid clip"),
    )
}

fn track(
    id: &str,
    with_instrument: bool,
    clips: Vec<Clip>,
    edit: impl FnOnce(&mut TrackData),
) -> Track {
    let mut data = TrackData::new(id, id, TrackKind::Instrument);
    data.clips = clips;
    if with_instrument {
        let mut ids = SequentialIds::new(0);
        data.instrument = Some(
            Device::create(
                &mut ids,
                "sonara.synth.poly",
                "Poly",
                Params::default(),
                None,
            )
            .expect("device"),
        );
    }
    edit(&mut data);
    Valid::new(data).expect("valid track")
}

fn project(tracks: Vec<Track>) -> ProjectData {
    let mut data = ProjectData::new("prj_1", "Quality");
    data.tracks = tracks;
    data
}

/// Eight notes a step apart, with varying velocities.
fn good_notes() -> Vec<MidiNote> {
    (0..8_i32)
        .map(|i| {
            note(
                &format!("n{i}"),
                60 + i64::from(i),
                f64::from(i),
                0.5,
                70 + i64::from(i) * 3,
            )
        })
        .collect()
}

fn codes(findings: &[sonara_app::quality::Finding]) -> Vec<&'static str> {
    findings.iter().map(|f| f.code).collect()
}

#[test]
fn a_clean_project_has_no_findings() {
    let data = project(vec![track(
        "trk_a",
        true,
        vec![clip("clp_a", 8.0, good_notes())],
        |_| {},
    )]);
    let report = check_project(&data);
    assert_eq!(report.findings, []);
    assert!(report.is_sound());
}

#[test]
fn each_known_problem_is_found_with_its_severity_place_and_explanation() {
    let bad = vec![
        note("n_dup1", 60, 0.0, 1.0, 90),
        note("n_dup2", 60, 0.0, 1.0, 91),
        note("n_ov1", 64, 2.0, 2.0, 90),
        note("n_ov2", 64, 3.0, 1.0, 90),
        note("n_late", 67, 9.0, 1.0, 90),
    ];
    let data = project(vec![
        track("trk_none", false, vec![clip("clp_x", 8.0, bad)], |_| {}),
        track("trk_hot", true, vec![clip("clp_e", 4.0, vec![])], |t| {
            t.mixer.volume_db = 9.0;
        }),
        track(
            "trk_flat",
            true,
            vec![clip(
                "clp_f",
                16.0,
                (0..8)
                    .map(|i| note(&format!("f{i}"), 60, f64::from(i), 0.5, 100))
                    .collect(),
            )],
            |_| {},
        ),
    ]);
    let report = check_project(&data);
    let found = codes(&report.findings);
    for code in [
        "INSTRUMENT_MISSING",
        "DUPLICATE_NOTE",
        "NOTE_AFTER_CLIP_END",
        "OVERLAPPING_NOTES",
        "FADER_HOT",
        "EMPTY_CLIP",
        "UNIFORM_VELOCITY",
    ] {
        assert!(found.contains(&code), "{code} missing from {found:?}");
    }
    // Errors first, then warnings, then recommendations.
    let severities: Vec<Severity> = report.findings.iter().map(|f| f.severity).collect();
    assert!(
        severities.windows(2).all(|w| w[0] <= w[1]),
        "{severities:?}"
    );
    assert!(!report.is_sound());
    let by = |code: &str| {
        report
            .findings
            .iter()
            .find(|f| f.code == code)
            .expect("finding")
    };
    assert_eq!(by("INSTRUMENT_MISSING").severity, Severity::Error);
    assert_eq!(
        by("INSTRUMENT_MISSING").track_id.as_deref(),
        Some("trk_none")
    );
    assert_eq!(by("DUPLICATE_NOTE").severity, Severity::Error);
    assert_eq!(by("DUPLICATE_NOTE").note_ids, ["n_dup2"]);
    assert_eq!(by("OVERLAPPING_NOTES").severity, Severity::Warning);
    assert_eq!(by("OVERLAPPING_NOTES").note_ids, ["n_ov2"]);
    assert_eq!(by("NOTE_AFTER_CLIP_END").note_ids, ["n_late"]);
    assert_eq!(by("NOTE_AFTER_CLIP_END").clip_id.as_deref(), Some("clp_x"));
    assert_eq!(by("EMPTY_CLIP").severity, Severity::Recommendation);
    assert_eq!(by("FADER_HOT").track_id.as_deref(), Some("trk_hot"));
    assert_eq!(by("UNIFORM_VELOCITY").track_id.as_deref(), Some("trk_flat"));
    // Every finding says what, why and where.
    for finding in &report.findings {
        assert!(
            finding.message.len() > 10 && finding.explanation.len() > 20,
            "{finding:?}"
        );
        assert!(finding.track_id.is_some(), "{finding:?}");
    }
    let json = report.to_json();
    assert_eq!(json["sound"], false);
    assert_eq!(json["errors"], 2);
}

#[test]
fn a_solo_silences_the_other_tracks_with_content() {
    let data = project(vec![
        track(
            "trk_solo",
            true,
            vec![clip("clp_1", 8.0, good_notes())],
            |t| t.mixer.solo = true,
        ),
        track(
            "trk_quiet",
            true,
            vec![clip("clp_2", 8.0, good_notes())],
            |_| {},
        ),
        track(
            "trk_safe",
            true,
            vec![clip("clp_3", 8.0, good_notes())],
            |t| t.mixer.solo_safe = true,
        ),
    ]);
    let found = check_project(&data).findings;
    assert_eq!(codes(&found), ["SILENCED_BY_SOLO"]);
    assert_eq!(found[0].track_id.as_deref(), Some("trk_quiet"));
}

#[test]
fn checking_does_not_change_the_project() {
    let data = project(vec![track(
        "trk_a",
        false,
        vec![clip("clp_a", 4.0, vec![])],
        |_| {},
    )]);
    let before = data.clone();
    let _ = check_project(&data);
    assert_eq!(data.tracks.len(), before.tracks.len());
    assert_eq!(data.tracks[0].clips.len(), before.tracks[0].clips.len());
}

#[test]
fn audio_issues_get_the_same_shape_and_an_explanation() {
    // A full-scale square wave for two seconds: clipping and over-loud.
    let mut analyzer = Analyzer::new(48_000, 2).expect("analyzer");
    let block: Vec<f64> = (0..96_000)
        .flat_map(|i| {
            let s = if (i / 50) % 2 == 0 { 1.0 } else { -1.0 };
            [s, s]
        })
        .collect();
    analyzer.feed(&block);
    let findings = audio_findings(&analyzer.finish());
    assert_ne!(findings, []);
    let found = codes(&findings);
    assert!(found.contains(&"CLIPPING"), "{found:?}");
    for finding in &findings {
        assert!(finding.explanation.len() > 20, "{finding:?}");
    }
    let severities: Vec<Severity> = findings.iter().map(|f| f.severity).collect();
    assert!(severities.windows(2).all(|w| w[0] <= w[1]));
}
