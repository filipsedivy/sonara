//! The privacy proof of the AI Chat: what is said in a chat never reaches a saved project.
//!
//! The chat lives in memory only (`sonara_app::chat`); a project file is what `save` writes. This
//! test pins that the two never meet:
//!
//! - **Byte identity.** Two separate application contexts do the same fixed script of project
//!   edits and saves (a bundle, an in-place save with its backup, a packaged archive). One has a
//!   chat that is used all the while, with a sentinel text in its prompts and replies, and
//!   done, failed, cancelled, edited, removed, moved and wiped tasks. Every file of the saved
//!   bundle and the archive must be identical, byte for byte, and no file anywhere under the
//!   context's folder may hold the sentinel.
//! - **No side effect.** A chat-only call never changes the session's version or undo history
//!   and never marks the project dirty.
//! - **No path.** The chat sources import nothing from the project crates or from the
//!   application's project services, and the project schema has no chat property.
//!
//! The clock and the ids of both contexts are the ones the golden tests use (a clock that
//! ticks a microsecond per reading, sequential ids), so no byte differs for a reason that has
//! nothing to do with the chat.

#![cfg(unix)]

use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde_json::{Value, json};
use sonara_app::chat::{
    Availability, ChatConfig, ChatController, ChatEvent, Provider, ProviderId, ProviderStatus,
    QueueMode, Sink, TaskId, TaskState, Turn,
};
use sonara_app::clock::WallClock;
use sonara_app::composer::Workspace;
use sonara_app::composer::locate::Version;
use sonara_app::composer::{ClaudeParser, Event, Invocation, LineParser, Located, StopPolicy};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_app::projects::NewProject;
use sonara_app::settings::Settings;
use sonara_core::commands::Command;
use sonara_core::commands::project::{AddAsset, AddMarker, RenameProject, SetTempo};
use sonara_core::commands::track::CreateTrack;
use sonara_core::ids::{self, IdGenerator as _, SequentialIds};
use sonara_core::model::JsonMap;
use sonara_core::model::media::{MediaAsset, MediaAssetData, MediaKind, MediaLocation};
use sonara_core::model::track::TrackKind;
use sonara_project::atomic::sha256_file;

/// What the chat says and is told. Non-ASCII on purpose, so that an encoding slip shows too.
const SENTINEL: &str = "CHAT-SENTINEL-7f3a ü 日本語";

const AUDIO_REL: &str = "media/audio/med_vox-vox.wav";
const FROZEN_UNIX_SECONDS: u64 = 1_767_268_800;
const FRAMES: u32 = 24_000;

// -- the project side -----------------------------------------------------------------------

/// The golden tests' clock: frozen at 2026-01-01T12:00:00Z, a microsecond per reading.
fn ticking_clock() -> WallClock {
    let ticks = AtomicU64::new(0);
    Arc::new(move || {
        UNIX_EPOCH
            + Duration::from_secs(FROZEN_UNIX_SECONDS)
            + Duration::from_micros(ticks.fetch_add(1, Ordering::Relaxed))
    })
}

fn app(dir: &Path) -> Arc<AppContext> {
    let settings = Settings {
        // Pinned: the number of backups kept must not depend on a default.
        backup_retention: 3,
        ..Settings::default()
    };
    Arc::new(
        AppContext::create(AppOptions {
            workspaces: Some(dir.join("workspaces")),
            settings: Some(settings),
            persist_settings: false,
            audio_backend: Some("null".into()),
            ids: Some(IdSource::new(Box::new(SequentialIds::new(0)))),
            wall_clock: Some(ticking_clock()),
            ..AppOptions::default()
        })
        .expect("context"),
    )
}

fn write_wav(path: &Path) {
    fs::create_dir_all(path.parent().expect("parent")).expect("directory");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("WAV writer");
    for frame in 0..FRAMES {
        let value = (i32::try_from(frame % 4096).expect("small") - 2048) * 1000;
        writer.write_sample(value).expect("sample");
        writer.write_sample(-value).expect("sample");
    }
    writer.finalize().expect("WAV written");
}

