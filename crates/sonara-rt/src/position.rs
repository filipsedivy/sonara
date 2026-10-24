//! Timeline positions and latency estimates: two different numbers with two different types
//! (RT-07).

/// A frame on the project's output timeline: a block's tag, or the stream's software position.
///
/// The software position is the last frame handed to the device, not the frame the listener
/// hears; that one is later by the device latency, a [`LatencyFrames`] estimate (RT-07).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimelineFrame(i64);

impl TimelineFrame {
    /// Timeline frame `frame`. Tags may be negative: the player tags a block with its source
    /// frame minus the graph's latency.
    #[must_use]
    pub const fn new(frame: i64) -> Self {
        Self(frame)
    }

    /// The raw frame number.
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }

    /// The frame `frames` later; wraps on overflow (RT-05).
    #[must_use]
    pub fn advanced_by(self, frames: u32) -> Self {
        Self(self.0.wrapping_add(i64::from(frames)))
    }
}

/// A device latency estimate in frames: the backend's period × periods (the `latency_frames`
/// and `input_latency_frames` stats). It is never a position (RT-07).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LatencyFrames(u32);

impl LatencyFrames {
    /// A latency of `frames` frames.
    #[must_use]
    pub const fn new(frames: u32) -> Self {
        Self(frames)
    }

    /// The latency in frames.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}
