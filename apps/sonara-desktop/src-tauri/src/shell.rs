//! The desktop's session: the application, the IPC server agents attach to, the autosave timer
//! and the event pump, started and stopped in the prototype's order.
//!
//! **Startup** ([`Shell::start`]): `AppContext::create` with `--audio` or the settings'
//! backend; the project argument, or "Untitled" from the "empty" template; unless `--no-ipc`,
//! the IPC server with app kind `gui` and its discovery file, so that `sonara mcp` attaches to
//! this window's session (a failure is logged, not fatal). The autosave timer starts with the
//! window, as the prototype's `MainWindow` started it ([`Shell::start_autosave`]): each autosave
//! shows "Autosaved" in the status bar.
//!
//! The project opens before the IPC server starts, where the prototype started the server
//! first: an agent that finds the discovery file then never meets a session without a project.
//!
//! **Shutdown** ([`Shell::shutdown`], after the unsaved-changes prompt): the autosave timer
//! and the live feed stop, then the transport; a project whose changes the user discarded drops
//! its autosave; the event pump and the IPC server close, and `AppContext::close` runs. It runs
//! once.
//!
//! **The window's data**: the event pump hands the session's events, the jobs'
//! changes and the ready waveform pyramids to the window ([`Shell::start_pump`]), and the live
//! feed sends a frame to every subscribed web view ([`Shell::start_live`]).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use sonara_agent::ipc::{IpcServer, ServerOptions};
use sonara_agent::service::{AgentOptions, AgentService};
use sonara_app::autosave::{Autosaver, OnAutosaved, autosave_interval};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::events::{EventPump, Notice, Subscriptions};
use sonara_app::live::LiveFeed;
use sonara_app::projects::NewProject;
use sonara_app::window_layout::{PaneLayout, load_window_layout, window_layout_path};

use crate::live::LiveChannels;
use crate::options::Options;

/// What the window shows of the agent connection: "agent: on" with the address, or "agent:
/// off".
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentStatus {
    /// Whether agents can attach.
    pub on: bool,
    /// The IPC address (the socket path), when on.
    pub address: Option<String>,
    /// The discovery file agents find the window by, when on (Agent Connection).
    pub discovery: Option<String>,
}

/// The outcome of opening the project at startup.
#[derive(Debug, Default)]
struct InitialProject {
    /// The status-bar messages: a project that could not be opened, load warnings.
    messages: Vec<String>,
    /// The opened project's path, when a newer autosave of it exists.
    autosave_of: Option<PathBuf>,
}

/// The application and the services around it, shared by the window's commands.
pub struct Shell {
    app: Arc<AppContext>,
    agent: AgentStatus,
    server: Mutex<Option<IpcServer>>,
    autosaver: Mutex<Option<Autosaver>>,
    /// What each autosave tells the window, once the window set it ([`Shell::start_autosave`]).
    on_autosaved: Mutex<Option<OnAutosaved>>,
    /// The session's events and the jobs' and pyramids' changes since before the IPC server
    /// started, until the pump takes them.
    subscriptions: Mutex<Option<Subscriptions>>,
    pump: Mutex<Option<EventPump>>,
    /// The web views that receive the live frames.
    live: LiveChannels,
    feed: Mutex<Option<LiveFeed>>,
    /// Status-bar messages of the startup, kept until the window asks for them: events sent
    /// before the page listens would be lost.
    startup_messages: Mutex<Vec<String>>,
    /// The panes of the window, as the page last reported them; saved with the window
    /// layout.
    panes: Mutex<PaneLayout>,
    /// Set once the shutdown ran (`AcqRel`: the first caller wins).
    stopped: AtomicBool,
    /// Set while a lifecycle flow's panel or prompt is open (`AcqRel`: one at a time).
    prompting: AtomicBool,
}

