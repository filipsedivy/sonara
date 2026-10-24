//! The benchmarks' hook (ADR 0009's timeline, meter and piano-roll
//! thresholds), built only with the `bench` feature, which the release bundle never enables.
//!
//! `SONARA_BENCH_OUT=<report.json>` starts the window in benchmark mode: it stays on top and on
//! every Space (the web view stops drawing a hidden window), and the page runs the suite of
//! `SONARA_BENCH_SUITE` (`src/lib/bench/timeline.ts`, `e2e.ts`, `meters.ts`, `pianoroll.ts` or
//! `playback.ts`) on the opened project, hands the report to [`bench_report`], which writes it,
//! and the application exits.
//! `scripts/bench-timeline.mjs` builds the project, the binary, runs it and checks the report.
//!
//! The meter suite measures from the Rust sample to the pixel: [`FrameStamps`] records when the
//! live channel sent each frame, on a monotonic clock that the page reads through
//! [`bench_clock`] (the spike's handshake maps it onto `performance.now()`), and the page matches
//! the frames it drew by their sequence numbers ([`bench_live_stamps`]).
//!
//! The end-to-end harness of the menus (`src/lib/bench/e2e-menus.ts`) answers
//! the native prompts and panels from a queue ([`Prompts`], [`answer`]: a benchmark run never
//! shows one), reads the menu items the desktop handled ([`MenuLog`]), reads the native menu bar
//! ([`bench_native_menu`]), chooses a native item as a click does ([`bench_activate_menu`]), and
//! posts real key events into the application's event queue ([`bench_post_keys`]), which reach
//! the web view and the menu along the path of a key typed on the keyboard.
//!
//! The accessibility check (`src/lib/bench/e2e-a11y.ts`, which
//! `tests/e2e/a11y-tree.mjs` runs) holds the window at checkpoints ([`bench_checkpoint`]) while
//! the harness reads the application's native accessibility tree from outside, as the screen
//! reader reads it.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sonara_app::live::LiveFrame;
use tauri::{AppHandle, Manager, State};

use crate::prompt::{Buttons, Choice};
use crate::shell::Shell;

/// How many frame stamps are kept (over two minutes at 60 Hz).
const STAMPS_KEPT: usize = 8192;

/// When the live channel sent each frame: its sequence number and the milliseconds since the
/// clock's start.
#[derive(Clone)]
pub struct FrameStamps {
    epoch: Instant,
    sent: Arc<Mutex<VecDeque<(u32, f64)>>>,
}

impl Default for FrameStamps {
    fn default() -> Self {
        Self {
            epoch: Instant::now(),
            sent: Arc::new(Mutex::new(VecDeque::with_capacity(STAMPS_KEPT))),
        }
    }
}

impl FrameStamps {
    fn sent(&self) -> MutexGuard<'_, VecDeque<(u32, f64)>> {
        self.sent.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The clock in milliseconds.
    pub fn now_ms(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64() * 1000.0
    }

    /// Records the frame `bytes` (its sequence number is at bytes 8–11) as sent now.
    pub fn record(&self, bytes: &[u8]) {
        let Some(sequence) = bytes
            .get(8..12)
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
            .map(u32::from_le_bytes)
        else {
            return;
        };
        let now = self.now_ms();
        let mut sent = self.sent();
        if sent.len() == STAMPS_KEPT {
            sent.pop_front();
        }
        sent.push_back((sequence, now));
    }

    /// The stamps recorded since the last call.
    pub fn take(&self) -> Vec<(u32, f64)> {
        self.sent().drain(..).collect()
    }
}

/// The environment variable that names the report file.
pub const OUT_VAR: &str = "SONARA_BENCH_OUT";

