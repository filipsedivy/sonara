//! Background jobs (`sonara.application.services.jobs`): rendering, export, analysis, media
//! decoding.
//!
//! Jobs run on a fixed pool of worker threads, report progress, can be cancelled cooperatively
//! and keep their result or structured error for later status queries
//! (`get_operation_status`). Jobs never touch the audio device thread.
//!
//! The pool is plain `std::thread` workers sharing one [`crossbeam_channel`] queue (the
//! prototype's `ThreadPoolExecutor`); it needs no async runtime. The JSON shapes are the
//! prototype's `Job.to_json()`, recorded in `golden/coverage/transcript.json`.
//!
//! Operation IDs come from the application's [`IdSource`], so that they interleave with the
//! other IDs as in the golden transcripts.
//!
//! **Locks.** The job list is locked before a job's state, never the other way round, and no
//! lock is held while a job function or a listener runs.
//!
//! **Deviations.** After [`JobManager::shutdown`], queued jobs end as `cancelled` (the
//! prototype leaves them `queued` for ever), and a later [`JobManager::submit`] fails with
//! `INTERNAL` "the job manager is shut down" instead of registering a job that never runs.

use std::cell::Cell;
use std::collections::VecDeque;
use std::fmt;
use std::num::NonZeroUsize;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Value, json};
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::ids::IdGenerator as _;
use sonara_core::model::JsonMap;
use sonara_core::pyfmt::{py_round, str_repr};

use crate::clock::unix_seconds;
use crate::error::{AppError, AppResult, ErrorReport};
use crate::ids::IdSource;
use crate::sync::lock;

/// Finished jobs kept for status queries; older finished jobs are forgotten.
pub const MAX_RETAINED: usize = 200;
/// Unfinished (queued or running) jobs allowed at once.
pub const DEFAULT_MAX_PENDING: usize = 16;
/// The prototype's default number of worker threads.
pub const DEFAULT_WORKERS: NonZeroUsize = NonZeroUsize::new(2).expect("2 is not zero");
/// The ID prefix of operations.
pub const OPERATION_PREFIX: &str = "op";
/// The least time between two throttled progress notifications.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// Where a job is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobStatus {
    /// Waiting for a worker.
    Queued,
    /// Running on a worker.
    Running,
    /// Finished with a result.
    Succeeded,
    /// Finished with an error.
    Failed,
    /// Cancelled before or while running.
    Cancelled,
}

