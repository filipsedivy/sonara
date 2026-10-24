//! Not real-time: the Analyzer's stereo measurements (see the parent module).

/// Dot product in the order of the recording host's BLAS `ddot` (Accelerate, arm64) for strided
/// vectors, which is what NumPy calls for the columns `x[:, 0]` and `x[:, 1]`: four
/// accumulators, element `i` into accumulator `i % 4` with a fused multiply-add (the tail
/// included), combined as `(a0 + a2) + (a1 + a3)`. Verified bit-exact on every length from 1 to
/// 65536 with stride 2, independent of alignment. The
/// slices have equal lengths; extra elements of the longer one are ignored.
///
/// NumPy's contiguous `ddot` (the prototype's `mid` and `side` vectors) depends on the
/// buffer's alignment on that host, so no order reproduces it; this one is used there too.
pub(super) fn dot(x: &[f64], y: &[f64]) -> f64 {
    let mut acc = [0.0_f64; 4];
    let ((x4, x_tail), (y4, y_tail)) = (x.as_chunks::<4>(), y.as_chunks::<4>());
    for (a, b) in x4.iter().zip(y4) {
        for ((s, &u), &v) in acc.iter_mut().zip(a).zip(b) {
            *s = u.mul_add(v, *s);
        }
    }
    if x4.len() == y4.len() {
        for ((s, &u), &v) in acc.iter_mut().zip(x_tail).zip(y_tail) {
            *s = u.mul_add(v, *s);
        }
    }
    let [a0, a1, a2, a3] = acc;
    (a0 + a2) + (a1 + a3)
}

/// The prototype's stereo accumulators: the energies and the correlation over the whole signal
/// and over non-overlapping 400 ms windows.
#[derive(Clone, Debug)]
pub(super) struct StereoMeter {
    pub(super) lr: f64,
    pub(super) ll: f64,
    pub(super) rr: f64,
    pub(super) mid: f64,
    pub(super) side: f64,
    /// Correlation per complete 400 ms window with energy in both channels.
    pub(super) windows: Vec<f64>,
    /// `lr`, `ll`, `rr` of the incomplete window.
    window: [f64; 3],
    window_frames: usize,
    /// Frames per window (`round(0.4 · sr)`).
    window_len: usize,
    /// The block's channels 0 and 1, `(l + r) / 2` and `(l − r) / 2`.
    left: Vec<f64>,
    right: Vec<f64>,
    mid_signal: Vec<f64>,
    side_signal: Vec<f64>,
}

impl StereoMeter {
    pub(super) fn new(window_len: usize) -> Self {
        Self {
            lr: 0.0,
            ll: 0.0,
            rr: 0.0,
            mid: 0.0,
            side: 0.0,
            windows: Vec::new(),
            window: [0.0; 3],
            window_frames: 0,
            window_len: window_len.max(1),
            left: Vec::new(),
            right: Vec::new(),
            mid_signal: Vec::new(),
            side_signal: Vec::new(),
        }
    }

    /// Accumulates one interleaved block (channels 0 and 1 of at least two).
    #[expect(
        clippy::disallowed_methods,
        reason = "not real-time: appends to the analysis accumulator"
    )]
    pub(super) fn feed(&mut self, x: &[f64], channels: usize) {
        let frames = x.chunks_exact(channels.max(2));
        self.left.clear();
        self.right.clear();
        self.left
            .extend(frames.clone().filter_map(|f| f.first().copied()));
        self.right.extend(frames.filter_map(|f| f.get(1).copied()));
        let (left, right) = (self.left.as_slice(), self.right.as_slice());
        self.lr += dot(left, right);
        self.ll += dot(left, left);
        self.rr += dot(right, right);
        self.mid_signal.clear();
        self.side_signal.clear();
        let pairs = left.iter().zip(right);
        self.mid_signal
            .extend(pairs.clone().map(|(l, r)| (l + r) * 0.5));
        self.side_signal.extend(pairs.map(|(l, r)| (l - r) * 0.5));
        self.mid += dot(&self.mid_signal, &self.mid_signal);
        self.side += dot(&self.side_signal, &self.side_signal);

        // The prototype's `_windowed_correlation`: one dot product per piece of a window.
        let mut rest = (left, right);
        while !rest.0.is_empty() {
            let take = (self.window_len - self.window_frames).min(rest.0.len());
            let (l, later_l) = rest.0.split_at_checked(take).unwrap_or((rest.0, &[]));
            let (r, later_r) = rest.1.split_at_checked(take).unwrap_or((rest.1, &[]));
            rest = (later_l, later_r);
            let [lr, ll, rr] = &mut self.window;
            *lr += dot(l, r);
            *ll += dot(l, l);
            *rr += dot(r, r);
            self.window_frames += take;
            if self.window_frames == self.window_len {
                let [lr, ll, rr] = self.window;
                if ll > 1e-9 && rr > 1e-9 {
                    self.windows.push(lr / (ll * rr).sqrt());
                }
                self.window = [0.0; 3];
                self.window_frames = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{StereoMeter, dot};

    #[test]
    fn dot_uses_four_fused_accumulators() {
        let x = [1.0, 1e16, 1.0, -1e16, 0.5];
        let y = [1.0; 5];
        // Accumulators [1 + 0.5, 1e16, 1, -1e16]: (1.5 + 1) + (1e16 - 1e16). A sequential sum
        // loses the ones in 1e16 and gives 0.5.
        assert_eq!(dot(&x, &y), 2.5);
        assert_eq!(dot(&[], &[]), 0.0);
    }

    #[test]
    fn identical_channels_correlate_fully() {
        let x: Vec<f64> = (0..2000)
            .flat_map(|i| {
                let v = (f64::from(i) * 0.1).sin();
                [v, v]
            })
            .collect();
        let mut meter = StereoMeter::new(300);
        for block in x.chunks(2 * 333) {
            meter.feed(block, 2);
        }
        assert_eq!(meter.lr, meter.ll);
        assert_eq!(meter.ll, meter.rr);
        assert_eq!(meter.side, 0.0);
        assert_eq!(meter.windows.len(), 6);
        assert!(
            meter
                .windows
                .iter()
                .all(|c| c.to_bits() == 1.0_f64.to_bits())
        );
    }
}
