//! The loom shim: Sonara's protocol atomics and their `Arc`.
//!
//! Under `--cfg loom` (`scripts/dev.sh rust-loom`) these are loom's model-checked types, so the
//! loom tests explore every interleaving of the protocol around the rings: the generation, the
//! playing flag, the read-position snapshot, the queue ledger, the capture request and the
//! stats. `rtrb`'s own atomics are not instrumented; under loom its operations run as single,
//! sequentially consistent steps between the model's scheduling points.
//!
//! The memory orderings are the same in both builds. `Ordering` is std's in both, because loom
//! re-exports it.

#[cfg(loom)]
pub(crate) use loom::sync::Arc;
#[cfg(loom)]
pub(crate) use loom::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64};

#[cfg(not(loom))]
pub(crate) use std::sync::Arc;
#[cfg(not(loom))]
pub(crate) use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64};

pub(crate) use std::sync::atomic::Ordering;

/// Lets the loom models switch threads before one of `rtrb`'s operations, which loom does not
/// instrument; nothing outside `--cfg loom`.
///
/// Without it, loom runs a ring operation and the code up to the next protocol atomic as one
/// step, so it could not catch a protocol atomic published before the ring operation it must
/// follow. Under loom it adds 0 to `point`, the protocol atomic the other thread reads: a
/// write, so loom's partial-order reduction treats it as conflicting with that read and
/// explores the read on both sides of it. A relaxed read-modify-write of the same value
/// continues any release sequence, so it changes no synchronization.
#[inline]
pub(crate) fn interleave(point: &AtomicU64) {
    #[cfg(loom)]
    point.fetch_add(0, Ordering::Relaxed);
    #[cfg(not(loom))]
    let _ = point;
}
