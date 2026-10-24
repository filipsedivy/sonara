//! Real-time: [`PartitionedConvolver::process`], [`install`](PartitionedConvolver::install),
//! [`reset`](PartitionedConvolver::reset) and
//! [`take_retired`](PartitionedConvolver::take_retired) run on the audio render thread (see
//! docs/audio-engine.md); the constructors ([`PartitionedConvolver::new`],
//! [`PartitionedConvolver::with_input`], [`PreparedIr::new`], [`PreparedIr::with_input`]) and
//! [`PartitionedConvolver::set_ir`] are setup.
//!
//! Uniformly partitioned overlap-save convolution (UPOLS): the prototype's
//! `PartitionedConvolver` (`dsp/time_frequency/convolution.py`), the building block of the
//! convolver and reverb processors. The partition equals the block size `b`, so the convolver
//! adds no latency; an impulse-response swap crossfades over one block.
//!
//! The float type `T` of the spectra is [`f64`] for the prototype's default mode and [`f32`] for
//! its `single_precision` mode (`convolution.py`). Per block and
//! channel, in the prototype's order (RT-11):
//!
//! 1. The input frame is the previous block followed by this one, `2b` samples; its real FFT is
//!    computed in f64 and rounded to `T` into slot `head` of the frequency-domain delay line
//!    (NumPy's assignment into a `complex64` array rounds each component).
//! 2. The partitions are summed per bin in `T` as NumPy's compiled `einsum("pcf,pcf->cf")` does
//!    on the recording host: partitions ascending, from zero,
//!    `re = fma(h.re, x.re, −(h.im·x.im)) + re` and `im = fma(h.re, x.im, h.im·x.re) + im`, in
//!    two halves (partitions `0..=head` against slots `head, head−1, …, 0`, then the rest
//!    against slots `P−1, …, head+1`) whose sums are added at the end. Verified bit-exact for
//!    both dtypes at 9, 47 and 2813 partitions.
//! 3. The inverse real FFT runs in `T` (NumPy's `irfft` of `complex64` computes in float32) and
//!    is scaled by `1/(2b)` computed in `T`, as pocketfft's `fct`; the last `b` samples are the
//!    output. For a power-of-two block the scaling is exact.
//!
//! The transforms are `realfft`'s, whose factorization differs from NumPy's pocketfft: their
//! results agree to the last few ulps, not bit for bit. That is the only inexact step; the
//! golden test documents the measured tolerance (`tests/golden_convolution.rs`).
//!
//! **Speed without a change of arithmetic** (PERF-01: step 2 is about 99 % of a block with a
//! long response; `benches/convolution.rs`). Every bin still takes exactly the terms above,
//! partition after partition, into one running sum from zero, and the two halves are added as
//! before (RT-11), so the output is bit-identical to the plain loop's (checked against the
//! previous implementation, by a unit test against that loop and by the golden tests). Only the
//! memory layout and the loop order changed:
//!
//! - Bins are kept in tiles of 8 bins (the private `Tile` and `LANES`), real parts apart from imaginary parts, so that
//!   the vector loop loads whole registers; the last tile of a channel is padded with zeros that
//!   no output reads.
//! - The layout is bin-major: the spectra are `(tiles, channels, partitions)` and the delay line
//!   `(tiles, inputs, slots)`. Step 2 takes one row at a time, a tile of one channel, and reads
//!   one run of spectra forward and one run of the delay line backward, with the running sum in
//!   registers for the whole run. Long sequential runs suit the hardware prefetcher, which
//!   matters because between two blocks the rest of the graph evicts the spectra from the
//!   caches.
//! - The delay line keeps the conjugate of each spectrum, which saves a negation per term
//!   without changing any product (the private `Tile::accumulate`).
//! - [`InputMode::Shared`] keeps one delay-line row per tile for an input that is the same on
//!   every channel (the reverb's mono sum): a quarter less memory per block in stereo, and two
//!   channels' rows are summed in one pass over it.
//!
//! **Swapping the impulse response** ([`install`](PartitionedConvolver::install) with
//! `crossfade`) seeds the new delay line with the input history (the newest old spectrum goes to
//! the new line's last slot, which the new head reaches one block later) and crossfades the two
//! outputs over the next block with `linspace(0, 1, b)` in f64: `y·(1 − fade) + y_new·fade`.
//! Without a crossfade, or without a current response, the new one starts with an empty delay
//! line at once. The input history (`prev`) is kept either way.
//!
//! Memory: an impulse response is prepared off the real-time thread ([`PreparedIr::new`]);
//! [`install`](PartitionedConvolver::install) moves it in without allocating and hands the
//! displaced spectra back as [`Retired`], to be dropped off the real-time thread (RT-09).
//! Buffers are interleaved `(frames, channels)`, like the rest of the crate.

mod precision;

pub use precision::{OutputPrecision, SpectrumFloat};

use std::fmt;
use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

use crate::numpy::math::linspace;

/// Errors of the convolver's setup and of a wrongly sized block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConvolverError {
    /// The block size or the channel count is zero, or the buffers would not fit in memory.
    Layout {
        /// Requested frames per block.
        block_size: usize,
        /// Requested channels.
        channels: usize,
    },
    /// The impulse response has a channel count the convolver cannot use: zero, or more than
    /// one but fewer than the convolver has (NumPy cannot broadcast it; a mono response is
    /// repeated and extra channels are dropped, as in the prototype).
    IrChannels {
        /// Channels of the impulse response.
        ir_channels: usize,
        /// Channels of the convolver.
        channels: usize,
    },
    /// The impulse response is not a whole number of frames.
    IrLength {
        /// Interleaved samples given.
        samples: usize,
        /// Channels of the impulse response.
        ir_channels: usize,
    },
    /// A block passed to [`PartitionedConvolver::process`] does not have `block_size ·
    /// channels` samples; nothing was processed.
    BlockSize {
        /// Samples a block must have.
        expected: usize,
        /// Samples of the input.
        input: usize,
        /// Samples of the output.
        output: usize,
    },
    /// `realfft` rejected a buffer. The buffers are sized in setup, so this does not happen; it
    /// is reported instead of panicking on the real-time thread.
    Transform,
}

impl fmt::Display for ConvolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Layout {
                block_size,
                channels,
            } => write!(
                f,
                "cannot convolve blocks of {block_size} frames with {channels} channels"
            ),
            Self::IrChannels {
                ir_channels,
                channels,
            } => write!(
                f,
                "an impulse response with {ir_channels} channels cannot feed {channels} channels"
            ),
            Self::IrLength {
                samples,
                ir_channels,
            } => write!(
                f,
                "{samples} samples are not whole frames of {ir_channels} channels"
            ),
            Self::BlockSize {
                expected,
                input,
                output,
            } => write!(
                f,
                "a block has {expected} samples, got {input} in and {output} out"
            ),
            Self::Transform => write!(f, "the FFT rejected a buffer"),
        }
    }
}