impl JobStatus {
    /// The status as tool results report it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Whether the job has finished (succeeded, failed or cancelled).
    #[must_use]
    pub const fn is_finished(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

/// A job's state at one moment.
#[derive(Debug, Clone, PartialEq)]
pub struct JobSnapshot {
    /// The operation ID (`op_…`).
    pub id: String,
    /// What the job does, for example `render_preview`.
    pub kind: String,
    /// A human-readable label.
    pub label: String,
    /// Where the job is.
    pub status: JobStatus,
    /// Progress, 0 to 1.
    pub progress: f64,
    /// The latest progress message.
    pub message: String,
    /// When the job was submitted (Unix seconds).
    pub created: f64,
    /// When the job started running.
    pub started: Option<f64>,
    /// When the job finished.
    pub finished: Option<f64>,
    /// The result of a successful job.
    pub result: Option<JsonMap>,
    /// The error of a failed job, or of one cancelled while running.
    pub error: Option<ErrorReport>,
}

impl JobSnapshot {
    /// The prototype's `Job.to_json()`: `elapsed_seconds` only once the job started, `result`
    /// and `error` only when present. Progress is rounded to 4 digits and the elapsed time to 3,
    /// like Python's `round`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        self.to_json_at(unix_seconds(SystemTime::now()))
    }

    /// [`Self::to_json`], measuring a running job's elapsed time up to `now` (Unix seconds).
    fn to_json_at(&self, now: f64) -> Value {
        let mut out = json!({
            "operation_id": self.id,
            "kind": self.kind,
            "label": self.label,
            "status": self.status.as_str(),
            "progress": py_round(self.progress, 4),
            "message": self.message,
            "created": self.created,
            "started": self.started,
            "finished": self.finished,
        });
        if let Some(started) = self.started {
            out["elapsed_seconds"] = json!(py_round(self.finished.unwrap_or(now) - started, 3));
        }
        if let Some(result) = &self.result {
            out["result"] = Value::Object(result.clone());
        }
        if let Some(error) = &self.error {
            out["error"] = error.to_json();
        }
        out
    }
}

/// The changing part of a job.
#[derive(Debug)]
struct JobState {
    status: JobStatus,
    progress: f64,
    message: String,
    started: Option<f64>,
    finished: Option<f64>,
    result: Option<JsonMap>,
    error: Option<ErrorReport>,
}

#[derive(Debug)]
struct JobShared {
    id: String,
    kind: String,
    label: String,
    created: f64,
    state: Mutex<JobState>,
    /// Signalled when the job finishes.
    done: Condvar,
    /// Set by `cancel` and `shutdown`; read by the job and its worker.
    cancel: AtomicBool,
}

/// A submitted job: a cheap handle that others can poll, wait for and cancel.
#[derive(Debug, Clone)]
pub struct Job(Arc<JobShared>);

impl Job {
    fn new(id: String, kind: &str, label: &str) -> Self {
        Self(Arc::new(JobShared {
            id,
            kind: kind.to_owned(),
            label: label.to_owned(),
            created: now(),
            state: Mutex::new(JobState {
                status: JobStatus::Queued,
                progress: 0.0,
                message: String::new(),
                started: None,
                finished: None,
                result: None,
                error: None,
            }),
            done: Condvar::new(),
            cancel: AtomicBool::new(false),
        }))
    }

    /// The operation ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.0.id
    }

    /// What the job does.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.0.kind
    }

    /// The job's label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.0.label
    }

    /// Where the job is now.
    #[must_use]
    pub fn status(&self) -> JobStatus {
        lock(&self.0.state).status
    }

    /// Whether cancellation was requested.
    #[must_use]
    pub fn is_cancel_requested(&self) -> bool {
        // Acquire pairs with the Release store in `request_cancel`.
        self.0.cancel.load(Ordering::Acquire)
    }

    /// The job's state now.
    #[must_use]
    pub fn snapshot(&self) -> JobSnapshot {
        let s = lock(&self.0.state);
        JobSnapshot {
            id: self.0.id.clone(),
            kind: self.0.kind.clone(),
            label: self.0.label.clone(),
            status: s.status,
            progress: s.progress,
            message: s.message.clone(),
            created: self.0.created,
            started: s.started,
            finished: s.finished,
            result: s.result.clone(),
            error: s.error.clone(),
        }
    }

    /// [`JobSnapshot::to_json`] of the state now.
    #[must_use]
    pub fn to_json(&self) -> Value {
        self.snapshot().to_json()
    }

    /// Waits until the job finishes, at most `timeout` (`None`: no limit). Returns whether it
    /// has finished.
    #[must_use]
    pub fn wait(&self, timeout: Option<Duration>) -> bool {
        let state = lock(&self.0.state);
        let unfinished = |s: &mut JobState| !s.status.is_finished();
        let state = match timeout {
            None => self
                .0
                .done
                .wait_while(state, unfinished)
                .unwrap_or_else(PoisonError::into_inner),
            Some(timeout) => {
                self.0
                    .done
                    .wait_timeout_while(state, timeout, unfinished)
                    .unwrap_or_else(PoisonError::into_inner)
                    .0
            }
        };
        state.status.is_finished()
    }

    /// Sets the cancel flag of an unfinished job; with `message`, also says so.
    fn request_cancel(&self, message: Option<&str>) {
        let mut s = lock(&self.0.state);
        if !s.status.is_finished() {
            // Release pairs with the Acquire loads in `is_cancel_requested`.
            self.0.cancel.store(true, Ordering::Release);
            if let Some(message) = message {
                message.clone_into(&mut s.message);
            }
        }
    }

    /// Records the end of the job and wakes its waiters.
    fn finish(&self, change: impl FnOnce(&mut JobState)) {
        {
            let mut s = lock(&self.0.state);
            change(&mut s);
            s.finished = Some(now());
        }
        self.0.done.notify_all();
    }
}

