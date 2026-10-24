//! Real-time: read on the audio render thread (see docs/audio-engine.md).
//!
//! The prototype's `SciPy` filter designs as exact tables. Every design in the audio path depends
//! only on the sample rate and on constants, so `scripts/golden/gen_dsp_tables.py` captures the
//! prototype's own calls for the eight supported rates and writes their f64 bit patterns into
//! `generated.rs` ("Filter designs are tables"). Porting `SciPy`'s
//! `butter`, `ellip` and `firwin` would not reproduce them to the bit.
//!
//! - [`butter_bandpass`] and [`butter_highpass`]: the drum kit's filters (`drums._bandpass`,
//!   `drums._highpass`), named after their call sites.
//! - [`reverb_bands`]: the band filters of the synthesized reverb impulse response.
//! - [`ellip_oversampling`]: the saturation's anti-imaging and anti-aliasing filter.
//! - [`limiter_taps`] and [`true_peak_taps`]: the limiter's and the Analyzer's true-peak FIRs.
//! - [`spectrum_window`]: the Analyzer's periodic Hann window.

mod generated;

use crate::filters::Section;

/// One of the eight project sample rates the prototype supports (`SUPPORTED_SAMPLE_RATES`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SupportedRate {
    /// 22 050 Hz.
    Hz22050,
    /// 32 000 Hz.
    Hz32000,
    /// 44 100 Hz.
    Hz44100,
    /// 48 000 Hz.
    Hz48000,
    /// 88 200 Hz.
    Hz88200,
    /// 96 000 Hz.
    Hz96000,
    /// 176 400 Hz.
    Hz176400,
    /// 192 000 Hz.
    Hz192000,
}

impl SupportedRate {
    /// Every supported rate, ascending.
    pub const ALL: [Self; 8] = [
        Self::Hz22050,
        Self::Hz32000,
        Self::Hz44100,
        Self::Hz48000,
        Self::Hz88200,
        Self::Hz96000,
        Self::Hz176400,
        Self::Hz192000,
    ];

    /// The rate for `hz`, or `None` when the prototype does not support it.
    #[must_use]
    pub fn from_hz(hz: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|rate| rate.hz() == hz)
    }

    /// The rate in Hz.
    #[must_use]
    pub fn hz(self) -> u32 {
        match self {
            Self::Hz22050 => 22_050,
            Self::Hz32000 => 32_000,
            Self::Hz44100 => 44_100,
            Self::Hz48000 => 48_000,
            Self::Hz88200 => 88_200,
            Self::Hz96000 => 96_000,
            Self::Hz176400 => 176_400,
            Self::Hz192000 => 192_000,
        }
    }
}

/// The entry for `rate` of a table in the order of [`SupportedRate::ALL`].
fn for_rate<T>(table: &'static [T; 8], rate: SupportedRate) -> &'static T {
    let [
        r22050,
        r32000,
        r44100,
        r48000,
        r88200,
        r96000,
        r176400,
        r192000,
    ] = table;
    match rate {
        SupportedRate::Hz22050 => r22050,
        SupportedRate::Hz32000 => r32000,
        SupportedRate::Hz44100 => r44100,
        SupportedRate::Hz48000 => r48000,
        SupportedRate::Hz88200 => r88200,
        SupportedRate::Hz96000 => r96000,
        SupportedRate::Hz176400 => r176400,
        SupportedRate::Hz192000 => r192000,
    }
}

/// A band-pass call site of the drum kit: `butter(2, [lo, min(hi, 0.45 · sr)], 'bandpass')`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BandpassDesign {
    /// Snare noise, 1800–9000 Hz.
    Snare,
    /// Rim shot noise, 2000–6000 Hz.
    Rim,
    /// Clap noise, 900–3500 Hz.
    Clap,
    /// Cowbell, 400–2500 Hz.
    Cowbell,
}

/// A high-pass call site of the drum kit: `butter(order, min(f, 0.45 · sr), 'highpass')`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HighpassDesign {
    /// Kick click noise: 1500 Hz, order 2.
    KickClick,
    /// Closed and pedal hi-hat: 7000 Hz, order 4.
    ClosedHat,
    /// Open hi-hat: 6000 Hz, order 4.
    OpenHat,
    /// Crash, china and splash: 3500 Hz, order 2.
    Cymbal,
    /// Ride and ride bell: 5000 Hz, order 2.
    Ride,
    /// Tambourine jingles: 6000 Hz, order 4.
    Tambourine,
    /// Shaker: 5000 Hz, order 4.
    Shaker,
}

/// The two sections of a drum band-pass at `rate`, applied from zero state.
#[must_use]
pub fn butter_bandpass(design: BandpassDesign, rate: SupportedRate) -> &'static [Section; 2] {
    let table = match design {
        BandpassDesign::Snare => &generated::BANDPASS_SNARE,
        BandpassDesign::Rim => &generated::BANDPASS_RIM,
        BandpassDesign::Clap => &generated::BANDPASS_CLAP,
        BandpassDesign::Cowbell => &generated::BANDPASS_COWBELL,
    };
    for_rate(table, rate)
}

