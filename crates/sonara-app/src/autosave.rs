//! The autosave timer of an interactive session (the prototype main window's `QTimer` and
//! `_do_autosave`;).
//!
//! Every interval, [`Autosaver`] writes the open project's autosave when it has unsaved
//! changes ([`ProjectService::autosave`](crate::projects::ProjectService::autosave)), on a thread
//! of its own, so that the window never waits for the disk. A written autosave is logged and
//! handed to the hook of [`Autosaver::start_notifying`], which the desktop turns into the status
//! bar's "Autosaved" for 1.5 s ([`StatusMessage::autosaved`](crate::events::StatusMessage::autosaved)),
//! as the prototype showed it. A failure is logged and the timer goes on, as in the prototype.
//! The interval comes from `settings.autosave_seconds`; zero or less turns autosaving off
//! ([`autosave_interval`]).

use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::context::AppContext;

/// What runs after each autosave that was written, with its path, on the autosave thread.
pub type OnAutosaved = Arc<dyn Fn(&Path) + Send + Sync>;

/// The interval of `settings.autosave_seconds`, or `None` when it turns autosaving off (zero or
/// less).
#[must_use]
pub fn autosave_interval(seconds: i64) -> Option<Duration> {
    u64::try_from(seconds)
        .ok()
        .filter(|&seconds| seconds > 0)
        .map(Duration::from_secs)
}

