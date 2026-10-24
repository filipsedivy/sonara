//! The job manager (`sonara.application.services.jobs`). The prototype tests it through the
//! operation tools (`tests/agent/test_tools.py`: `test_render_preview…`, `test_cancel_operation`)
//! and records the shapes in `golden/coverage/transcript.json`; these tests check the same
//! states and shapes at the service.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value, json};
use sonara_app::error::AppError;
use sonara_app::ids::IdSource;
use sonara_app::jobs::{JobManager, JobStatus, MAX_RETAINED};
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::ids::{IdGenerator as _, SequentialIds};
use sonara_core::model::JsonMap;
use sonara_testkit::Golden;

const WAIT: Option<Duration> = Some(Duration::from_secs(30));

fn manager(workers: usize, max_pending: usize) -> (JobManager, IdSource) {
    let ids = IdSource::new(Box::new(SequentialIds::new(0)));
    let workers = NonZeroUsize::new(workers).expect("at least one worker");
    let jobs = JobManager::new(workers, max_pending, ids.clone()).expect("workers start");
    (jobs, ids)
}

fn result(pairs: &[(&str, Value)]) -> JsonMap {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect()
}

/// The coverage transcript's result of step `id`.
fn golden_step(golden: &Value, id: &str) -> Value {
    golden["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .find(|step| step["id"] == id)
        .map_or_else(|| panic!("no step {id}"), |step| step["result"].clone())
}

/// `actual` has exactly the recorded keys (without the tool envelope's `ok` and `tool`) and
/// the recorded values, except the operation ID and the values recorded as volatile.
fn assert_matches(actual: &Value, recorded: &Value) {
    let recorded = recorded.as_object().expect("recorded object");
    let actual = actual.as_object().expect("actual object");
    let mut expected_keys: Vec<&String> = recorded
        .keys()
        .filter(|k| *k != "ok" && *k != "tool")
        .collect();
    let mut actual_keys: Vec<&String> = actual.keys().collect();
    expected_keys.sort();
    actual_keys.sort();
    assert_eq!(actual_keys, expected_keys);
    for (key, value) in recorded {
        if key == "ok" || key == "tool" || key == "operation_id" || value == "<VOLATILE>" {
            continue;
        }
        assert_eq!(&actual[key], value, "{key}");
    }
}

#[test]
fn a_successful_job_keeps_its_result() {
    let (jobs, _) = manager(2, 16);
    let job = jobs
        .submit("decode", "Decode tone.wav", |ctx| {
            ctx.progress(0.5, Some("half way"));
            Ok(result(&[("decoded", json!(true))]))
        })
        .expect("submitted");
    assert_eq!(job.id(), "op_000000000001");
    assert!(job.wait(WAIT));
    let snapshot = job.snapshot();
    assert_eq!(snapshot.status, JobStatus::Succeeded);
    assert_eq!(snapshot.progress, 1.0);
    assert_eq!(snapshot.message, "half way");
    assert!(snapshot.started.is_some() && snapshot.finished.is_some());
    // The recorded decode operation (`get_operation_status` with `wait_seconds`).
    let golden = Golden::open().expect("golden data");
    let transcript: Value = golden.json("coverage/transcript.json").expect("transcript");
    let mut recorded = golden_step(&transcript, "step73");
    recorded["label"] = json!("Decode tone.wav");
    recorded["message"] = json!("half way");
    recorded["result"] = json!({"decoded": true});
    assert_matches(&job.to_json(), &recorded);
}

#[test]
fn a_job_cancelled_while_queued_matches_the_golden_transcript() {
    let golden = Golden::open().expect("golden data");
    let transcript: Value = golden.json("coverage/transcript.json").expect("transcript");
    let (jobs, _) = manager(2, 16);
    let ran = Arc::new(AtomicBool::new(false));
    // `hold_jobs`: every worker busy, so the render stays queued.
    let hold = jobs.hold_workers();
    let flag = Arc::clone(&ran);
    let job = jobs
        .submit("render_preview", "Render preview", move |_| {
            flag.store(true, Ordering::SeqCst);
            Ok(JsonMap::new())
        })
        .expect("submitted");
    assert_matches(&job.to_json(), &golden_step(&transcript, "step134"));
    let cancelled = jobs.cancel(job.id()).expect("known");
    assert_matches(&cancelled.to_json(), &golden_step(&transcript, "step135"));
    // `list_operations` overlays `result: null` on every entry.
    let listed: Vec<Value> = jobs
        .list(30)
        .iter()
        .map(|j| {
            let mut entry = j.to_json();
            entry["result"] = Value::Null;
            entry
        })
        .collect();
    let recorded_list = golden_step(&transcript, "step136");
    let recorded_entry = recorded_list["operations"]
        .as_array()
        .and_then(|ops| ops.last())
        .expect("the pending operation is listed last");
    assert_eq!(listed.len(), 1);
    assert_matches(&listed[0], recorded_entry);
    // `release_jobs`, then `get_operation_status` with `wait_seconds`.
    hold.release();
    assert!(
        jobs.wait(job.id(), WAIT)
            .expect("known")
            .wait(Some(Duration::ZERO))
    );
    assert_matches(&job.to_json(), &golden_step(&transcript, "step138"));
    assert_eq!(job.snapshot().error, None);
    assert!(
        !ran.load(Ordering::SeqCst),
        "a job cancelled while queued never runs"
    );
    // Cancelling a finished job changes nothing (the finished render).
    let again = jobs.cancel(job.id()).expect("known");
    assert_eq!(again.status(), JobStatus::Cancelled);
}

#[test]
fn a_running_job_ends_cancelled_through_check() {
    let (jobs, _) = manager(1, 16);
    let (started_tx, started_rx) = mpsc::channel();
    let job = jobs
        .submit("render_preview", "Render preview", move |ctx| {
            started_tx.send(()).expect("test listens");
            while !ctx.is_cancelled() {
                std::thread::sleep(Duration::from_millis(1));
            }
            ctx.check()?;
            Ok(JsonMap::new())
        })
        .expect("submitted");
    started_rx.recv().expect("job started");
    assert_eq!(job.status(), JobStatus::Running);
    jobs.cancel(job.id()).expect("known");
    assert!(job.wait(WAIT));
    let out = job.to_json();
    // `test_cancel_operation`.
    assert_eq!(out["status"], "cancelled");
    assert_eq!(
        out["error"],
        json!({"code": "CANCELLED", "message": "operation cancelled"})
    );
    assert_eq!(out["message"], "cancelling");
    assert!(out.get("elapsed_seconds").is_some());
}

#[test]
fn failures_keep_their_error_and_panics_are_internal() {
    let (jobs, _) = manager(2, 16);
    let failed = jobs
        .submit("export", "Export", |ctx| {
            ctx.progress(7.0, None);
            Err(DomainError::new(ErrorCode::InvalidArgument, "bad range")
                .with_detail("start", 4)
                .into())
        })
        .expect("submitted");
    let clamped = jobs
        .submit("export", "Export", |ctx| {
            ctx.progress(-0.5, Some("negative"));
            Err(AppError::from(DomainError::new(ErrorCode::Timeout, "slow")))
        })
        .expect("submitted");
    let panicked = jobs
        .submit("analysis", "Analyze", |_| panic!("boom"))
        .expect("submitted");
    for job in [&failed, &clamped, &panicked] {
        assert!(job.wait(WAIT));
        assert_eq!(job.status(), JobStatus::Failed);
        assert!(job.to_json().get("result").is_none());
    }
    assert_eq!(
        failed.to_json()["error"],
        json!({"code": "INVALID_ARGUMENT", "message": "bad range", "details": {"start": 4}})
    );
    assert_eq!(failed.snapshot().progress, 1.0, "progress is clamped to 1");
    assert_eq!(clamped.snapshot().progress, 0.0, "progress is clamped to 0");
    assert_eq!(
        panicked.to_json()["error"],
        json!({"code": "INTERNAL", "message": "panic: boom"})
    );
    // The workers survive the panic.
    let after = jobs
        .submit("decode", "Decode", |_| Ok(JsonMap::new()))
        .expect("submitted");
    assert!(after.wait(WAIT));
    assert_eq!(after.status(), JobStatus::Succeeded);
}

#[test]
fn the_pending_limit_uses_no_operation_id() {
    let (jobs, mut ids) = manager(1, 2);
    let hold = jobs.hold_workers();
    let first = jobs
        .submit("a", "A", |_| Ok(JsonMap::new()))
        .expect("first");
    let second = jobs
        .submit("b", "B", |_| Ok(JsonMap::new()))
        .expect("second");
    let err = jobs
        .submit("c", "C", |_| Ok(JsonMap::new()))
        .expect_err("over the limit");
    assert_eq!(err.code(), ErrorCode::LimitExceeded);
    assert_eq!(
        err.to_string(),
        "[LIMIT_EXCEEDED] too many operations in progress (2); wait or cancel some"
    );
    assert_eq!(ids.new_id("trk"), "trk_000000000003");
    drop(hold);
    for job in [&first, &second] {
        assert!(job.wait(WAIT));
    }
    jobs.submit("d", "D", |_| Ok(JsonMap::new()))
        .expect("room again");
}

#[test]
fn unknown_operations_are_not_found() {
    let (jobs, _) = manager(1, 16);
    // Steps 131 and 141 of the coverage transcript.
    for err in [
        jobs.get("op_missing").expect_err("unknown"),
        jobs.cancel("op_missing").expect_err("unknown"),
        jobs.wait("op_missing", None).expect_err("unknown"),
    ] {
        assert_eq!(err.code, ErrorCode::NotFound);
        assert_eq!(err.message, "operation 'op_missing' not found");
        assert!(err.details.is_empty());
    }
}

#[test]
fn old_finished_jobs_are_forgotten_and_lists_are_ordered() {
    let (jobs, _) = manager(2, 16);
    let mut ids = Vec::new();
    for _ in 0..=MAX_RETAINED {
        let job = jobs
            .submit("decode", "Decode", |_| Ok(JsonMap::new()))
            .expect("submitted");
        assert!(job.wait(WAIT));
        ids.push(job.id().to_owned());
    }
    assert!(
        jobs.get(&ids[0]).is_err(),
        "the oldest finished job is gone"
    );
    assert!(jobs.get(&ids[1]).is_ok());
    assert_eq!(jobs.list(0).len(), MAX_RETAINED);
    let newest: Vec<String> = jobs.list(2).iter().map(|j| j.id().to_owned()).collect();
    assert_eq!(newest, ids[ids.len() - 2..]);
}

#[test]
fn listeners_hear_the_changes() {
    let (jobs, _) = manager(1, 16);
    let events = jobs.subscribe();
    let job = jobs
        .submit("render_preview", "Render preview", |ctx| {
            ctx.progress(0.1, Some("first"));
            ctx.progress(1.0, Some("done"));
            Ok(result(&[("path", json!("/x.wav"))]))
        })
        .expect("submitted");
    assert!(job.wait(WAIT));
    jobs.shutdown(true);
    let seen: Vec<(JobStatus, String)> = events
        .try_iter()
        .map(|s| (s.status, s.message.clone()))
        .collect();
    assert!(
        seen.contains(&(JobStatus::Running, String::new())),
        "{seen:?}"
    );
    assert!(
        seen.contains(&(JobStatus::Running, "first".to_owned())),
        "{seen:?}"
    );
    assert!(
        seen.contains(&(JobStatus::Running, "done".to_owned())),
        "{seen:?}"
    );
    assert!(
        seen.contains(&(JobStatus::Succeeded, "done".to_owned())),
        "{seen:?}"
    );
}

#[test]
fn shutdown_cancels_queued_jobs_and_refuses_new_ones() {
    let (jobs, _) = manager(1, 16);
    let hold = jobs.hold_workers();
    let ran = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&ran);
    let queued = jobs
        .submit("render_preview", "Render preview", move |_| {
            flag.store(true, Ordering::SeqCst);
            Ok(JsonMap::new())
        })
        .expect("submitted");
    jobs.shutdown(false);
    drop(hold);
    assert!(queued.wait(WAIT));
    assert_eq!(queued.status(), JobStatus::Cancelled);
    assert!(!ran.load(Ordering::SeqCst));
    let err = jobs
        .submit("x", "X", |_| Ok(JsonMap::new()))
        .expect_err("shut down");
    assert_eq!(err.code(), ErrorCode::Internal);
    assert_eq!(
        err.to_json(),
        json!({"code": "INTERNAL", "message": "the job manager is shut down"})
    );
}