/// The benchmark's options.
#[derive(Debug, Clone, Serialize)]
pub struct BenchConfig {
    /// Where the report goes.
    pub out: PathBuf,
    /// A short run (seconds instead of tens of seconds), to check the harness.
    pub quick: bool,
    /// `timeline` (the timeline benchmark), `meters` (the meter benchmark), `pianoroll` (the piano
    /// roll benchmark, `src/lib/bench/pianoroll.ts`), `playback` (playback on the audio
    /// device while the timeline scrolls, `src/lib/bench/playback.ts`), `e2e` (the page's
    /// end-to-end check,
    /// `src/lib/bench/e2e.ts`, which `scripts/e2e-timeline.mjs` runs) or `e2e-dialogs` (the
    /// dialogs' and the menus' end-to-end check, `src/lib/bench/e2e-dialogs.ts` and
    /// `e2e-menus.ts`, which `scripts/e2e-dialogs.mjs` runs) or `e2e-a11y` (the accessibility
    /// check, `src/lib/bench/e2e-a11y.ts`, which `tests/e2e/a11y-tree.mjs` runs).
    pub suite: String,
    /// An audio file the end-to-end check imports.
    pub media: Option<PathBuf>,
}

impl BenchConfig {
    /// The options from the environment; `None` unless [`OUT_VAR`] is set.
    pub fn from_env() -> Option<Self> {
        let out = std::env::var_os(OUT_VAR)?;
        Some(Self {
            out: PathBuf::from(out),
            quick: std::env::var_os("SONARA_BENCH_QUICK").is_some(),
            suite: std::env::var("SONARA_BENCH_SUITE").unwrap_or_else(|_| "timeline".to_owned()),
            media: std::env::var_os("SONARA_BENCH_MEDIA").map(PathBuf::from),
        })
    }
}

/// The benchmark's options, or `null` outside benchmark mode.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn bench_config(config: State<'_, Option<BenchConfig>>) -> Option<BenchConfig> {
    config.inner().clone()
}

/// Writes the page's report and exits.
///
/// # Errors
///
/// Not in benchmark mode, or the report cannot be written.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn bench_report(
    app: AppHandle,
    config: State<'_, Option<BenchConfig>>,
    report: Value,
) -> Result<(), String> {
    let config = config
        .inner()
        .as_ref()
        .ok_or("not started in benchmark mode")?;
    let text = serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?;
    std::fs::write(&config.out, text)
        .map_err(|error| format!("cannot write {}: {error}", config.out.display()))?;
    log::info!("benchmark report written to {}", config.out.display());
    app.exit(0);
    Ok(())
}

/// `<out>.<suffix>`, beside the report.
fn beside(out: &Path, suffix: &str) -> PathBuf {
    let mut name = out.as_os_str().to_owned();
    name.push(".");
    name.push(suffix);
    PathBuf::from(name)
}

/// How long [`bench_checkpoint`] waits for the harness.
const CHECKPOINT_WAIT: Duration = Duration::from_secs(120);

