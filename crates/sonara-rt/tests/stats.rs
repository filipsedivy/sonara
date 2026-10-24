//! The stats atomics: every key of the prototype's `stats()` (`native/src/engine.cpp`), peaks
//! that reset only on a `stats()` read, and the device notifications.
#![cfg(not(loom))]

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use sonara_rt::{BLOCK_FRAMES, Generation, LatencyFrames, Stream, StreamConfig, TimelineFrame};

/// RT-02: the calls of the real-time threads (the device callback, the render thread's producer,
/// the stream's atomics) run inside `assert_no_alloc`, which aborts on any allocation or free.
#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

fn stream(output_channels: usize, input_channels: usize) -> Stream {
    Stream::new(&StreamConfig {
        output_channels,
        input_channels,
        block_frames: BLOCK_FRAMES,
        ring_blocks: 4,
        capture_frames: 1024,
    })
    .expect("a valid configuration")
}

#[test]
fn peaks_reset_on_read() {
    let stream = stream(2, 0);
    let block: Vec<f32> = [0.25, -0.125].repeat(256);
    assert_no_alloc(|| stream.callback.meter_output(&block));
    assert_eq!(
        assert_no_alloc(|| stream.control.stats()).peak.as_slice(),
        [0.25, 0.125]
    );
    assert_eq!(
        assert_no_alloc(|| stream.control.stats()).peak.as_slice(),
        [0.0, 0.0]
    );
    // The maximum since the previous read, over several callbacks.
    assert_no_alloc(|| stream.callback.meter_output(&[0.1, 0.0, -0.3, 0.0]));
    assert_no_alloc(|| stream.callback.meter_output(&[0.2, 0.05, 0.0, 0.0]));
    assert_eq!(
        assert_no_alloc(|| stream.control.stats()).peak.as_slice(),
        [0.3, 0.05]
    );
}

/// The producer's own queries and the other non-resetting reads leave the peaks
/// alone; only `stats()` resets them.
#[test]
fn peaks_reset_only_on_stats_read() {
    let mut stream = stream(2, 1);
    assert_no_alloc(|| {
        stream
            .producer
            .push_block(&[0.0; 64], TimelineFrame::new(0), Generation::new(1))
    })
    .unwrap();
    assert_no_alloc(|| stream.callback.meter_output(&[0.5, -0.5]));
    assert_no_alloc(|| stream.callback.meter_input(&[0.75]));
    for _ in 0..3 {
        let _ = assert_no_alloc(|| stream.producer.queued_frames());
        let _ = assert_no_alloc(|| stream.producer.free_blocks());
        let _ = assert_no_alloc(|| stream.control.queued_frames());
        let _ = assert_no_alloc(|| stream.control.position());
        let _ = assert_no_alloc(|| stream.control.latency());
        let _ = assert_no_alloc(|| stream.control.is_playing());
    }
    let stats = assert_no_alloc(|| stream.control.stats());
    assert_eq!(stats.peak.as_slice(), [0.5, 0.5]);
    assert_eq!(stats.input_peak.as_slice(), [0.75]);
    let stats = assert_no_alloc(|| stream.control.stats());
    assert_eq!(stats.peak.as_slice(), [0.0, 0.0]);
    assert_eq!(stats.input_peak.as_slice(), [0.0]);
}

#[test]
fn peaks_skip_nan_as_std_max_does() {
    let stream = stream(2, 0);
    assert_no_alloc(|| {
        stream
            .callback
            .meter_output(&[f32::NAN, -0.7, 0.2, f32::NAN, -0.1, 0.0]);
    });
    assert_eq!(
        assert_no_alloc(|| stream.control.stats()).peak.as_slice(),
        [0.2, 0.7]
    );
    assert_no_alloc(|| stream.callback.meter_output(&[f32::NAN, f32::NAN]));
    assert_eq!(
        assert_no_alloc(|| stream.control.stats()).peak.as_slice(),
        [0.0, 0.0]
    );
    assert_no_alloc(|| {
        stream
            .callback
            .meter_output(&[f32::INFINITY, f32::NEG_INFINITY]);
    });
    assert_eq!(
        assert_no_alloc(|| stream.control.stats()).peak.as_slice(),
        [f32::INFINITY, f32::INFINITY]
    );
}