fn asset_for(workspace: &Path) -> MediaAsset {
    let path = workspace.join(AUDIO_REL);
    let mut metadata = JsonMap::new();
    metadata.insert("title".into(), json!("Vox"));
    MediaAsset::new(MediaAssetData {
        id: "med_vox".into(),
        kind: MediaKind::Audio,
        name: "vox.wav".into(),
        path: AUDIO_REL.into(),
        location: MediaLocation::Embedded,
        size_bytes: fs::metadata(&path).expect("asset file").len(),
        sha256: Some(sha256_file(&path).expect("hash")),
        sample_rate: 48_000,
        channels: 2,
        frames: u64::from(FRAMES),
        format: "WAV".into(),
        subtype: "PCM_24".into(),
        original_path: Some("/original/vox.wav".into()),
        relative_hint: None,
        metadata,
        extra: JsonMap::new(),
    })
    .expect("valid asset")
}

type Script = VecDeque<Box<dyn Command>>;

/// The first part of the edit script: what an agent builds while the chat runs.
fn first_edits(app: &AppContext, workspace: &Path) -> Script {
    write_wav(&workspace.join(AUDIO_REL));
    let mut source = app.ids();
    let mut track = |name: &str| CreateTrack {
        name: name.into(),
        kind: TrackKind::Audio,
        index: None,
        color: None,
        output_id: ids::MASTER_ID.into(),
        instrument_plugin: None,
        instrument_preset: None,
        parent_id: None,
        track_id: source.new_id(ids::TRACK),
        instrument_id: source.new_id(ids::DEVICE),
    };
    let commands: Vec<Box<dyn Command>> = vec![
        Box::new(AddAsset {
            asset: asset_for(workspace),
        }),
        Box::new(RenameProject {
            name: "Chat Proof".into(),
        }),
        Box::new(SetTempo {
            bpm: 96.0,
            beat: 0.0,
            ramp: false,
            clear_changes: true,
        }),
        Box::new(track("Vox")),
        Box::new(track("Bass")),
        Box::new(AddMarker {
            beat: 8.0,
            name: "Drop".into(),
            color: None,
            marker_id: source.new_id(ids::MARKER),
        }),
    ];
    commands.into()
}

/// The second part: what the user does after the first save.
fn second_edits(app: &AppContext) -> Script {
    let mut source = app.ids();
    let commands: Vec<Box<dyn Command>> = vec![
        Box::new(RenameProject {
            name: "Chat Proof 2".into(),
        }),
        Box::new(AddMarker {
            beat: 32.0,
            name: "Outro".into(),
            color: Some("#ff8800".into()),
            marker_id: source.new_id(ids::MARKER),
        }),
        Box::new(SetTempo {
            bpm: 100.0,
            beat: 16.0,
            ramp: true,
            clear_changes: false,
        }),
    ];
    commands.into()
}

/// Runs the next command of `script`, if any. The lock is held while it runs, so that the
/// order of execution is the order of the script whichever thread asks.
fn step(app: &AppContext, script: &Mutex<Script>) -> bool {
    let mut script = script.lock().expect("script");
    let Some(command) = script.pop_front() else {
        return false;
    };
    app.session()
        .lock()
        .execute(command.as_ref(), false, "agent")
        .expect("command applies");
    true
}

fn drain(app: &AppContext, script: &Mutex<Script>) {
    while step(app, script) {}
}

/// What a context's session says: any chat-only call must leave it as it was.
fn session_marks(app: &AppContext) -> (u64, bool, bool, bool) {
    let session = app.session().lock();
    (
        session.version(),
        session.is_dirty(),
        session.history().can_undo(),
        session.history().can_redo(),
    )
}

// -- the saved bytes ------------------------------------------------------------------------