/// The sections of a drum high-pass at `rate` (one for order 2, two for order 4), applied from
/// zero state.
#[must_use]
pub fn butter_highpass(design: HighpassDesign, rate: SupportedRate) -> &'static [Section] {
    match design {
        HighpassDesign::KickClick => for_rate(&generated::HIGHPASS_KICK_CLICK, rate),
        HighpassDesign::ClosedHat => for_rate(&generated::HIGHPASS_CLOSED_HAT, rate),
        HighpassDesign::OpenHat => for_rate(&generated::HIGHPASS_OPEN_HAT, rate),
        HighpassDesign::Cymbal => for_rate(&generated::HIGHPASS_CYMBAL, rate),
        HighpassDesign::Ride => for_rate(&generated::HIGHPASS_RIDE, rate),
        HighpassDesign::Tambourine => for_rate(&generated::HIGHPASS_TAMBOURINE, rate),
        HighpassDesign::Shaker => for_rate(&generated::HIGHPASS_SHAKER, rate),
    }
}

/// One band of the synthesized reverb impulse response (`reverb.synthesize_ir`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReverbBand {
    /// Lower band edge in Hz; 0 for the first band, a low-pass.
    pub lo: f64,
    /// Upper band edge in Hz, as the prototype computed it (the last is `0.999 · nyquist`).
    pub hi: f64,
    /// `butter(4, hi, 'lowpass')` (two sections) for the first band,
    /// `butter(4, [lo, hi], 'bandpass')` (four sections) for the others.
    pub sections: &'static [Section],
}

/// The reverb's bands at `rate`, lowest first: edges at 250, 1000, 4000 and 12 000 Hz where
/// they lie below `0.95 · nyquist`, and the last band up to `0.999 · nyquist`. That is four bands
/// at 22 050 Hz and five at every other rate.
#[must_use]
pub fn reverb_bands(rate: SupportedRate) -> &'static [ReverbBand] {
    for_rate(&generated::REVERB_BANDS, rate)
}

/// Oversampling factor of the saturation and of the true-peak interpolators.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Oversampling {
    /// Twice the sample rate.
    X2,
    /// Four times the sample rate.
    X4,
}

impl Oversampling {
    /// The factor, 2 or 4.
    #[must_use]
    pub fn factor(self) -> usize {
        match self {
            Self::X2 => 2,
            Self::X4 => 4,
        }
    }
}

/// The saturation's oversampling filter at `rate`: `ellip(10, 0.05, 90, 0.46 · sr, 'lowpass',
/// fs = sr · factor)`, five sections, used with its own state for both the up- and the
/// down-sampling (`saturation._Resampler`).
#[must_use]
pub fn ellip_oversampling(rate: SupportedRate, factor: Oversampling) -> &'static [Section; 5] {
    match factor {
        Oversampling::X2 => for_rate(&generated::ELLIP_X2, rate),
        Oversampling::X4 => for_rate(&generated::ELLIP_X4, rate),
    }
}

/// The limiter's true-peak detector taps, `firwin(48, 0.95 / 4, window=('kaiser', 8.0)) · 4`
/// (independent of the rate).
#[must_use]
pub fn limiter_taps() -> &'static [f64; 48] {
    &generated::LIMITER_TAPS
}

/// The Analyzer's true-peak interpolator taps, `firwin(24 · os, 0.9 / os, window=('kaiser',
/// 9.0)) · os`: 96 taps at 4×, 48 at 2× (independent of the rate; the Analyzer picks 4× below
/// 88.2 kHz, 2× below 176.4 kHz and none above).
#[must_use]
pub fn true_peak_taps(os: Oversampling) -> &'static [f64] {
    match os {
        Oversampling::X2 => &generated::TRUE_PEAK_TAPS_X2,
        Oversampling::X4 => &generated::TRUE_PEAK_TAPS_X4,
    }
}

/// The Analyzer's spectrum window, `SciPy`'s periodic `windows.hann(8192, sym=False)` (F16: the
/// textbook formula is one ulp off in places).
#[must_use]
pub fn spectrum_window() -> &'static [f64; 8192] {
    &generated::SPECTRUM_WINDOW
}

#[cfg(test)]
mod tests {
    use super::{
        BandpassDesign, HighpassDesign, Oversampling, SupportedRate, butter_bandpass,
        butter_highpass, ellip_oversampling, limiter_taps, reverb_bands, spectrum_window,
        true_peak_taps,
    };
    use crate::filters::Section;

    const BANDPASS: [BandpassDesign; 4] = [
        BandpassDesign::Snare,
        BandpassDesign::Rim,
        BandpassDesign::Clap,
        BandpassDesign::Cowbell,
    ];
    /// Every high-pass call site with its section count (order / 2).
    const HIGHPASS: [(HighpassDesign, usize); 7] = [
        (HighpassDesign::KickClick, 1),
        (HighpassDesign::ClosedHat, 2),
        (HighpassDesign::OpenHat, 2),
        (HighpassDesign::Cymbal, 1),
        (HighpassDesign::Ride, 1),
        (HighpassDesign::Tambourine, 2),
        (HighpassDesign::Shaker, 2),
    ];

