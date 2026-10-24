//! Session events, job changes and status messages for an interface that shows the project (the
//! prototype's `SessionBridge`).
//!
//! Edits reach the one session from many threads: the GUI's own, the IPC connection threads of
//! agents, the job workers. A [`Subscription`] queues every event from the moment it is taken,
//! and an [`EventPump`] hands the queued and later events to its sink on one thread of its own,
//! in commit order, so the sink sees every event once, whoever caused it, and never runs on the
//! thread of the edit (which holds no lock by then, but must not wait for a window either). The
//! desktop application subscribes before agents can attach and starts the pump once its window
//! system runs.
//!
//! **What the interface hears** ([`Notice`]):
//!
//! - every session event as a [`SessionNotice`] `{version, kind, label, source, changes}`, with
//!   the change set's flags and the prototype's choice between rebuilding and refreshing
//!   ([`ChangeNotice::structure_changed`]);
//! - every change of a background job as a [`JobNotice`];
//! - every waveform pyramid that became ready ([`PeaksReady`], from [`crate::peaks`]).
//!
//! [`EventPump::start_notices`] drains all three on the same `sonara-events` thread: it waits
//! for the session's events and looks at the jobs and the peaks between them, at least every
//! [`EventPump::SIDE_INTERVAL`], so a job change reaches the sink within that time (job progress
//! is throttled to 100 ms anyway). Status-bar texts ([`StatusMessage`]) are worded here, so the
//! page only shows them.
//!
//! The pump stops when it is stopped or dropped: it polls for that every
//! [`EventPump::POLL_INTERVAL`] while no event arrives.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use sonara_core::commands::{ChangeSet, Flag, ObjectRef};
use sonara_core::error::DomainError;
use sonara_core::session::{EventKind, SessionEvent};

use crate::error::{AppError, ErrorReport};
use crate::jobs::{JobManager, JobSnapshot, JobStatus};
use crate::peaks::PeaksReady;
use crate::session::SharedSession;

/// How long an edit from another source stays in the status bar.
pub const FOREIGN_EDIT_MS: u32 = 3_000;
/// How long a message stays in the status bar (the prototype's 4 s).
pub const MESSAGE_MS: u32 = 4_000;
/// How long an error or a load warning stays in the status bar (the prototype's 8 s).
pub const ERROR_MS: u32 = 8_000;
/// How long "Autosaved" stays in the status bar (MW-25: the prototype's 1.5 s).
pub const AUTOSAVED_MS: u32 = 1_500;

/// What an interface shows of a session event: `{version, kind, label, source, changes}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionNotice {
    /// The project version after the event.
    pub version: u64,
    /// `command`, `undo`, `redo`, `loaded`, `closed` or `saved`.
    pub kind: &'static str,
    /// The command label (`Create Track`), empty for loads, closes and saves.
    pub label: String,
    /// Who caused the event: `gui`, `user`, `agent`, a client's name, …
    pub source: String,
    /// What changed.
    pub changes: ChangeNotice,
}

impl SessionNotice {
    /// The status-bar text of an edit from another source: `source: label` when the
    /// event has a label and came neither from the GUI nor from the user, else `None`.
    #[must_use]
    pub fn foreign_edit(&self) -> Option<String> {
        let own = matches!(self.source.as_str(), "gui" | "user");
        (!own && !self.label.is_empty()).then(|| format!("{}: {}", self.source, self.label))
    }
}

impl From<&SessionEvent> for SessionNotice {
    fn from(event: &SessionEvent) -> Self {
        Self {
            version: event.version,
            kind: event.kind.as_str(),
            label: event.label.clone(),
            source: event.source.clone(),
            changes: ChangeNotice::of(event.kind, &event.changes),
        }
    }
}