/// The accessibility check's checkpoint: writes `name` to `<out>.checkpoint`, so the
/// harness knows which state the window shows, and waits until `<out>.resume` holds `name`, when
/// the harness has read the accessibility tree. It waits on a worker thread: the main thread must
/// stay free, as it answers the accessibility queries.
///
/// # Errors
///
/// Not in benchmark mode, a file that cannot be written, or no resume within two minutes.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn bench_checkpoint(
    config: State<'_, Option<BenchConfig>>,
    name: String,
) -> Result<(), String> {
    let config = config
        .inner()
        .as_ref()
        .ok_or("not started in benchmark mode")?;
    let checkpoint = beside(&config.out, "checkpoint");
    let resume = beside(&config.out, "resume");
    std::fs::write(&checkpoint, &name)
        .map_err(|error| format!("cannot write {}: {error}", checkpoint.display()))?;
    let deadline = Instant::now() + CHECKPOINT_WAIT;
    loop {
        if std::fs::read_to_string(&resume).is_ok_and(|text| text.trim() == name) {
            return Ok(());
        }
        if Instant::now() > deadline {
            return Err(format!("the harness did not resume the checkpoint {name}"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The benchmark clock in milliseconds (the time base of [`bench_live_stamps`]).
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn bench_clock(shell: State<'_, Shell>) -> f64 {
    shell.live().stamps().now_ms()
}

/// When the live channel sent each frame since the last call: `[sequence, ms]` pairs.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn bench_live_stamps(shell: State<'_, Shell>) -> Vec<(u32, f64)> {
    shell.live().stamps().take()
}

/// The mean cost in milliseconds of building and encoding one live frame of the open project
/// (the part of the sample-to-pixel latency before a stamp is taken).
///
/// # Errors
///
/// None; the `Result` is Tauri's requirement for an asynchronous command with borrowed state.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "Tauri passes command arguments by value; async commands return a Result"
)]
pub fn bench_frame_cost(shell: State<'_, Shell>) -> Result<f64, String> {
    const FRAMES: u32 = 100;
    let started = Instant::now();
    let mut bytes = 0;
    for sequence in 0..FRAMES {
        bytes = LiveFrame::build(shell.app(), sequence).encode().len();
    }
    let mean = started.elapsed().as_secs_f64() * 1000.0 / f64::from(FRAMES);
    log::info!("a live frame of {bytes} bytes costs {mean:.3} ms");
    Ok(mean)
}

/// How late the main thread runs what is posted to it: every 10 ms for `duration_ms`, a closure
/// is posted with `run_on_main_thread`, and its delay is recorded. The window's IPC, its events
/// and the channel's deliveries all pass through the main thread, so this is their queue.
///
/// # Errors
///
/// A closure that cannot be posted.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn bench_main_lag(app: AppHandle, duration_ms: u64) -> Result<Vec<f64>, String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let started = Instant::now();
    let mut posted = 0_usize;
    while started.elapsed().as_millis() < u128::from(duration_ms) {
        let sender = sender.clone();
        let at = Instant::now();
        app.run_on_main_thread(move || {
            // The receiver waits for every closure; a closed one ends the measurement.
            let _sent = sender.send(at.elapsed().as_secs_f64() * 1000.0);
        })
        .map_err(|error| error.to_string())?;
        posted += 1;
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    drop(sender);
    let mut lags = Vec::with_capacity(posted);
    while let Ok(lag) = receiver.recv_timeout(std::time::Duration::from_secs(5)) {
        lags.push(lag);
    }
    Ok(lags)
}

/// Whether the main window's close button shows the dot of unsaved changes (macOS), read on the
/// main thread.
///
/// # Errors
///
/// The question cannot be posted to the main thread, or gets no answer.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn bench_document_edited(app: AppHandle) -> Result<Option<bool>, String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        #[cfg(target_os = "macos")]
        let edited = crate::chrome::shows_edited(&handle);
        #[cfg(not(target_os = "macos"))]
        let edited = {
            let _ = &handle;
            None
        };
        // The receiver waits below; a closed one has given up.
        let _sent = sender.send(edited);
    })
    .map_err(|error| error.to_string())?;
    receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| format!("no answer from the main thread: {error}"))
}

/// The labels of the web views that receive the live frames (the mixer's own window subscribes
/// once its page runs).
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn bench_live_subscribers(shell: State<'_, Shell>) -> Vec<String> {
    shell.live().labels()
}

/// Hides the main window, or shows it again and brings it to the front (the end-to-end check of
/// a window that is not drawn: its page gets no animation frames).
///
/// # Errors
///
/// The message of a missing main window or a refused hide or show.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn bench_show_main(app: AppHandle, visible: bool) -> Result<(), String> {
    let window = app
        .get_webview_window(crate::window::MAIN)
        .ok_or_else(|| "no main window".to_owned())?;
    let shown = if visible {
        window.show().and_then(|()| window.set_focus())
    } else {
        window.hide()
    };
    shown.map_err(|error| error.to_string())
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

// -- the native prompts and panels --------------------------------------------------------

/// How the page answers the next prompt or panel: a button, a path, several paths, or a cancel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BenchAnswer {
    /// A message box's button.
    Choice {
        /// The button.
        choice: Choice,
    },
    /// The file or folder chosen in a panel.
    Path {
        /// The path.
        path: PathBuf,
    },
    /// The files chosen in a panel that takes several.
    Paths {
        /// The paths.
        paths: Vec<PathBuf>,
    },
    /// The prompt or panel is cancelled (also when the queue is empty).
    Cancel {
        /// Always `true`.
        cancel: bool,
    },
}

impl BenchAnswer {
    const CANCEL: Self = Self::Cancel { cancel: true };

    /// The button a message box with `buttons` gets; a cancel closes it.
    pub fn choice(&self, buttons: Buttons) -> Choice {
        match (self, buttons) {
            (Self::Choice { choice }, _) => *choice,
            (_, Buttons::Ok) => Choice::Ok,
            (_, Buttons::YesNo) => Choice::No,
            (_, Buttons::SaveDiscardCancel) => Choice::Cancel,
        }
    }

