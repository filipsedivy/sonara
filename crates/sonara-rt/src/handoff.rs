//! The hand-off of owned values to a real-time thread and back, for render graphs and the
//! setup prepared for them (RT-09).
//!
//! The control thread [`send`](Sender::send)s a value through one ring. The real-time thread
//! [`receive`](Receiver::receive)s it: it changes the value in place (for a graph, it swaps the
//! value with its current graph) and returns it through a second ring, so whatever the value
//! holds afterwards is dropped on the control thread
//! ([`take_returned`](Sender::take_returned)), never on the real-time thread. The receiver
//! takes a value only after it has reserved the return slot, so it never holds a value it
//! cannot hand back.
//!
//! **The protocol** (loom-checked in `tests/loom.rs`, RT-04):
//!
//! - Values are received in the order they were sent, and come back in that order.
//! - The receiver's count of received values ([`Sender::received`]) is published with
//!   `Release` after the value is in the return ring and read with `Acquire`, so once the
//!   control thread sees the count reach `n`, the first `n` values have come back: it can tell
//!   which update the real-time thread has applied (the graph version of an edit).
//! - A full return ring makes the receiver leave the value in the ring; nothing is dropped.

use rtrb::{Consumer, Producer, RingBuffer};

use crate::queue::Full;
use crate::sync::{self, Arc, AtomicU64, Ordering};

/// A hand-off for up to `capacity` values in flight, split into its two ends. The return ring
/// holds one more, so every value in flight plus the receiver's current one can come back.
///
/// Setup: allocates, so it runs off the real-time thread before the stream starts.
#[must_use]
pub fn channel<T: Send>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let (to_receiver, from_sender) = RingBuffer::new(capacity);
    let (to_sender, from_receiver) = RingBuffer::new(capacity.saturating_add(1));
    let received = Arc::new(AtomicU64::new(0));
    (
        Sender {
            to_receiver,
            from_receiver,
            received: Arc::clone(&received),
        },
        Receiver {
            from_sender,
            to_sender,
            received,
        },
    )
}

/// The control thread's end of a [`channel`]. Not real-time.
///
/// Values still in flight when both ends are gone are dropped with the rings, on the thread
/// that drops the last end; drop the [`Receiver`] off the real-time thread (RT-09).
pub struct Sender<T> {
    to_receiver: Producer<T>,
    from_receiver: Consumer<T>,
    received: Arc<AtomicU64>,
}

impl<T> Sender<T> {
    /// Sends `value` to the real-time thread.
    ///
    /// # Errors
    ///
    /// [`Full`] with the value when the receiver has not taken the earlier ones yet.
    pub fn send(&mut self, value: T) -> Result<(), Full<T>> {
        self.to_receiver
            .push(value)
            .map_err(|rtrb::PushError::Full(value)| Full(value))
    }

    /// Takes back a value the receiver returned, so it is dropped here.
    pub fn take_returned(&mut self) -> Option<T> {
        self.from_receiver.pop().ok()
    }

    /// How many values the receiver has received and returned so far. When it reaches `n`,
    /// the first `n` values sent are waiting in [`take_returned`](Self::take_returned) or
    /// were taken already.
    #[must_use]
    pub fn received(&self) -> u64 {
        // Acquire: pairs with the Release increment in `Receiver::receive`, after the return.
        self.received.load(Ordering::Acquire)
    }
}

/// The real-time thread's end of a [`channel`].
pub struct Receiver<T> {
    from_sender: Consumer<T>,
    to_sender: Producer<T>,
    received: Arc<AtomicU64>,
}

impl<T> Receiver<T> {
    /// Takes the oldest value sent, lets `apply` change it in place, and returns it to the
    /// sender, which drops what it then holds. Returns what `apply` returned, or `None`, without
    /// calling `apply`, when no value is waiting or when the sender has not taken back enough
    /// returned values to make room. Never drops a `T`.
    ///
    /// Real-time when `apply` is: no allocation, lock or wait here.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn receive<R>(&mut self, apply: impl FnOnce(&mut T) -> R) -> Option<R> {
        if self.from_sender.is_empty() {
            return None;
        }
        // Reserve the return slot first: an unused reservation holds no value.
        let Ok(slot) = self.to_sender.write_chunk_uninit(1) else {
            return None;
        };
        sync::interleave(&self.received);
        let Ok(mut value) = self.from_sender.pop() else {
            return None;
        };
        let result = apply(&mut value);
        sync::interleave(&self.received);
        // Writes exactly one value: the reserved chunk has one slot, and `once` yields one.
        slot.fill_from_iter(std::iter::once(value));
        // Release: publishes the returned value with the count (pairs with the Acquire load in
        // `Sender::received`).
        self.received.fetch_add(1, Ordering::Release);
        Some(result)
    }

    /// Replaces `current` with the oldest value sent and returns the old `current` to the
    /// sender: [`receive`](Self::receive) with a swap. Returns false when nothing was swapped.
    /// Never drops a `T`.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn swap(&mut self, current: &mut T) -> bool {
        self.receive(|next| std::mem::swap(current, next)).is_some()
    }

    /// The oldest value sent, without taking it: the real-time thread decides from it whether
    /// to [`receive`](Self::receive) it now.
    #[must_use]
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn peek(&self) -> Option<&T> {
        self.from_sender.peek().ok()
    }

    /// Whether a sent value is waiting.
    #[must_use]
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn has_pending(&self) -> bool {
        !self.from_sender.is_empty()
    }
}