/// Every file under `root`: its path relative to `root` and its bytes.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, directory: &Path, into: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries: Vec<_> = fs::read_dir(directory)
            .expect("directory")
            .map(|entry| entry.expect("entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(root, &path, into);
            } else {
                let relative = path.strip_prefix(root).expect("inside");
                into.insert(
                    relative.to_string_lossy().into_owned(),
                    fs::read(&path).expect("file"),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}

fn assert_same_bytes(what: &str, a: &[u8], b: &[u8]) {
    if a != b {
        let at = a
            .iter()
            .zip(b)
            .position(|(x, y)| x != y)
            .unwrap_or(a.len().min(b.len()));
        panic!(
            "{what} differs: {} vs {} bytes, first difference at byte {at}",
            a.len(),
            b.len()
        );
    }
}

fn assert_same_tree(what: &str, a: &BTreeMap<String, Vec<u8>>, b: &BTreeMap<String, Vec<u8>>) {
    assert_eq!(
        a.keys().collect::<Vec<_>>(),
        b.keys().collect::<Vec<_>>(),
        "{what}: the files differ"
    );
    for (path, bytes) in a {
        assert_same_bytes(&format!("{what}: {path}"), bytes, &b[path]);
    }
}

/// The paths under `root` whose bytes contain `needle`.
fn files_containing(root: &Path, needle: &str) -> Vec<String> {
    tree(root)
        .into_iter()
        .filter(|(_, bytes)| bytes.windows(needle.len()).any(|w| w == needle.as_bytes()))
        .map(|(path, _)| path)
        .collect()
}

/// Sets the modification time of every file under `directory` to the frozen clock's start.
fn freeze_modification_times(directory: &Path) {
    let frozen = UNIX_EPOCH + Duration::from_secs(FROZEN_UNIX_SECONDS);
    for entry in fs::read_dir(directory).expect("directory") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            freeze_modification_times(&path);
        } else {
            fs::File::options()
                .write(true)
                .open(&path)
                .expect("file")
                .set_modified(frozen)
                .expect("modification time");
        }
    }
}

/// What an arm leaves behind.
struct Saved {
    bundle: BTreeMap<String, Vec<u8>>,
    archive: Vec<u8>,
    /// The folder the arm worked in, for the search for the sentinel.
    root: PathBuf,
}

// -- the chat side --------------------------------------------------------------------------

/// A fake agent: it reads its message, may fail or wait to be stopped, and answers with the
/// whole message, so the sentinel comes back in the reply. It writes only into `$OUT`, which is
/// outside the context's folders.
const AGENT: &str = r#"
input=$(cat)
last=$(printf '%s' "$input" | tail -n 1)
word=${last%% *}
case "$word" in
  SLOW) trap 'exit 130' INT ;;
esac
echo '{"type":"system","subtype":"init","session_id":"s","model":"m","tools":[],"mcp_servers":[{"name":"sonara","status":"connected"}]}'
case "$word" in
  FAIL) echo "boom" >&2; exit 1 ;;
  SLOW)
    echo '{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"thinking"}}}'
    while true; do sleep 0.05; done ;;
esac
echo '{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"working"}}}'
sleep 0.05
printf '{"type":"result","subtype":"success","is_error":false,"result":"reply: %s"}\n' "$last"
"#;

struct FakeAgent {
    script: PathBuf,
}

impl Provider for FakeAgent {
    fn id(&self) -> ProviderId {
        ProviderId::Claude
    }

    fn name(&self) -> &'static str {
        "Fake Agent"
    }

    fn detect(&self) -> ProviderStatus {
        ProviderStatus {
            id: ProviderId::Claude,
            name: "Fake Agent".into(),
            availability: Availability::Ready,
            version: Some("1.0.0".into()),
            path: None,
            hint: "ready".into(),
            beta: false,
            min_version: "1.0.0".into(),
            located: Some(Located {
                path: "/bin/sh".into(),
                version: Version::parse("1.0.0").expect("version"),
            }),
        }
    }

    fn build(
        &self,
        located: &Located,
        _sonara_cli: &Path,
        workspace: &Workspace,
        turn: &Turn,
    ) -> Invocation {
        Invocation {
            program: located.path.clone(),
            args: vec![self.script.clone().into_os_string()],
            cwd: workspace.cwd(),
            env: Vec::new(),
            stdin: turn.stdin_text(),
        }
    }

    fn parser(&self) -> Box<dyn LineParser> {
        Box::new(ClaudeParser)
    }

    fn stop_policy(&self) -> StopPolicy {
        StopPolicy {
            interrupt_grace: Duration::from_secs(5),
            terminate_grace: Duration::from_secs(5),
        }
    }
}