/// A running autosave timer. It stops when [`Autosaver::stop`] is called or when it is
/// dropped; an autosave being written is finished first.
#[derive(Debug)]
pub struct Autosaver {
    stop: Option<Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Autosaver {
    /// Autosaves `app`'s open project every `interval` on a thread named `sonara-autosave`; a
    /// written autosave is only logged.
    ///
    /// # Errors
    ///
    /// The I/O error of starting the thread.
    pub fn start(app: Arc<AppContext>, interval: Duration) -> io::Result<Self> {
        Self::start_notifying(app, interval, Arc::new(|_: &Path| {}))
    }

    /// As [`start`](Self::start), and calls `on_autosaved` with the path of each autosave that
    /// was written. A clean project writes none, and a failed autosave calls nothing.
    ///
    /// # Errors
    ///
    /// The I/O error of starting the thread.
    pub fn start_notifying(
        app: Arc<AppContext>,
        interval: Duration,
        on_autosaved: OnAutosaved,
    ) -> io::Result<Self> {
        let (stop, stopped) = mpsc::channel::<()>();
        let thread = thread::Builder::new()
            .name("sonara-autosave".to_owned())
            .spawn(move || {
                // A message or a dropped sender ends the timer; a timeout is a tick.
                while let Err(RecvTimeoutError::Timeout) = stopped.recv_timeout(interval) {
                    match app.projects().autosave() {
                        Ok(Some(path)) => {
                            log::info!("autosaved {}", path.display());
                            on_autosaved(&path);
                        }
                        Ok(None) => {}
                        Err(error) => log::warn!("autosave failed: {error}"),
                    }
                }
            })?;
        Ok(Self {
            stop: Some(stop),
            thread: Some(thread),
        })
    }

    /// Stops the timer and waits until a running autosave has finished.
    pub fn stop(mut self) {
        self.halt();
    }

    fn halt(&mut self) {
        // Dropping the sender wakes the thread at once.
        self.stop.take();
        if let Some(thread) = self.thread.take() {
            // ERR-06: a panicking autosave has ended the timer already and was reported on its
            // thread; stopping has nothing left to do.
            let _ = thread.join();
        }
    }
}

impl Drop for Autosaver {
    fn drop(&mut self) {
        self.halt();
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use sonara_core::commands::track::CreateTrack;
    use sonara_core::ids::{self, IdGenerator as _};
    use sonara_core::model::track::TrackKind;
    use tempfile::TempDir;

    use super::{Autosaver, OnAutosaved};
    use crate::context::{AppContext, AppOptions};
    use crate::events::{AUTOSAVED_MS, StatusMessage};
    use crate::projects::NewProject;
    use crate::settings::Settings;

    const PATIENCE: Duration = Duration::from_secs(10);

    /// An application on the null backend with a new "Untitled" project; its workspace.
    fn untitled(dir: &TempDir) -> (Arc<AppContext>, PathBuf) {
        let app = Arc::new(
            AppContext::create(AppOptions {
                workspaces: Some(dir.path().join("workspaces")),
                settings: Some(Settings::default()),
                persist_settings: false,
                audio_backend: Some("null".to_owned()),
                ..AppOptions::default()
            })
            .expect("the application"),
        );
        app.projects()
            .new_project(&NewProject::default())
            .expect("a new project");
        let workspace = app
            .session()
            .lock()
            .location()
            .expect("a location")
            .workspace
            .clone();
        (app, workspace)
    }

    /// An edit that makes the project dirty.
    fn create_track(app: &AppContext) {
        let mut ids = app.ids();
        let command = CreateTrack {
            name: "Vocals".into(),
            kind: TrackKind::Audio,
            index: None,
            color: None,
            output_id: ids::MASTER_ID.into(),
            instrument_plugin: None,
            instrument_preset: None,
            parent_id: None,
            track_id: ids.new_id(ids::TRACK),
            instrument_id: ids.new_id(ids::DEVICE),
        };
        app.session()
            .lock()
            .execute(&command, false, "gui")
            .expect("the track is created");
    }

    /// A hook that records the paths it was called with.
    fn recorder() -> (OnAutosaved, Arc<Mutex<Vec<PathBuf>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let hook: OnAutosaved = Arc::new(move |path| {
            sink.lock().expect("the recorder").push(path.to_path_buf());
        });
        (hook, seen)
    }

    #[test]
    fn the_hook_hears_each_written_autosave_and_nothing_else() {
        let dir = TempDir::new().expect("a temporary folder");
        let (app, workspace) = untitled(&dir);
        let (hook, seen) = recorder();
        let autosaver =
            Autosaver::start_notifying(Arc::clone(&app), Duration::from_millis(20), hook)
                .expect("the timer runs");
        // A clean project writes no autosave, so the hook hears nothing.
        thread::sleep(Duration::from_millis(150));
        assert_eq!(*seen.lock().expect("the recorder"), Vec::<PathBuf>::new());
        create_track(&app);
        let started = Instant::now();
        while seen.lock().expect("the recorder").is_empty() {
            assert!(
                started.elapsed() < PATIENCE,
                "timed out waiting for the hook"
            );
            thread::sleep(Duration::from_millis(10));
        }
        autosaver.stop();
        let paths = seen.lock().expect("the recorder").clone();
        // The path the store wrote, inside the project's workspace, and it exists.
        assert!(
            paths.iter().all(|path| path.starts_with(&workspace)),
            "{paths:?}"
        );
        assert!(paths[0].exists(), "{}", paths[0].display());
        // A stopped timer calls nothing more.
        let heard = paths.len();
        create_track(&app);
        thread::sleep(Duration::from_millis(150));
        assert_eq!(seen.lock().expect("the recorder").len(), heard);
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_autosave_does_not_call_the_hook() {
        let dir = TempDir::new().expect("a temporary folder");
        let (app, workspace) = untitled(&dir);
        create_track(&app);
        // A file where the autosave folder belongs: the store's write fails for every user,
        // root included (the container runs the tests as root, where a read-only folder
        // would still be writable).
        let autosave_dir = workspace.join(".autosave");
        let _ = std::fs::remove_dir_all(&autosave_dir);
        std::fs::write(&autosave_dir, b"not a folder").expect("a blocking file");
        let failed = app.projects().autosave();
        let (hook, seen) = recorder();
        let autosaver =
            Autosaver::start_notifying(Arc::clone(&app), Duration::from_millis(20), hook)
                .expect("the timer runs");
        thread::sleep(Duration::from_millis(200));
        autosaver.stop();
        assert!(failed.is_err(), "the write must fail: {failed:?}");
        assert_eq!(*seen.lock().expect("the recorder"), Vec::<PathBuf>::new());
    }

    #[test]
    fn the_autosaved_message_is_the_prototypes() {
        // `self.statusBar().showMessage("Autosaved", 1500)`: a plain message, not an error.
        assert_eq!(
            StatusMessage::autosaved(),
            StatusMessage {
                text: "Autosaved".to_owned(),
                duration_ms: 1_500,
                error: false,
            }
        );
        assert_eq!(AUTOSAVED_MS, 1_500);
    }
}
