//! The desktop's native menu bar against
//! the prototype's `MainWindow._build_menus`: the tree with its items, order and shortcuts, the
//! macOS placement, the routes of the items, the Track menu's tracks, and the texts and enabled
//! states of Undo, Redo, Open Recent and the Edit menu's clip items.
//!
//! The menu's model is also the page's: `apps/sonara-desktop/tests/fixtures/menus-v1.json`, which
//! the page's `tests/unit/menus.test.ts` checks its command table and focus names against, is
//! written from it here.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};
use sonara_app::chrome::WindowChrome;
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::edit::{self, Edit};
use sonara_app::menus::{
    EditFocus, Entry, MenuAction, PageCommand, Route, SystemItem, edit_items, menu_bar, new_track,
    recent_index, recent_items,
};
use sonara_app::projects::NewProject;
use sonara_app::settings::Settings;
use sonara_core::model::track::TrackKind;
use tempfile::TempDir;

/// The prototype's menu tree as `_build_menus` builds it, before Qt moves
/// Settings…, Quit and About Sonara into the macOS application menu.
const PROTOTYPE_TREE: &str = "\
File
  New Project…  ⌘N
  Open…  ⌘O
  Open Recent ▸
  ───
  Save  ⌘S
  Save As…  ⇧⌘S
  Save Packaged Copy (.sonara archive)…
  ───
  Import Audio…  ⌘I
  Import MIDI…  ⇧⌘I
  Export Audio…  ⇧⌘E
  Export MIDI…
  ───
  Verify Project
  Embed Linked Media
  Remove Unused Media…
  ───
  Settings…  ⌘,
  Quit  ⌘Q
Edit
  Undo  ⌘Z
  Redo  ⇧⌘Z
  ───
  Split Clips at Playhead  ⌘E
  Duplicate Clips  ⌘D
  Delete Clips
Track
  New Instrument Track  ⌘T
  New Drum Track
  New Audio Track  ⇧⌘T
  New Bus
  New Folder
  ───
  Delete Selected Track
Transport
  Play / Stop  Space
  Return to Start  Home
  Record  R
  Loop  L
  Metronome  C
View
  Browser
  Inspector
  Editors
  ───
  Mixer  ⌘1
  Piano Roll  ⌘2
  Devices  ⌘3
Help
  Agent Connection…
  About Sonara
";

/// The menu bar as the desktop builds it: the prototype's tree after the macOS move, with the
/// system's items in brackets and the checkable panes marked. The AI menu is the one deliberate
/// addition to the prototype's menus (GLD-01: it has no counterpart in the golden data, and the
/// prototype's menus are still compared in full below).
const MACOS_TREE: &str = "\
Sonara
  About Sonara
  ───
  Settings…  ⌘,
  ───
  [Services]
  ───
  [Hide]
  [Hide Others]
  [Show All]
  ───
  Quit Sonara  ⌘Q
File
  New Project…  ⌘N
  Open…  ⌘O
  Open Recent ▸
  ───
  Save  ⌘S
  Save As…  ⇧⌘S
  Save Packaged Copy (.sonara archive)…
  ───
  Import Audio…  ⌘I
  Import MIDI…  ⇧⌘I
  Export Audio…  ⇧⌘E
  Export MIDI…
  ───
  Verify Project
  Embed Linked Media
  Remove Unused Media…
Edit
  Undo  ⌘Z
  Redo  ⇧⌘Z
  ───
  Split Clips at Playhead  ⌘E
  Duplicate Clips  ⌘D
  Delete Clips
  ───
  [Cut]
  [Copy]
  [Paste]
  [Select All]
Track
  New Instrument Track  ⌘T
  New Drum Track
  New Audio Track  ⇧⌘T
  New Bus
  New Folder
  ───
  Delete Selected Track
Transport
  Play / Stop  Space
  Return to Start  Home
  Record  R
  Loop  L
  Metronome  C
View
  [x] Browser
  [x] Inspector
  [x] Editors
  ───
  Mixer  ⌘1
  Piano Roll  ⌘2
  Devices  ⌘3
AI
  AI Composer…
Window
  [Minimize]
  [Zoom]
  ───
  [Close Window]
Help
  Agent Connection…
";

