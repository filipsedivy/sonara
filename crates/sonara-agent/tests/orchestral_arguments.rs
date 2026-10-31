//! The orchestral arguments of agent API 1.1 through the tools (CTR-01): a note's
//! `articulation` in `create_midi_clip`, `add_midi_notes` and `edit_midi_notes.replace_all`, and
//! the `part` of `create_track`. What a call stores is read back from the session's project;
//! every refusal is checked to change nothing (TST-03).

use std::sync::Arc;

use serde_json::{Value, json};
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::permissions::sequential_tokens;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget, shared_tokens};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_core::ids::SequentialIds;
use sonara_core::model::clip::Clip;
use sonara_core::model::part::{Clef, Notation, Part};
use sonara_core::model::project::Project;

/// The agent on a headless application with sequential IDs, and the application (to read the
/// session's project back), with the temporary directory kept alive.
fn world() -> (tempfile::TempDir, Arc<AppContext>, AgentService) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let root = dir.path().to_path_buf();
    for sub in ["work", "projects", "workspaces"] {
        std::fs::create_dir(root.join(sub)).expect("a folder");
    }
    let app = Arc::new(
        AppContext::create(AppOptions {
            workspaces: Some(root.join("workspaces")),
            persist_settings: false,
            audio_backend: Some("null".to_owned()),
            ids: Some(IdSource::new(Box::new(SequentialIds::new(0)))),
            ..AppOptions::default()
        })
        .expect("the application"),
    );
    let service = AgentService::new(
        Arc::clone(&app),
        AgentOptions {
            base_dir: Some(root.join("work")),
            audit: AuditTarget::File(root.join("audit.jsonl")),
            tokens: Some(shared_tokens(sequential_tokens())),
            environment: Some(PolicyEnvironment {
                allowed: None,
                projects_dir: root.join("projects"),
            }),
            ..AgentOptions::default()
        },
    )
    .expect("the service");
    call(&service, "create_project", &json!({"name": "Score"}));
    (dir, app, service)
}

fn call(service: &AgentService, tool: &str, args: &Value) -> Value {
    let reply = service.call(tool, Some(args));
    assert_eq!(reply["ok"], true, "{tool}: {reply}");
    reply
}

/// The error of a call that must fail; the project must be unchanged by it.
fn refused(app: &AppContext, service: &AgentService, tool: &str, args: &Value) -> Value {
    let before = app.session().lock().version();
    let reply = service.call(tool, Some(args));
    assert_eq!(reply["ok"], false, "{tool}: {reply}");
    assert_eq!(
        app.session().lock().version(),
        before,
        "{tool}: a refused call changes nothing"
    );
    reply["error"].clone()
}

fn project(app: &AppContext) -> Project {
    app.session()
        .lock()
        .require_project()
        .expect("an open project")
        .clone()
}

/// The articulations of the notes of the MIDI clip `clip_id`, in note order.
fn articulations(app: &AppContext, clip_id: &str) -> Vec<Option<String>> {
    let project = project(app);
    let (_, clip) = project.clip(clip_id).expect("the clip");
    let Clip::Midi(clip) = clip else {
        panic!("{clip_id} is a MIDI clip");
    };
    clip.notes.iter().map(|n| n.articulation.clone()).collect()
}

/// `ids` as [`articulations`] reports them.
fn stored(ids: &[Option<&str>]) -> Vec<Option<String>> {
    ids.iter().map(|id| id.map(str::to_owned)).collect()
}

fn instrument_track(service: &AgentService) -> String {
    call(service, "create_track", &json!({"name": "Violins I"}))["result"]["track_id"]
        .as_str()
        .expect("a track ID")
        .to_owned()
}

/// A MIDI clip with one ordinary note on a new instrument track: the track and the clip.
fn clip(service: &AgentService) -> (String, String) {
    let track = instrument_track(service);
    let reply = call(
        service,
        "create_midi_clip",
        &json!({"track_id": track, "start": 0, "length": 4,
                "notes": [{"pitch": 60, "start": 0, "length": 1}]}),
    );
    let clip = reply["result"]["clip_id"]
        .as_str()
        .expect("a clip ID")
        .to_owned();
    (track, clip)
}