/// A chat with its recorder; `on_run_event` is called for every run event the window would see.
struct Rig {
    _scratch: tempfile::TempDir,
    chat: ChatController,
    events: Arc<Mutex<Vec<ChatEvent>>>,
}

impl Rig {
    /// `on_run_event` runs on the delivery thread for each run event, like a window's handler
    /// (and like an agent that edits the project while it talks).
    fn new(on_run_event: impl Fn() + Send + Sync + 'static) -> Self {
        let scratch = tempfile::tempdir().expect("scratch");
        let script = scratch.path().join("agent.sh");
        fs::write(&script, AGENT).expect("script");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o644)).expect("mode");
        let discovery = scratch.path().join("session-1.json");
        fs::write(&discovery, "{}").expect("discovery file");
        let events: Arc<Mutex<Vec<ChatEvent>>> = Arc::default();
        let sink: Sink = {
            let events = Arc::clone(&events);
            Arc::new(move |event| {
                if matches!(event, ChatEvent::Run { .. }) {
                    on_run_event();
                }
                events.lock().expect("events").push(event);
            })
        };
        let chat = ChatController::new(
            vec![Arc::new(FakeAgent { script })],
            ChatConfig {
                sonara_cli: "/App/sonara".into(),
                discovery_file: discovery,
            },
            sink,
        );
        Self {
            _scratch: scratch,
            chat,
            events,
        }
    }
}

fn wait_until<T>(what: &str, check: impl Fn() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(value) = check() {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(10));
    }
}

/// What a scenario made the chat do, for the anti-vacuity guard.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Outcomes {
    done: usize,
    failed: usize,
    cancelled: usize,
    removed: usize,
    edited: usize,
    moved: usize,
}

impl std::ops::AddAssign for Outcomes {
    fn add_assign(&mut self, other: Self) {
        self.done += other.done;
        self.failed += other.failed;
        self.cancelled += other.cancelled;
        self.removed += other.removed;
        self.edited += other.edited;
        self.moved += other.moved;
    }
}

/// One conversation with every kind of outcome. `after_call` runs after every chat call.
fn scenario(rig: &Rig, tag: &str, after_call: &dyn Fn()) -> Outcomes {
    let chat = &rig.chat;
    let say = |what: &str| format!("{SENTINEL} {tag} {what}");
    let state_of = |id: TaskId| {
        chat.snapshot()
            .state
            .tasks
            .into_iter()
            .find(|task| task.id == id)
            .map(|task| task.state)
    };
    let wait_state = |id: TaskId, state: TaskState| {
        wait_until(&format!("task {id} to be {state:?}"), || {
            (state_of(id) == Some(state)).then_some(())
        });
    };

    let first = chat.enqueue(&say("first")).expect("enqueued");
    after_call();
    // A failing message pauses the queue; the others wait behind it.
    let failing = chat
        .enqueue(&format!("FAIL {SENTINEL} {tag}"))
        .expect("enqueued");
    let third = chat.enqueue(&say("third")).expect("enqueued");
    let fourth = chat.enqueue(&say("fourth")).expect("enqueued");
    let fifth = chat.enqueue(&say("fifth")).expect("enqueued");
    after_call();
    wait_state(first, TaskState::Done);
    wait_state(failing, TaskState::Failed);
    assert_eq!(chat.snapshot().state.mode, QueueMode::Paused);
    chat.edit(third, &say("third, edited")).expect("edited");
    after_call();
    chat.remove(fourth).expect("removed");
    after_call();
    chat.move_task(fifth, 0).expect("moved");
    after_call();
    let _ = chat.resume();
    after_call();
    wait_state(third, TaskState::Done);
    wait_state(fifth, TaskState::Done);
    // A message that is stopped.
    let slow = chat
        .enqueue(&format!("SLOW {SENTINEL} {tag}"))
        .expect("enqueued");
    wait_until("the slow run to start", || {
        rig.events
            .lock()
            .expect("events")
            .iter()
            .any(|event| {
                matches!(event, ChatEvent::Run { task_id, event: Event::Init { .. }, .. } if *task_id == slow)
            })
            .then_some(())
    });
    let _ = chat.stop();
    after_call();
    wait_state(slow, TaskState::Cancelled);
    wait_until("the chat to be idle", || (!chat.is_busy()).then_some(()));

    let tasks = chat.snapshot().state.tasks;
    let count = |state: TaskState| tasks.iter().filter(|task| task.state == state).count();
    // The edit and the move did what they say, and the removed task is gone.
    let by_prompt = |what: &str| tasks.iter().position(|task| task.prompt == say(what));
    assert!(by_prompt("third, edited").is_some() && by_prompt("third").is_none());
    assert!(by_prompt("fourth").is_none(), "the removed task is gone");
    assert!(
        by_prompt("fifth") < by_prompt("third, edited"),
        "the moved task came first"
    );
    // The replies carry the sentinel: the chat really had text to leak.
    let reply = tasks
        .iter()
        .find(|task| task.state == TaskState::Done)
        .and_then(|task| task.result.as_ref())
        .map(|result| result.text.clone())
        .expect("a reply");
    assert!(reply.contains(SENTINEL), "{reply}");
    // The next conversation starts with a clean thread.
    let _ = chat.clear();
    after_call();
    Outcomes {
        done: count(TaskState::Done),
        failed: count(TaskState::Failed),
        cancelled: count(TaskState::Cancelled),
        removed: 1,
        edited: 1,
        moved: 1,
    }
}

