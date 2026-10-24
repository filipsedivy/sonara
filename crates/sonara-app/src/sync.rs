//! The lock policy of the application services (MEM-08).

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Locks `mutex`, keeping its data when a thread panicked while holding it.
///
/// Every value behind these locks changes only through whole assignments made after the
/// fallible work succeeded (the session replaces its project after a command ran, settings
/// replace their recent list, a job replaces its fields), and no lock is held while foreign code
/// runs. A panicking holder therefore leaves the data consistent, and refusing every later lock
/// would only turn one defect into many.
pub(crate) fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