#[test]
fn notes_keep_their_articulation_in_every_note_tool() {
    let (_dir, app, service) = world();
    let track = instrument_track(&service);
    let reply = call(
        &service,
        "create_midi_clip",
        &json!({"track_id": track, "start": 0, "length": 4, "notes": [
            {"pitch": 60, "start": 0, "length": 1, "articulation": "legato"},
            {"pitch": 62, "start": 1, "length": 1},
            {"pitch": 64, "start": 2, "length": 0.5, "articulation": "staccato"},
            {"pitch": 65, "start": 3, "length": 1, "articulation": null},
        ]}),
    );
    let clip = reply["result"]["clip_id"].as_str().expect("a clip ID");
    assert_eq!(
        articulations(&app, clip),
        stored(&[Some("legato"), None, Some("staccato"), None])
    );

    call(
        &service,
        "add_midi_notes",
        &json!({"clip_id": clip, "notes": [
            {"pitch": 67, "start": 3.5, "length": 0.5, "articulation": "pizzicato"},
        ]}),
    );
    assert_eq!(
        articulations(&app, clip),
        stored(&[
            Some("legato"),
            None,
            Some("staccato"),
            None,
            Some("pizzicato")
        ])
    );

    call(
        &service,
        "edit_midi_notes",
        &json!({"clip_id": clip, "replace_all": [
            {"pitch": 48, "start": 0, "length": 4, "articulation": "tremolo"},
            {"pitch": 55, "start": 0, "length": 4},
        ]}),
    );
    assert_eq!(articulations(&app, clip), stored(&[Some("tremolo"), None]));

    // The note transforms keep a note's articulation.
    call(
        &service,
        "edit_midi_notes",
        &json!({"clip_id": clip, "transpose": 12, "velocity_scale": 0.5}),
    );
    assert_eq!(articulations(&app, clip), stored(&[Some("tremolo"), None]));
}

#[test]
fn an_unknown_articulation_is_refused_with_suggestions_in_every_note_tool() {
    let (_dir, app, service) = world();
    let (track, clip) = clip(&service);
    let note = json!({"pitch": 60, "start": 0, "length": 1, "articulation": "stacato"});
    let calls = [
        (
            "create_midi_clip",
            json!({"track_id": track, "start": 8, "length": 4, "notes": [note]}),
        ),
        ("add_midi_notes", json!({"clip_id": clip, "notes": [note]})),
        (
            "edit_midi_notes",
            json!({"clip_id": clip, "replace_all": [note]}),
        ),
    ];
    for (tool, args) in calls {
        let error = refused(&app, &service, tool, &args);
        assert_eq!(error["code"], "INVALID_ARGUMENT", "{tool}");
        assert_eq!(
            error["message"], "unknown articulation 'stacato'; did you mean 'staccato'?",
            "{tool}"
        );
        assert_eq!(error["details"]["articulation"], "stacato", "{tool}");
        assert_eq!(
            error["details"]["suggestions"],
            json!(["staccato"]),
            "{tool}"
        );
        let available = error["details"]["available"]
            .as_array()
            .expect("the accepted ids");
        assert!(available.contains(&json!("pizzicato")), "{tool}");
        assert!(!available.contains(&json!("con_sordino")), "{tool}");
    }
    assert_eq!(articulations(&app, &clip), stored(&[None]));

    // Another spelling of an id is not the id; the error names it.
    let error = refused(
        &app,
        &service,
        "add_midi_notes",
        &json!({"clip_id": clip, "notes": [
            {"pitch": 60, "start": 0, "length": 1, "articulation": "Sul Ponticello"},
        ]}),
    );
    assert_eq!(
        error["message"],
        "unknown articulation 'Sul Ponticello'; did you mean 'sul_ponticello'?"
    );
    // An id close to nothing gets no suggestion.
    let error = refused(
        &app,
        &service,
        "add_midi_notes",
        &json!({"clip_id": clip, "notes": [
            {"pitch": 60, "start": 0, "length": 1, "articulation": "zzz"},
        ]}),
    );
    assert_eq!(error["message"], "unknown articulation 'zzz'");
    assert_eq!(error["details"]["suggestions"], json!([]));
}