    fn assert_normalized(what: impl std::fmt::Debug, rate: SupportedRate, sections: &[Section]) {
        let hz = rate.hz();
        assert!(!sections.is_empty(), "{what:?} at {hz} Hz: no sections");
        for section in sections {
            assert!(section.iter().all(|v| v.is_finite()), "{what:?} at {hz} Hz");
            assert_eq!(section.get(3), Some(&1.0), "{what:?} at {hz} Hz: a0 is 1");
        }
    }

    #[test]
    fn rates_round_trip() {
        let hz: Vec<u32> = SupportedRate::ALL.iter().map(|r| r.hz()).collect();
        assert_eq!(
            hz,
            [
                22_050, 32_000, 44_100, 48_000, 88_200, 96_000, 176_400, 192_000
            ]
        );
        for rate in SupportedRate::ALL {
            assert_eq!(SupportedRate::from_hz(rate.hz()), Some(rate));
        }
        for hz in [0, 8000, 44_000, 48_001, 384_000] {
            assert_eq!(SupportedRate::from_hz(hz), None);
        }
    }

    /// The table set is complete: every design exists at every rate with the expected shape.
    #[test]
    fn table_set_is_complete_for_all_rates() {
        for rate in SupportedRate::ALL {
            let hz = rate.hz();
            for design in BANDPASS {
                assert_normalized(design, rate, butter_bandpass(design, rate));
            }
            for (design, sections) in HIGHPASS {
                let table = butter_highpass(design, rate);
                assert_eq!(table.len(), sections, "{design:?} at {hz} Hz");
                assert_normalized(design, rate, table);
            }

            let bands = reverb_bands(rate);
            let expected = if rate == SupportedRate::Hz22050 { 4 } else { 5 };
            assert_eq!(bands.len(), expected, "reverb bands at {hz} Hz");
            let nyquist = f64::from(hz) / 2.0;
            assert_eq!(
                bands.first().map(|b| (b.lo, b.sections.len())),
                Some((0.0, 2))
            );
            assert_eq!(bands.last().map(|b| b.hi), Some(nyquist * 0.999));
            for (i, band) in bands.iter().enumerate() {
                assert!(band.lo < band.hi, "reverb band {i} at {hz} Hz");
                if i > 0 {
                    assert_eq!(band.sections.len(), 4, "reverb band {i} at {hz} Hz");
                }
                assert_normalized(("reverb band", i), rate, band.sections);
            }
            for pair in bands.windows(2) {
                if let [low, high] = pair {
                    assert_eq!(low.hi, high.lo, "reverb bands are contiguous at {hz} Hz");
                }
            }

            for factor in [Oversampling::X2, Oversampling::X4] {
                assert_normalized(factor, rate, ellip_oversampling(rate, factor));
            }
        }
        // The designs depend on the rate.
        assert_ne!(
            butter_bandpass(BandpassDesign::Snare, SupportedRate::Hz44100),
            butter_bandpass(BandpassDesign::Snare, SupportedRate::Hz48000)
        );
    }

    #[test]
    fn fir_tables_have_their_lengths_and_gain() {
        assert_eq!(true_peak_taps(Oversampling::X4).len(), 96);
        assert_eq!(true_peak_taps(Oversampling::X2).len(), 48);
        for (taps, factor) in [
            (limiter_taps().as_slice(), 4.0),
            (true_peak_taps(Oversampling::X4), 4.0),
            (true_peak_taps(Oversampling::X2), 2.0),
        ] {
            // An interpolator scaled by its factor has a DC gain of the factor.
            let sum: f64 = taps.iter().sum();
            assert!((sum - factor).abs() < 1e-12, "{sum} vs {factor}");
            // Linear phase: SciPy's taps are exactly symmetric.
            assert!(
                taps.iter()
                    .zip(taps.iter().rev())
                    .all(|(a, b)| a.to_bits() == b.to_bits())
            );
        }
    }

    #[test]
    fn spectrum_window_is_a_periodic_hann() {
        let w = spectrum_window();
        assert_eq!(
            (w.first(), w.get(2048), w.get(4096)),
            (Some(&0.0), Some(&0.5), Some(&1.0))
        );
        assert!(w.iter().all(|v| (0.0..=1.0).contains(v)));
        // Periodic: w[k] equals w[N − k] for 0 < k < N up to rounding (SciPy's linspace-based
        // form is not bit-symmetric).
        let tail = w.get(1..).unwrap_or_default();
        assert!(
            tail.iter()
                .zip(tail.iter().rev())
                .all(|(a, b)| (a - b).abs() <= 2.0 * f64::EPSILON)
        );
    }
}
