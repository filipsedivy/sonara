//! Microbenchmark of the sample player's block (PERF-01, PERF-08, QG-11).
//!
//! Every case plays the dense passage of `tests/support/dense_sampler.rs` in the player's block
//! of 512 frames (`sonara_rt::BLOCK_FRAMES`): the sampler's 64 voices on a two-patch set of 120
//! looping stereo regions, a held chord of 30 keys over two layers and a melody that changes
//! note every second block, while `dynamics` steps and the `articulation`, `mute` and `vibrato`
//! choices change inside blocks. The cases:
//!
//! - `sampler/plain`: the layers at full gain, the melody's notes attacking and releasing;
//! - `sampler/controllers`: the layers crossfading and changing level on the modulation wheel,
//!   which moves every block on each channel, behind a pedal gate;
//! - `sampler/legato`: the melody's notes joined by the overlap legato, each crossfading from
//!   the note before over 20 ms along the equal-power curve;
//! - `sampler/controllers_legato`: both.
//!
//! The measurement starts in steady state, after the warm-up and one period of the passage, and
//! each iteration plays the passage's next block, so a case's time is the mean over the
//! blocks of its eight-block period, with and without a melody note. The test
//! `a_dense_passage_allocates_nothing_and_holds_its_voices` in `tests/sampler.rs` plays the
//! same blocks under `assert_no_alloc` (RT-02).
//!
//! ```text
//! cargo bench -p sonara-dsp --bench sampler
//! ```

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

#[path = "../tests/support/dense_sampler.rs"]
mod dense_sampler;

use dense_sampler::{DensePassage, Load, PERIOD, WARM_UP};

fn sampler(c: &mut Criterion) {
    let mut group = c.benchmark_group("sampler");
    for load in Load::ALL {
        let mut passage = DensePassage::new(load);
        let mut block = 0;
        while block < WARM_UP + PERIOD {
            passage.play(block);
            block += 1;
        }
        group.bench_function(load.name(), |b| {
            b.iter(|| {
                passage.play(block);
                block += 1;
                black_box(passage.output());
            });
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(60);
    targets = sampler
}
criterion_main!(benches);