#[test]
fn a_mute_or_vibrato_is_not_a_note_articulation() {
    let (_dir, app, service) = world();
    let (_, clip) = clip(&service);
    for (id, what) in [
        ("con_sordino", "a mute"),
        ("mute_harmon", "a mute"),
        ("non_vibrato", "a vibrato setting"),
    ] {
        let error = refused(
            &app,
            &service,
            "add_midi_notes",
            &json!({"clip_id": clip, "notes": [
                {"pitch": 60, "start": 1, "length": 1},
                {"pitch": 62, "start": 2, "length": 1, "articulation": id},
            ]}),
        );
        assert_eq!(error["code"], "INVALID_ARGUMENT");
        assert_eq!(
            error["message"],
            format!(
                "articulation '{id}' is {what}, which holds over a passage, not a note's \
                 articulation"
            )
        );
    }
    assert_eq!(articulations(&app, &clip), stored(&[None]));
}

#[test]
fn an_articulation_that_is_not_a_string_fails_validation() {
    let (_dir, app, service) = world();
    let (_, clip) = clip(&service);
    let error = refused(
        &app,
        &service,
        "add_midi_notes",
        &json!({"clip_id": clip, "notes": [
            {"pitch": 60, "start": 0, "length": 1, "articulation": 3},
        ]}),
    );
    assert_eq!(error["code"], "INVALID_ARGUMENT");
    assert_eq!(error["message"], "invalid arguments for add_midi_notes");
    assert_eq!(
        error["details"]["errors"],
        json!([{"field": "notes.0.articulation", "message": "Input should be a valid string"}])
    );
}

#[test]
fn create_track_stores_the_part() {
    let (_dir, app, service) = world();
    let reply = call(
        &service,
        "create_track",
        &json!({"name": "Horn 3", "part": {
            "instrument": "horn_f", "name": "Horn 3", "players": 1, "solo": true,
            "notation": {"transposition": -7, "clef": "bass"},
        }}),
    );
    // The result is the one of 1.0.
    let result = reply["result"].as_object().expect("a result");
    assert_eq!(
        result.keys().collect::<Vec<_>>(),
        ["track_id", "instrument_device_id"]
    );
    let track_id = result["track_id"].as_str().expect("a track ID");
    let project = project(&app);
    let track = project.track(track_id).expect("the track");
    assert_eq!(
        track.part,
        Some(Part {
            instrument: "horn_f".to_owned(),
            name: Some("Horn 3".to_owned()),
            players: Some(1),
            solo: Some(true),
            player: None,
            notation: Notation {
                transposition: Some(-7),
                clef: Some(Clef::Bass),
            },
        })
    );
    // A track without a part states none, and a part may state only its instrument.
    let plain = instrument_track(&service);
    let piccolo = call(
        &service,
        "create_track",
        &json!({"name": "Piccolo", "part": {"instrument": "piccolo", "player": "Flute 2"}}),
    )["result"]["track_id"]
        .clone();
    let project = self::project(&app);
    assert_eq!(project.track(&plain).expect("the track").part, None);
    let part = project
        .track(piccolo.as_str().expect("a track ID"))
        .expect("the track")
        .part
        .clone()
        .expect("a part");
    assert_eq!(part.instrument, "piccolo");
    assert_eq!(part.player.as_deref(), Some("Flute 2"));
    assert!(part.name.is_none() && part.players.is_none() && part.notation.is_empty());
}