/// Everything the chat said, as the window saw it: the sentinel must be in there.
fn sentinel_events(rig: &Rig) -> usize {
    rig.events
        .lock()
        .expect("events")
        .iter()
        .filter(|event| match event {
            ChatEvent::Changed { state } => state
                .tasks
                .iter()
                .any(|task| task.prompt.contains(SENTINEL)),
            ChatEvent::Run { event, .. } => serde_json::to_string(event)
                .expect("json")
                .contains("CHAT-SENTINEL-7f3a"),
        })
        .count()
}

// -- the two arms ---------------------------------------------------------------------------

fn save_everything(app: &AppContext, dir: &Path, script: &dyn Fn(&str)) -> Saved {
    let svc = app.projects();
    script("bundle");
    let bundle_path = dir.join("Proof.sonara");
    svc.save(Some(&bundle_path), None).expect("bundle saved");
    script("in place");
    svc.save(None, None).expect("saved in place, with a backup");
    let bundle = tree(&bundle_path);
    // The archive stores each media file's modification time (to two seconds), which is real
    // file-system time and has nothing to do with the chat: freeze it, so that the two arms
    // cannot differ by when they happened to run.
    freeze_modification_times(&bundle_path);
    script("archive");
    let archive_path = dir.join("Proof Packaged.sonara");
    svc.save(Some(&archive_path), Some(true))
        .expect("archive saved");
    Saved {
        bundle,
        archive: fs::read(&archive_path).expect("archive"),
        root: dir.to_path_buf(),
    }
}

/// Arm A: the project alone.
fn arm_without_chat(dir: &Path) -> Saved {
    let app = app(dir);
    app.projects()
        .new_project(&NewProject {
            name: "Chat Proof".into(),
            ..NewProject::default()
        })
        .expect("created");
    let workspace = app
        .session()
        .lock()
        .require_location()
        .expect("open")
        .workspace
        .clone();
    let first = Mutex::new(first_edits(&app, &workspace));
    drain(&app, &first);
    let second = Mutex::new(second_edits(&app));
    save_everything(&app, dir, &|stage| {
        if stage == "in place" {
            drain(&app, &second);
        }
    })
}