fn system_text(item: SystemItem) -> &'static str {
    match item {
        SystemItem::Services => "Services",
        SystemItem::Hide => "Hide",
        SystemItem::HideOthers => "Hide Others",
        SystemItem::ShowAll => "Show All",
        SystemItem::Cut => "Cut",
        SystemItem::Copy => "Copy",
        SystemItem::Paste => "Paste",
        SystemItem::SelectAll => "Select All",
        SystemItem::Minimize => "Minimize",
        SystemItem::Zoom => "Zoom",
        SystemItem::CloseWindow => "Close Window",
    }
}

/// The menu bar in the notation of [`MACOS_TREE`].
fn rendered() -> String {
    let mut text = String::new();
    for menu in menu_bar() {
        text.push_str(menu.title);
        text.push('\n');
        for entry in &menu.entries {
            let line = match entry {
                Entry::Item(item) => match item.shortcut {
                    Some(shortcut) => format!("{}  {}", item.text, shortcut.glyphs()),
                    None => item.text.to_owned(),
                },
                Entry::Check(item) => format!("[x] {}", item.text),
                Entry::Separator => "───".to_owned(),
                Entry::System(system) => format!("[{}]", system_text(*system)),
                Entry::OpenRecent => "Open Recent ▸".to_owned(),
            };
            text.push_str("  ");
            text.push_str(&line);
            text.push('\n');
        }
    }
    text
}

/// The menus of `tree` as (title, lines).
fn menus_of(tree: &str) -> Vec<(String, Vec<String>)> {
    let mut menus: Vec<(String, Vec<String>)> = Vec::new();
    for line in tree.lines() {
        if let Some(entry) = line.strip_prefix("  ") {
            menus
                .last_mut()
                .expect("an entry under a menu")
                .1
                .push(entry.to_owned());
        } else {
            menus.push((line.to_owned(), Vec::new()));
        }
    }
    menus
}

#[test]
fn the_menu_bar_is_the_prototypes_tree_with_the_macos_placement() {
    assert_eq!(rendered(), MACOS_TREE);

    // The prototype's own menus are the bar's, without the system's items and the macOS move:
    // Settings…, Quit (and the separator before them) leave File, About Sonara leaves Help.
    let built = menus_of(MACOS_TREE);
    let built_prototype_part = |title: &str| -> Vec<String> {
        let (_, lines) = built
            .iter()
            .find(|(t, _)| t == title)
            .expect("the menu is built");
        let mut lines: Vec<String> = lines
            .iter()
            .map(|line| line.trim_start_matches("[x] ").to_owned())
            .filter(|line| !line.starts_with('['))
            .collect();
        // A separator that only stood before the system's items goes with them.
        while lines.last().is_some_and(|line| line == "───") {
            lines.pop();
        }
        lines
    };
    for (title, mut lines) in menus_of(PROTOTYPE_TREE) {
        match title.as_str() {
            "File" => {
                assert_eq!(
                    lines.split_off(lines.len() - 3),
                    ["───", "Settings…  ⌘,", "Quit  ⌘Q"]
                );
            }
            "Help" => assert_eq!(lines.pop().as_deref(), Some("About Sonara")),
            _ => {}
        }
        assert_eq!(built_prototype_part(&title), lines, "the {title} menu");
    }
    let app_menu = &built[0];
    assert_eq!(app_menu.0, "Sonara", "the application menu comes first");
    for moved in ["About Sonara", "Settings…  ⌘,", "Quit Sonara  ⌘Q"] {
        assert!(app_menu.1.iter().any(|line| line == moved), "{moved}");
    }
}

#[test]
fn every_item_has_one_action_whose_id_round_trips() {
    let mut seen = Vec::new();
    for menu in menu_bar() {
        for entry in menu.entries {
            if let Entry::Item(item) | Entry::Check(item) = entry {
                seen.push(item.action);
            }
        }
    }
    assert_eq!(
        seen,
        MenuAction::ALL,
        "every action once, in the menu's order"
    );
    let ids: HashSet<&str> = MenuAction::ALL.iter().map(|a| a.id()).collect();
    assert_eq!(ids.len(), MenuAction::ALL.len(), "the IDs are unique");
    for action in MenuAction::ALL {
        assert_eq!(MenuAction::from_id(action.id()), Some(action));
        assert_eq!(
            recent_index(action.id()),
            None,
            "{action:?} is no recent entry"
        );
        assert_eq!(
            serde_json::to_value(action).expect("JSON"),
            json!(action.id())
        );
    }
    assert_eq!(MenuAction::from_id("recent:0"), None);
    assert_eq!(MenuAction::from_id("no-recent"), None);
}

