//! Preallocated single-producer, single-consumer queues of `Copy` values between a real-time
//! thread and another thread: player commands, [`MidiEvent`](crate::MidiEvent)s, meter frames.
//!
//! `T: Copy` means `T` has no destructor, so pushing, popping or leaving values behind never
//! runs one on a real-time thread (RT-02, RT-09). The queue is allocated once by [`channel`];
//! a full queue rejects the value instead of growing.

use std::fmt;

use rtrb::{Consumer, Producer, RingBuffer};

/// A queue holding up to `capacity` values, split into its two ends.
///
/// Setup: allocates, so it runs off the real-time thread before the stream starts. A capacity
/// of 0 makes a queue that is always full.
#[must_use]
pub fn channel<T: Copy + Send>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let (producer, consumer) = RingBuffer::new(capacity);
    (Sender { inner: producer }, Receiver { inner: consumer })
}

/// A value that did not fit: the queue was full. The caller counts it (RT-08).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Full<T>(pub T);

impl<T> fmt::Display for Full<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the queue is full")
    }
}

impl<T: fmt::Debug> std::error::Error for Full<T> {}

/// The sending end of a [`channel`].
pub struct Sender<T> {
    inner: Producer<T>,
}

impl<T: Copy> Sender<T> {
    /// Appends `value`.
    ///
    /// # Errors
    ///
    /// [`Full`] with the value when the queue is full.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn push(&mut self, value: T) -> Result<(), Full<T>> {
        self.inner
            .push(value)
            .map_err(|rtrb::PushError::Full(value)| Full(value))
    }

    /// Values that can be pushed now; more may become free at any time.
    #[must_use]
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn free_slots(&self) -> usize {
        self.inner.slots()
    }
}

/// The receiving end of a [`channel`].
pub struct Receiver<T> {
    inner: Consumer<T>,
}

impl<T: Copy> Receiver<T> {
    /// Takes the oldest value, if any.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn pop(&mut self) -> Option<T> {
        self.inner.pop().ok()
    }

    /// The oldest value, without taking it.
    #[must_use]
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn peek(&self) -> Option<T> {
        self.inner.peek().ok().copied()
    }

    /// Values waiting; more may arrive at any time.
    #[must_use]
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn len(&self) -> usize {
        self.inner.slots()
    }

    /// Whether no value is waiting.
    #[must_use]
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}