#[test]
fn only_the_first_eight_channels_are_metered() {
    let stream = stream(10, 3);
    let frame: Vec<f32> = (1..=10_u8).map(|c| f32::from(c) / 16.0).collect();
    assert_no_alloc(|| stream.callback.meter_output(&frame));
    assert_no_alloc(|| stream.callback.meter_input(&[0.1, 0.2, 0.3]));
    let stats = assert_no_alloc(|| stream.control.stats());
    assert_eq!(stats.peak.as_slice(), &frame[..8]);
    assert_eq!(stats.input_peak.as_slice(), [0.1, 0.2, 0.3]);
}

#[test]
fn stats_report_every_key() {
    let mut stream = stream(2, 2);
    let fresh = assert_no_alloc(|| stream.control.stats());
    assert!(!fresh.running && !fresh.playing && !fresh.stopped_by_device);
    assert_eq!(fresh.timeline_frame, None);
    assert_eq!((fresh.sample_rate, fresh.buffer_frames), (0, 0));
    assert_eq!(fresh.input_peak.as_slice(), [0.0, 0.0]);

    stream.control.set_running(true);
    stream.control.set_device_format(48_000, 256);
    stream
        .control
        .set_latency(LatencyFrames::new(768), LatencyFrames::new(512));
    assert_no_alloc(|| stream.control.set_playing(true));
    assert_no_alloc(|| {
        stream
            .producer
            .push_block(&[0.5; 1024], TimelineFrame::new(0), Generation::new(1))
    })
    .unwrap();
    for _ in 0..3 {
        let mut out = [0.0; 512];
        assert_no_alloc(|| stream.callback.fill(&mut out));
        assert_no_alloc(|| stream.callback.meter_output(&out));
        assert_no_alloc(|| stream.callback.end_callback(256, 0.125));
    }
    stream.control.note_rerouted();
    stream.control.note_interruption();

    let stats = assert_no_alloc(|| stream.control.stats());
    assert!(stats.running && stats.playing && !stats.stopped_by_device);
    assert_eq!((stats.sample_rate, stats.buffer_frames), (48_000, 256));
    assert_eq!((stats.callbacks, stats.callback_frames), (3, 768));
    assert_eq!((stats.underruns, stats.overruns), (1, 0));
    assert_eq!(stats.queued_frames, 0);
    assert_eq!(stats.timeline_frame, Some(TimelineFrame::new(512)));
    assert_eq!(stats.latency_frames, LatencyFrames::new(768));
    assert_eq!(stats.input_latency_frames, LatencyFrames::new(512));
    assert_eq!(stats.peak.as_slice(), [0.5, 0.5]);
    assert_eq!(stats.callback_load.to_bits(), 0.125_f32.to_bits());
    assert_eq!((stats.device_errors, stats.rerouted), (1, 1));
    assert_eq!(
        assert_no_alloc(|| stream.control.latency()),
        (LatencyFrames::new(768), LatencyFrames::new(512))
    );
}

#[test]
fn a_device_stop_counts_only_while_running() {
    let stream = stream(2, 0);
    stream.control.note_device_stopped();
    let stats = assert_no_alloc(|| stream.control.stats());
    assert_eq!(stats.device_errors, 0);
    assert!(!stats.stopped_by_device);

    stream.control.set_running(true);
    stream.control.note_device_stopped();
    let stats = assert_no_alloc(|| stream.control.stats());
    assert_eq!(stats.device_errors, 1);
    assert!(stats.stopped_by_device);
}

#[test]
fn monitor_gain_is_clamped_like_the_prototype() {
    let stream = stream(2, 1);
    assert_eq!(
        assert_no_alloc(|| stream.callback.monitor_gain()).to_bits(),
        0.0_f32.to_bits()
    );
    assert_no_alloc(|| stream.control.set_monitor(0.5));
    assert_eq!(
        assert_no_alloc(|| stream.callback.monitor_gain()).to_bits(),
        0.5_f32.to_bits()
    );
    assert_no_alloc(|| stream.control.set_monitor(-1.0));
    assert_eq!(
        assert_no_alloc(|| stream.callback.monitor_gain()).to_bits(),
        0.0_f32.to_bits()
    );
    assert_no_alloc(|| stream.control.set_monitor(f32::NAN));
    assert_eq!(
        assert_no_alloc(|| stream.callback.monitor_gain()).to_bits(),
        0.0_f32.to_bits()
    );
}