    /// The path a panel for one file or folder returns.
    pub fn path(&self) -> Option<PathBuf> {
        match self {
            Self::Path { path } => Some(path.clone()),
            Self::Paths { paths } => paths.first().cloned(),
            Self::Choice { .. } | Self::Cancel { .. } => None,
        }
    }

    /// The paths a panel for several files returns.
    pub fn paths(&self) -> Vec<PathBuf> {
        match self {
            Self::Path { path } => vec![path.clone()],
            Self::Paths { paths } => paths.clone(),
            Self::Choice { .. } | Self::Cancel { .. } => Vec::new(),
        }
    }
}

/// A prompt or panel a benchmark run answered.
#[derive(Debug, Clone, Serialize)]
pub struct Asked {
    /// `message`, `save`, `open`, `open-files` or `folder`.
    pub kind: &'static str,
    /// Its title.
    pub title: String,
    /// A message box's text, or a panel's proposed file name (else its folder).
    pub text: String,
    /// How it was answered.
    pub answer: BenchAnswer,
}

/// The answers the page queued for the next prompts and panels, and the ones answered.
#[derive(Debug, Default)]
pub struct Prompts {
    answers: Mutex<VecDeque<BenchAnswer>>,
    asked: Mutex<Vec<Asked>>,
}

/// In a benchmark run, the answer to the prompt or panel `title` of `kind` (with `text`): the
/// queue's next one, else a cancel; it is recorded. `None` outside a benchmark run, where the
/// prompt shows.
pub fn answer(app: &AppHandle, kind: &'static str, title: &str, text: &str) -> Option<BenchAnswer> {
    let config = app.try_state::<Option<BenchConfig>>()?;
    config.inner().as_ref()?;
    let prompts = app.try_state::<Prompts>()?;
    let answer = lock(&prompts.answers)
        .pop_front()
        .unwrap_or(BenchAnswer::CANCEL);
    log::info!("benchmark: the {kind} “{title}” is answered with {answer:?}");
    lock(&prompts.asked).push(Asked {
        kind,
        title: title.to_owned(),
        text: text.to_owned(),
        answer: answer.clone(),
    });
    Some(answer)
}

/// Queues `answers` for the next prompts and panels, in order.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn bench_answer_prompts(prompts: State<'_, Prompts>, answers: Vec<BenchAnswer>) {
    lock(&prompts.answers).extend(answers);
}

/// The prompts and panels answered since the last call, and the answers left unused (which are
/// dropped).
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn bench_prompts_asked(prompts: State<'_, Prompts>) -> (Vec<Asked>, Vec<BenchAnswer>) {
    let asked = std::mem::take(&mut *lock(&prompts.asked));
    let unused = lock(&prompts.answers).drain(..).collect();
    (asked, unused)
}

// -- the menu ----------------------------------------------------------------------------

/// A menu item the desktop handled, and whether it declined it (a prompt or a dialog was open).
#[derive(Debug, Clone, Serialize)]
pub struct MenuHandled {
    /// The item's ID.
    pub id: String,
    /// Whether it was declined.
    pub declined: bool,
}

/// The menu items the desktop handled, in order.
#[derive(Debug, Default)]
pub struct MenuLog(Mutex<Vec<MenuHandled>>);

/// Records that the desktop handled the item `id`.
pub fn menu_handled(app: &AppHandle, id: &str, declined: bool) {
    if let Some(log) = app.try_state::<MenuLog>() {
        lock(&log.0).push(MenuHandled {
            id: id.to_owned(),
            declined,
        });
    }
}

/// The menu items handled since the last call.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn bench_menu_log(log: State<'_, MenuLog>) -> Vec<MenuHandled> {
    std::mem::take(&mut *lock(&log.0))
}

/// Runs `work` on the main thread and waits for its answer (5 s at most).
fn on_main<T, F>(app: &AppHandle, work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&AppHandle) -> T + Send + 'static,
{
    let (sender, receiver) = std::sync::mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        // The receiver waits below; a closed one has given up.
        let _sent = sender.send(work(&handle));
    })
    .map_err(|error| error.to_string())?;
    receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| format!("no answer from the main thread: {error}"))
}

