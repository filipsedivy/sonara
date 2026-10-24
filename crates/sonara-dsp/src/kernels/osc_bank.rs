//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! Band-limited oscillator bank for one synth voice.

use std::f64::consts::TAU;

/// The most oscillators [`osc_bank`] processes; further entries are ignored.
pub const MAX_OSCILLATORS: usize = 64;

/// Highest phase increment per sample (cycles), just below Nyquist.
const MAX_PHASE_INCREMENT: f64 = 0.49;

/// Waveform of one oscillator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wave {
    /// `PolyBLEP` sawtooth.
    Saw,
    /// `PolyBLEP` square (50 % duty).
    Square,
    /// `PolyBLEP` pulse with the bank's pulse width.
    Pulse,
    /// Triangle (not band-limited).
    Triangle,
    /// Sine.
    Sine,
}

impl Wave {
    /// Maps the prototype's wave code: 0 saw, 1 square, 2 pulse, 3 triangle, and every other
    /// value selects `Sine`, as the native kernel's `default` branch does.
    #[must_use]
    pub fn from_raw(code: i32) -> Self {
        match code {
            0 => Self::Saw,
            1 => Self::Square,
            2 => Self::Pulse,
            3 => Self::Triangle,
            _ => Self::Sine,
        }
    }
}

/// One oscillator of the bank: its settings and its running phase.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oscillator {
    /// Waveform.
    pub wave: Wave,
    /// Output level. An oscillator with a level of zero or less is skipped, and its phase does
    /// not advance.
    pub level: f64,
    /// Pitch offset from the voice pitch, in semitones.
    pub pitch_offset: f64,
    /// Left gain (constant-power pan).
    pub pan_l: f64,
    /// Right gain (constant-power pan).
    pub pan_r: f64,
    /// Phase in cycles, `[0, 1)`; carried across calls and updated in place.
    pub phase: f64,
}

/// `PolyBLEP` residual for a discontinuity at phase 0 with phase increment `dt`, in the oracle's
/// contracted form: `x + x - x*x - 1` and `x*x + x + x + 1` fuse their product.
#[inline]
fn polyblep(t: f64, dt: f64) -> f64 {
    if t < dt {
        let x = t / dt;
        return (-x).mul_add(x, x + x) - 1.0;
    }
    if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        return x.mul_add(x, x) + x + 1.0;
    }
    0.0
}