impl std::error::Error for ConvolverError {}

/// Which input channel feeds each channel of a [`PartitionedConvolver`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputMode {
    /// Input channel `c` feeds channel `c` (the prototype's `PartitionedConvolver`).
    PerChannel,
    /// Input channel 0 feeds every channel; the other input channels are not read. For an input
    /// with the same signal on every channel (the reverb's mono sum) the output is bit-identical
    /// to [`InputMode::PerChannel`]'s: every channel's input spectrum would be the same, so one
    /// transform and one delay-line channel serve them all, which saves memory traffic.
    Shared,
}

/// Error of [`PartitionedConvolver::install`]: the response was prepared for another block
/// size, channel count or input mode. It is handed back unchanged, so that it is not dropped on
/// the real-time thread.
pub struct LayoutMismatch<T: SpectrumFloat>(pub PreparedIr<T>);

impl<T: SpectrumFloat> fmt::Debug for LayoutMismatch<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("LayoutMismatch").field(&self.0).finish()
    }
}

impl<T: SpectrumFloat> fmt::Display for LayoutMismatch<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the impulse response was prepared for blocks of {} frames with {} channels ({:?} \
             input)",
            self.0.block_size, self.0.channels, self.0.input
        )
    }
}

impl<T: SpectrumFloat> std::error::Error for LayoutMismatch<T> {}

/// Bins per [`Tile`].
const LANES: usize = 8;

/// [`LANES`] consecutive bins of one channel, the real parts apart from the imaginary parts.
#[derive(Clone, Copy)]
struct Tile<T> {
    re: [T; LANES],
    im: [T; LANES],
}

impl<T: SpectrumFloat> Tile<T> {
    /// Every lane `0 + 0i`.
    fn zero() -> Self {
        let zero = T::narrow(0.0);
        Self {
            re: [zero; LANES],
            im: [zero; LANES],
        }
    }

    /// One partition's term of the einsum, accumulated into the running sum `self` in NumPy's
    /// contracted form (see the module docs), lane by lane. `conj_x` is the conjugate of the
    /// input spectrum `x`, as the delay line keeps it: with `x.im = −conj_x.im`,
    /// `−(h.im·x.im) = h.im·conj_x.im` and `h.re·x.im = (−h.re)·conj_x.im` are the same
    /// products, signs of zero included, so each term rounds exactly as NumPy's does, and the
    /// vector loop needs no negation.
    #[inline]
    fn accumulate(&mut self, h: &Self, conj_x: &Self) {
        let sums = self.re.iter_mut().zip(&mut self.im);
        let terms = h.re.iter().zip(&h.im).zip(conj_x.re.iter().zip(&conj_x.im));
        for ((s_re, s_im), ((&h_re, &h_im), (&x_re, &conj_im))) in sums.zip(terms) {
            // re = fma(h.re, x.re, −(h.im·x.im)), im = fma(h.re, x.im, h.im·x.re).
            let re = h_re.fused_mul_add(x_re, h_im * conj_im);
            let im = (-h_re).fused_mul_add(conj_im, h_im * x_re);
            *s_re = re + *s_re;
            *s_im = im + *s_im;
        }
    }

    /// `self + other`, lane by lane.
    fn add(&mut self, other: &Self) {
        for (s, &t) in self.re.iter_mut().zip(&other.re) {
            *s = *s + t;
        }
        for (s, &t) in self.im.iter_mut().zip(&other.im) {
            *s = *s + t;
        }
    }

    /// The complex bins `values` (at most [`LANES`]), rounded to `T`; with `conjugate`, the
    /// imaginary parts negated (exact). Lanes without a value are left as they are (zero).
    fn set(&mut self, values: &[Complex<f64>], conjugate: bool) {
        let lanes = self.re.iter_mut().zip(&mut self.im);
        for ((re, im), value) in lanes.zip(values) {
            *re = T::narrow(value.re);
            let narrowed = T::narrow(value.im);
            *im = if conjugate { -narrowed } else { narrowed };
        }
    }
}

/// An impulse response's partition spectra with its frequency-domain delay line, bin-major
/// (see the module docs): row `(t, c)` holds tile `t` of channel `c` for every partition.
struct IrState<T> {
    /// `P`, at least 1.
    partitions: usize,
    /// `(tiles, channels, partitions)` spectra: the prototype's `_h`, which is `(partitions,
    /// channels, bins)`, transposed.
    spectra: Vec<Tile<T>>,
    /// The delay line, `(tiles, inputs, slots)`: the prototype's `_fdl`, transposed, with one
    /// input channel in [`InputMode::Shared`]. It keeps the complex conjugate of each spectrum
    /// (the imaginary parts negated, which is exact), so that the multiply-accumulate needs no
    /// negation ([`Tile::accumulate`]).
    delay_line: Vec<Tile<T>>,
}

/// An impulse response prepared for a [`PartitionedConvolver`] off the real-time thread: its
/// partition spectra and an empty delay line (the prototype's `_spectra` and
/// `np.zeros_like(h)`).
pub struct PreparedIr<T: SpectrumFloat> {
    block_size: usize,
    channels: usize,
    input: InputMode,
    state: IrState<T>,
}

impl<T: SpectrumFloat> fmt::Debug for PreparedIr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedIr")
            .field("block_size", &self.block_size)
            .field("channels", &self.channels)
            .field("input", &self.input)
            .field("partitions", &self.state.partitions)
            .finish_non_exhaustive()
    }
}

impl<T: SpectrumFloat> PreparedIr<T> {
    /// Partitions the interleaved `(taps, ir_channels)` response `ir` into blocks of
    /// `block_size` frames and transforms them for a convolver with `channels` channels and
    /// [`InputMode::PerChannel`]: the prototype's `set_ir` up to the swap. A mono response is
    /// repeated on every channel, extra channels are dropped, and an empty response gives one
    /// silent partition.
    ///
    /// Setup: plans an FFT and allocates, so it runs off the real-time thread.
    ///
    /// # Errors
    ///
    /// [`ConvolverError::Layout`] for a zero block size or channel count,
    /// [`ConvolverError::IrChannels`] and [`ConvolverError::IrLength`] for a response the
    /// convolver cannot use.
    pub fn new(
        block_size: usize,
        channels: usize,
        ir: &[f64],
        ir_channels: usize,
    ) -> Result<Self, ConvolverError> {
        Self::with_input(block_size, channels, InputMode::PerChannel, ir, ir_channels)
    }