/// A native menu item as the menu bar shows it.
#[derive(Debug, Clone, Serialize)]
pub struct NativeItem {
    /// Its title.
    pub title: String,
    /// Whether it is a separator.
    pub separator: bool,
    /// Its key equivalent ("n", " ", or a function key's character).
    pub key: String,
    /// The key equivalent's modifiers: `command`, `shift`, `option`, `control`.
    pub modifiers: Vec<&'static str>,
    /// Whether it can be chosen.
    pub enabled: bool,
    /// Whether it shows a check mark.
    pub checked: bool,
    /// Its submenu's items.
    pub items: Vec<NativeItem>,
}

#[cfg(target_os = "macos")]
fn native_items(menu: &objc2_app_kit::NSMenu) -> Vec<NativeItem> {
    use objc2_app_kit::{NSControlStateValueOn, NSEventModifierFlags};
    menu.itemArray()
        .iter()
        .map(|item| {
            let mask = item.keyEquivalentModifierMask();
            let modifiers = [
                (NSEventModifierFlags::Control, "control"),
                (NSEventModifierFlags::Option, "option"),
                (NSEventModifierFlags::Shift, "shift"),
                (NSEventModifierFlags::Command, "command"),
            ]
            .into_iter()
            .filter(|(flag, _)| mask.contains(*flag))
            .map(|(_, name)| name)
            .collect();
            NativeItem {
                title: item.title().to_string(),
                separator: item.isSeparatorItem(),
                key: item.keyEquivalent().to_string(),
                modifiers,
                enabled: item.isEnabled(),
                checked: item.state() == NSControlStateValueOn,
                items: item
                    .submenu()
                    .map(|menu| native_items(&menu))
                    .unwrap_or_default(),
            }
        })
        .collect()
}

/// The menu bar as the application shows it: its menus and their items, read on the main thread.
///
/// # Errors
///
/// No menu bar, or no answer from the main thread.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn bench_native_menu(app: AppHandle) -> Result<Vec<NativeItem>, String> {
    #[cfg(target_os = "macos")]
    {
        on_main(&app, |_| {
            let main_thread = objc2::MainThreadMarker::new().ok_or("not the main thread")?;
            let menu = objc2_app_kit::NSApplication::sharedApplication(main_thread)
                .mainMenu()
                .ok_or("no menu bar")?;
            Ok(native_items(&menu))
        })?
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err("the native menu is read on macOS only".to_owned())
    }
}

/// Chooses the native item at `path` (the menu's title, then the items' titles), as a click does:
/// its action goes to the menu library, which hands it to the desktop's handler. A disabled item
/// is refused, as the menu bar refuses a click on it.
///
/// # Errors
///
/// An item that does not exist or is disabled, or no answer from the main thread.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn bench_activate_menu(app: AppHandle, path: Vec<String>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        on_main(&app, move |_| {
            use objc2_foundation::NSString;
            let main_thread = objc2::MainThreadMarker::new().ok_or("not the main thread")?;
            let mut menu = objc2_app_kit::NSApplication::sharedApplication(main_thread)
                .mainMenu()
                .ok_or("no menu bar")?;
            let (last, menus) = path.split_last().ok_or("an empty path")?;
            for title in menus {
                let item = menu
                    .itemWithTitle(&NSString::from_str(title))
                    .ok_or_else(|| format!("no menu “{title}”"))?;
                menu = item
                    .submenu()
                    .ok_or_else(|| format!("“{title}” has no submenu"))?;
            }
            let index = menu.indexOfItemWithTitle(&NSString::from_str(last));
            let item = menu
                .itemAtIndex(index)
                .ok_or_else(|| format!("no item “{last}”"))?;
            if !item.isEnabled() {
                return Err(format!("“{last}” is disabled"));
            }
            menu.performActionForItemAtIndex(index);
            Ok(())
        })?
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, path);
        Err("the native menu is chosen on macOS only".to_owned())
    }
}

/// A key to post: its name (the letters of `key_code`, `Space`, `Home`, `Backspace`, `Tab`,
/// `Escape`, `ArrowUp`, `ArrowDown`) and its modifiers.
#[derive(Debug, Clone, Deserialize)]
pub struct BenchKey {
    /// The key.
    pub key: String,
    /// With ⌘.
    #[serde(default)]
    pub command: bool,
    /// With ⇧.
    #[serde(default)]
    pub shift: bool,
}