/// A session event's change set, as the panels decide what to redraw.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangeNotice {
    /// New objects, in order.
    pub created: Vec<ObjectNotice>,
    /// Changed objects, in order.
    pub updated: Vec<ObjectNotice>,
    /// Removed objects, in order.
    pub deleted: Vec<ObjectNotice>,
    /// The change categories, sorted by name (`tracks`, `mixer`, `notes`, …).
    pub flags: Vec<&'static str>,
    /// The prototype's `structure_changed`: the panels rebuild instead of refreshing in place.
    /// True for the flags `tracks`, `routing`, `devices` and `project`, and for the kinds
    /// `loaded`, `closed`, `undo` and `redo`.
    pub structure_changed: bool,
}

impl ChangeNotice {
    /// The notice of `changes` from an event of `kind`.
    #[must_use]
    pub fn of(kind: EventKind, changes: &ChangeSet) -> Self {
        let objects = |refs: &[ObjectRef]| refs.iter().map(ObjectNotice::of).collect();
        let mut flags: Vec<&'static str> = changes.flags.iter().map(|f| f.as_str()).collect();
        flags.sort_unstable();
        Self {
            created: objects(&changes.created),
            updated: objects(&changes.updated),
            deleted: objects(&changes.deleted),
            flags,
            structure_changed: structure_changed(kind, changes),
        }
    }
}

/// The prototype's `structure_changed(event)` (`ui/bridge.py`): whether the track list or its
/// routing changed, so that panels rebuild instead of refreshing in place.
#[must_use]
pub fn structure_changed(kind: EventKind, changes: &ChangeSet) -> bool {
    let structural = [Flag::Tracks, Flag::Routing, Flag::Devices, Flag::Project];
    structural.iter().any(|flag| changes.flags.contains(flag))
        || matches!(
            kind,
            EventKind::Loaded | EventKind::Closed | EventKind::Undo | EventKind::Redo
        )
}

/// A changed object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ObjectNotice {
    /// `project`, `track`, `clip`, `note`, `device`, `send`, `automation_lane`, `marker`,
    /// `section` or `asset`.
    pub kind: &'static str,
    /// Its ID.
    pub id: String,
}

impl ObjectNotice {
    fn of(object: &ObjectRef) -> Self {
        Self {
            kind: object.kind.as_str(),
            id: object.id.clone(),
        }
    }
}

/// What the status bar shows of a background job (the job label and progress bar).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JobNotice {
    /// The operation ID.
    pub id: String,
    /// What the job does (`export`, `decode`, …).
    pub kind: String,
    /// The label (`Export audio`).
    pub label: String,
    /// `queued`, `running`, `succeeded`, `failed` or `cancelled`.
    pub status: &'static str,
    /// Whether the job has finished; the progress bar shows while it has not.
    pub finished: bool,
    /// Progress, 0 to 1.
    pub progress: f64,
    /// The latest progress message.
    pub message: String,
    /// The result of a successful job.
    pub result: Option<Value>,
    /// The error of a failed job.
    pub error: Option<ErrorNotice>,
}

impl From<&JobSnapshot> for JobNotice {
    fn from(job: &JobSnapshot) -> Self {
        Self {
            id: job.id.clone(),
            kind: job.kind.clone(),
            label: job.label.clone(),
            status: job.status.as_str(),
            finished: job.status.is_finished(),
            progress: job.progress,
            message: job.message.clone(),
            result: job.result.clone().map(Value::Object),
            error: job.error.as_ref().map(ErrorNotice::from),
        }
    }
}

impl JobNotice {
    /// The status-bar line while the job runs: "label — message", or the label alone.
    #[must_use]
    pub fn progress_text(&self) -> String {
        if self.message.is_empty() {
            self.label.clone()
        } else {
            format!("{} — {}", self.label, self.message)
        }
    }

    /// The status-bar message when the job failed: "label failed: message"; `None`
    /// for every other state.
    #[must_use]
    pub fn failure(&self) -> Option<StatusMessage> {
        let error = self.error.as_ref()?;
        (self.status == JobStatus::Failed.as_str())
            .then(|| StatusMessage::error(format!("{} failed: {}", self.label, error.message)))
    }
}

/// An error as an interface reports it: the code the agent API uses, and the message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorNotice {
    /// The error code (`NOT_FOUND`, `ROUTING_CYCLE`, …).
    pub code: &'static str,
    /// The message, worded as in the prototype.
    pub message: String,
}

