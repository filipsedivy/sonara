//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! Fixed integer delay.

/// Fixed delay of whole frames for interleaved multichannel audio: the graph's latency
/// compensation (`DelayLine` in `audio/engine/graph.py`) and the limiter's audio path. It starts
/// with zeros. Copies only, so it is exact.
#[derive(Clone, Debug)]
pub struct DelayLine {
    channels: usize,
    delay: usize,
    /// `delay` frames, the oldest at `offset`.
    ring: Vec<f64>,
    /// Sample offset of the oldest frame in `ring`.
    offset: usize,
}

impl DelayLine {
    /// A delay of `delay` frames for `channels` interleaved channels, filled with zeros.
    ///
    /// Setup: allocates, so it runs before the stream starts, off the real-time thread.
    #[must_use]
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    pub fn new(delay: usize, channels: usize) -> Self {
        Self {
            channels,
            delay,
            ring: vec![0.0; delay.saturating_mul(channels)],
            offset: 0,
        }
    }

    /// The delay in frames.
    #[must_use]
    pub fn delay(&self) -> usize {
        self.delay
    }

    /// Number of interleaved channels.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Refills the line with zeros.
    pub fn reset(&mut self) {
        self.ring.fill(0.0);
        self.offset = 0;
    }

    /// Writes the frames of `x`, delayed, into `out`.
    pub fn process(&mut self, x: &[f64], out: &mut [f64]) {
        let channels = self.channels;
        if channels == 0 {
            return;
        }
        for (x_frame, y_frame) in x.chunks_exact(channels).zip(out.chunks_exact_mut(channels)) {
            let Some(slot) = self.oldest_frame() else {
                // No delay: a copy.
                for (y, v) in y_frame.iter_mut().zip(x_frame) {
                    *y = *v;
                }
                continue;
            };
            for ((y, v), stored) in y_frame.iter_mut().zip(x_frame).zip(slot) {
                *y = *stored;
                *stored = *v;
            }
            self.advance();
        }
    }

    /// Delays the frames of `buffer` in place.
    pub fn process_in_place(&mut self, buffer: &mut [f64]) {
        let channels = self.channels;
        if channels == 0 || self.delay == 0 {
            return;
        }
        for frame in buffer.chunks_exact_mut(channels) {
            if let Some(slot) = self.oldest_frame() {
                for (v, stored) in frame.iter_mut().zip(slot) {
                    std::mem::swap(v, stored);
                }
            }
            self.advance();
        }
    }

    fn oldest_frame(&mut self) -> Option<&mut [f64]> {
        let end = self.offset.checked_add(self.channels)?;
        self.ring.get_mut(self.offset..end)
    }

    fn advance(&mut self) {
        self.offset = self.offset.saturating_add(self.channels);
        if self.offset >= self.ring.len() {
            self.offset = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DelayLine;

    #[test]
    fn delays_by_whole_frames_across_calls() {
        let mut line = DelayLine::new(3, 2);
        let x: Vec<f64> = (1..=10).map(f64::from).collect();
        let mut out = [0.0; 10];
        let (x_head, x_tail) = x.split_at(4);
        let (out_head, out_tail) = out.split_at_mut(4);
        line.process(x_head, out_head);
        line.process(x_tail, out_tail);
        assert_eq!(out, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn in_place_equals_out_of_place() {
        let x: Vec<f64> = (0..21).map(|i| f64::from(i) * 0.5).collect();
        let mut a = DelayLine::new(4, 3);
        let mut out = x.clone();
        a.process(&x, &mut out);
        let mut b = DelayLine::new(4, 3);
        let mut buffer = x;
        b.process_in_place(&mut buffer);
        assert_eq!(buffer, out);
    }

    #[test]
    fn zero_delay_copies_and_reset_clears() {
        let mut line = DelayLine::new(0, 1);
        let mut out = [0.0; 2];
        line.process(&[1.0, 2.0], &mut out);
        assert_eq!(out, [1.0, 2.0]);
        let mut buffer = [3.0];
        line.process_in_place(&mut buffer);
        assert_eq!(buffer, [3.0]);

        let mut line = DelayLine::new(1, 1);
        line.process(&[5.0], &mut out);
        line.reset();
        line.process(&[6.0], &mut out);
        assert_eq!(out.first(), Some(&0.0));
        assert_eq!((line.delay(), line.channels()), (1, 1));
    }
}
