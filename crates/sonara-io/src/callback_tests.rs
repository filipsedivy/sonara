//! The device callback driven without a device (TST-01, TST-10): the prototype's order of fill,
//! direct monitor, meters, capture and load (`Engine::process` in `native/src/engine.cpp`), and
//! the device notifications. The null device captures silence, so only these tests can check
//! the monitor mix and the meters after it.

use std::num::NonZeroUsize;
use std::time::Duration;

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use sonara_rt::{
    BLOCK_FRAMES, BlockProducer, CaptureReader, Generation, Stream, StreamConfig, StreamControl,
    TimelineFrame,
};

use crate::callback::{DeviceProcess, Notification, Period, load, notify};

/// RT-02: `assert_no_alloc` aborts on any allocation inside an `assert_no_alloc` block.
#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const PERIOD: usize = 256;
const RATE: u32 = 48_000;
const GEN1: Generation = Generation::new(1);

struct Harness {
    process: DeviceProcess,
    producer: BlockProducer,
    capture: CaptureReader,
    control: StreamControl,
    output_channels: usize,
    input_channels: usize,
}

fn harness(output_channels: usize, input_channels: usize) -> Harness {
    let Stream {
        producer,
        callback,
        capture,
        control,
    } = Stream::new(&StreamConfig {
        output_channels,
        input_channels,
        block_frames: BLOCK_FRAMES,
        ring_blocks: 4,
        capture_frames: 4096,
    })
    .unwrap();
    let process = DeviceProcess::new(
        callback,
        NonZeroUsize::new(output_channels).unwrap(),
        input_channels,
    );
    Harness {
        process,
        producer,
        capture,
        control,
        output_channels,
        input_channels,
    }
}

impl Harness {
    /// One callback of `PERIOD` frames with the stream's channel counts.
    fn run(&mut self, output: &mut [f32], input: Option<&[f32]>) {
        self.process.process(Period {
            output,
            output_channels: u32::try_from(self.output_channels).unwrap(),
            input,
            input_channels: u32::try_from(self.input_channels).unwrap(),
            frames: u32::try_from(PERIOD).unwrap(),
            sample_rate: RATE,
        });
    }

    fn push(&mut self, value: impl Fn(usize) -> f32, tag: i64) {
        let block: Vec<f32> = (0..BLOCK_FRAMES * self.output_channels)
            .map(value)
            .collect();
        self.producer
            .push_block(&block, TimelineFrame::new(tag), GEN1)
            .unwrap();
    }
}

/// A mono input at gain 1 reaches both output channels (`in[c % in_ch]`), and the output peaks
/// are measured after the monitor.
#[test]
fn monitor_duplicates_mono_input_and_output_peaks_follow_it() {
    let mut h = harness(2, 1);
    h.control.set_monitor(1.0);
    let input = vec![0.5_f32; PERIOD];
    let mut output = vec![0.0_f32; 2 * PERIOD];
    h.run(&mut output, Some(&input));

    assert_eq!(output, vec![0.5; 2 * PERIOD]);
    let stats = h.control.stats();
    assert_eq!(stats.peak.as_slice(), [0.5, 0.5]);
    assert_eq!(stats.input_peak.as_slice(), [0.5]);
    assert_eq!(stats.callbacks, 1);
    assert_eq!(stats.callback_frames, 256);
    assert!(stats.callback_load.is_finite() && stats.callback_load >= 0.0);
    // Not playing: the empty ring is no underrun.
    assert_eq!(stats.underruns, 0);
}

/// The monitor adds `in[c % in_ch] × gain` to exactly what the ring played, sample by sample,
/// through the fade-in and after it.
#[test]
fn monitor_adds_scaled_input_to_the_played_block() {
    let gain = 0.5;
    let mut reference = harness(4, 3);
    let mut monitored = harness(4, 3);
    monitored.control.set_monitor(gain);
    for h in [&mut reference, &mut monitored] {
        h.push(|i| f32::from(u16::try_from(i % 1000).unwrap()) / 4000.0, 0);
    }
    for period in 0..2_u16 {
        let input: Vec<f32> = (0..3 * u16::try_from(PERIOD).unwrap())
            .map(|i| f32::from(i + period) / 1000.0)
            .collect();
        let mut played = vec![0.0_f32; 4 * PERIOD];
        let mut mixed = vec![0.0_f32; 4 * PERIOD];
        reference.run(&mut played, Some(&input));
        monitored.run(&mut mixed, Some(&input));

        let expected: Vec<f32> = played
            .as_chunks::<4>()
            .0
            .iter()
            .zip(input.as_chunks::<3>().0)
            .flat_map(|(out, inp)| (0..4).map(move |c| out[c] + inp[c % 3] * gain))
            .collect();
        assert_eq!(mixed, expected, "period {period}");
    }
}

