//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! Second-order-section filtering: `SciPy`'s `sosfilt` with persistent state.

use std::fmt;

use realfft::num_complex::Complex64;

/// One second-order section `[b0, b1, b2, a0, a1, a2]`, a row of `SciPy`'s `sos` array. Sections
/// are normalized (`a0 == 1`); [`SosFilter`] ignores `a0`, as `sosfilt` does after validating it.
pub type Section = [f64; 6];

/// The section that passes its input through unchanged (the prototype's `IDENTITY_SECTION`).
pub const IDENTITY_SECTION: Section = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// Error of [`SosFilter::set_sections`]: the design has more sections than the filter was
/// prepared for. The filter is left unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectionCapacityError {
    /// Sections in the rejected design.
    pub requested: usize,
    /// Sections the filter holds at most.
    pub capacity: usize,
}

impl fmt::Display for SectionCapacityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} sections exceed the filter's capacity of {}",
            self.requested, self.capacity
        )
    }
}

impl std::error::Error for SectionCapacityError {}

/// Multichannel cascade of second-order sections with persistent state: the prototype's
/// `SosFilter` (`dsp/filters/biquad.py`), which runs `SciPy`'s `sosfilt(sos, x, axis=0, zi)`.
///
/// Processing is bit-exact with `SciPy` on the recording host.
/// Per channel, sample and section, with state `z0`, `z1`:
///
/// ```text
/// y  = fma(b0, x, z0)
/// z0 = fma(b1, x, -(a1 * y)) + z1
/// z1 = fma(b2, x, -(a2 * y))
/// ```
///
/// The state follows the prototype's rules:
///
/// - A new filter is an identity with one stored identity section and zero state.
/// - [`set_sections`](Self::set_sections) with a different section count than the stored design
///   zeroes the state; with the same count it keeps the state (a click-free coefficient swap).
/// - Switching to identity keeps the stored design and its now stale state; a later design with
///   the same count continues from that state.
/// - [`reset`](Self::reset) zeroes the state.
#[derive(Clone, Debug)]
pub struct SosFilter {
    channels: usize,
    /// Sections of the stored design (the prototype's `_sos.shape[0]`).
    count: usize,
    identity: bool,
    /// Preallocated storage; the first `count` entries are the design.
    sections: Vec<Section>,
    /// `[z0, z1]` per channel and section, laid out `[channel][section]` with the full capacity
    /// per channel.
    state: Vec<[f64; 2]>,
}

impl SosFilter {
    /// A filter for `channels` interleaved channels that holds designs of up to `max_sections`
    /// sections (at least one). It starts as an identity.
    ///
    /// Setup: allocates, so it runs before the stream starts, off the real-time thread.
    #[must_use]
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    pub fn new(channels: usize, max_sections: usize) -> Self {
        let capacity = max_sections.max(1);
        Self {
            channels,
            count: 1,
            identity: true,
            sections: vec![IDENTITY_SECTION; capacity],
            state: vec![[0.0; 2]; channels.saturating_mul(capacity)],
        }
    }

    /// Number of interleaved channels.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// The most sections a design may have.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.sections.len()
    }

    /// Whether the filter passes its input through unchanged.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.identity
    }

    /// The stored design. It stays stored while the filter is an identity.
    #[must_use]
    pub fn sections(&self) -> &[Section] {
        self.sections.get(..self.count).unwrap_or_default()
    }

    /// The state `[z0, z1]` of every section of the stored design for `channel` (`SciPy`'s
    /// `zi[:, :, channel]`), or `None` for a channel the filter does not have.
    #[must_use]
    pub fn state(&self, channel: usize) -> Option<&[[f64; 2]]> {
        self.state
            .chunks_exact(self.sections.len())
            .nth(channel)?
            .get(..self.count)
    }

    /// Uses `sections` from the next sample on (the prototype's `set_sos`). An empty slice
    /// switches to identity, like `set_sos(None)`.
    ///
    /// Allocation-free: the design is copied into the storage prepared by [`new`](Self::new).
    ///
    /// # Errors
    ///
    /// [`SectionCapacityError`] when `sections` is longer than [`capacity`](Self::capacity);
    /// the filter is then left unchanged.
    pub fn set_sections(&mut self, sections: &[Section]) -> Result<(), SectionCapacityError> {
        if sections.is_empty() {
            self.identity = true;
            return Ok(());
        }
        if sections.len() > self.sections.len() {
            return Err(SectionCapacityError {
                requested: sections.len(),
                capacity: self.sections.len(),
            });
        }
        if sections.len() != self.count {
            self.state.fill([0.0; 2]);
        }
        for (stored, section) in self.sections.iter_mut().zip(sections) {
            *stored = *section;
        }
        self.count = sections.len();
        self.identity = false;
        Ok(())
    }

    /// Switches to identity and keeps the stored design and state (`set_sos(None)`).
    pub fn set_identity(&mut self) {
        self.identity = true;
    }

    /// Zeroes the state (the prototype's `reset`).
    pub fn reset(&mut self) {
        self.state.fill([0.0; 2]);
    }

    /// Filters the interleaved frames of `x` into `out`. An identity copies them.
    pub fn process(&mut self, x: &[f64], out: &mut [f64]) {
        let channels = self.channels;
        if channels == 0 {
            return;
        }
        let frames = x.chunks_exact(channels).zip(out.chunks_exact_mut(channels));
        if self.identity {
            for (x_frame, y_frame) in frames {
                for (y, v) in y_frame.iter_mut().zip(x_frame) {
                    *y = *v;
                }
            }
            return;
        }
        let capacity = self.sections.len();
        let sections = self.sections.get(..self.count).unwrap_or_default();
        for (x_frame, y_frame) in frames {
            let channel_states = self.state.chunks_exact_mut(capacity);
            for ((y, v), state) in y_frame.iter_mut().zip(x_frame).zip(channel_states) {
                *y = filter_sample(sections, state, *v);
            }
        }
    }

    /// Filters the interleaved frames of `buffer` in place. An identity leaves them unchanged.
    pub fn process_in_place(&mut self, buffer: &mut [f64]) {
        let channels = self.channels;
        if channels == 0 || self.identity {
            return;
        }
        let capacity = self.sections.len();
        let sections = self.sections.get(..self.count).unwrap_or_default();
        for frame in buffer.chunks_exact_mut(channels) {
            let channel_states = self.state.chunks_exact_mut(capacity);
            for (v, state) in frame.iter_mut().zip(channel_states) {
                *v = filter_sample(sections, state, *v);
            }
        }
    }
}