#[test]
fn a_dry_run_with_a_part_creates_nothing() {
    let (_dir, app, service) = world();
    let before = project(&app).tracks.len();
    let reply = call(
        &service,
        "create_track",
        &json!({"name": "Violins I", "dry_run": true, "part": {"instrument": "violin"}}),
    );
    assert_eq!(reply["committed"], false);
    assert_eq!(project(&app).tracks.len(), before);
}

#[test]
fn only_an_instrument_track_takes_a_part() {
    let (_dir, app, service) = world();
    for kind in ["audio", "bus", "folder"] {
        let error = refused(
            &app,
            &service,
            "create_track",
            &json!({"name": "Strings", "kind": kind, "part": {"instrument": "violin"}}),
        );
        assert_eq!(error["code"], "INVALID_ARGUMENT", "{kind}");
        assert_eq!(
            error["message"], "only instrument tracks have a part",
            "{kind}"
        );
    }
}

#[test]
fn an_unknown_part_instrument_is_refused_with_suggestions_and_draws_no_ids() {
    let (_dir, app, service) = world();
    let cases = [
        (
            "French horn",
            "unknown instrument 'French horn'; did you mean 'horn_f'?",
        ),
        (
            "Violin",
            "unknown instrument 'Violin'; did you mean 'violin' or 'viola'?",
        ),
    ];
    for (instrument, message) in cases {
        let error = refused(
            &app,
            &service,
            "create_track",
            &json!({"name": "Part", "part": {"instrument": instrument}}),
        );
        assert_eq!(error["code"], "INVALID_ARGUMENT", "{instrument}");
        assert_eq!(error["message"], message, "{instrument}");
        assert_eq!(error["details"]["instrument"], instrument, "{instrument}");
        assert!(
            error["details"]["available"]
                .as_array()
                .expect("every instrument type")
                .contains(&json!("timpani")),
            "{instrument}"
        );
    }
    // The refused calls drew no IDs: the next track gets the ID it gets in a fresh session.
    let after_refusals = instrument_track(&service);
    let (_fresh_dir, _fresh_app, fresh) = world();
    assert_eq!(after_refusals, instrument_track(&fresh));
}

#[test]
fn a_part_outside_the_project_format_fails_validation() {
    let (_dir, app, service) = world();
    let cases = [
        (
            json!({"instrument": "violin", "players": 0}),
            json!([{"field": "part.players",
                    "message": "Input should be greater than or equal to 1"}]),
        ),
        (
            json!({"instrument": "violin", "players": 129}),
            json!([{"field": "part.players",
                    "message": "Input should be less than or equal to 128"}]),
        ),
        (
            json!({"instrument": "violin", "name": ""}),
            json!([{"field": "part.name",
                    "message": "String should have at least 1 character"}]),
        ),
        (
            json!({"instrument": "horn_f", "notation": {"transposition": 37}}),
            json!([{"field": "part.notation.transposition",
                    "message": "Input should be less than or equal to 36"}]),
        ),
        (
            json!({"instrument": "viola", "notation": {"clef": "soprano"}}),
            json!([{"field": "part.notation.clef",
                    "message": "Input should be 'treble', 'treble_8vb', 'bass', 'alto', \
                                'tenor', 'percussion' or 'single_line'"}]),
        ),
        (
            json!({"name": "Violins I"}),
            json!([{"field": "part.instrument", "message": "Field required"}]),
        ),
        (
            json!({"instrument": "violin", "section": true}),
            json!([{"field": "part.section", "message": "Extra inputs are not permitted"}]),
        ),
    ];
    for (part, errors) in cases {
        let error = refused(
            &app,
            &service,
            "create_track",
            &json!({"name": "Strings", "part": part}),
        );
        assert_eq!(error["code"], "INVALID_ARGUMENT", "{part}");
        assert_eq!(
            error["message"], "invalid arguments for create_track",
            "{part}"
        );
        assert_eq!(error["details"]["errors"], errors, "{part}");
    }
}