    /// [`PreparedIr::new`] for a convolver with the input mode `input`.
    ///
    /// Setup: plans an FFT and allocates, so it runs off the real-time thread.
    ///
    /// # Errors
    ///
    /// Those of [`PreparedIr::new`].
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    pub fn with_input(
        block_size: usize,
        channels: usize,
        input: InputMode,
        ir: &[f64],
        ir_channels: usize,
    ) -> Result<Self, ConvolverError> {
        let layout = Layout::new(block_size, channels, input)?;
        if ir_channels == 0 || (ir_channels != 1 && ir_channels < channels) {
            return Err(ConvolverError::IrChannels {
                ir_channels,
                channels,
            });
        }
        if !ir.len().is_multiple_of(ir_channels) {
            return Err(ConvolverError::IrLength {
                samples: ir.len(),
                ir_channels,
            });
        }
        let taps = ir.len() / ir_channels;
        let partitions = taps.div_ceil(block_size).max(1);
        let error = ConvolverError::Layout {
            block_size,
            channels,
        };
        let size = partitions.checked_mul(layout.slot_tiles).ok_or(error)?;
        let delay_size = partitions.checked_mul(layout.input_tiles).ok_or(error)?;
        let mut spectra = vec![Tile::zero(); size];
        let forward = RealFftPlanner::<f64>::new().plan_fft_forward(layout.fft_size);
        let mut frame = forward.make_input_vec();
        let mut spectrum = forward.make_output_vec();
        let mut scratch = forward.make_scratch_vec();
        for p in 0..partitions {
            let rows = ir
                .chunks_exact(ir_channels)
                .skip(p.saturating_mul(block_size))
                .take(block_size);
            for c in 0..channels {
                let source = if ir_channels == 1 { 0 } else { c };
                // `rfft(parts, n=2b, axis=1)`: the partition zero-padded to the FFT size.
                frame.fill(0.0);
                for (sample, row) in frame.iter_mut().zip(rows.clone()) {
                    *sample = row.get(source).copied().unwrap_or(0.0);
                }
                forward
                    .process_with_scratch(&mut frame, &mut spectrum, &mut scratch)
                    .map_err(|_| ConvolverError::Transform)?;
                // Row `(t, c)` holds tile `t` of channel `c` for every partition.
                let tile_rows = spectra
                    .chunks_exact_mut(partitions)
                    .skip(c)
                    .step_by(channels);
                for (row, values) in tile_rows.zip(spectrum.chunks(LANES)) {
                    if let Some(tile) = row.get_mut(p) {
                        tile.set(values, false);
                    }
                }
            }
        }
        Ok(Self {
            block_size,
            channels,
            input,
            state: IrState {
                partitions,
                spectra,
                delay_line: vec![Tile::zero(); delay_size],
            },
        })
    }

    /// Number of partitions (`ceil(taps / block_size)`, at least one).
    #[must_use]
    pub fn partitions(&self) -> usize {
        self.state.partitions
    }
}

/// Impulse responses a convolver no longer uses: the displaced current or pending response and
/// one that a finished crossfade retired. Dropping it frees their memory, so drop it off the
/// real-time thread (RT-09).
#[must_use = "dropping frees memory: drop it off the real-time thread (RT-09)"]
pub struct Retired<T: SpectrumFloat> {
    states: [Option<IrState<T>>; 3],
}

impl<T: SpectrumFloat> Retired<T> {
    /// Whether nothing was retired.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.states.iter().all(Option::is_none)
    }
}

impl<T: SpectrumFloat> fmt::Debug for Retired<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.states.iter().filter(|s| s.is_some()).count();
        f.debug_struct("Retired")
            .field("responses", &count)
            .finish()
    }
}

/// Sizes derived from the block size, the channel count and the input mode.
#[derive(Clone, Copy, Debug)]
struct Layout {
    /// The input mode.
    input: InputMode,
    /// FFT size, `2b`.
    fft_size: usize,
    /// Bins per channel, `b + 1`.
    bins: usize,
    /// Channels.
    channels: usize,
    /// Input channels: `channels`, or 1 for [`InputMode::Shared`].
    inputs: usize,
    /// Bins of all input channels: the forward spectra of a block.
    input_bins: usize,
    /// Tiles of all channels, `ceil(bins / LANES) · channels`: one partition of the spectra, and
    /// the sum.
    slot_tiles: usize,
    /// Tiles of all input channels: one slot of the delay line.
    input_tiles: usize,
    /// Samples of one interleaved block, `b · channels`.
    block_samples: usize,
}

impl Layout {
    fn new(block_size: usize, channels: usize, input: InputMode) -> Result<Self, ConvolverError> {
        let error = ConvolverError::Layout {
            block_size,
            channels,
        };
        if block_size == 0 || channels == 0 {
            return Err(error);
        }
        let fft_size = block_size.checked_mul(2).ok_or(error)?;
        let bins = block_size.checked_add(1).ok_or(error)?;
        let tiles = bins.div_ceil(LANES);
        let inputs = match input {
            InputMode::PerChannel => channels,
            InputMode::Shared => 1,
        };
        Ok(Self {
            input,
            fft_size,
            bins,
            channels,
            inputs,
            input_bins: bins.checked_mul(inputs).ok_or(error)?,
            slot_tiles: tiles.checked_mul(channels).ok_or(error)?,
            input_tiles: tiles.checked_mul(inputs).ok_or(error)?,
            block_samples: block_size.checked_mul(channels).ok_or(error)?,
        })
    }
}

/// The FFT plans and the per-block work buffers.
struct Transforms<T: SpectrumFloat> {
    forward: Arc<dyn RealToComplex<f64>>,
    forward_frame: Vec<f64>,
    forward_scratch: Vec<Complex<f64>>,
    /// This block's input spectra, `(inputs, bins)`.
    spectrum: Vec<Complex<f64>>,
    inverse: Arc<dyn ComplexToReal<T>>,
    inverse_bins: Vec<Complex<T>>,
    inverse_frame: Vec<T>,
    inverse_scratch: Vec<Complex<T>>,
    /// pocketfft's `fct`, `1 / (2b)` in `T`.
    scale: T,
    /// The einsum's result, `(tiles, channels)`.
    sum: Vec<Tile<T>>,
}

impl<T: SpectrumFloat> Transforms<T> {
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    fn new(layout: Layout) -> Self {
        let forward = RealFftPlanner::<f64>::new().plan_fft_forward(layout.fft_size);
        let inverse = RealFftPlanner::<T>::new().plan_fft_inverse(layout.fft_size);
        Self {
            forward_frame: forward.make_input_vec(),
            forward_scratch: forward.make_scratch_vec(),
            spectrum: vec![Complex::new(0.0, 0.0); layout.input_bins],
            inverse_bins: inverse.make_input_vec(),
            inverse_frame: inverse.make_output_vec(),
            inverse_scratch: inverse.make_scratch_vec(),
            forward,
            inverse,
            scale: T::narrow(1.0) / T::from_len(layout.fft_size),
            sum: vec![Tile::zero(); layout.slot_tiles],
        }
    }

