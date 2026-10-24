//! Property tests of the kernels (TST-09): state continuity and stability.
//!
//! - Processing a signal in one call gives bit-identical output and state to processing it
//!   split across arbitrary chunks (the state is the only memory).
//! - Output stays finite at extreme cutoffs below Nyquist and any `q > 0`.

use proptest::collection::vec;
use proptest::prelude::*;
use sonara_dsp::kernels::{
    GateState, Oscillator, SvfMode, Wave, ar_smooth, clamp_cutoff, gate_hold, osc_bank, svf,
};

const SR: f64 = 48_000.0;

/// Bit patterns, so that equality means identical, and a NaN would not hide a difference.
fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|v| v.to_bits()).collect()
}

/// Splits `0..len` into consecutive chunks whose sizes cycle through `sizes` (each ≥ 1).
fn chunks(len: usize, sizes: &[usize]) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for &size in sizes.iter().cycle() {
        if start >= len {
            break;
        }
        let end = len.min(start + size);
        ranges.push(start..end);
        start = end;
    }
    ranges
}

fn sizes() -> impl Strategy<Value = Vec<usize>> {
    vec(1_usize..70, 1..6)
}

fn signal(max_len: usize) -> impl Strategy<Value = Vec<f64>> {
    vec(-1.0_f64..1.0, 0..max_len)
}