/// Locks `mutex`, keeping the data of a thread that panicked while holding it: every value
/// here is replaced whole.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shell {
    /// Creates the application, opens the project, and starts the IPC server (unless
    /// `--no-ipc`). Also returns the opened project's path when a newer autosave of it exists, to
    /// offer once the window is open.
    ///
    /// # Errors
    ///
    /// The message of an application that cannot be created (no home directory, no worker
    /// threads); the IPC server only logs its failure.
    pub fn start(options: &Options) -> Result<(Self, Option<PathBuf>), String> {
        let app = AppContext::create(AppOptions {
            audio_backend: options.audio.clone(),
            ..AppOptions::default()
        })
        .map_err(|error| format!("cannot start the application: {error}"))?;
        let app = Arc::new(app);
        let initial = open_initial_project(&app, options.project.as_deref());
        // Before agents can attach: their first edit must reach the window too.
        let subscriptions =
            Subscriptions::new(app.session(), app.jobs()).with_peaks(app.peaks().subscribe());
        let server = if options.no_ipc {
            None
        } else {
            start_ipc(&app)
        };
        let agent = AgentStatus {
            on: server.is_some(),
            address: server.as_ref().map(|server| server.address().to_owned()),
            discovery: server
                .as_ref()
                .map(|server| server.discovery_path().display().to_string()),
        };
        let shell = Self {
            app,
            agent,
            server: Mutex::new(server),
            autosaver: Mutex::new(None),
            on_autosaved: Mutex::new(None),
            subscriptions: Mutex::new(Some(subscriptions)),
            pump: Mutex::new(None),
            live: LiveChannels::default(),
            feed: Mutex::new(None),
            startup_messages: Mutex::new(initial.messages),
            panes: Mutex::new(saved_panes()),
            stopped: AtomicBool::new(false),
            prompting: AtomicBool::new(false),
        };
        Ok((shell, initial.autosave_of))
    }

    /// The application.
    pub fn app(&self) -> &Arc<AppContext> {
        &self.app
    }

    /// Whether agents can attach, and where.
    pub fn agent(&self) -> &AgentStatus {
        &self.agent
    }

    /// Starts handing the session's events, from before agents could attach, the jobs'
    /// changes and the ready pyramids to `sink`, until the shutdown. Only the first call starts
    /// the pump.
    ///
    /// # Errors
    ///
    /// The I/O error of starting the pump's thread.
    pub fn start_pump<F>(&self, sink: F) -> std::io::Result<()>
    where
        F: FnMut(Notice) + Send + 'static,
    {
        let Some(subscriptions) = lock(&self.subscriptions).take() else {
            return Ok(());
        };
        let pump = EventPump::start_notices(subscriptions, sink)?;
        *lock(&self.pump) = Some(pump);
        Ok(())
    }

    /// The web views that receive the live frames.
    pub fn live(&self) -> &LiveChannels {
        &self.live
    }

    /// Starts sending live frames to the subscribed web views, until the shutdown. Only the
    /// first call starts the feed.
    ///
    /// # Errors
    ///
    /// The I/O error of starting the feed's thread.
    pub fn start_live(&self) -> std::io::Result<()> {
        let mut feed = lock(&self.feed);
        if feed.is_none() {
            let channels = self.live.clone();
            *feed = Some(LiveFeed::start(Arc::clone(&self.app), move |frame| {
                channels.send(frame);
            })?);
        }
        Ok(())
    }

    /// The startup's status-bar messages; each is handed out once.
    pub fn take_startup_messages(&self) -> Vec<String> {
        std::mem::take(&mut *lock(&self.startup_messages))
    }

    /// Claims the right to show a lifecycle flow's panels and prompts; `false` while another
    /// flow shows them.
    pub fn begin_prompt(&self) -> bool {
        !self.prompting.swap(true, Ordering::AcqRel)
    }

    /// Releases the claim of [`begin_prompt`](Self::begin_prompt).
    pub fn end_prompt(&self) {
        self.prompting.store(false, Ordering::Release);
    }

    /// Whether a flow's or a command's panel or prompt is open now (the menu does nothing then).
    pub fn is_prompting(&self) -> bool {
        self.prompting.load(Ordering::Acquire)
    }

    /// The panes of the window, as last reported.
    pub fn panes(&self) -> PaneLayout {
        *lock(&self.panes)
    }

    /// Records the panes the page shows now.
    pub fn set_panes(&self, panes: PaneLayout) {
        *lock(&self.panes) = panes.sanitized();
    }

    /// Starts the autosave timer with the settings' interval (`autosave_seconds`; 0 turns it
    /// off); `on_autosaved` runs after each autosave written, on the timer's thread (the window's
    /// "Autosaved"), and again after every restart. A timer that cannot start is logged.
    /// Does nothing after the shutdown.
    pub fn start_autosave(&self, on_autosaved: OnAutosaved) {
        *lock(&self.on_autosaved) = Some(on_autosaved);
        self.restart_autosave(self.app.settings().autosave_seconds);
    }

    /// Restarts the autosave timer with `seconds` between autosaves, or stops it at 0 (the
    /// Settings dialog). Does nothing after the shutdown.
    pub fn restart_autosave(&self, seconds: i64) {
        if self.is_stopped() {
            return;
        }
        let mut slot = lock(&self.autosaver);
        if let Some(running) = slot.take() {
            running.stop();
        }
        let on_autosaved = lock(&self.on_autosaved).clone();
        *slot = autosave_interval(seconds).and_then(|interval| {
            let app = Arc::clone(&self.app);
            let started = match on_autosaved {
                Some(hook) => Autosaver::start_notifying(app, interval, hook),
                None => Autosaver::start(app, interval),
            };
            started
                .inspect_err(|error| log::warn!("autosave timer not started: {error}"))
                .ok()
        });
    }

    /// Whether the shutdown ran.
    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }

    /// Stops everything in the prototype's order; `discard_changes` drops the autosave of a
    /// project the user chose not to save. Later calls do nothing.
    pub fn shutdown(&self, discard_changes: bool) {
        if self.stopped.swap(true, Ordering::AcqRel) {
            return;
        }
        log::info!("shutting down");
        if let Some(autosaver) = lock(&self.autosaver).take() {
            autosaver.stop();
        }
        if let Some(feed) = lock(&self.feed).take() {
            feed.stop();
        }
        if self.app.has_transport()
            && let Err(error) = self.app.transport().stop()
        {
            log::warn!("stopping the transport: {error}");
        }
        if discard_changes && self.app.projects().discard_autosave() {
            log::info!("discarded the unsaved changes and their autosave");
        }
        if let Some(pump) = lock(&self.pump).take() {
            pump.stop();
        }
        if let Some(server) = lock(&self.server).take() {
            server.close();
        }
        self.app.close();
    }
}

