//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! Gate state machine with hysteresis and hold.

/// State of [`gate_hold`], carried across calls. The default is a closed gate with no hold left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GateState {
    /// Whether the gate is open.
    pub is_open: bool,
    /// Samples the level may still stay below the close threshold before the gate closes.
    pub hold_remaining: i64,
}

/// Gate with hysteresis and hold. Writes `1.0` (open) or `0.0` (closed) per sample.
///
/// - A level at or above `open_db` opens the gate and reloads the hold counter with `hold`.
/// - While open, a level below `close_db` first counts the hold down (one per sample) and
///   closes the gate on the first such sample after the counter reached zero, so `hold` quiet
///   samples are held open. Levels between `close_db` and `open_db`
///   change nothing.
///
/// `state` is read at the start and updated, so consecutive calls continue the same signal. If
/// `level_db` and `out` differ in length, the common length is processed (see the
/// [module docs](super)).
///
/// # Examples
///
/// ```
/// use sonara_dsp::kernels::{GateState, gate_hold};
///
/// let mut out = [0.0; 4];
/// let mut state = GateState::default();
/// gate_hold(&[-10.0, -60.0, -60.0, -60.0], &mut out, -40.0, -50.0, 1, &mut state);
/// assert_eq!(out, [1.0, 1.0, 0.0, 0.0]);
/// assert!(!state.is_open);
/// ```
pub fn gate_hold(
    level_db: &[f64],
    out: &mut [f64],
    open_db: f64,
    close_db: f64,
    hold: i64,
    state: &mut GateState,
) {
    let mut is_open = state.is_open;
    let mut counter = state.hold_remaining;
    for (&v, o) in level_db.iter().zip(out.iter_mut()) {
        if v >= open_db {
            is_open = true;
            counter = hold;
        } else if is_open && v < close_db {
            if counter > 0 {
                counter -= 1;
            } else {
                is_open = false;
            }
        }
        *o = if is_open { 1.0 } else { 0.0 };
    }
    *state = GateState {
        is_open,
        hold_remaining: counter,
    };
}

#[cfg(test)]
mod tests {
    use super::{GateState, gate_hold};

    fn run<const N: usize>(levels: [f64; N], hold: i64, state: &mut GateState) -> [f64; N] {
        let mut out = [0.0; N];
        gate_hold(&levels, &mut out, -40.0, -50.0, hold, state);
        out
    }

    #[test]
    fn stays_closed_below_the_open_threshold() {
        let mut state = GateState::default();
        assert_eq!(run([-41.0, -60.0], 0, &mut state), [0.0, 0.0]);
        assert_eq!(state, GateState::default());
    }

    #[test]
    fn hysteresis_band_keeps_the_gate_open_and_the_counter() {
        let mut state = GateState::default();
        // -45 dB is below the open and above the close threshold: no change either way.
        assert_eq!(run([-45.0, -30.0, -45.0], 3, &mut state), [0.0, 1.0, 1.0]);
        assert_eq!(
            state,
            GateState {
                is_open: true,
                hold_remaining: 3
            }
        );
    }

    #[test]
    fn hold_delays_closing_by_exactly_hold_samples() {
        let mut state = GateState::default();
        let out = run([-30.0, -60.0, -60.0, -60.0, -60.0], 2, &mut state);
        assert_eq!(out, [1.0, 1.0, 1.0, 0.0, 0.0]);
        assert!(!state.is_open);
        assert_eq!(state.hold_remaining, 0);
    }

    #[test]
    fn zero_hold_closes_on_the_first_quiet_sample() {
        let mut state = GateState::default();
        let out = run([-30.0, -60.0, -60.0], 0, &mut state);
        assert_eq!(out, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn state_continues_across_calls() {
        let levels = [
            -30.0, -60.0, -60.0, -45.0, -60.0, -60.0, -60.0, -30.0, -60.0,
        ];
        let mut whole = GateState::default();
        let expected = run(levels, 2, &mut whole);

        let mut split = GateState::default();
        let (head, tail) = levels.split_at(4);
        let mut out = [0.0; 9];
        let (out_head, out_tail) = out.split_at_mut(4);
        gate_hold(head, out_head, -40.0, -50.0, 2, &mut split);
        gate_hold(tail, out_tail, -40.0, -50.0, 2, &mut split);
        assert_eq!(out, expected);
        assert_eq!(split, whole);
    }

    #[test]
    fn shorter_slice_sets_the_length() {
        let mut state = GateState::default();
        let mut out = [9.0; 2];
        gate_hold(
            &[-30.0, -30.0, -30.0],
            &mut out,
            -40.0,
            -50.0,
            0,
            &mut state,
        );
        assert_eq!(out, [1.0, 1.0]);
    }
}
