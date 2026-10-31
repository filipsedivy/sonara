//! The project view's size and build time on the timeline's large project (PERF-01): 100 instrument tracks × 40 MIDI clips of 6 notes, the prototype's
//! `_big_project` of `test_large_project_repaint_is_fast`.
//!
//! The page asks for the whole view after every session event, so its size and the time to
//! build and serialize it bound how fast an edit shows. The timing is printed, not asserted
//! (HW-01: no hardware numbers as gates); run it with
//! `cargo test --release -p sonara-app --test gui_view_size -- --ignored --nocapture`.

use std::time::Instant;

use sonara_app::context::{AppContext, AppOptions};
use sonara_app::projects::NewProject;
use sonara_app::settings::Settings;
use sonara_app::view::project_view;
use sonara_core::commands::clip::CreateMidiClip;
use sonara_core::commands::track::CreateTrack;
use sonara_core::ids::{self, IdGenerator as _};
use sonara_core::midi::MidiNote;
use sonara_core::model::track::TrackKind;

/// The prototype's `_big_project(100, 40)`, built through the commands.
fn big_project(tracks: u32, clips: u32) -> (AppContext, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let app = AppContext::create(AppOptions {
        workspaces: Some(dir.path().join("ws")),
        settings: Some(Settings::default()),
        persist_settings: false,
        audio_backend: Some("null".to_owned()),
        ..AppOptions::default()
    })
    .expect("the application");
    app.projects()
        .new_project(&NewProject {
            name: "Big".to_owned(),
            ..NewProject::default()
        })
        .expect("a project");
    let mut ids = app.ids();
    let mut session = app.session().lock();
    for i in 0..tracks {
        let track_id = ids.new_id(ids::TRACK);
        session
            .execute(
                &CreateTrack {
                    name: format!("Track {}", i + 1),
                    kind: TrackKind::Instrument,
                    index: None,
                    color: None,
                    output_id: ids::MASTER_ID.to_owned(),
                    instrument_plugin: Some("sonara.synth.poly".to_owned()),
                    instrument_preset: None,
                    parent_id: None,
                    track_id: track_id.clone(),
                    instrument_id: ids.new_id(ids::DEVICE),
                    part: None,
                },
                false,
                "user",
            )
            .expect("a track");
        for k in 0..clips {
            let notes = (0..6_u32)
                .map(|j| {
                    let pitch = 48 + i64::from((j * 5 + i) % 24);
                    MidiNote::create(&mut ids, pitch, f64::from(j) * 0.5, 0.4, 100, 0)
                        .expect("a note")
                })
                .collect();
            session
                .execute(
                    &CreateMidiClip {
                        track_id: track_id.clone(),
                        start: f64::from(k) * 4.0,
                        length: 3.5,
                        name: format!("c{k}"),
                        notes,
                        loop_length: None,
                        color: None,
                        clip_id: ids.new_id(ids::CLIP),
                    },
                    false,
                    "user",
                )
                .expect("a clip");
        }
    }
    drop(session);
    (app, dir)
}

#[test]
fn the_large_view_holds_every_clip_and_note() {
    let (app, _dir) = big_project(100, 40);
    let view = project_view(app.session(), app.plugins());
    let project = view.project.expect("an open project");
    assert_eq!(project.tracks.len(), 100);
    assert_eq!(
        project.tracks.iter().map(|t| t.clips.len()).sum::<usize>(),
        4000
    );
}

#[test]
#[ignore = "a measurement (PERF-01): run with --release --ignored --nocapture"]
fn the_large_view_size_and_refresh_time() {
    let (app, _dir) = big_project(100, 40);
    let mut build = Vec::new();
    let mut serialize = Vec::new();
    let mut bytes = 0;
    for _ in 0..20 {
        let t0 = Instant::now();
        let view = project_view(app.session(), app.plugins());
        let t1 = Instant::now();
        let json = serde_json::to_vec(&view).expect("the view serializes");
        let t2 = Instant::now();
        build.push((t1 - t0).as_secs_f64() * 1000.0);
        serialize.push((t2 - t1).as_secs_f64() * 1000.0);
        bytes = json.len();
    }
    build.sort_by(f64::total_cmp);
    serialize.sort_by(f64::total_cmp);
    println!(
        "project view of 100 tracks x 40 clips x 6 notes: {bytes} bytes of JSON; \
         build median {:.2} ms (min {:.2}), serialize median {:.2} ms (min {:.2})",
        build[build.len() / 2],
        build[0],
        serialize[serialize.len() / 2],
        serialize[0],
    );
}