/// One sample of one channel through the cascade, in `SciPy`'s contracted form.
#[inline]
fn filter_sample(sections: &[Section], state: &mut [[f64; 2]], x: f64) -> f64 {
    let mut v = x;
    for (&[b0, b1, b2, _, a1, a2], z) in sections.iter().zip(state.iter_mut()) {
        let [z0, z1] = *z;
        let y = b0.mul_add(v, z0);
        *z = [b1.mul_add(v, -(a1 * y)) + z1, b2.mul_add(v, -(a2 * y))];
        v = y;
    }
    v
}

/// The magnitude response of the cascade `sections` at `freq` Hz, in dB: `SciPy`'s
/// `sosfreqz(sos, worN=[freq], fs=sample_rate)` as the prototype's `response_db` reads it,
/// `20 · log10(max(|H|, 1e-12))`.
///
/// Each section is evaluated as `freqz` does, `(b0 + b1·z⁻¹ + b2·z⁻²) / (a0 + a1·z⁻¹ + a2·z⁻²)`
/// at `z⁻¹ = e^(−jω)` with `ω = 2π · freq / sample_rate` (Horner's scheme), and the sections'
/// responses are multiplied. An empty cascade is 0 dB.
///
/// # Examples
///
/// ```
/// use sonara_dsp::filters::{IDENTITY_SECTION, response_db};
///
/// assert_eq!(response_db(&[IDENTITY_SECTION; 3], 1000.0, 48_000.0), 0.0);
/// // A pure gain of 0.5 is about −6.02 dB at every frequency.
/// let half = [0.5, 0.0, 0.0, 1.0, 0.0, 0.0];
/// assert!((response_db(&[half], 440.0, 48_000.0) + 6.0206).abs() < 1e-4);
/// ```
#[must_use]
pub fn response_db(sections: &[Section], freq: f64, sample_rate: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * freq / sample_rate;
    let zm1 = Complex64::new(0.0, -w).exp();
    let h = sections
        .iter()
        .fold(Complex64::new(1.0, 0.0), |h, &[b0, b1, b2, a0, a1, a2]| {
            let num = (zm1 * b2 + b1) * zm1 + b0;
            let den = (zm1 * a2 + a1) * zm1 + a0;
            h * (num / den)
        });
    20.0 * h.norm().max(1e-12).log10()
}

#[cfg(test)]
mod tests {
    use super::{IDENTITY_SECTION, Section, SectionCapacityError, SosFilter, response_db};

    const A: Section = [0.2, 0.4, 0.2, 1.0, -0.5, 0.25];
    const B: Section = [0.3, 0.1, 0.3, 1.0, -0.2, 0.1];

    fn run(filter: &mut SosFilter, x: &[f64]) -> Vec<f64> {
        let mut out = x.to_vec();
        filter.process(x, &mut out);
        out
    }

    #[test]
    fn first_sample_is_b0_times_x() {
        let mut f = SosFilter::new(1, 1);
        f.set_sections(&[A]).unwrap();
        assert_eq!(run(&mut f, &[2.0]), [0.4]);
        // z0 = b1·x − a1·y = 0.8 + 0.2, z1 = b2·x − a2·y = 0.4 − 0.1.
        assert_eq!(f.state(0), Some(&[[1.0, 0.300_000_000_000_000_04]][..]));
    }