#[test]
fn the_accelerators_are_the_menu_librarys_and_the_transport_keys_are_plain() {
    let accelerators: Vec<(&str, String)> = menu_bar()
        .into_iter()
        .flat_map(|menu| menu.entries)
        .filter_map(|entry| match entry {
            Entry::Item(item) => item.shortcut.map(|s| (item.action.id(), s.accelerator())),
            _ => None,
        })
        .collect();
    let expected = [
        ("settings", "CmdOrCtrl+,"),
        ("quit", "CmdOrCtrl+Q"),
        ("new-project", "CmdOrCtrl+N"),
        ("open", "CmdOrCtrl+O"),
        ("save", "CmdOrCtrl+S"),
        ("save-as", "CmdOrCtrl+Shift+S"),
        ("import-audio", "CmdOrCtrl+I"),
        ("import-midi", "CmdOrCtrl+Shift+I"),
        ("export-audio", "CmdOrCtrl+Shift+E"),
        ("undo", "CmdOrCtrl+Z"),
        ("redo", "CmdOrCtrl+Shift+Z"),
        ("split-clips", "CmdOrCtrl+E"),
        ("duplicate-clips", "CmdOrCtrl+D"),
        ("new-instrument-track", "CmdOrCtrl+T"),
        ("new-audio-track", "CmdOrCtrl+Shift+T"),
        ("play-stop", "Space"),
        ("return-to-start", "Home"),
        ("record", "R"),
        ("loop", "L"),
        ("metronome", "C"),
        ("view:mixer", "CmdOrCtrl+1"),
        ("view:piano-roll", "CmdOrCtrl+2"),
        ("view:devices", "CmdOrCtrl+3"),
    ];
    let expected: Vec<(&str, String)> = expected
        .iter()
        .map(|(id, accelerator)| (*id, (*accelerator).to_owned()))
        .collect();
    assert_eq!(accelerators, expected);
}

#[test]
fn each_item_runs_where_its_state_is() {
    let routes: Vec<(&str, Route)> = MenuAction::ALL
        .iter()
        .map(|action| (action.id(), action.route()))
        .collect();
    let page = |ids: &[&str]| {
        ids.iter()
            .all(|id| routes.contains(&(*id, Route::PageCommand)))
    };
    // The focused editor, the selection, the playhead and the transport bar are the page's.
    assert!(page(&[
        "split-clips",
        "duplicate-clips",
        "delete-clips",
        "new-instrument-track",
        "new-drum-track",
        "new-audio-track",
        "new-bus",
        "new-folder",
        "delete-track",
        "play-stop",
        "return-to-start",
        "record",
        "loop",
        "metronome",
    ]));
    for id in [
        "about",
        "settings",
        "new-project",
        "import-audio",
        "export-audio",
        "agent-connection",
    ] {
        assert!(routes.contains(&(id, Route::PageDialog)), "{id}");
    }
    for action in MenuAction::ALL {
        assert_eq!(
            action.route() == Route::PageView,
            action.id().starts_with("view:"),
            "{action:?}"
        );
        assert_eq!(
            PageCommand::of(action).is_some(),
            action.route() == Route::PageCommand
        );
    }
    for id in [
        "quit",
        "open",
        "save",
        "save-as",
        "save-packaged",
        "import-midi",
        "export-midi",
    ] {
        assert!(routes.contains(&(id, Route::Desktop)), "{id}");
    }
    for id in [
        "verify",
        "embed-media",
        "remove-unused-media",
        "undo",
        "redo",
    ] {
        assert!(routes.contains(&(id, Route::Desktop)), "{id}");
    }
}

// -- the Track menu ----------------------------------------------------------------------

fn application() -> (AppContext, TempDir) {
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
            name: "Song".to_owned(),
            ..NewProject::default()
        })
        .expect("a project");
    (app, dir)
}