/// Unix seconds now (Python's `time.time()`).
fn now() -> f64 {
    unix_seconds(SystemTime::now())
}

/// The observers of job changes.
type Listeners = Mutex<Vec<Sender<JobSnapshot>>>;

/// Sends `job`'s state to every listener, forgetting those that hung up.
fn notify(listeners: &Listeners, job: &Job) {
    let snapshot = job.snapshot();
    lock(listeners).retain(|listener| listener.send(snapshot.clone()).is_ok());
}

/// What a running job sees: progress reporting and cancellation.
pub struct JobContext<'a> {
    job: &'a Job,
    listeners: &'a Arc<Listeners>,
    last_notified: Cell<Option<Instant>>,
}

/// Sets `job`'s progress (clamped to 0..1) and, optionally, its message, and tells the listeners
/// at most every 100 ms (`last_notified`), and always at completion.
fn report_progress(
    job: &Job,
    listeners: &Listeners,
    last_notified: &Cell<Option<Instant>>,
    fraction: f64,
    message: Option<&str>,
) {
    {
        let mut s = lock(&job.0.state);
        s.progress = fraction.clamp(0.0, 1.0);
        if let Some(message) = message {
            message.clone_into(&mut s.message);
        }
    }
    let now = Instant::now();
    let due = last_notified
        .get()
        .is_none_or(|last| now.duration_since(last) > PROGRESS_INTERVAL);
    if due || fraction >= 1.0 {
        last_notified.set(Some(now));
        notify(listeners, job);
    }
}

impl JobContext<'_> {
    /// Reports progress (clamped to 0..1) and, optionally, a message. Listeners hear of it at
    /// most every 100 ms, and always at completion.
    pub fn progress(&self, fraction: f64, message: Option<&str>) {
        report_progress(
            self.job,
            self.listeners,
            &self.last_notified,
            fraction,
            message,
        );
    }

    /// An owned handle that reports this job's progress, for a callback that cannot borrow the
    /// context (one that must be `'static`, such as the exporter's).
    #[must_use]
    pub fn progress_handle(&self) -> JobProgress {
        JobProgress {
            job: self.job.clone(),
            listeners: Arc::clone(self.listeners),
            last_notified: Cell::new(self.last_notified.get()),
        }
    }

    /// Whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.job.is_cancel_requested()
    }

    /// Fails once cancellation was requested, so that `?` ends the job as cancelled.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::Cancelled`]: `operation cancelled`.
    pub fn check(&self) -> Result<(), DomainError> {
        if self.is_cancelled() {
            Err(DomainError::new(
                ErrorCode::Cancelled,
                "operation cancelled",
            ))
        } else {
            Ok(())
        }
    }
}

/// Reports a running job's progress like [`JobContext::progress`], without borrowing the
/// context ([`JobContext::progress_handle`]).
pub struct JobProgress {
    job: Job,
    listeners: Arc<Listeners>,
    last_notified: Cell<Option<Instant>>,
}

impl fmt::Debug for JobProgress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JobProgress")
            .field("job", &self.job.id())
            .finish_non_exhaustive()
    }
}

impl JobProgress {
    /// Reports progress (clamped to 0..1) and, optionally, a message, as
    /// [`JobContext::progress`] does.
    pub fn progress(&self, fraction: f64, message: Option<&str>) {
        report_progress(
            &self.job,
            &self.listeners,
            &self.last_notified,
            fraction,
            message,
        );
    }
}

/// A unit of work for a worker thread.
type Task = Box<dyn FnOnce() + Send>;