    #[test]
    fn starts_as_identity_and_copies() {
        let mut f = SosFilter::new(2, 4);
        assert!(f.is_identity());
        assert_eq!(f.sections(), [IDENTITY_SECTION]);
        assert_eq!(run(&mut f, &[1.0, -2.0, 3.0, 4.0]), [1.0, -2.0, 3.0, 4.0]);
        let mut buffer = [5.0, 6.0];
        f.process_in_place(&mut buffer);
        assert_eq!(buffer, [5.0, 6.0]);
    }

    #[test]
    fn same_count_keeps_and_other_count_zeroes_the_state() {
        let mut f = SosFilter::new(1, 2);
        f.set_sections(&[A]).unwrap();
        run(&mut f, &[1.0, 0.5]);
        let before = f.state(0).unwrap().to_vec();
        assert_ne!(before, [[0.0; 2]]);
        f.set_sections(&[B]).unwrap();
        assert_eq!(f.state(0).unwrap(), before, "same count keeps the state");
        f.set_sections(&[A, B]).unwrap();
        assert_eq!(f.state(0).unwrap(), [[0.0; 2]; 2], "a new count zeroes it");
    }

    #[test]
    fn identity_keeps_the_stale_state() {
        let mut f = SosFilter::new(1, 1);
        f.set_sections(&[A]).unwrap();
        run(&mut f, &[1.0]);
        let stale = f.state(0).unwrap().to_vec();
        f.set_identity();
        assert_eq!(run(&mut f, &[7.0]), [7.0]);
        assert_eq!(f.state(0).unwrap(), stale);
        f.set_sections(&[]).unwrap();
        assert!(f.is_identity());
        f.set_sections(&[B]).unwrap();
        assert_eq!(f.state(0).unwrap(), stale, "same count continues from it");
    }

    #[test]
    fn too_many_sections_are_rejected_without_change() {
        let mut f = SosFilter::new(1, 1);
        f.set_sections(&[A]).unwrap();
        assert_eq!(
            f.set_sections(&[A, B]),
            Err(SectionCapacityError {
                requested: 2,
                capacity: 1
            })
        );
        assert_eq!(f.sections(), [A]);
        assert!(!f.is_identity());
    }

    #[test]
    fn in_place_equals_out_of_place_and_channels_are_independent() {
        let x: Vec<f64> = (0..64).map(|i| (f64::from(i) * 0.37).sin()).collect();
        let mut out_of_place = SosFilter::new(2, 2);
        out_of_place.set_sections(&[A, B]).unwrap();
        let expected = run(&mut out_of_place, &x);

        let mut in_place = SosFilter::new(2, 2);
        in_place.set_sections(&[A, B]).unwrap();
        let mut buffer = x.clone();
        in_place.process_in_place(&mut buffer);
        assert_eq!(buffer, expected);

        let left: Vec<f64> = x.iter().step_by(2).copied().collect();
        let mut mono = SosFilter::new(1, 2);
        mono.set_sections(&[A, B]).unwrap();
        let mono_out = run(&mut mono, &left);
        let expected_left: Vec<f64> = expected.iter().step_by(2).copied().collect();
        assert_eq!(mono_out, expected_left);
    }

    #[test]
    fn reset_zeroes_and_odd_lengths_are_safe() {
        let mut f = SosFilter::new(2, 1);
        f.set_sections(&[A]).unwrap();
        let mut out = [9.0; 3];
        f.process(&[1.0, 1.0, 1.0], &mut out);
        assert_eq!(
            out.get(2),
            Some(&9.0),
            "a trailing partial frame is left untouched"
        );
        f.reset();
        assert_eq!(f.state(1), Some(&[[0.0; 2]][..]));
        assert_eq!(f.state(2), None);
        let mut nothing = SosFilter::new(0, 1);
        nothing.process(&[1.0], &mut out);
    }

    /// Cascading multiplies the responses, so the dB values add; at DC a section's response is
    /// `sum(b) / sum(a)`, and at Nyquist the alternating sum.
    #[test]
    fn the_response_of_a_cascade_is_the_sum_of_its_sections_in_db() {
        let sr = 48_000.0;
        for freq in [20.0, 440.0, 5_000.0, 19_999.0] {
            let both = response_db(&[A, B], freq, sr);
            let sum = response_db(&[A], freq, sr) + response_db(&[B], freq, sr);
            assert!((both - sum).abs() < 1e-9, "{freq} Hz: {both} vs {sum}");
        }
        let dc = |[b0, b1, b2, a0, a1, a2]: Section| (b0 + b1 + b2) / (a0 + a1 + a2);
        let expected = 20.0 * dc(A).abs().log10();
        assert!((response_db(&[A], 0.0, sr) - expected).abs() < 1e-12);
        let nyquist = |[b0, b1, b2, a0, a1, a2]: Section| (b0 - b1 + b2) / (a0 - a1 + a2);
        let expected = 20.0 * nyquist(B).abs().log10();
        assert!((response_db(&[B], sr / 2.0, sr) - expected).abs() < 1e-9);
        // A zero response is floored at 1e-12, -240 dB.
        let null = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        assert_eq!(response_db(&[null], 100.0, sr), -240.0);
    }
}