#[test]
fn the_track_menu_creates_the_prototypes_tracks() {
    let (app, _dir) = application();
    let cases = [
        (
            MenuAction::NewInstrumentTrack,
            "Instrument",
            TrackKind::Instrument,
            Some("sonara.synth.poly"),
        ),
        (
            MenuAction::NewDrumTrack,
            "Drums",
            TrackKind::Instrument,
            Some("sonara.drums"),
        ),
        (MenuAction::NewAudioTrack, "Audio", TrackKind::Audio, None),
        (MenuAction::NewBus, "Bus", TrackKind::Bus, None),
        (MenuAction::NewFolder, "Folder", TrackKind::Folder, None),
    ];
    for (action, name, kind, plugin) in cases {
        let command = PageCommand::of(action).expect("a page command");
        assert_eq!(command.command, action.id());
        // The page runs the payload's edit as it is: it must be the edit boundary's own.
        let payload = serde_json::to_value(&command).expect("JSON");
        let edit = Edit::from_json(payload["edit"].clone()).expect("a create_track edit");
        let outcome = edit::apply(&app, edit).expect("the track is created");
        assert_eq!(outcome.label, "Create Track");
        let track_id = outcome.result["track_id"]
            .as_str()
            .expect("the new track's ID, which the page selects")
            .to_owned();
        let project = app.session().project().expect("a project");
        let track = project
            .tracks
            .iter()
            .find(|t| t.id == track_id)
            .expect("the track");
        assert_eq!(
            (track.name.as_str(), track.kind),
            (name, kind),
            "{action:?}"
        );
        assert_eq!(
            track.instrument.as_ref().map(|d| d.plugin_id.as_str()),
            plugin,
            "{action:?}"
        );
        assert_eq!(track.output_id.as_deref(), Some("master"));
    }
    // The other page commands carry no edit; the page has what they act on.
    for action in [
        MenuAction::DeleteSelectedTrack,
        MenuAction::DuplicateClips,
        MenuAction::PlayStop,
        MenuAction::Metronome,
    ] {
        let command = PageCommand::of(action).expect("a page command");
        assert_eq!(new_track(action), None);
        assert_eq!(
            serde_json::to_value(&command).expect("JSON"),
            json!({"command": action.id()})
        );
    }
}

// -- texts and enabled states -----------------------------------------------------------------------

#[test]
fn undo_redo_and_open_recent_follow_the_session() {
    let (app, dir) = application();
    let chrome = WindowChrome::of(app.session());
    assert_eq!((chrome.undo.as_str(), chrome.can_undo), ("Undo", false));
    assert_eq!((chrome.redo.as_str(), chrome.can_redo), ("Redo", false));
    let command = PageCommand::of(MenuAction::NewBus).expect("a page command");
    let payload = serde_json::to_value(&command).expect("JSON");
    edit::apply(
        &app,
        Edit::from_json(payload["edit"].clone()).expect("an edit"),
    )
    .expect("a bus");
    let chrome = WindowChrome::of(app.session());
    assert_eq!(
        (chrome.undo.as_str(), chrome.can_undo),
        ("Undo Create Track", true)
    );
    edit::undo(&app).expect("an undo");
    let chrome = WindowChrome::of(app.session());
    assert_eq!((chrome.undo.as_str(), chrome.can_undo), ("Undo", false));
    assert_eq!(
        (chrome.redo.as_str(), chrome.can_redo),
        ("Redo Create Track", true)
    );

    // Open Recent: "No recent projects", disabled, until a project is saved.
    let none = recent_items(&app.projects().recent_projects());
    assert_eq!(
        serde_json::to_value(&none).expect("JSON"),
        json!([{"id": "no-recent", "text": "No recent projects", "enabled": false}])
    );
    let path = dir.path().join("Song.sonara");
    app.projects().save(Some(&path), None).expect("saved");
    let recent = app.projects().recent_projects();
    let items = recent_items(&recent);
    assert_eq!(items.len(), recent.len());
    assert_eq!(
        items[0].text, "Song",
        "named by the file stem, as `_fill_recent`"
    );
    assert!(items.iter().all(|item| item.enabled));
    assert_eq!(recent_index(&items[0].id), Some(0));
    let paths = ["/a/Song.sonara".to_owned(), "/b/Demo Mix.sonara".to_owned()];
    let named: Vec<(String, String)> = recent_items(&paths)
        .into_iter()
        .map(|item| (item.id, item.text))
        .collect();
    assert_eq!(
        named,
        [
            ("recent:0".to_owned(), "Song".to_owned()),
            ("recent:1".to_owned(), "Demo Mix".to_owned())
        ]
    );
}