impl From<&DomainError> for ErrorNotice {
    fn from(error: &DomainError) -> Self {
        Self {
            code: error.code.as_str(),
            message: error.message.clone(),
        }
    }
}

impl From<DomainError> for ErrorNotice {
    fn from(error: DomainError) -> Self {
        Self::from(&error)
    }
}

impl From<&AppError> for ErrorNotice {
    /// A domain error keeps its code and message; any other failure is `INTERNAL` (or the
    /// render's and export's own code) with its text.
    fn from(error: &AppError) -> Self {
        match error {
            AppError::Domain(error) => Self::from(error),
            other => Self {
                code: other.code().as_str(),
                message: other.to_string(),
            },
        }
    }
}

impl From<&ErrorReport> for ErrorNotice {
    fn from(report: &ErrorReport) -> Self {
        Self {
            code: report.code.as_str(),
            message: report.message.clone(),
        }
    }
}

/// A status-bar message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StatusMessage {
    /// The text.
    pub text: String,
    /// How long it stays, in milliseconds.
    pub duration_ms: u32,
    /// Whether it reports an error (announced assertively).
    pub error: bool,
}

impl StatusMessage {
    /// An error or warning, shown for 8 s (the bridge's `errorOccurred`).
    #[must_use]
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            duration_ms: ERROR_MS,
            error: true,
        }
    }

    /// A message, shown for 4 s (the bridge's `message`, for example "Undo Create Track").
    #[must_use]
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            duration_ms: MESSAGE_MS,
            error: false,
        }
    }

    /// "Autosaved", shown for 1.5 s after each autosave (the prototype's
    /// `_do_autosave`; [`Autosaver::start_notifying`](crate::autosave::Autosaver::start_notifying)).
    #[must_use]
    pub fn autosaved() -> Self {
        Self {
            text: "Autosaved".to_owned(),
            duration_ms: AUTOSAVED_MS,
            error: false,
        }
    }

    /// The message of an edit from another source, shown for 3 s; `None` for the
    /// GUI's and the user's own edits.
    #[must_use]
    pub fn foreign_edit(notice: &SessionNotice) -> Option<Self> {
        notice.foreign_edit().map(|text| Self {
            text,
            duration_ms: FOREIGN_EDIT_MS,
            error: false,
        })
    }
}

/// What the pump of [`EventPump::start_notices`] hands to its sink.
#[derive(Debug, Clone, PartialEq)]
pub enum Notice {
    /// A session event.
    Session(SessionNotice),
    /// A job's change.
    Job(JobNotice),
    /// A waveform pyramid is ready, or failed.
    Peaks(PeaksReady),
}

/// The session's events from the moment of subscribing, queued until an [`EventPump`] takes
/// them. Dropping it unsubscribes.
#[derive(Debug)]
pub struct Subscription(Receiver<SessionEvent>);

impl Subscription {
    /// Subscribes to `session`'s events.
    #[must_use]
    pub fn new(session: &SharedSession) -> Self {
        Self(session.lock().subscribe())
    }
}

/// The application's notices from the moment of subscribing: the session's events, the jobs'
/// changes and, when given, the waveform pyramids that became ready.
#[derive(Debug)]
pub struct Subscriptions {
    session: Subscription,
    jobs: Receiver<JobSnapshot>,
    peaks: Option<Receiver<PeaksReady>>,
}

impl Subscriptions {
    /// Subscribes to `session` and `jobs`.
    #[must_use]
    pub fn new(session: &SharedSession, jobs: &JobManager) -> Self {
        Self {
            session: Subscription::new(session),
            jobs: jobs.subscribe(),
            peaks: None,
        }
    }

    /// Adds the ready pyramids of `peaks` ([`crate::peaks::PeakCache::subscribe`]).
    #[must_use]
    pub fn with_peaks(mut self, peaks: Receiver<PeaksReady>) -> Self {
        self.peaks = Some(peaks);
        self
    }
}

