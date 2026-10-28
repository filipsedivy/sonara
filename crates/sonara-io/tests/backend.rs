//! The [`AudioBackend`] contract, checked on the real null-backend engine and on the mock
//! (TST-02, TST-03, TST-07): the same lifecycle assertions run against both.

use sonara_io::{AudioBackend, ContextKind, DeviceConfig, Engine, EngineError, MockBackend};
use sonara_rt::StreamConfig;

fn config() -> DeviceConfig {
    DeviceConfig {
        sample_rate: 48_000,
        buffer_frames: 256,
        stream: StreamConfig {
            output_channels: 2,
            input_channels: 0,
            block_frames: 512,
            ring_blocks: 8,
            capture_frames: 0,
        },
        output_device: None,
        input_device: None,
    }
}

/// The lifecycle every backend obeys.
fn exercise(backend: &mut dyn AudioBackend) {
    assert!(!backend.is_running());
    assert!(matches!(backend.devices(), Err(EngineError::NoContext)));
    assert!(matches!(
        backend.start(&config()),
        Err(EngineError::NoContext)
    ));
    backend.stop();
    backend.shutdown();

    backend
        .init_context(ContextKind::Null)
        .expect("the null backend is always available");
    backend
        .init_context(ContextKind::Null)
        .expect("the null backend is always available");
    assert_ne!(backend.backend_name(), "");
    assert!(!backend.is_running());

    let handles = backend.start(&config()).expect("a valid config starts");
    assert!(backend.is_running());
    assert_eq!(handles.format.sample_rate, 48_000);
    assert_eq!(handles.format.output_channels, 2);

    backend.stop();
    assert!(!backend.is_running());
    backend.stop();
    assert!(!handles.control.is_playing());

    // A bad config is an error, and leaves nothing running.
    let mut bad = config();
    bad.stream.output_channels = 0;
    assert!(matches!(backend.start(&bad), Err(EngineError::Config(_))));
    assert!(!backend.is_running());

    // Restart, then shut down while running: the context is released and may be recreated.
    let _again = backend.start(&config()).expect("a valid config starts");
    assert!(backend.is_running());
    backend.shutdown();
    assert!(!backend.is_running());
    assert!(matches!(backend.devices(), Err(EngineError::NoContext)));
    backend
        .init_context(ContextKind::Null)
        .expect("the null backend is always available");
    let _third = backend.start(&config()).expect("a valid config starts");
    assert!(backend.is_running());
}

#[test]
fn engine_obeys_the_lifecycle() {
    exercise(&mut Engine::new());
}

#[test]
fn mock_obeys_the_lifecycle() {
    exercise(&mut MockBackend::new());
}

#[test]
fn dropping_a_running_engine_releases_the_device() {
    let mut engine = Engine::new();
    engine
        .init_context(ContextKind::Null)
        .expect("the null backend is always available");
    let handles = engine.start(&config()).expect("a valid config starts");
    drop(engine);
    assert_eq!(handles.control.stats().underruns, 0);
    assert!(!handles.control.is_playing());
}

#[test]
fn mock_reports_a_scripted_start_failure_once_and_stops_the_old_stream() {
    let mut mock = MockBackend::new();
    mock.init_context(ContextKind::Null)
        .expect("the null backend is always available");
    let _stream = mock.start(&config()).expect("a valid config starts");
    mock.fail_next_start(EngineError::NoContext);
    assert!(matches!(mock.start(&config()), Err(EngineError::NoContext)));
    assert!(!mock.is_running());
    assert_eq!(mock.closed_streams(), 1);
    assert!(mock.start(&config()).is_ok());
}