/// Runs `job` on the current worker, recording its outcome.
fn run<F>(job: &Job, listeners: &Arc<Listeners>, work: F)
where
    F: FnOnce(&JobContext<'_>) -> AppResult<JsonMap>,
{
    if job.is_cancel_requested() {
        // Cancelled while queued: no error, `started` stays unset.
        job.finish(|s| s.status = JobStatus::Cancelled);
        notify(listeners, job);
        return;
    }
    {
        let mut s = lock(&job.0.state);
        s.status = JobStatus::Running;
        s.started = Some(now());
    }
    notify(listeners, job);
    let ctx = JobContext {
        job,
        listeners,
        last_notified: Cell::new(None),
    };
    // ERR-07: a panicking job is a defect; the worker survives it and reports it as INTERNAL
    // (the release profile unwinds, quality-gate.md). The job's state is not locked meanwhile.
    let outcome = catch_unwind(AssertUnwindSafe(|| work(&ctx)));
    job.finish(|s| match outcome {
        Ok(Ok(result)) => {
            s.result = Some(result);
            s.progress = 1.0;
            s.status = JobStatus::Succeeded;
        }
        Ok(Err(err)) => {
            s.status = if err.code() == ErrorCode::Cancelled {
                JobStatus::Cancelled
            } else {
                JobStatus::Failed
            };
            s.error = Some(err.report());
        }
        Err(payload) => {
            let reason = payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_owned());
            log::error!("job {} ({}) failed: {reason}", job.0.id, job.0.kind);
            s.status = JobStatus::Failed;
            s.error = Some(ErrorReport {
                code: ErrorCode::Internal,
                message: format!("panic: {reason}"),
                details: JsonMap::new(),
            });
        }
    });
    notify(listeners, job);
}

/// A gate that holds workers until it opens.
#[derive(Debug, Default)]
struct Gate {
    open: Mutex<bool>,
    opened: Condvar,
}

impl Gate {
    fn wait(&self) {
        let open = lock(&self.open);
        let _open = self
            .opened
            .wait_while(open, |open| !*open)
            .unwrap_or_else(PoisonError::into_inner);
    }

    fn open(&self) {
        *lock(&self.open) = true;
        self.opened.notify_all();
    }
}

/// Every worker held by [`JobManager::hold_workers`]; dropping it releases them.
#[derive(Debug)]
#[must_use = "dropping the hold releases the workers at once"]
pub struct WorkerHold {
    gate: Arc<Gate>,
}

impl WorkerHold {
    /// Releases the workers (the same as dropping the hold).
    pub fn release(self) {}
}

impl Drop for WorkerHold {
    fn drop(&mut self) {
        self.gate.open();
    }
}

struct Inner {
    jobs: Mutex<VecDeque<Job>>,
    listeners: Arc<Listeners>,
    /// `None` once shut down.
    queue: Mutex<Option<crossbeam_channel::Sender<Task>>>,
    workers: Mutex<Vec<JoinHandle<()>>>,
    worker_count: NonZeroUsize,
    max_pending: usize,
    ids: IdSource,
}

impl Inner {
    fn shutdown(&self, wait: bool) {
        for job in lock(&self.jobs).iter() {
            job.request_cancel(None);
        }
        // Closing the queue lets the workers drain it: the cancelled jobs end at once.
        drop(lock(&self.queue).take());
        if wait {
            let workers: Vec<JoinHandle<()>> = lock(&self.workers).drain(..).collect();
            for worker in workers {
                // ERR-06: a worker only ends by panicking outside a job, which the panic hook
                // has reported already; shutting down goes on regardless.
                let _ = worker.join();
            }
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.shutdown(false);
    }
}

/// The job manager: a bounded pool of worker threads and the jobs it ran. Clones share it; the
/// workers stop once the last clone is dropped or [`Self::shutdown`] is called.
#[derive(Clone)]
pub struct JobManager(Arc<Inner>);

impl fmt::Debug for JobManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JobManager")
            .field("workers", &self.0.worker_count)
            .field("max_pending", &self.0.max_pending)
            .finish_non_exhaustive()
    }
}