/// Without a monitor gain the input stays out of the output, but it is still metered and
/// captured from the first current-generation frame of the same callback.
#[test]
fn input_is_metered_and_captured_without_monitoring() {
    let mut h = harness(2, 2);
    h.push(|_| 0.25, 0);
    let take = h.control.arm_capture();
    let input: Vec<f32> = (0..PERIOD).flat_map(|_| [0.1_f32, -0.2]).collect();
    let mut output = vec![0.0_f32; 2 * PERIOD];
    h.run(&mut output, Some(&input));

    // The block fades in over 128 frames and then plays at 0.25; no input in it.
    assert!(output.iter().all(|sample| (0.0..=0.25).contains(sample)));
    assert_eq!(output[2 * 200], 0.25);
    let stats = h.control.stats();
    assert_eq!(stats.peak.as_slice(), [0.25, 0.25]);
    assert_eq!(stats.input_peak.as_slice(), [0.1, 0.2]);

    let mut captured = vec![0.0_f32; 2 * 1024];
    let chunk = h.capture.read(&mut captured);
    assert_eq!(chunk.frames, PERIOD);
    assert_eq!(
        chunk.take.map(|t| (t.id, t.start)),
        Some((take, TimelineFrame::new(0)))
    );
    assert_eq!(&captured[..2 * PERIOD], input.as_slice());
}

/// A period whose channel counts are not the stream's is left as miniaudio pre-silenced it and
/// is not counted (the shim requests the stream's counts, so this does not happen).
#[test]
fn a_period_of_other_channel_counts_is_left_alone() {
    let mut h = harness(2, 0);
    h.push(|_| 0.25, 0);
    let mut output = vec![7.0_f32; PERIOD];
    h.process.process(Period {
        output: &mut output,
        output_channels: 1,
        input: None,
        input_channels: 0,
        frames: u32::try_from(PERIOD).unwrap(),
        sample_rate: RATE,
    });
    assert_eq!(output, vec![7.0; PERIOD]);

    let input = vec![0.5_f32; PERIOD];
    let mut output = vec![7.0_f32; 2 * PERIOD];
    h.process.process(Period {
        output: &mut output,
        output_channels: 2,
        input: Some(&input),
        input_channels: 1,
        frames: u32::try_from(PERIOD).unwrap(),
        sample_rate: RATE,
    });
    assert_eq!(output, vec![7.0; 2 * PERIOD]);
    assert_eq!(h.control.stats().callbacks, 0);
}

/// The prototype's `callback_load`: elapsed time over `frames / max(1, sample_rate)` seconds.
#[test]
fn load_is_processing_time_over_the_period() {
    // 48 frames at 48 kHz last 1 ms.
    assert_eq!(load(Duration::from_millis(1), 48, 48_000), 1.0);
    assert_eq!(load(Duration::from_micros(500), 48, 48_000), 0.5);
    assert_eq!(
        load(Duration::from_secs(1), 2, 0),
        0.5,
        "a rate of 0 counts as 1"
    );
    assert_eq!(
        load(Duration::from_secs(1), 0, 48_000),
        0.0,
        "no frames, no load"
    );
}

/// Notifications only set the stats atomics (RT-08, RT-12): a device stop counts while the
/// stream runs, a reroute is counted, an interruption is a device error.
#[test]
fn notifications_set_the_device_counters() {
    let h = harness(2, 0);
    let control = &h.control;
    control.set_running(true);
    notify(control, Notification::Stopped);
    let stats = control.stats();
    assert!(stats.stopped_by_device);
    assert_eq!(stats.device_errors, 1);

    notify(control, Notification::Rerouted);
    notify(control, Notification::InterruptionBegan);
    for ignored in [
        Notification::Started,
        Notification::InterruptionEnded,
        Notification::Unlocked,
    ] {
        notify(control, ignored);
    }
    let stats = control.stats();
    assert_eq!(stats.rerouted, 1);
    assert_eq!(stats.device_errors, 2);

    // Our own stop clears `running` first: its notification is not a device error.
    control.set_running(false);
    notify(control, Notification::Stopped);
    assert_eq!(control.stats().device_errors, 2);
}

/// RT-02: the whole callback (fill, monitor, meters, capture, load) and the notifications
/// neither allocate nor free.
#[test]
fn the_callback_does_not_allocate() {
    let mut h = harness(2, 2);
    h.control.set_monitor(0.5);
    h.control.set_playing(true);
    let _take = h.control.arm_capture();
    let block = vec![0.25_f32; 2 * BLOCK_FRAMES];
    let input = vec![0.125_f32; 2 * PERIOD];
    let mut output = vec![0.0_f32; 2 * PERIOD];
    let mut captured = vec![0.0_f32; 2 * PERIOD];
    let mut tag = 0;
    assert_no_alloc(|| {
        for _ in 0..64 {
            while h.producer.free_blocks() > 0 {
                h.producer
                    .push_block(&block, TimelineFrame::new(tag), GEN1)
                    .unwrap();
                tag += 512;
            }
            h.run(&mut output, Some(&input));
            notify(&h.control, Notification::Rerouted);
            let _ = h.capture.read(&mut captured);
        }
    });
    let stats = h.control.stats();
    assert_eq!(stats.callbacks, 64);
    assert_eq!(stats.rerouted, 64);
    assert_eq!(stats.underruns, 0);
}