/// The panes of the saved window layout, else the prototype's sizes.
fn saved_panes() -> PaneLayout {
    window_layout_path()
        .ok()
        .and_then(|path| load_window_layout(&path))
        .map(|layout| layout.panes)
        .unwrap_or_default()
}

/// Opens `project` (the launch argument), as the prototype's `window.open_project`; without
/// one, or when it cannot be opened, creates "Untitled" from the "empty" template.
fn open_initial_project(app: &AppContext, project: Option<&Path>) -> InitialProject {
    let mut outcome = InitialProject::default();
    if let Some(path) = project {
        match app.projects().open_project(path, false) {
            Ok(loaded) => {
                outcome.autosave_of = loaded.autosave_available.then_some(loaded.path);
                if !loaded.warnings.is_empty() {
                    outcome.messages.push(loaded.warnings.join("; "));
                }
            }
            Err(error) => {
                log::warn!("cannot open {}: {error}", path.display());
                outcome.messages.push(error.to_string());
            }
        }
    }
    if app.session().project().is_none()
        && let Err(error) = app.projects().new_project(&NewProject::default())
    {
        log::error!("cannot create the Untitled project: {error}");
        outcome.messages.push(error.to_string());
    }
    outcome
}

/// `IPCServer(AgentService(app, source="agent"), app_kind="gui")`; a failure is logged, as in
/// the prototype, and the window shows "agent: off".
fn start_ipc(app: &Arc<AppContext>) -> Option<IpcServer> {
    let agent = AgentService::new(Arc::clone(app), AgentOptions::default())
        .inspect_err(|error| log::warn!("agent IPC server not started: {error}"))
        .ok()?;
    IpcServer::start(
        Arc::new(agent),
        ServerOptions {
            app_kind: "gui".to_owned(),
            directory: None,
        },
    )
    .inspect(|server| {
        log::info!(
            "agents attach at {} (discovery {})",
            server.address(),
            server.discovery_path().display()
        );
    })
    .inspect_err(|error| log::warn!("agent IPC server not started: {error}"))
    .ok()
}