/// A thread that hands the events of a [`Subscription`] to a sink.
#[derive(Debug)]
pub struct EventPump {
    /// Set by [`EventPump::stop`] (`Release`) and read by the pump between events (`Acquire`).
    stopping: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl EventPump {
    /// How long the pump waits for an event before it checks whether it should stop.
    pub const POLL_INTERVAL: Duration = Duration::from_millis(50);
    /// How long [`Self::start_notices`] waits for a session event before it looks at the jobs
    /// and the peaks.
    pub const SIDE_INTERVAL: Duration = Duration::from_millis(20);

    /// Calls `sink` with every event of `subscription`, queued ones first, in order, on a
    /// thread named `sonara-events`.
    ///
    /// # Errors
    ///
    /// The I/O error of starting the thread; the subscription ends with it.
    pub fn start<F>(subscription: Subscription, mut sink: F) -> io::Result<Self>
    where
        F: FnMut(&SessionEvent) + Send + 'static,
    {
        let Subscription(events) = subscription;
        Self::spawn(move |stop| {
            while !stop.load(Ordering::Acquire) {
                match events.recv_timeout(Self::POLL_INTERVAL) {
                    Ok(event) => sink(&event),
                    Err(RecvTimeoutError::Timeout) => {}
                    // The session is gone, and with it every later event.
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        })
    }

    /// Calls `sink` with every notice of `subscriptions` on a thread named `sonara-events`:
    /// the session's events in commit order, queued ones first, and between them the jobs'
    /// changes and the ready pyramids, each stream in its own order.
    ///
    /// # Errors
    ///
    /// The I/O error of starting the thread; the subscriptions end with it.
    pub fn start_notices<F>(subscriptions: Subscriptions, mut sink: F) -> io::Result<Self>
    where
        F: FnMut(Notice) + Send + 'static,
    {
        let Subscriptions {
            session: Subscription(events),
            jobs,
            peaks,
        } = subscriptions;
        Self::spawn(move |stop| {
            while !stop.load(Ordering::Acquire) {
                match events.recv_timeout(Self::SIDE_INTERVAL) {
                    Ok(event) => sink(Notice::Session(SessionNotice::from(&event))),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
                for job in jobs.try_iter() {
                    sink(Notice::Job(JobNotice::from(&job)));
                }
                if let Some(peaks) = &peaks {
                    for ready in peaks.try_iter() {
                        sink(Notice::Peaks(ready));
                    }
                }
            }
        })
    }

    fn spawn(body: impl FnOnce(&AtomicBool) + Send + 'static) -> io::Result<Self> {
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&stopping);
        let thread = thread::Builder::new()
            .name("sonara-events".to_owned())
            .spawn(move || body(&stop))?;
        Ok(Self {
            stopping,
            thread: Some(thread),
        })
    }

    /// Stops the pump and waits for its thread; events not yet handed over are dropped.
    pub fn stop(mut self) {
        self.halt();
    }

    fn halt(&mut self) {
        self.stopping.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            // ERR-06: a sink that panicked has ended the pump already; there is nothing left to
            // stop, and the panic was reported on that thread.
            let _ = thread.join();
        }
    }
}

impl Drop for EventPump {
    fn drop(&mut self) {
        self.halt();
    }
}

#[cfg(test)]
mod tests {
    use sonara_core::commands::{ChangeSet, Flag, ObjectKind};
    use sonara_core::error::ErrorCode;
    use sonara_core::model::JsonMap;
    use sonara_core::session::{EventKind, SessionEvent};

    use super::{JobNotice, SessionNotice, StatusMessage, structure_changed};
    use crate::error::ErrorReport;
    use crate::jobs::{JobSnapshot, JobStatus};

    fn event(kind: EventKind, flags: &[Flag], source: &str) -> SessionEvent {
        SessionEvent {
            kind,
            version: 7,
            changes: ChangeSet::of(
                [(ObjectKind::Track, "trk_1")],
                [(ObjectKind::Device, "dev_1")],
                [(ObjectKind::Clip, "clp_1")],
                flags.iter().copied(),
            ),
            label: "Create Track".to_owned(),
            source: source.to_owned(),
        }
    }

    /// The prototype's `structure_changed` for every flag and kind.
    #[test]
    fn structure_changes_follow_the_prototype() {
        let structural = [Flag::Tracks, Flag::Routing, Flag::Devices, Flag::Project];
        let refreshing = [
            Flag::Clips,
            Flag::Notes,
            Flag::Mixer,
            Flag::Automation,
            Flag::Tempo,
            Flag::Media,
            Flag::Markers,
            Flag::Transport,
            Flag::Settings,
            Flag::Metadata,
            Flag::Editor,
        ];
        for flag in structural {
            let changes = ChangeSet::flags([flag]);
            assert!(structure_changed(EventKind::Command, &changes), "{flag:?}");
        }
        for flag in refreshing {
            let changes = ChangeSet::flags([flag, Flag::Clips]);
            assert!(!structure_changed(EventKind::Command, &changes), "{flag:?}");
            assert!(!structure_changed(EventKind::Saved, &changes), "{flag:?}");
            for kind in [
                EventKind::Loaded,
                EventKind::Closed,
                EventKind::Undo,
                EventKind::Redo,
            ] {
                assert!(structure_changed(kind, &changes), "{kind:?} {flag:?}");
            }
        }
    }

    #[test]
    fn a_session_notice_carries_the_change_set() {
        let notice = SessionNotice::from(&event(
            EventKind::Command,
            &[Flag::Tracks, Flag::Mixer],
            "agent",
        ));
        assert_eq!(
            serde_json::to_value(&notice).unwrap(),
            serde_json::json!({
                "version": 7, "kind": "command", "label": "Create Track", "source": "agent",
                "changes": {
                    "created": [{"kind": "track", "id": "trk_1"}],
                    "updated": [{"kind": "device", "id": "dev_1"}],
                    "deleted": [{"kind": "clip", "id": "clp_1"}],
                    "flags": ["mixer", "tracks"],
                    "structure_changed": true,
                },
            })
        );
        let message = StatusMessage::foreign_edit(&notice).unwrap();
        assert_eq!(
            (message.text.as_str(), message.duration_ms, message.error),
            ("agent: Create Track", 3_000, false)
        );
        let own = SessionNotice::from(&event(EventKind::Command, &[Flag::Mixer], "gui"));
        assert!(!own.changes.structure_changed);
        assert_eq!(StatusMessage::foreign_edit(&own), None);
        let undo = SessionNotice::from(&event(EventKind::Undo, &[Flag::Mixer], "gui"));
        assert_eq!((undo.kind, undo.changes.structure_changed), ("undo", true));
    }

    #[test]
    fn job_notices_word_the_status_bar() {
        let mut job = JobSnapshot {
            id: "op_1".into(),
            kind: "export".into(),
            label: "Export audio".into(),
            status: JobStatus::Running,
            progress: 0.5,
            message: "rendering".into(),
            created: 1.0,
            started: Some(1.0),
            finished: None,
            result: None,
            error: None,
        };
        let running = JobNotice::from(&job);
        assert!(!running.finished);
        assert_eq!(running.progress_text(), "Export audio — rendering");
        assert_eq!(running.failure(), None);
        job.status = JobStatus::Failed;
        job.error = Some(ErrorReport {
            code: ErrorCode::IoError,
            message: "disk full".into(),
            details: JsonMap::new(),
        });
        let failed = JobNotice::from(&job);
        assert!(failed.finished);
        let message = failed.failure().unwrap();
        assert_eq!(message.text, "Export audio failed: disk full");
        assert!(message.error);
        assert_eq!(
            serde_json::to_value(&failed).unwrap()["error"],
            serde_json::json!({"code": "IO_ERROR", "message": "disk full"})
        );
        job.message.clear();
        job.status = JobStatus::Cancelled;
        assert_eq!(JobNotice::from(&job).progress_text(), "Export audio");
        assert_eq!(JobNotice::from(&job).failure(), None);
    }
}