proptest! {
    #[test]
    fn ar_smooth_is_chunk_invariant(
        x in vec(-10.0_f64..10.0, 0..400),
        attack in 0.0_f64..1.0,
        release in 0.0_f64..1.0,
        initial in -1.0_f64..1.0,
        sizes in sizes(),
    ) {
        let mut whole = vec![0.0; x.len()];
        let mut whole_state = initial;
        ar_smooth(&x, &mut whole, attack, release, &mut whole_state);

        let mut split = vec![0.0; x.len()];
        let mut split_state = initial;
        for range in chunks(x.len(), &sizes) {
            ar_smooth(&x[range.clone()], &mut split[range], attack, release, &mut split_state);
        }
        prop_assert_eq!(bits(&whole), bits(&split));
        prop_assert_eq!(whole_state.to_bits(), split_state.to_bits());
    }

    #[test]
    fn ar_smooth_stays_between_the_input_extremes(
        x in vec(0.0_f64..1.0, 1..400),
        attack in 0.0_f64..1.0,
        release in 0.0_f64..1.0,
    ) {
        // A convex combination of the previous output and the input never leaves [0, 1].
        let mut out = vec![0.0; x.len()];
        ar_smooth(&x, &mut out, attack, release, &mut 0.0);
        prop_assert!(out.iter().all(|y| (0.0..=1.0).contains(y)));
    }

    #[test]
    fn svf_is_chunk_invariant(
        channels in 1_usize..4,
        frames in 0_usize..200,
        seed in vec(-1.0_f64..1.0, 600),
        cutoff in vec(5.0_f64..23_520.0, 200),
        q in 0.05_f64..50.0,
        raw_mode in -1_i32..8,
        sizes in sizes(),
    ) {
        let mode = SvfMode::from_raw(raw_mode);
        let x = &seed[..frames * channels];
        let cutoff = &cutoff[..frames];

        let mut whole = vec![0.0; x.len()];
        let mut whole_state = vec![[0.0; 2]; channels];
        svf(x, &mut whole, cutoff, q, mode, SR, &mut whole_state);

        let mut split = vec![0.0; x.len()];
        let mut split_state = vec![[0.0; 2]; channels];
        for range in chunks(frames, &sizes) {
            let samples = range.start * channels..range.end * channels;
            svf(&x[samples.clone()], &mut split[samples], &cutoff[range], q, mode, SR, &mut split_state);
        }
        prop_assert_eq!(bits(&whole), bits(&split));
        prop_assert_eq!(
            bits(&whole_state.concat()),
            bits(&split_state.concat())
        );
    }

    #[test]
    fn svf_stays_finite_at_extreme_cutoffs_and_q(
        x in signal(300),
        // From far below the audible range to 0.4999 of the sample rate; the wrapper's own clamp
        // stops at 0.49, so this goes beyond what the prototype ever feeds the kernel.
        cutoff in vec(prop_oneof![
            Just(1e-3), Just(5.0), Just(0.4999 * SR), Just(0.49 * SR), 1e-3_f64..0.4999 * SR,
        ], 300),
        q in prop_oneof![Just(1e-3), Just(0.05), Just(1e3), Just(1e6), 1e-3_f64..1e6],
        raw_mode in 0_i32..6,
    ) {
        let mut out = vec![0.0; x.len()];
        svf(&x, &mut out, &cutoff, q, SvfMode::from_raw(raw_mode), SR, &mut [[0.0; 2]]);
        prop_assert!(out.iter().all(|y| y.is_finite()), "{out:?}");
    }

    #[test]
    fn clamped_cutoff_is_always_below_nyquist(cutoff in -1e9_f64..1e9) {
        let clamped = clamp_cutoff(cutoff, SR);
        prop_assert!((5.0..=0.49 * SR).contains(&clamped));
    }

    #[test]
    fn gate_hold_is_chunk_invariant(
        level in vec(-90.0_f64..0.0, 0..500),
        hold in 0_i64..100,
        open in prop::bool::ANY,
        counter in 0_i64..100,
        sizes in sizes(),
    ) {
        let initial = GateState { is_open: open, hold_remaining: counter };
        let mut whole = vec![0.0; level.len()];
        let mut whole_state = initial;
        gate_hold(&level, &mut whole, -40.0, -50.0, hold, &mut whole_state);

        let mut split = vec![0.0; level.len()];
        let mut split_state = initial;
        for range in chunks(level.len(), &sizes) {
            gate_hold(&level[range.clone()], &mut split[range], -40.0, -50.0, hold, &mut split_state);
        }
        prop_assert_eq!(&whole, &split);
        prop_assert_eq!(whole_state, split_state);
        prop_assert!(whole.iter().all(|g| [0.0, 1.0].contains(g)));
    }

    #[test]
    fn gate_never_closes_before_the_hold_has_run_out(
        quiet in 0_usize..50,
        hold in 0_i64..50,
    ) {
        // One loud sample, then `quiet` samples below the close threshold: the gate stays open
        // for exactly `hold` of them (and the loud one).
        let mut level = vec![-10.0];
        level.extend(std::iter::repeat_n(-80.0, quiet));
        let mut out = vec![0.0; level.len()];
        gate_hold(&level, &mut out, -40.0, -50.0, hold, &mut GateState::default());
        let open = out.iter().filter(|g| **g > 0.5).count();
        let expected = 1 + quiet.min(usize::try_from(hold).unwrap_or(0));
        prop_assert_eq!(open, expected);
    }

    #[test]
    fn osc_bank_is_chunk_invariant_and_bounded(
        pitch in vec(-40.0_f64..150.0, 0..300),
        params in vec(
            (0_i32..6, -0.5_f64..1.5, -36.0_f64..36.0, 0.0_f64..1.0, 0.0_f64..1.0, 0.0_f64..1.0),
            0..10,
        ),
        pulse_width in 0.05_f64..0.95,
        sizes in sizes(),
    ) {
        let bank: Vec<Oscillator> = params
            .iter()
            .map(|&(wave, level, pitch_offset, pan_l, pan_r, phase)| Oscillator {
                wave: Wave::from_raw(wave),
                level,
                pitch_offset,
                pan_l,
                pan_r,
                phase,
            })
            .collect();

        let n = pitch.len();
        let (mut whole_l, mut whole_r) = (vec![0.25; n], vec![-0.25; n]);
        let mut whole_bank = bank.clone();
        osc_bank(&pitch, &mut whole_bank, pulse_width, SR, &mut whole_l, &mut whole_r);

        let (mut split_l, mut split_r) = (vec![0.25; n], vec![-0.25; n]);
        let mut split_bank = bank;
        for range in chunks(n, &sizes) {
            osc_bank(
                &pitch[range.clone()],
                &mut split_bank,
                pulse_width,
                SR,
                &mut split_l[range.clone()],
                &mut split_r[range],
            );
        }
        prop_assert_eq!(bits(&whole_l), bits(&split_l));
        prop_assert_eq!(bits(&whole_r), bits(&split_r));
        prop_assert_eq!(&whole_bank, &split_bank);

        prop_assert!(whole_bank.iter().all(|o| (0.0..1.0).contains(&o.phase)));
        // Each oscillator contributes at most `level × pan` × the peak of its waveform; PolyBLEP
        // overshoots 1 only slightly, so 2 is a safe bound.
        let bound: f64 = whole_bank.iter().map(|o| o.level.max(0.0) * 2.0).sum();
        prop_assert!(whole_l.iter().all(|v| (v - 0.25).abs() <= bound + 1e-9));
        prop_assert!(whole_r.iter().all(|v| (v + 0.25).abs() <= bound + 1e-9));
    }
}