/// Renders a voice's band-limited oscillators into `out_l` and `out_r`, **accumulating** (`+=`).
///
/// - `pitch` is the per-sample MIDI pitch in semitones (including bend, vibrato and glide).
///   A4 = MIDI 69 = 440 Hz. The phase increment is limited to 0.49 cycles per sample.
/// - `pulse_width` is the duty of [`Wave::Pulse`] oscillators, in `(0, 1)`.
/// - Each oscillator's phase is updated in place, so consecutive calls continue the same
///   signal.
/// - Only the first [`MAX_OSCILLATORS`] oscillators are processed.
///
/// The samples rendered are the common length of `pitch`, `out_l` and `out_r` (see the
/// [module docs](super)).
///
/// # Examples
///
/// ```
/// use sonara_dsp::kernels::{Oscillator, Wave, osc_bank};
///
/// let mut oscillators = [Oscillator {
///     wave: Wave::Sine,
///     level: 0.5,
///     pitch_offset: 0.0,
///     pan_l: 1.0,
///     pan_r: 0.0,
///     phase: 0.0,
/// }];
/// let (mut left, mut right) = ([0.0; 48], [0.0; 48]);
/// osc_bank(&[69.0; 48], &mut oscillators, 0.5, 48_000.0, &mut left, &mut right);
/// // 440 Hz at 48 kHz: the phase advances by 440 / 48000 cycles per sample.
/// assert!((left[0] - 0.5 * (std::f64::consts::TAU * 440.0 / 48_000.0).sin()).abs() < 1e-12);
/// assert!(right.iter().all(|r| *r == 0.0));
/// ```
pub fn osc_bank(
    pitch: &[f64],
    oscillators: &mut [Oscillator],
    pulse_width: f64,
    sample_rate: f64,
    out_l: &mut [f64],
    out_r: &mut [f64],
) {
    // Frequency multiplier of every oscillator's offset, hoisted out of the sample loop.
    let mut mult = [0.0_f64; MAX_OSCILLATORS];
    for (m, osc) in mult.iter_mut().zip(oscillators.iter()) {
        *m = (osc.pitch_offset / 12.0).exp2();
    }
    for ((&p, l_out), r_out) in pitch.iter().zip(out_l.iter_mut()).zip(out_r.iter_mut()) {
        let base = 440.0 * ((p - 69.0) / 12.0).exp2() / sample_rate;
        let (mut l, mut r) = (0.0, 0.0);
        for (osc, &m) in oscillators.iter_mut().zip(mult.iter()) {
            if osc.level <= 0.0 {
                continue;
            }
            // `if (dt > 0.49) dt = 0.49`, as the oracle writes it (a NaN stays NaN).
            let dt = base * m;
            let dt = if dt > MAX_PHASE_INCREMENT {
                MAX_PHASE_INCREMENT
            } else {
                dt
            };
            let mut t = osc.phase + dt;
            t -= t.floor();
            osc.phase = t;
            let v = match osc.wave {
                // `2·t` and `4·|t − 0.5|` are exact, so the oracle's fused forms equal these.
                Wave::Saw => 2.0 * t - 1.0 - polyblep(t, dt),
                Wave::Square | Wave::Pulse => {
                    let pw = if osc.wave == Wave::Square {
                        0.5
                    } else {
                        pulse_width
                    };
                    let mut t2 = t + 1.0 - pw;
                    t2 -= t2.floor();
                    (if t < pw { 1.0 } else { -1.0 }) + polyblep(t, dt) - polyblep(t2, dt)
                }
                Wave::Triangle => 4.0 * (t - 0.5).abs() - 1.0,
                Wave::Sine => (TAU * t).sin(),
            };
            let v = v * osc.level;
            // F8: the oracle fuses the pan products into the sums.
            l = v.mul_add(osc.pan_l, l);
            r = v.mul_add(osc.pan_r, r);
        }
        *l_out += l;
        *r_out += r;
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_OSCILLATORS, Oscillator, Wave, osc_bank};

    const SR: f64 = 48_000.0;

    fn osc(wave: Wave) -> Oscillator {
        Oscillator {
            wave,
            level: 1.0,
            pitch_offset: 0.0,
            pan_l: 1.0,
            pan_r: 1.0,
            phase: 0.0,
        }
    }

    fn render<const N: usize>(
        oscs: &mut [Oscillator],
        pitch: [f64; N],
        pw: f64,
    ) -> ([f64; N], [f64; N]) {
        let (mut l, mut r) = ([0.0; N], [0.0; N]);
        osc_bank(&pitch, oscs, pw, SR, &mut l, &mut r);
        (l, r)
    }

    #[test]
    fn unknown_wave_codes_select_sine() {
        assert_eq!(Wave::from_raw(0), Wave::Saw);
        assert_eq!(Wave::from_raw(1), Wave::Square);
        assert_eq!(Wave::from_raw(2), Wave::Pulse);
        assert_eq!(Wave::from_raw(3), Wave::Triangle);
        for code in [4, 5, 99, -1, i32::MIN] {
            assert_eq!(Wave::from_raw(code), Wave::Sine);
        }
    }

    #[test]
    fn sine_follows_the_phase() {
        let mut oscs = [osc(Wave::Sine)];
        let (l, _) = render(&mut oscs, [69.0; 4], 0.5);
        let dt = 440.0 / SR;
        for (n, v) in (1..).zip(&l) {
            let expected = (std::f64::consts::TAU * dt * f64::from(n)).sin();
            assert!((v - expected).abs() < 1e-12, "{n}: {v} vs {expected}");
        }
    }

    #[test]
    fn triangle_spans_minus_one_to_one() {
        let mut oscs = [osc(Wave::Triangle)];
        let (l, _) = render(&mut oscs, [69.0; 400], 0.5);
        let (lo, hi) = l
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
        assert!((-1.0..-0.97).contains(&lo), "{lo}");
        assert!((0.97..=1.0).contains(&hi), "{hi}");
    }

    #[test]
    fn output_accumulates_and_pans() {
        let voice = Oscillator {
            level: 0.5,
            pan_l: 0.25,
            pan_r: 0.75,
            ..osc(Wave::Saw)
        };
        let (l0, r0) = render(&mut [voice], [60.0; 8], 0.5);

        let (mut l, mut r) = ([1.0; 8], [-1.0; 8]);
        osc_bank(&[60.0; 8], &mut [voice], 0.5, SR, &mut l, &mut r);
        for ((a, b), (c, d)) in l.iter().zip(&r).zip(l0.iter().zip(&r0)) {
            assert!((a - (1.0 + c)).abs() < 1e-15);
            assert!((b - (-1.0 + d)).abs() < 1e-15);
        }
        // The pan gains keep the ratio of the two channels.
        assert!(l0.iter().zip(&r0).all(|(a, b)| (a * 3.0 - b).abs() < 1e-15));
    }

    #[test]
    fn silent_oscillator_neither_sounds_nor_advances() {
        let mut oscs = [Oscillator {
            level: 0.0,
            phase: 0.3,
            ..osc(Wave::Saw)
        }];
        let (l, r) = render(&mut oscs, [69.0; 16], 0.5);
        assert!(l.iter().chain(&r).all(|v| *v == 0.0));
        assert_eq!(oscs.first().map(|o| o.phase), Some(0.3));
    }

    #[test]
    fn phase_increment_is_limited_below_nyquist() {
        // MIDI 140 is far above Nyquist; the increment is capped at 0.49 cycles per sample.
        let mut oscs = [osc(Wave::Sine)];
        render(&mut oscs, [140.0], 0.5);
        assert_eq!(oscs.first().map(|o| o.phase), Some(0.49));
    }

    #[test]
    fn only_the_first_64_oscillators_play() {
        let mut oscs = [osc(Wave::Sine); MAX_OSCILLATORS + 1];
        render(&mut oscs, [69.0], 0.5);
        let moved = oscs.iter().filter(|o| o.phase != 0.0).count();
        assert_eq!(moved, MAX_OSCILLATORS);
        assert_eq!(oscs.last().map(|o| o.phase), Some(0.0));
    }

    #[test]
    fn shorter_slice_sets_the_length() {
        let mut oscs = [osc(Wave::Sine)];
        let (mut l, mut r) = ([0.0; 4], [0.0; 2]);
        osc_bank(&[69.0; 4], &mut oscs, 0.5, SR, &mut l, &mut r);
        assert!(r.iter().all(|v| *v != 0.0));
        assert!(l.iter().take(2).all(|v| *v != 0.0));
        assert!(l.iter().skip(2).all(|v| *v == 0.0));
    }
}
