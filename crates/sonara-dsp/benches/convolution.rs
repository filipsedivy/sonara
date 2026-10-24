//! Microbenchmark of the partitioned convolver's block (PERF-01, PERF-08, QG-11).
//!
//! Every case uses the player's block of 512 frames (`sonara_rt::BLOCK_FRAMES`) and runs in
//! steady state: the delay line is full of non-silent spectra before the measurement starts, so
//! no partition multiplies silence.
//!
//! - `reverb_f32_shared`: the reverb's single-precision stereo convolver as `sonara.reverb` runs
//!   it, with Ascension's response (`hall`, 7 s decay, 45 ms pre-delay: 510 720 taps, 998
//!   partitions) and a mono signal shared by both channels;
//! - `reverb_f32_per_channel`: the same with an input spectrum per channel;
//! - `cold/reverb_f32_shared`: `reverb_f32_shared` with the caches evicted before every block
//!   (only the block is timed). In the player the rest of the graph runs between two blocks of
//!   the reverb and evicts its spectra, so this is the closer model of real-time playback;
//! - `convolver_f64`: the convolver's double-precision stereo path with a 2 s stereo response
//!   (188 partitions) and a stereo input;
//! - `partitions_f32/<P>`: the shared reverb path with `P` partitions, for the cost per
//!   partition while the spectra fit the caches.
//!
//! ```text
//! cargo bench -p sonara-dsp --bench convolution
//! ```
//!
//! The responses are decaying deterministic noise, not the synthesized ones: the cost of a block
//! depends only on the partition count and the layout, not on the values.

use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use sonara_dsp::convolution::{InputMode, PartitionedConvolver, SpectrumFloat};

const BLOCK: usize = 512;
const SAMPLE_RATE: f64 = 48_000.0;
/// Bytes written between two cold blocks: more than the caches hold.
const EVICTION_BYTES: usize = 96 << 20;

/// A deterministic noise source (a 64-bit LCG), uniform in [−0.5, 0.5).
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let top = u32::try_from(self.0 >> 40).expect("24 bits fit u32");
        f64::from(top) / f64::from(1_u32 << 24) - 0.5
    }
}

/// An interleaved stereo response of `taps` frames: noise under an exponential decay.
fn response(taps: usize, decay_s: f64, seed: u64) -> Vec<f64> {
    let mut noise = Noise(seed);
    let mut ir = Vec::with_capacity(2 * taps);
    for frame in 0..taps {
        let t = f64::from(u32::try_from(frame).expect("taps fit u32")) / SAMPLE_RATE;
        let envelope = (-6.907_755 * t / decay_s).exp();
        ir.push(noise.next() * envelope);
        ir.push(noise.next() * envelope);
    }
    ir
}

/// `count` interleaved stereo blocks of noise, the same value on both channels when `mono`.
fn blocks(count: usize, mono: bool, seed: u64) -> Vec<Vec<f64>> {
    let mut noise = Noise(seed);
    (0..count)
        .map(|_| {
            let mut block = Vec::with_capacity(2 * BLOCK);
            for _ in 0..BLOCK {
                let left = noise.next();
                let right = if mono { left } else { noise.next() };
                block.push(left);
                block.push(right);
            }
            block
        })
        .collect()
}

/// A stereo convolver with `ir` whose delay line is full of non-silent spectra, and its input
/// blocks.
fn warmed<T: SpectrumFloat>(
    ir: &[f64],
    input: InputMode,
) -> (PartitionedConvolver<T>, Vec<Vec<f64>>) {
    let inputs = blocks(64, input == InputMode::Shared, 7);
    let mut conv = PartitionedConvolver::<T>::with_input(BLOCK, 2, input).expect("a valid layout");
    conv.set_ir(ir, 2, false).expect("a usable response");
    let mut out = vec![0.0; 2 * BLOCK];
    for block in inputs.iter().cycle().take(conv.partitions() + 1) {
        conv.process(block, &mut out).expect("a full block");
    }
    (conv, inputs)
}

/// Measures one block with the convolver's data as the previous blocks left it in the caches.
fn bench_block<T: SpectrumFloat>(c: &mut Criterion, name: &str, ir: &[f64], input: InputMode) {
    let (mut conv, inputs) = warmed::<T>(ir, input);
    let mut out = vec![0.0; 2 * BLOCK];
    let mut next = inputs.iter().cycle();
    c.bench_function(name, |b| {
        b.iter(|| {
            let block = next.next().expect("a cycle never ends");
            conv.process(black_box(block), &mut out)
                .expect("a full block");
            black_box(&out);
        });
    });
}

/// Measures one block after writing [`EVICTION_BYTES`], one store per 128-byte cache line, so
/// that the convolver's data comes from memory; only the block is timed.
fn bench_cold_block(c: &mut Criterion, name: &str, ir: &[f64], input: InputMode) {
    let (mut conv, inputs) = warmed::<f32>(ir, input);
    let mut out = vec![0.0; 2 * BLOCK];
    let mut next = inputs.iter().cycle();
    let mut eviction = vec![0_u64; EVICTION_BYTES / 8];
    let mut stamp = 0_u64;
    let mut group = c.benchmark_group("cold");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(2));
    group.bench_function(name, |b| {
        b.iter_custom(|iterations| {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iterations {
                stamp = stamp.wrapping_add(1);
                for word in eviction.iter_mut().step_by(16) {
                    *word = word.wrapping_add(stamp);
                }
                black_box(&eviction);
                let block = next.next().expect("a cycle never ends");
                let started = Instant::now();
                conv.process(black_box(block), &mut out)
                    .expect("a full block");
                elapsed += started.elapsed();
                black_box(&out);
            }
            elapsed
        });
    });
    group.finish();
}

fn convolution(c: &mut Criterion) {
    // Ascension's reverb: `n = int((7 · 1.5 + 0.045 + 0.05) · 48000)` frames after 2160
    // frames of pre-delay.
    let reverb_ir = response(508_560 + 2160, 7.0, 1);
    bench_block::<f32>(c, "reverb_f32_shared", &reverb_ir, InputMode::Shared);
    let per_channel = InputMode::PerChannel;
    bench_block::<f32>(c, "reverb_f32_per_channel", &reverb_ir, per_channel);
    bench_cold_block(c, "reverb_f32_shared", &reverb_ir, InputMode::Shared);
    let convolver_ir = response(96_000, 2.0, 2);
    bench_block::<f64>(c, "convolver_f64", &convolver_ir, per_channel);
    for partitions in [8_usize, 32, 128, 512] {
        let ir = response(partitions * BLOCK, 7.0, 3);
        let name = format!("partitions_f32/{partitions}");
        bench_block::<f32>(c, &name, &ir, InputMode::Shared);
    }
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(60);
    targets = convolution
}
criterion_main!(benches);