/// Arm B: the same project, with a chat that is used throughout.
fn arm_with_chat(dir: &Path) -> (Saved, Outcomes, usize) {
    let app = app(dir);
    app.projects()
        .new_project(&NewProject {
            name: "Chat Proof".into(),
            ..NewProject::default()
        })
        .expect("created");
    let workspace = app
        .session()
        .lock()
        .require_location()
        .expect("open")
        .workspace
        .clone();
    let first = Arc::new(Mutex::new(first_edits(&app, &workspace)));
    let mut outcomes = Outcomes::default();

    // 1. Chat only, on the new project: the session must not notice.
    let quiet = Rig::new(|| {});
    let marks = session_marks(&app);
    outcomes += scenario(&quiet, "quiet", &|| {
        assert_eq!(
            session_marks(&app),
            marks,
            "a chat call touched the session"
        );
    });
    let _ = quiet.chat.on_project_replaced();
    assert_eq!(session_marks(&app), marks);
    let mut sentinels = sentinel_events(&quiet);

    // 2. The first edits are made by the "agent" while the chat talks: from the delivery thread
    //    of its run events, one command per event.
    let busy = {
        let app = Arc::clone(&app);
        let first = Arc::clone(&first);
        Rig::new(move || {
            step(&app, &first);
        })
    };
    outcomes += scenario(&busy, "busy", &|| {});
    drain(&app, &first);
    sentinels += sentinel_events(&busy);

    // 3. The saves. The user's second edits run while the chat is in use on another thread.
    let second = Mutex::new(second_edits(&app));
    let concurrent = Mutex::new(Outcomes::default());
    let running = Arc::new(Rig::new(|| {}));
    let saved = save_everything(&app, dir, &|stage| {
        if stage == "in place" {
            let rig = Arc::clone(&running);
            let chat_thread = thread::spawn(move || scenario(&rig, "concurrent", &|| {}));
            while step(&app, &second) {
                thread::sleep(Duration::from_millis(20));
            }
            *concurrent.lock().expect("outcomes") += chat_thread.join().expect("the chat thread");
        }
    });
    outcomes += *concurrent.lock().expect("outcomes");
    sentinels += sentinel_events(&running);

    // 4. After the saves the project is clean; chat-only calls keep it so.
    let marks = session_marks(&app);
    assert!(!marks.1, "the saved project is clean");
    outcomes += scenario(&running, "after", &|| {
        assert_eq!(
            session_marks(&app),
            marks,
            "a chat call touched the session"
        );
    });
    let _ = running.chat.clear();
    let _ = running.chat.on_project_replaced();
    assert_eq!(session_marks(&app), marks);
    sentinels += sentinel_events(&running);
    (saved, outcomes, sentinels)
}

#[test]
fn saving_after_a_chat_is_byte_identical_to_saving_without_one() {
    let a = tempfile::tempdir().expect("dir A");
    let b = tempfile::tempdir().expect("dir B");
    // The same folder layout in both: only the root differs.
    let (dir_a, dir_b) = (
        fs::canonicalize(a.path()).expect("resolved"),
        fs::canonicalize(b.path()).expect("resolved"),
    );
    let without = arm_without_chat(&dir_a);
    let (with, outcomes, sentinels) = arm_with_chat(&dir_b);

    // Anti-vacuity: the chat was used, with every kind of outcome, and had the sentinel in
    // what it said. (Chat events carry text of the prompts and the replies.)
    assert!(outcomes.done >= 9, "{outcomes:?}");
    assert!(
        outcomes.failed >= 4 && outcomes.cancelled >= 4,
        "{outcomes:?}"
    );
    assert!(outcomes.removed >= 4 && outcomes.edited >= 4 && outcomes.moved >= 4);
    assert!(
        sentinels > 20,
        "the chat had text to leak: {sentinels} events"
    );

    // The files of the project are the same.
    assert!(
        without
            .bundle
            .keys()
            .any(|path| path.starts_with("backups/")),
        "a backup was made"
    );
    assert!(without.bundle.contains_key("project.json"));
    assert!(
        without.bundle.contains_key(AUDIO_REL),
        "the media was copied"
    );
    assert_same_tree("the bundle", &without.bundle, &with.bundle);
    assert!(without.archive.len() > 1_000, "an archive was written");
    assert_same_bytes("the packaged archive", &without.archive, &with.archive);

    // Nothing of the chat is anywhere under the contexts' folders: not in the saved files,
    // not in the workspaces, backups or settings. The archive is compressed, so look in what
    // opening it extracts as well.
    for saved in [&without, &with] {
        assert_eq!(
            files_containing(&saved.root, "CHAT-SENTINEL"),
            Vec::<String>::new()
        );
        assert_eq!(
            files_containing(&saved.root, "日本語"),
            Vec::<String>::new()
        );
    }
    let reopened = app(&dir_b.join("reopen"));
    reopened
        .projects()
        .open_project(&dir_b.join("Proof Packaged.sonara"), false)
        .expect("the archive opens");
    assert_eq!(
        files_containing(&dir_b.join("reopen"), "CHAT-SENTINEL"),
        Vec::<String>::new()
    );
    let project: Value = serde_json::from_slice(&with.bundle["project.json"]).expect("JSON");
    assert_eq!(project["name"], "Chat Proof 2", "the edits are in the file");
    assert!(!project.to_string().to_lowercase().contains("chat-sentinel"));
}