/// Where the posted keys went: whether the main window was the key window, and the class of its
/// first responder (the web view when the page has the keyboard).
#[derive(Debug, Clone, Serialize)]
pub struct KeyTarget {
    /// Whether the main window was the key window when the keys were posted.
    pub key_window: bool,
    /// The class of the main window's first responder.
    pub first_responder: String,
}

/// The characters and the virtual key code (`kVK_…`) of `key`, as the keyboard sends them.
fn key_code(key: &str) -> Option<(&'static str, u16)> {
    Some(match key {
        "a" => ("a", 0x00),
        "c" => ("c", 0x08),
        "d" => ("d", 0x02),
        "e" => ("e", 0x0E),
        "l" => ("l", 0x25),
        "r" => ("r", 0x0F),
        "s" => ("s", 0x01),
        "x" => ("x", 0x07),
        "z" => ("z", 0x06),
        "Space" => (" ", 0x31),
        // NSUpArrowFunctionKey and NSDownArrowFunctionKey, kVK_UpArrow and kVK_DownArrow (the
        // track list and the fader).
        "ArrowUp" => ("\u{F700}", 0x7E),
        "ArrowDown" => ("\u{F701}", 0x7D),
        // NSHomeFunctionKey, kVK_Home.
        "Home" => ("\u{F729}", 0x73),
        // The Delete key above Return, kVK_Delete.
        "Backspace" => ("\u{7F}", 0x33),
        // kVK_Tab and kVK_Escape (a dialog's focus).
        "Tab" => ("\t", 0x30),
        "Escape" => ("\u{1B}", 0x35),
        _ => return None,
    })
}

/// Posts a key-down and a key-up of each of `keys` to the main window, at the end of the
/// application's event queue: they reach the menu bar and the web view along the path a key typed
/// on the keyboard takes. Brings the window to the front first.
///
/// # Errors
///
/// A key this hook does not know, no main window, or no answer from the main thread.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn bench_post_keys(app: AppHandle, keys: Vec<BenchKey>) -> Result<KeyTarget, String> {
    #[cfg(target_os = "macos")]
    {
        on_main(&app, move |app| {
            use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType};
            use objc2_foundation::{NSPoint, NSProcessInfo, NSString};
            let main_thread = objc2::MainThreadMarker::new().ok_or("not the main thread")?;
            let window = app
                .get_webview_window(crate::window::MAIN)
                .ok_or("no main window")?;
            let ns_window = crate::chrome::ns_window_of(&window).ok_or("no native window")?;
            let application = NSApplication::sharedApplication(main_thread);
            if !ns_window.isKeyWindow() {
                application.activate();
                ns_window.makeKeyAndOrderFront(None);
            }
            let first_responder = ns_window.firstResponder().map_or_else(
                || "none".to_owned(),
                |responder| responder.class().name().to_string_lossy().into_owned(),
            );
            let target = KeyTarget {
                key_window: ns_window.isKeyWindow(),
                first_responder,
            };
            for key in &keys {
                let (characters, code) =
                    key_code(&key.key).ok_or_else(|| format!("no key code for {}", key.key))?;
                let mut flags = NSEventModifierFlags::empty();
                if key.command {
                    flags |= NSEventModifierFlags::Command;
                }
                if key.shift {
                    flags |= NSEventModifierFlags::Shift;
                }
                let typed = if key.shift {
                    characters.to_uppercase()
                } else {
                    characters.to_owned()
                };
                let typed = NSString::from_str(&typed);
                let plain = NSString::from_str(characters);
                for kind in [NSEventType::KeyDown, NSEventType::KeyUp] {
                    let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                        kind,
                        NSPoint::new(0.0, 0.0),
                        flags,
                        NSProcessInfo::processInfo().systemUptime(),
                        ns_window.windowNumber(),
                        None,
                        &typed,
                        &plain,
                        false,
                        code,
                    )
                    .ok_or("no key event")?;
                    application.postEvent_atStart(&event, false);
                }
            }
            Ok(target)
        })?
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, keys);
        Err("keys are posted on macOS only".to_owned())
    }
}