impl JobManager {
    /// A manager with `workers` threads (named `sonara-job-<n>`), allowing `max_pending`
    /// unfinished jobs, drawing operation IDs from `ids`.
    ///
    /// # Errors
    ///
    /// The I/O error of spawning a worker thread.
    pub fn new(workers: NonZeroUsize, max_pending: usize, ids: IdSource) -> std::io::Result<Self> {
        let (sender, receiver) = crossbeam_channel::unbounded::<Task>();
        let mut handles = Vec::with_capacity(workers.get());
        for index in 0..workers.get() {
            let receiver = receiver.clone();
            let handle = thread::Builder::new()
                .name(format!("sonara-job-{index}"))
                .spawn(move || {
                    for task in receiver {
                        task();
                    }
                })?;
            handles.push(handle);
        }
        Ok(Self(Arc::new(Inner {
            jobs: Mutex::new(VecDeque::new()),
            listeners: Arc::new(Mutex::new(Vec::new())),
            queue: Mutex::new(Some(sender)),
            workers: Mutex::new(handles),
            worker_count: workers,
            max_pending,
            ids,
        })))
    }

    /// The number of worker threads.
    #[must_use]
    pub fn workers(&self) -> NonZeroUsize {
        self.0.worker_count
    }

    /// Queues `work` as a job of `kind` with `label`. The job's result is the map `work`
    /// returns; an error with [`ErrorCode::Cancelled`] ends it as cancelled, any other error or
    /// a panic as failed.
    ///
    /// # Errors
    ///
    /// - [`ErrorCode::LimitExceeded`]: `too many operations in progress (<n>); wait or cancel
    ///   some`, without using an operation ID;
    /// - [`ErrorCode::Internal`]: `the job manager is shut down`.
    pub fn submit<F>(&self, kind: &str, label: &str, work: F) -> AppResult<Job>
    where
        F: FnOnce(&JobContext<'_>) -> AppResult<JsonMap> + Send + 'static,
    {
        let shut_down = || {
            AppError::from(DomainError::new(
                ErrorCode::Internal,
                "the job manager is shut down",
            ))
        };
        if lock(&self.0.queue).is_none() {
            return Err(shut_down());
        }
        let job = {
            let mut jobs = lock(&self.0.jobs);
            let pending = jobs.iter().filter(|j| !j.status().is_finished()).count();
            if pending >= self.0.max_pending {
                return Err(DomainError::new(
                    ErrorCode::LimitExceeded,
                    format!("too many operations in progress ({pending}); wait or cancel some"),
                )
                .into());
            }
            let job = Job::new(self.0.ids.clone().new_id(OPERATION_PREFIX), kind, label);
            jobs.push_back(job.clone());
            while jobs.len() > MAX_RETAINED {
                match jobs.front() {
                    Some(oldest) if oldest.status().is_finished() => {
                        jobs.pop_front();
                    }
                    _ => break,
                }
            }
            job
        };
        let task: Task = {
            let job = job.clone();
            let listeners = Arc::clone(&self.0.listeners);
            Box::new(move || run(&job, &listeners, work))
        };
        let queued = lock(&self.0.queue)
            .as_ref()
            .is_some_and(|queue| queue.send(task).is_ok());
        if !queued {
            // Shut down meanwhile: the job never runs.
            job.request_cancel(None);
            job.finish(|s| s.status = JobStatus::Cancelled);
            return Err(shut_down());
        }
        notify(&self.0.listeners, &job);
        Ok(job)
    }

    /// The job with `job_id`.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NotFound`]: `operation '<id>' not found`.
    pub fn get(&self, job_id: &str) -> Result<Job, DomainError> {
        lock(&self.0.jobs)
            .iter()
            .find(|job| job.id() == job_id)
            .cloned()
            .ok_or_else(|| {
                DomainError::new(
                    ErrorCode::NotFound,
                    format!("operation {} not found", str_repr(job_id)),
                )
            })
    }

    /// Requests cancellation of an unfinished job (its message becomes `cancelling`); a
    /// finished job is unchanged.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NotFound`], as [`Self::get`].
    pub fn cancel(&self, job_id: &str) -> Result<Job, DomainError> {
        let job = self.get(job_id)?;
        if !job.status().is_finished() {
            job.request_cancel(Some("cancelling"));
            notify(&self.0.listeners, &job);
        }
        Ok(job)
    }

    /// The job with `job_id`, after waiting at most `timeout` for it to finish.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NotFound`], as [`Self::get`].
    pub fn wait(&self, job_id: &str, timeout: Option<Duration>) -> Result<Job, DomainError> {
        let job = self.get(job_id)?;
        // The caller reads the status; finishing in time is not an error.
        let _finished = job.wait(timeout);
        Ok(job)
    }

    /// The newest `limit` jobs, oldest first (Python's `[-limit:]`: 0 means all).
    #[must_use]
    pub fn list(&self, limit: usize) -> Vec<Job> {
        let jobs = lock(&self.0.jobs);
        let skip = if limit == 0 {
            0
        } else {
            jobs.len().saturating_sub(limit)
        };
        jobs.iter().skip(skip).cloned().collect()
    }

    /// A channel receiving every later change of every job. Dropping the receiver
    /// unsubscribes.
    #[must_use]
    pub fn subscribe(&self) -> Receiver<JobSnapshot> {
        let (sender, receiver) = channel();
        lock(&self.0.listeners).push(sender);
        receiver
    }

    /// Occupies every worker until the returned hold is released or dropped, so that jobs
    /// submitted meanwhile stay queued. No job is registered. For test harnesses: the golden
    /// coverage batch's `hold_jobs` and `release_jobs` steps.
    pub fn hold_workers(&self) -> WorkerHold {
        let gate = Arc::new(Gate::default());
        if let Some(queue) = lock(&self.0.queue).as_ref() {
            for _ in 0..self.0.worker_count.get() {
                let gate = Arc::clone(&gate);
                // ERR-06: sending fails only when every worker has exited; then there is
                // nothing left to hold.
                let _ = queue.send(Box::new(move || gate.wait()));
            }
        }
        WorkerHold { gate }
    }

    /// Requests cancellation of every unfinished job and stops accepting jobs. Queued jobs end
    /// as cancelled; with `wait`, returns once the workers have finished. Never call it with
    /// `wait` from inside a job, or while holding workers.
    pub fn shutdown(&self, wait: bool) {
        self.0.shutdown(wait);
    }
}

#[cfg(test)]
mod tests {
    use super::{JobSnapshot, JobStatus};
    use crate::error::ErrorReport;
    use serde_json::json;
    use sonara_core::error::ErrorCode;
    use sonara_core::model::JsonMap;

