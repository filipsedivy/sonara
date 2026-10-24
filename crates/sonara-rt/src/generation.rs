//! Block generations: `flush` makes every block of an older generation stale.

/// Generation of a playback block. A flush to generation `g` makes every block older than `g`
/// stale; the player starts at 1 and advances on every play, seek, pause and stop.
///
/// Generations wrap on overflow (RT-05) and compare by serial-number arithmetic (RFC 1982), so
/// the order stays right across the wrap as long as live generations are less than 2³¹ apart.
/// Below 2³¹ this equals the prototype's plain `<`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Generation(u32);

impl Generation {
    /// Generation `value`.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// The raw generation number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// The following generation; wraps from `u32::MAX` to 0.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    /// Whether `self` comes before `other`: such a block is stale once the stream is flushed to
    /// `other`.
    #[must_use]
    pub const fn is_older_than(self, other: Self) -> bool {
        // Serial-number order: the wrapped distance, read as signed, is negative.
        self.0.wrapping_sub(other.0) > u32::MAX >> 1
    }
}
