//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! Sliding minimum over a backward window.

/// Sliding minimum over the current sample and the `lookahead` samples before it: the limiter's
/// gain hold, `minimum_filter1d(concat(history, x), size=la + 1, origin=la // 2,
/// mode='nearest')[la:]` with the last `la` inputs carried as history
/// (`dsp/dynamics/limiter.py`).
///
/// A monotonic deque of the window's candidates gives the same values as the prototype: a
/// minimum is one of its inputs, so no arithmetic is involved. The inputs are finite (the
/// limiter's required gain); a NaN input is not ordered like `ndimage` orders it.
#[derive(Clone, Debug)]
pub struct BackwardMinimum {
    lookahead: usize,
    /// Ring of `(value, position)`, increasing in both from `head`; capacity `lookahead + 1`.
    deque: Vec<(f64, u64)>,
    head: usize,
    len: usize,
    /// Position of the next input; wraps.
    next: u64,
}

impl BackwardMinimum {
    /// A window over `lookahead + 1` samples whose history is `lookahead` samples of `initial`
    /// (the limiter starts from `np.ones(la)`).
    ///
    /// Setup: allocates, so it runs before the stream starts, off the real-time thread.
    #[must_use]
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    pub fn new(lookahead: usize, initial: f64) -> Self {
        let mut filter = Self {
            lookahead,
            deque: vec![(0.0, 0); lookahead.saturating_add(1)],
            head: 0,
            len: 0,
            next: 0,
        };
        filter.reset(initial);
        filter
    }

    /// The number of past samples in the window.
    #[must_use]
    pub fn lookahead(&self) -> usize {
        self.lookahead
    }

    /// Replaces the history with `lookahead` samples of `initial`.
    pub fn reset(&mut self, initial: f64) {
        self.head = 0;
        self.len = 0;
        self.next = 0;
        for _ in 0..self.lookahead {
            self.push(initial);
        }
    }

    /// Writes, for every sample of `x`, the minimum of it and the `lookahead` samples before it
    /// into `out`.
    pub fn process(&mut self, x: &[f64], out: &mut [f64]) {
        for (y, &v) in out.iter_mut().zip(x) {
            *y = self.push(v);
        }
    }

    /// Adds `value` and returns the minimum of the window that ends with it.
    fn push(&mut self, value: f64) -> f64 {
        let position = self.next;
        self.next = position.wrapping_add(1);
        let window = u64::try_from(self.lookahead).unwrap_or(u64::MAX);
        // Drop candidates that leave the window, then those the new value dominates; at most
        // `lookahead` remain, so the ring has room for the new one.
        while let Some(&(_, front)) = self.deque.get(self.head).filter(|_| self.len > 0) {
            if position.wrapping_sub(front) <= window {
                break;
            }
            self.head = self.wrap(self.head + 1);
            self.len -= 1;
        }
        while self.len > 0 {
            let back = self.wrap(self.head + self.len - 1);
            match self.deque.get(back) {
                Some(&(candidate, _)) if candidate >= value => self.len -= 1,
                _ => break,
            }
        }
        let slot = self.wrap(self.head + self.len);
        if let Some(entry) = self.deque.get_mut(slot) {
            *entry = (value, position);
            self.len += 1;
        }
        self.deque
            .get(self.head)
            .map_or(value, |&(minimum, _)| minimum)
    }

    /// `index` reduced into the ring; `index < 2 · capacity`.
    fn wrap(&self, index: usize) -> usize {
        if index >= self.deque.len() {
            index - self.deque.len()
        } else {
            index
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BackwardMinimum;

    /// The window minimum computed directly over `history ++ x`.
    fn reference(lookahead: usize, initial: f64, x: &[f64]) -> Vec<f64> {
        let extended: Vec<f64> = std::iter::repeat_n(initial, lookahead)
            .chain(x.iter().copied())
            .collect();
        extended
            .windows(lookahead + 1)
            .map(|w| w.iter().copied().fold(f64::INFINITY, f64::min))
            .collect()
    }

    fn signal(n: u32) -> Vec<f64> {
        (0..n)
            .map(|i| (f64::from(i) * 0.731).sin().abs() + (f64::from(i) * 0.05).cos() * 0.1)
            .collect()
    }

    #[test]
    fn equals_the_direct_minimum_across_calls() {
        let x = signal(300);
        for lookahead in [0, 1, 2, 5, 17, 96] {
            let mut filter = BackwardMinimum::new(lookahead, 1.0);
            let mut out = x.clone();
            let (x_head, x_tail) = x.split_at(41);
            let (out_head, out_tail) = out.split_at_mut(41);
            filter.process(x_head, out_head);
            filter.process(x_tail, out_tail);
            assert_eq!(out, reference(lookahead, 1.0, &x), "lookahead {lookahead}");
            assert_eq!(filter.lookahead(), lookahead);
        }
    }

    #[test]
    fn equal_values_and_reset() {
        let mut filter = BackwardMinimum::new(2, 0.5);
        let mut out = [0.0; 5];
        filter.process(&[0.7, 0.5, 0.5, 0.9, 0.9], &mut out);
        assert_eq!(out, [0.5, 0.5, 0.5, 0.5, 0.5]);
        filter.process(&[0.9], &mut out);
        assert_eq!(out.first(), Some(&0.9));
        filter.reset(0.25);
        filter.process(&[1.0, 1.0, 1.0], &mut out);
        assert_eq!(out.get(..3), Some(&[0.25, 0.25, 1.0][..]));
    }
}