    /// Step 1 for every input channel: the spectrum of `[history, input]`; `history` becomes
    /// the input.
    fn transform_input(
        &mut self,
        layout: Layout,
        history: &mut [f64],
        input: &[f64],
    ) -> Result<(), ConvolverError> {
        let block = layout.fft_size / 2;
        let channels = layout.block_samples / block;
        let channel_spectra = self.spectrum.chunks_exact_mut(layout.bins);
        for (c, (previous, spectrum)) in history
            .chunks_exact_mut(block)
            .zip(channel_spectra)
            .enumerate()
        {
            let Some((older, newer)) = self.forward_frame.split_at_mut_checked(block) else {
                return Err(ConvolverError::Transform);
            };
            for (sample, &old) in older.iter_mut().zip(previous.iter()) {
                *sample = old;
            }
            for ((sample, kept), frame) in newer
                .iter_mut()
                .zip(previous.iter_mut())
                .zip(input.chunks_exact(channels))
            {
                let value = frame.get(c).copied().unwrap_or(0.0);
                *sample = value;
                *kept = value;
            }
            // realfft uses the frame as scratch; it is refilled for every channel.
            self.forward
                .process_with_scratch(&mut self.forward_frame, spectrum, &mut self.forward_scratch)
                .map_err(|_| ConvolverError::Transform)?;
        }
        Ok(())
    }

    /// Step 2: `self.sum` = the prototype's `_convolve(h, fdl, head)`, one row (one tile of one
    /// channel) at a time ([`row_sum`]). With a shared input, two channels' rows go together, so
    /// that the delay line's row is read once for both ([`shared_row_sums`]).
    fn convolve(&mut self, ir: &IrState<T>, head: usize, layout: Layout) {
        let partitions = ir.partitions;
        let first = head.saturating_add(1);
        // `partitions` and the channel counts are at least 1, so no chunk size is zero.
        let spectra = ir
            .spectra
            .chunks_exact(partitions.saturating_mul(layout.channels));
        let delayed = ir
            .delay_line
            .chunks_exact(partitions.saturating_mul(layout.inputs));
        let sums = self.sum.chunks_exact_mut(layout.channels);
        for ((h_rows, x_rows), sums) in spectra.zip(delayed).zip(sums) {
            let h_rows = h_rows.chunks_exact(partitions);
            match layout.input {
                InputMode::PerChannel => {
                    let rows = h_rows.zip(x_rows.chunks_exact(partitions));
                    for ((h, x), sum) in rows.zip(sums) {
                        *sum = row_sum(h, x, first);
                    }
                }
                InputMode::Shared => {
                    // `x_rows` is the one row of the shared input.
                    let mut rows = h_rows.zip(sums);
                    while let Some((h, sum)) = rows.next() {
                        if let Some((h_next, sum_next)) = rows.next() {
                            (*sum, *sum_next) = shared_row_sums(h, h_next, x_rows, first);
                        } else {
                            *sum = row_sum(h, x_rows, first);
                        }
                    }
                }
            }
        }
    }

    /// Step 3: the inverse transform of `self.sum`, its last `b` samples into the interleaved
    /// block `out`.
    fn inverse_into(&mut self, layout: Layout, out: &mut [f64]) -> Result<(), ConvolverError> {
        let block = layout.fft_size / 2;
        let channels = layout.block_samples / block;
        let zero = T::narrow(0.0);
        for c in 0..layout.channels {
            // Channel `c`'s tiles of the `(tiles, channels)` sum; `channels` is not zero.
            let tiles = self.sum.iter().skip(c).step_by(layout.channels);
            // `LANES` is not zero, and the padding lanes of the last tile are not read.
            for (bins, tile) in self.inverse_bins.chunks_mut(LANES).zip(tiles) {
                for ((bin, &re), &im) in bins.iter_mut().zip(&tile.re).zip(&tile.im) {
                    *bin = Complex::new(re, im);
                }
            }
            // NumPy's irfft ignores the imaginary parts of the DC and Nyquist bins; realfft
            // would ignore them too but report an error.
            if let Some(dc) = self.inverse_bins.first_mut() {
                dc.im = zero;
            }
            if let Some(nyquist) = self.inverse_bins.last_mut() {
                nyquist.im = zero;
            }
            self.inverse
                .process_with_scratch(
                    &mut self.inverse_bins,
                    &mut self.inverse_frame,
                    &mut self.inverse_scratch,
                )
                .map_err(|_| ConvolverError::Transform)?;
            let samples = self.inverse_frame.iter().skip(block);
            for (frame, &sample) in out.chunks_exact_mut(channels).zip(samples) {
                if let Some(y) = frame.get_mut(c) {
                    *y = (sample * self.scale).widen();
                }
            }
        }
        Ok(())
    }
}

/// Uniformly partitioned overlap-save convolver: the prototype's `PartitionedConvolver`.
/// Channel `c` of the input is convolved with channel `c` of the impulse response. Without a
/// response it outputs silence.
pub struct PartitionedConvolver<T: SpectrumFloat> {
    block_size: usize,
    channels: usize,
    layout: Layout,
    transforms: Transforms<T>,
    /// The previous block per input channel, `(inputs, block)` (the prototype's `_prev`).
    history: Vec<f64>,
    current: Option<IrState<T>>,
    /// A swap that crossfades in the next block.
    pending: Option<IrState<T>>,
    /// The response a finished crossfade displaced, kept until [`Self::install`] or
    /// [`Self::take_retired`] hands it out (it must not be dropped on the real-time thread).
    retired: Option<IrState<T>>,
    /// Delay-line slot of the newest spectrum.
    head: usize,
    /// `linspace(0, 1, b)`.
    fade: Vec<f64>,
    /// `1 − fade`.
    fade_complement: Vec<f64>,
    /// The new response's output during a crossfade, interleaved.
    crossfade_out: Vec<f64>,
}

impl<T: SpectrumFloat> fmt::Debug for PartitionedConvolver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PartitionedConvolver")
            .field("block_size", &self.block_size)
            .field("channels", &self.channels)
            .field("partitions", &self.partitions())
            .field("pending", &self.pending.is_some())
            .field("head", &self.head)
            .finish_non_exhaustive()
    }
}

impl<T: SpectrumFloat> PartitionedConvolver<T> {
    /// A convolver for blocks of `block_size` frames with `channels` interleaved channels and
    /// [`InputMode::PerChannel`], without an impulse response.
    ///
    /// Setup: plans the FFTs and allocates every buffer, so it runs off the real-time thread.
    ///
    /// # Errors
    ///
    /// [`ConvolverError::Layout`] when `block_size` or `channels` is zero.
    pub fn new(block_size: usize, channels: usize) -> Result<Self, ConvolverError> {
        Self::with_input(block_size, channels, InputMode::PerChannel)
    }

