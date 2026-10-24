//! The project session behind the application's lock.
//!
//! `sonara_core::session::ProjectSession` is unsynchronized. The prototype's session takes an
//! `RLock` in every method; here every interface (GUI, CLI, IPC, MCP, jobs) shares one
//! [`SharedSession`] and holds its [`Mutex`] for one session call or one short read (MEM-08).
//! `std::sync::Mutex` is not reentrant: code holding the guard never calls back into a method
//! that locks it again, and never does file I/O while holding it.
//!
//! Observers subscribe through [`ProjectSession::subscribe`] (an mpsc receiver of
//! [`sonara_core::session::SessionEvent`]s).

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use sonara_core::model::project::Project;
use sonara_core::session::ProjectSession;

use crate::sync::lock;

/// The one project session of an application, shared by every interface.
#[derive(Clone)]
pub struct SharedSession(Arc<Mutex<ProjectSession>>);

impl SharedSession {
    /// Shares `session`.
    #[must_use]
    pub fn new(session: ProjectSession) -> Self {
        Self(Arc::new(Mutex::new(session)))
    }

    /// Exclusive access to the session until the guard drops. Edits through the guard are
    /// serialized with every other interface's.
    pub fn lock(&self) -> MutexGuard<'_, ProjectSession> {
        lock(&self.0)
    }

    /// A snapshot of the open project (cheap: projects share their structure).
    #[must_use]
    pub fn project(&self) -> Option<Project> {
        self.lock().project().cloned()
    }
}

impl fmt::Debug for SharedSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `try_lock`: formatting while the guard is held must not deadlock.
        match self.0.try_lock() {
            Ok(session) => f.debug_tuple("SharedSession").field(&*session).finish(),
            Err(_) => f.write_str("SharedSession(<locked>)"),
        }
    }
}