    #[test]
    fn json_has_the_prototype_shape() {
        let mut snapshot = JobSnapshot {
            id: "op_1".into(),
            kind: "render_preview".into(),
            label: "Render preview".into(),
            status: JobStatus::Queued,
            progress: 0.0,
            message: String::new(),
            created: 10.0,
            started: None,
            finished: None,
            result: None,
            error: None,
        };
        assert_eq!(
            snapshot.to_json_at(20.0),
            json!({"operation_id": "op_1", "kind": "render_preview", "label": "Render preview",
                   "status": "queued", "progress": 0.0, "message": "", "created": 10.0,
                   "started": null, "finished": null})
        );
        snapshot.status = JobStatus::Running;
        snapshot.started = Some(11.0);
        snapshot.progress = 0.123_456;
        assert_eq!(snapshot.to_json_at(12.5)["elapsed_seconds"], json!(1.5));
        assert_eq!(snapshot.to_json_at(12.5)["progress"], json!(0.1235));
        snapshot.status = JobStatus::Failed;
        snapshot.finished = Some(11.25);
        snapshot.error = Some(ErrorReport {
            code: ErrorCode::Internal,
            message: "boom".into(),
            details: JsonMap::new(),
        });
        let out = snapshot.to_json_at(99.0);
        assert_eq!(out["elapsed_seconds"], json!(0.25));
        assert_eq!(out["error"], json!({"code": "INTERNAL", "message": "boom"}));
        assert!(out.get("result").is_none());
    }
}