// -- no path from the chat to the project ---------------------------------------------------

fn sources(directory: &Path, into: &mut Vec<(PathBuf, String)>) {
    for entry in fs::read_dir(directory).expect("directory") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            sources(&path, into);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let text = fs::read_to_string(&path).expect("source");
            into.push((path, text));
        }
    }
}

/// The code of a source file: comment lines and the text after `//` are left out.
fn code_of(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .map(|line| line.split(" // ").next().unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_chat_sources_reach_neither_the_project_crates_nor_the_project_services() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/chat");
    let mut files = Vec::new();
    sources(&root, &mut files);
    assert!(
        files.len() >= 10,
        "the chat sources were found: {}",
        files.len()
    );
    // The crates that hold the project, its format, the ids and the audio engine.
    let crates = [
        "sonara_core",
        "sonara_project",
        "sonara_midi",
        "sonara_engine",
        "sonara_media",
        "sonara_dsp",
        "sonara_io",
        "sonara_rt",
        "sonara_agent",
    ];
    // The only parts of this crate the chat may use: itself and the process harness.
    let allowed = ["chat", "composer"];
    for (path, text) in &files {
        let code = code_of(text);
        for name in crates {
            assert!(!code.contains(name), "{} uses {name}", path.display());
        }
        for piece in code.split("crate::").skip(1) {
            let module: String = piece
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            assert!(
                allowed.contains(&module.as_str()),
                "{} uses crate::{module}, which is not the chat's to use",
                path.display()
            );
        }
        for token in [
            "AppContext",
            "IdSource",
            ".ids()",
            "SharedSession",
            "ProjectService",
        ] {
            assert!(!code.contains(token), "{} mentions {token}", path.display());
        }
    }
    // The guard is not vacuous: it sees what the composer imports, and nothing there either.
    let mut composer = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/composer"),
        &mut composer,
    );
    assert!(composer.len() >= 6);
    for (path, text) in &composer {
        let code = code_of(text);
        assert!(
            !code.contains("sonara_core") && !code.contains("sonara_project"),
            "{}",
            path.display()
        );
    }
    assert!(
        code_of("use sonara_project::store;").contains("sonara_project"),
        "the filter keeps real code"
    );
}

/// Every property name the schema declares, at any depth.
fn property_names(value: &Value, into: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                if key == "properties"
                    && let Value::Object(properties) = inner
                {
                    into.extend(properties.keys().cloned());
                }
                property_names(inner, into);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| property_names(item, into)),
        _ => {}
    }
}

#[test]
fn the_project_schema_has_no_chat_property() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative in [
        "../sonara-project/schemas/project-v1.schema.json",
        "../sonara-project/schemas/manifest-v1.schema.json",
        "../../golden/contracts/schemas/project-v1.schema.json",
    ] {
        let path = manifest.join(relative);
        let schema: Value =
            serde_json::from_slice(&fs::read(&path).expect("schema")).expect("JSON");
        let mut names = Vec::new();
        property_names(&schema, &mut names);
        assert!(
            names.len() > 10,
            "{relative} declares properties ({})",
            names.len()
        );
        for name in &names {
            let lower = name.to_lowercase();
            for word in [
                "chat",
                "conversation",
                "transcript",
                "agent_prompt",
                "task_queue",
            ] {
                assert!(!lower.contains(word), "{relative} has the property {name}");
            }
        }
    }
}