    /// [`PartitionedConvolver::new`] with the input mode `input`.
    ///
    /// Setup: plans the FFTs and allocates every buffer, so it runs off the real-time thread.
    ///
    /// # Errors
    ///
    /// [`ConvolverError::Layout`] when `block_size` or `channels` is zero.
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    pub fn with_input(
        block_size: usize,
        channels: usize,
        input: InputMode,
    ) -> Result<Self, ConvolverError> {
        let layout = Layout::new(block_size, channels, input)?;
        let mut fade = vec![0.0; block_size];
        linspace(0.0, 1.0, &mut fade);
        let fade_complement = fade.iter().map(|f| 1.0 - f).collect();
        Ok(Self {
            block_size,
            channels,
            layout,
            transforms: Transforms::new(layout),
            history: vec![0.0; layout.inputs.saturating_mul(block_size)],
            current: None,
            pending: None,
            retired: None,
            head: 0,
            fade,
            fade_complement,
            crossfade_out: vec![0.0; layout.block_samples],
        })
    }

    /// Frames per block.
    #[must_use]
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Interleaved channels.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Partitions of the current impulse response, 0 without one (the prototype's
    /// `partitions`; a pending swap does not count until its crossfade block).
    #[must_use]
    pub fn partitions(&self) -> usize {
        self.current.as_ref().map_or(0, |ir| ir.partitions)
    }

    /// Prepares and installs `ir`, an interleaved `(taps, ir_channels)` response: the
    /// prototype's `set_ir(ir, crossfade)`. Displaced responses are dropped here.
    ///
    /// Setup: allocates and frees, so it runs off the real-time thread. On the real-time
    /// thread, prepare the response elsewhere with [`PreparedIr::new`] and call
    /// [`install`](Self::install).
    ///
    /// # Errors
    ///
    /// The errors of [`PreparedIr::new`]; the convolver is then unchanged.
    pub fn set_ir(
        &mut self,
        ir: &[f64],
        ir_channels: usize,
        crossfade: bool,
    ) -> Result<(), ConvolverError> {
        let prepared = PreparedIr::with_input(
            self.block_size,
            self.channels,
            self.layout.input,
            ir,
            ir_channels,
        )?;
        match self.install(prepared, crossfade) {
            Ok(retired) => drop(retired),
            // The response was prepared with this convolver's layout.
            Err(mismatch) => drop(mismatch),
        }
        Ok(())
    }

    /// Moves a prepared response in, without allocating or freeing.
    ///
    /// With `crossfade` and a current response, the new one is seeded with the input history
    /// and fades in over the next block; otherwise it replaces the current one at once with an
    /// empty delay line. A previous pending swap is replaced either way.
    ///
    /// # Errors
    ///
    /// [`LayoutMismatch`], carrying `ir` back, when it was prepared for another block size,
    /// channel count or input mode; the convolver is then unchanged.
    pub fn install(
        &mut self,
        ir: PreparedIr<T>,
        crossfade: bool,
    ) -> Result<Retired<T>, LayoutMismatch<T>> {
        if ir.block_size != self.block_size
            || ir.channels != self.channels
            || ir.input != self.layout.input
        {
            return Err(LayoutMismatch(ir));
        }
        let finished = self.retired.take();
        let mut new = ir.state;
        let (displaced_current, displaced_pending) = match self.current.as_ref() {
            Some(current) if crossfade => {
                seed(&mut new, current, self.head);
                (None, self.pending.replace(new))
            }
            _ => {
                self.head = 0;
                (self.current.replace(new), self.pending.take())
            }
        };
        Ok(Retired {
            states: [finished, displaced_current, displaced_pending],
        })
    }

    /// Hands out a response that a finished crossfade displaced, if any.
    pub fn take_retired(&mut self) -> Retired<T> {
        Retired {
            states: [self.retired.take(), None, None],
        }
    }

    /// Clears the input history and the current delay line (the prototype's `reset`). A
    /// pending swap stays pending, as in the prototype.
    pub fn reset(&mut self) {
        if let Some(current) = self.current.as_mut() {
            current.delay_line.fill(Tile::zero());
        }
        self.history.fill(0.0);
        self.head = 0;
    }

    /// Convolves one interleaved block of `block_size` frames from `input` into `out`.
    ///
    /// Returns which NumPy dtype the prototype's output had for this block (see
    /// [`OutputPrecision`]). Without an impulse response `out` is silence and the history is not
    /// updated, as in the prototype.
    ///
    /// # Errors
    ///
    /// [`ConvolverError::BlockSize`] when `input` or `out` does not hold exactly one block;
    /// nothing is processed. [`ConvolverError::Transform`] cannot occur with the buffers sized
    /// in setup.
    pub fn process(
        &mut self,
        input: &[f64],
        out: &mut [f64],
    ) -> Result<OutputPrecision, ConvolverError> {
        let layout = self.layout;
        if input.len() != layout.block_samples || out.len() != layout.block_samples {
            return Err(ConvolverError::BlockSize {
                expected: layout.block_samples,
                input: input.len(),
                output: out.len(),
            });
        }
        let Some(current) = self.current.as_mut() else {
            out.fill(0.0);
            return Ok(OutputPrecision::Double);
        };
        self.transforms
            .transform_input(layout, &mut self.history, input)?;
        // `partitions >= 1` by construction, so the remainder exists.
        self.head = self
            .head
            .saturating_add(1)
            .checked_rem(current.partitions)
            .unwrap_or(0);
        write_slot(current, self.head, layout, &self.transforms.spectrum);
        self.transforms.convolve(current, self.head, layout);
        self.transforms.inverse_into(layout, out)?;
        let Some(mut next) = self.pending.take() else {
            return Ok(T::OUTPUT);
        };
        write_slot(&mut next, 0, layout, &self.transforms.spectrum);
        self.transforms.convolve(&next, 0, layout);
        self.transforms
            .inverse_into(layout, &mut self.crossfade_out)?;
        let fades = self.fade.iter().zip(&self.fade_complement);
        let frames = out
            .chunks_exact_mut(self.channels)
            .zip(self.crossfade_out.chunks_exact(self.channels));
        for ((old, new), (&fade, &keep)) in frames.zip(fades) {
            for (y, &y_new) in old.iter_mut().zip(new) {
                *y = *y * keep + y_new * fade;
            }
        }
        // `install` empties `retired` before it sets `pending`, so nothing is dropped here.
        self.retired = self.current.replace(next);
        self.head = 0;
        Ok(OutputPrecision::Double)
    }
}

/// The einsum of one row, tile `t` of one channel: `h` is the row of its spectra (partitions
/// ascending) and `x` the row of the delay line that feeds it (slots ascending). Partitions
/// `0..first` take slots `first − 1, …, 0`, the rest take slots `P − 1, …, first` into a second
/// sum from zero, and the two sums are added: the prototype's `_convolve` with
/// `first = head + 1`, its running sum in registers.
///
/// Not inlined (PERF-01, measured): inside its caller LLVM may vectorize the loop across
/// partitions with in-order scalar sums, which is exact but about three times slower than the
/// vector code over a tile's lanes that it emits here.
#[inline(never)]
fn row_sum<T: SpectrumFloat>(h: &[Tile<T>], x: &[Tile<T>], first: usize) -> Tile<T> {
    let mut sum = Tile::zero();
    let (Some((h_first, h_rest)), Some((x_first, x_rest))) =
        (h.split_at_checked(first), x.split_at_checked(first))
    else {
        // `first <= partitions`, so this does not happen.
        return sum;
    };
    for (h, x) in h_first.iter().zip(x_first.iter().rev()) {
        sum.accumulate(h, x);
    }
    if !h_rest.is_empty() {
        let mut tail = Tile::zero();
        for (h, x) in h_rest.iter().zip(x_rest.iter().rev()) {
            tail.accumulate(h, x);
        }
        sum.add(&tail);
    }
    sum
}

/// [`row_sum`] of two rows fed by the same delay-line row `x` (a shared input), in one pass
/// over `x`. Each sum takes exactly the operations [`row_sum`] does. Not inlined, for the reason
/// [`row_sum`] gives.
#[inline(never)]
fn shared_row_sums<T: SpectrumFloat>(
    h: &[Tile<T>],
    h_next: &[Tile<T>],
    x: &[Tile<T>],
    first: usize,
) -> (Tile<T>, Tile<T>) {
    let mut sum = Tile::zero();
    let mut sum_next = Tile::zero();
    let (Some((h_first, h_rest)), Some((next_first, next_rest)), Some((x_first, x_rest))) = (
        h.split_at_checked(first),
        h_next.split_at_checked(first),
        x.split_at_checked(first),
    ) else {
        // `first <= partitions`, so this does not happen.
        return (sum, sum_next);
    };
    for ((h, h_next), x) in h_first.iter().zip(next_first).zip(x_first.iter().rev()) {
        sum.accumulate(h, x);
        sum_next.accumulate(h_next, x);
    }
    if !h_rest.is_empty() {
        let mut tail = Tile::zero();
        let mut tail_next = Tile::zero();
        for ((h, h_next), x) in h_rest.iter().zip(next_rest).zip(x_rest.iter().rev()) {
            tail.accumulate(h, x);
            tail_next.accumulate(h_next, x);
        }
        sum.add(&tail);
        sum_next.add(&tail_next);
    }
    (sum, sum_next)
}

/// `fdl[slot] = spectrum`, rounded to `T` (NumPy's assignment into a `complex64` array), kept as
/// its conjugate (see [`IrState::delay_line`]).
fn write_slot<T: SpectrumFloat>(
    ir: &mut IrState<T>,
    slot: usize,
    layout: Layout,
    spectrum: &[Complex<f64>],
) {
    // `partitions` and the counts are at least 1, so no chunk size is zero.
    let rows = ir
        .delay_line
        .chunks_exact_mut(ir.partitions.saturating_mul(layout.inputs));
    for (t, tile_rows) in rows.enumerate() {
        let inputs = tile_rows.chunks_exact_mut(ir.partitions);
        for (row, bins) in inputs.zip(spectrum.chunks_exact(layout.bins)) {
            let values = bins.chunks(LANES).nth(t).unwrap_or(&[]);
            if let Some(tile) = row.get_mut(slot) {
                tile.set(values, true);
            }
        }
    }
}

/// The prototype's seeding of a new delay line from the old one:
/// `new[(-1 - k) % P_new] = old[(head - k) % P_old]` for `k < min(P_new - 1, P_old)`, row by
/// row (both lines have the same rows).
fn seed<T: SpectrumFloat>(new: &mut IrState<T>, old: &IrState<T>, head: usize) {
    let shared = new.partitions.saturating_sub(1).min(old.partitions);
    // Both partition counts are at least 1.
    let rows = new
        .delay_line
        .chunks_exact_mut(new.partitions)
        .zip(old.delay_line.chunks_exact(old.partitions));
    for (to, from) in rows {
        for k in 0..shared {
            // `k < old.partitions` and `head < old.partitions`, so neither step wraps.
            let source = head
                .saturating_add(old.partitions - k)
                .checked_rem(old.partitions)
                .unwrap_or(0);
            let target = new.partitions - 1 - k;
            if let (Some(dst), Some(src)) = (to.get_mut(target), from.get(source)) {
                *dst = *src;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use realfft::num_complex::Complex;

    use super::{
        ConvolverError, InputMode, LANES, Layout, OutputPrecision, PartitionedConvolver,
        PreparedIr, SpectrumFloat, Tile, Transforms, write_slot,
    };

    /// Direct convolution of channel `c`, the reference for the overlap-save result.
    fn direct(x: &[f64], h: &[f64], channels: usize, ir_channels: usize, c: usize) -> Vec<f64> {
        let source = if ir_channels == 1 { 0 } else { c };
        let column = |v: &[f64], stride: usize, at: usize| -> Vec<f64> {
            v.chunks_exact(stride)
                .filter_map(|f| f.get(at).copied())
                .collect()
        };
        let (x, h) = (column(x, channels, c), column(h, ir_channels, source));
        (0..x.len())
            .map(|n| {
                let past = x.iter().take(n + 1).rev();
                past.zip(&h).map(|(a, b)| a * b).sum()
            })
            .collect()
    }

    fn channel(y: &[f64], channels: usize, c: usize) -> impl Iterator<Item = f64> + '_ {
        y.iter().skip(c).step_by(channels).copied()
    }

    fn signal(frames: usize, channels: usize, seed: f64) -> Vec<f64> {
        (0..frames * channels)
            .map(|i| (f64::from(u32::try_from(i).unwrap()) * 0.37 + seed).sin() * 0.5)
            .collect()
    }

    fn run<T: SpectrumFloat>(
        conv: &mut PartitionedConvolver<T>,
        x: &[f64],
    ) -> (Vec<f64>, Vec<OutputPrecision>) {
        let block = conv.block_size() * conv.channels();
        let mut out: Vec<f64> = x.iter().map(|_| 0.0).collect();
        let precisions = x
            .chunks_exact(block)
            .zip(out.chunks_exact_mut(block))
            .map(|(xb, yb)| conv.process(xb, yb).unwrap())
            .collect();
        (out, precisions)
    }

    fn assert_close(actual: impl Iterator<Item = f64>, expected: &[f64], tolerance: f64) {
        let mut count = 0;
        for (n, (a, e)) in actual.zip(expected).enumerate() {
            assert!((a - e).abs() < tolerance, "frame {n}: {a} != {e}");
            count += 1;
        }
        assert_eq!(count, expected.len());
    }

    #[test]
    fn equals_direct_convolution_for_uneven_ir_lengths() {
        let block = 16;
        let x = signal(block * 12, 2, 0.1);
        // Up to 11 partitions: several partitions in both halves of the sum.
        for taps in [1, 5, 16, 17, 40, 150, 176] {
            let h = signal(taps, 2, 2.0);
            let mut conv = PartitionedConvolver::<f64>::new(block, 2).unwrap();
            conv.set_ir(&h, 2, true).unwrap();
            assert_eq!(conv.partitions(), taps.div_ceil(block));
            let (y, precisions) = run(&mut conv, &x);
            assert!(precisions.iter().all(|p| *p == OutputPrecision::Double));
            for c in 0..2 {
                assert_close(channel(&y, 2, c), &direct(&x, &h, 2, 2, c), 1e-12);
            }
        }
    }

    #[test]
    fn single_precision_rounds_the_output_to_f32() {
        let block = 8;
        let x = signal(block * 6, 2, 0.4);
        let h = signal(13, 1, 1.0);
        let mut conv = PartitionedConvolver::<f32>::new(block, 2).unwrap();
        conv.set_ir(&h, 1, false).unwrap();
        let (y, precisions) = run(&mut conv, &x);
        assert!(precisions.iter().all(|p| *p == OutputPrecision::Single));
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the test checks f32 values"
        )]
        let representable = y
            .iter()
            .all(|v| f64::from(*v as f32).to_bits() == v.to_bits());
        assert!(representable);
        for c in 0..2 {
            assert_close(channel(&y, 2, c), &direct(&x, &h, 2, 1, c), 1e-5);
        }
    }

    #[test]
    fn a_crossfaded_swap_blends_over_one_block() {
        let block = 8;
        let input = signal(block * 8, 1, 0.2);
        let first_ir = signal(20, 1, 3.0);
        let second_ir = signal(11, 1, 4.0);
        let with_first = direct(&input, &first_ir, 1, 1, 0);
        let with_second = direct(&input, &second_ir, 1, 1, 0);

        let mut conv = PartitionedConvolver::<f64>::new(block, 1).unwrap();
        conv.set_ir(&first_ir, 1, false).unwrap();
        let swap_block = 4;
        let mut output = Vec::new();
        for (index, chunk) in input.chunks_exact(block).enumerate() {
            if index == swap_block {
                conv.set_ir(&second_ir, 1, true).unwrap();
                assert_eq!(conv.partitions(), 3, "the swap waits for the next block");
            }
            let mut out = [0.0; 8];
            assert_eq!(conv.process(chunk, &mut out), Ok(OutputPrecision::Double));
            output.extend(out);
        }
        assert_eq!(conv.partitions(), 2);
        assert!(!conv.take_retired().is_empty());
        let start = swap_block * block;
        let expected: Vec<f64> = with_first
            .iter()
            .zip(&with_second)
            .enumerate()
            .map(|(frame, (&old, &new))| match frame.checked_sub(start) {
                None => old,
                Some(offset) if offset < block => {
                    let fade = f64::from(u32::try_from(offset).unwrap()) / 7.0;
                    old * (1.0 - fade) + new * fade
                }
                // The seeded history makes the new response continue seamlessly.
                Some(_) => new,
            })
            .collect();
        assert_close(output.into_iter(), &expected, 1e-12);
    }

    #[test]
    fn reset_keeps_a_pending_swap_and_errors_leave_state() {
        let mut conv = PartitionedConvolver::<f64>::new(4, 2).unwrap();
        let mut out = [9.0; 8];
        assert_eq!(
            conv.process(&[1.0; 8], &mut out),
            Ok(OutputPrecision::Double)
        );
        assert_eq!(out, [0.0; 8], "silence without a response");
        assert_eq!(
            conv.process(&[1.0; 6], &mut out),
            Err(ConvolverError::BlockSize {
                expected: 8,
                input: 6,
                output: 8
            })
        );
        conv.set_ir(&[1.0, 1.0], 2, false).unwrap();
        conv.set_ir(&[0.5, 0.5], 2, true).unwrap();
        conv.reset();
        assert_eq!(
            conv.process(&[1.0; 8], &mut out),
            Ok(OutputPrecision::Double)
        );
        // The pending half-gain response fades in: the first frame is the old (unit)
        // response's, the last one the new response's.
        let [first, .., last] = out;
        assert!((first - 1.0).abs() < 1e-12 && (last - 0.5).abs() < 1e-12);
        assert_eq!(conv.partitions(), 1);
    }

    #[test]
    fn rejects_unusable_layouts_and_responses() {
        assert!(PartitionedConvolver::<f64>::new(0, 2).is_err());
        assert!(PartitionedConvolver::<f32>::new(4, 0).is_err());
        assert_eq!(
            PreparedIr::<f64>::new(4, 3, &[0.0; 4], 2).unwrap_err(),
            ConvolverError::IrChannels {
                ir_channels: 2,
                channels: 3
            }
        );
        assert_eq!(
            PreparedIr::<f64>::new(4, 2, &[0.0; 3], 2).unwrap_err(),
            ConvolverError::IrLength {
                samples: 3,
                ir_channels: 2
            }
        );
        assert_eq!(
            PreparedIr::<f64>::new(4, 2, &[], 2).unwrap().partitions(),
            1
        );
        let foreign = PreparedIr::<f64>::new(8, 2, &[1.0; 4], 2).unwrap();
        let mut conv = PartitionedConvolver::<f64>::new(4, 2).unwrap();
        let back = conv.install(foreign, false).unwrap_err();
        assert_eq!(back.0.partitions(), 1);
        assert_eq!(conv.partitions(), 0);
    }

    /// A value of `T` for comparing bits.
    fn bits<T: SpectrumFloat>(value: T) -> u64 {
        value.widen().to_bits()
    }

    /// The tiled kernel against the plain einsum loop in the prototype's form (the previous
    /// implementation), bit for bit, for every head (so both halves at every split),
    /// both input modes, an odd channel count and padded tiles (RT-11).
    fn tiled_sum_equals_plain_einsum<T: SpectrumFloat>() {
        let cases = [
            (16, 2, InputMode::PerChannel, 16 * 11),
            (16, 2, InputMode::Shared, 16 * 13 + 3),
            (7, 3, InputMode::PerChannel, 7 * 9),
            (8, 1, InputMode::PerChannel, 8 * 6 - 1),
        ];
        for (block, channels, input, taps) in cases {
            let layout = Layout::new(block, channels, input).unwrap();
            let ir = signal(taps, channels, 0.3);
            let mut prepared =
                PreparedIr::<T>::with_input(block, channels, input, &ir, channels).unwrap();
            let partitions = prepared.partitions();
            assert!(partitions >= 6);
            // A spectrum per delay-line slot; zeros and negative zeros included.
            let spectra: Vec<Vec<Complex<f64>>> = (0..partitions)
                .map(|slot| {
                    let values = signal(
                        2 * layout.input_bins,
                        1,
                        0.7 * f64::from(u32::try_from(slot).unwrap()),
                    );
                    let (pairs, _) = values.as_chunks::<2>();
                    pairs
                        .iter()
                        .enumerate()
                        .map(|(i, &[re, im])| match i % 29 {
                            3 => Complex::new(0.0, -0.0),
                            11 => Complex::new(-0.0, im),
                            _ => Complex::new(re, im),
                        })
                        .collect()
                })
                .collect();
            for (slot, spectrum) in spectra.iter().enumerate() {
                write_slot(&mut prepared.state, slot, layout, spectrum);
            }
            let lane = |tile: &Tile<T>, bin: usize| -> Complex<T> {
                let re = tile.re.get(bin % LANES).unwrap();
                Complex::new(*re, *tile.im.get(bin % LANES).unwrap())
            };
            let h = |p: usize, c: usize, bin: usize| -> Complex<T> {
                let index = ((bin / LANES) * channels + c) * partitions + p;
                lane(prepared.state.spectra.get(index).unwrap(), bin)
            };
            let x = |slot: usize, c: usize, bin: usize| -> Complex<T> {
                let input_channel = if input == InputMode::Shared { 0 } else { c };
                let spectrum = spectra.get(slot).unwrap();
                let v = spectrum.get(input_channel * layout.bins + bin).unwrap();
                Complex::new(T::narrow(v.re), T::narrow(v.im))
            };
            let mut transforms = Transforms::<T>::new(layout);
            for head in 0..partitions {
                transforms.convolve(&prepared.state, head, layout);
                for c in 0..channels {
                    for bin in 0..layout.bins {
                        // `_convolve`: `h[:head+1]·fdl[head::-1] + h[head+1:]·fdl[:head:-1]`.
                        let half = |pairs: &mut dyn Iterator<Item = (usize, usize)>| {
                            let mut acc = Complex::new(T::narrow(0.0), T::narrow(0.0));
                            for (p, slot) in pairs {
                                let (h, x) = (h(p, c, bin), x(slot, c, bin));
                                let re = h.re.fused_mul_add(x.re, -(h.im * x.im));
                                let im = h.re.fused_mul_add(x.im, h.im * x.re);
                                acc = Complex::new(re + acc.re, im + acc.im);
                            }
                            acc
                        };
                        let mut expected = half(&mut (0..=head).map(|p| (p, head - p)));
                        if head + 1 < partitions {
                            let tail = half(
                                &mut (head + 1..partitions).map(|p| (p, partitions + head - p)),
                            );
                            expected = Complex::new(expected.re + tail.re, expected.im + tail.im);
                        }
                        let tile = transforms.sum.get((bin / LANES) * channels + c).unwrap();
                        let actual = lane(tile, bin);
                        assert_eq!(
                            (bits(actual.re), bits(actual.im)),
                            (bits(expected.re), bits(expected.im)),
                            "{input:?} block {block}, head {head}, channel {c}, bin {bin}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_tiled_sum_equals_the_plain_einsum_bit_for_bit() {
        tiled_sum_equals_plain_einsum::<f32>();
        tiled_sum_equals_plain_einsum::<f64>();
    }

    /// The shared input gives the per-channel output bit for bit when every channel carries the
    /// same signal, across swaps (prepared and crossfaded) and resets, and reads channel 0 only.
    fn shared_equals_per_channel<T: SpectrumFloat>(block: usize, channels: usize) {
        let mono = signal(block * 40, 1, 0.9);
        let responses = [
            signal(block * 9 + 5, channels, 1.5),
            signal(block * 4, 1, 2.5),
        ];
        let mut per_channel = PartitionedConvolver::<T>::new(block, channels).unwrap();
        let mut shared =
            PartitionedConvolver::<T>::with_input(block, channels, InputMode::Shared).unwrap();
        let mut expected: Vec<f64> = (0..block * channels).map(|_| 0.0).collect();
        let mut actual = expected.clone();
        for (index, chunk) in mono.chunks_exact(block).enumerate() {
            // A stereo response, a crossfade to a mono one, and back without a crossfade.
            if let Some(ir) = match index {
                0 | 27 => responses.first(),
                13 => responses.get(1),
                _ => None,
            } {
                let ir_channels = if index == 13 { 1 } else { channels };
                let crossfade = index != 27;
                per_channel.set_ir(ir, ir_channels, crossfade).unwrap();
                let prepared = PreparedIr::<T>::with_input(
                    block,
                    channels,
                    InputMode::Shared,
                    ir,
                    ir_channels,
                )
                .unwrap();
                drop(shared.install(prepared, crossfade).unwrap());
            }
            if index == 20 {
                per_channel.reset();
                shared.reset();
            }
            let same: Vec<f64> = chunk
                .iter()
                .flat_map(|&v| std::iter::repeat_n(v, channels))
                .collect();
            // Channel 0 only: the shared convolver never reads the others.
            let first_only: Vec<f64> = chunk
                .iter()
                .flat_map(|&v| std::iter::once(v).chain(std::iter::repeat_n(9.0, channels - 1)))
                .collect();
            let precision = per_channel.process(&same, &mut expected).unwrap();
            assert_eq!(shared.process(&first_only, &mut actual), Ok(precision));
            assert_eq!(shared.partitions(), per_channel.partitions());
            let to_bits = |v: &[f64]| v.iter().map(|s| s.to_bits()).collect::<Vec<_>>();
            assert_eq!(to_bits(&actual), to_bits(&expected), "block {index}");
        }
    }

    #[test]
    fn a_shared_input_equals_per_channel_inputs_with_the_same_signal() {
        shared_equals_per_channel::<f32>(16, 2);
        shared_equals_per_channel::<f64>(16, 2);
        shared_equals_per_channel::<f32>(7, 3);
    }

    #[test]
    fn a_response_for_another_input_mode_is_handed_back() {
        let prepared = PreparedIr::<f32>::new(8, 2, &[1.0; 4], 2).unwrap();
        let mut conv = PartitionedConvolver::<f32>::with_input(8, 2, InputMode::Shared).unwrap();
        let back = conv.install(prepared, false).unwrap_err();
        assert_eq!(back.0.partitions(), 1);
        assert_eq!(conv.partitions(), 0);
        assert!(back.to_string().contains("PerChannel"));
    }
}