#[test]
fn g9_the_edit_menus_clip_items_act_on_the_focused_editor() {
    let states = |focus| {
        edit_items(focus)
            .iter()
            .map(|item| (item.action.id(), item.text, item.enabled))
            .collect::<Vec<_>>()
    };
    let clips = vec![
        ("split-clips", "Split Clips at Playhead", true),
        ("duplicate-clips", "Duplicate Clips", true),
        ("delete-clips", "Delete Clips", true),
    ];
    // The timeline's own split, duplicate and delete; with no editor focused, the same, as the
    // prototype's items acted on the selected clips from anywhere.
    assert_eq!(states(EditFocus::Timeline), clips);
    assert_eq!(states(EditFocus::None), clips);
    // The piano roll's notes; it splits nothing.
    assert_eq!(
        states(EditFocus::PianoRoll),
        [
            ("split-clips", "Split Clips at Playhead", false),
            ("duplicate-clips", "Duplicate Notes", true),
            ("delete-clips", "Delete Notes", true),
        ]
    );
    // A device card has none of them.
    assert!(
        states(EditFocus::Devices)
            .iter()
            .all(|(_, _, enabled)| !enabled)
    );
    // The page names the focus as the desktop reads it.
    for (focus, name) in [
        (EditFocus::None, "none"),
        (EditFocus::Timeline, "timeline"),
        (EditFocus::PianoRoll, "piano-roll"),
        (EditFocus::Devices, "devices"),
    ] {
        assert_eq!(serde_json::to_value(focus).expect("JSON"), json!(name));
        assert_eq!(
            serde_json::from_value::<EditFocus>(json!(name)).expect("a focus"),
            focus
        );
    }
}

// -- the page's fixture ---------------------------------------------------------------------------

/// The menu's model as the page reads it.
fn page_document() -> Value {
    let menus: Vec<Value> = menu_bar()
        .into_iter()
        .map(|menu| {
            let entries: Vec<Value> = menu
                .entries
                .iter()
                .map(|entry| match entry {
                    Entry::Item(item) | Entry::Check(item) => json!({
                        "type": if matches!(entry, Entry::Check(_)) { "check" } else { "item" },
                        "id": item.action.id(),
                        "text": item.text,
                        "shortcut": item.shortcut.map(|s| s.glyphs()),
                    }),
                    Entry::Separator => json!({"type": "separator"}),
                    Entry::System(system) => {
                        json!({"type": "system", "text": system_text(*system)})
                    }
                    Entry::OpenRecent => json!({"type": "open-recent", "text": "Open Recent"}),
                })
                .collect();
            json!({"title": menu.title, "entries": entries})
        })
        .collect();
    let page_commands: Vec<Value> = MenuAction::ALL
        .into_iter()
        .filter_map(PageCommand::of)
        .map(|command| serde_json::to_value(command).expect("JSON"))
        .collect();
    let focus: Vec<Value> = [
        EditFocus::None,
        EditFocus::Timeline,
        EditFocus::PianoRoll,
        EditFocus::Devices,
    ]
    .into_iter()
    .map(|focus| json!({"focus": focus, "items": edit_items(focus)}))
    .collect();
    json!({
        "note": "Written by crates/sonara-app/tests/gui_menus.rs with SONARA_WRITE_FIXTURES=1 from \
                 sonara-app's menus module. The page's tests/unit/menus.test.ts checks its command \
                 table and its focus names against it.",
        "menus": menus,
        "page_commands": page_commands,
        "edit_items": focus,
    })
}

/// `apps/sonara-desktop/tests/fixtures/menus-v1.json`: `SONARA_WRITE_FIXTURES=1` rewrites it;
/// otherwise this test checks that the model still gives exactly it.
#[test]
fn the_page_shares_the_menu_fixture() {
    let document = page_document();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/sonara-desktop/tests/fixtures/menus-v1.json");
    if std::env::var_os("SONARA_WRITE_FIXTURES").is_some() {
        let text = serde_json::to_string_pretty(&document).expect("JSON") + "\n";
        fs::write(&path, text).expect("the fixture is written");
    }
    let fixture: Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("the shared fixture"))
            .expect("JSON");
    assert_eq!(
        fixture, document,
        "the menu's model still gives the fixture"
    );
}
