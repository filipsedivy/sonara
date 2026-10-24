//! [M] The device layer on the host's real devices: Core Audio links, the
//! real devices are listed with the prototype's ids, and a stream's latency estimate is 768
//! frames (256 × 3).
//!
//! Hardware results stay local and are never a CI gate (HW-01), so the tests are ignored. Run
//! them on the macOS host with
//!
//! ```text
//! SONARA_PROTOTYPE_DEVICES=<file> cargo test -p sonara-io --test hardware -- --ignored --nocapture
//! ```
//!
//! where `<file>` holds the prototype's device list as JSON: the output of `sonara devices` or
//! of the prototype's `Engine.devices()` in the same shape (`{"backend", "devices": [{"id",
//! "name", "kind", "default", "channels"}]}`). Without it, the tests print the Rust list only.

use std::time::{Duration, Instant};

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use serde_json::{Value, json};
use sonara_io::{ContextKind, DeviceConfig, Engine, StreamHandles};
use sonara_rt::{BLOCK_FRAMES, StreamConfig};

/// RT-02: the stream's control calls run inside `assert_no_alloc`, which aborts on any
/// allocation or free. The device thread's callback is checked by RealtimeSanitizer
/// (`rust-rtsan`), which sees every thread.
#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

fn system_engine() -> Engine {
    let mut engine = Engine::new();
    engine
        .init_context(ContextKind::Default)
        .expect("the system's audio context");
    engine
}

/// The device list in the prototype's JSON shape (`DeviceInfo.to_json` without the always
/// empty `sample_rates`).
fn listing(engine: &mut Engine) -> Value {
    let devices: Vec<Value> = engine
        .devices()
        .expect("the device list")
        .iter()
        .map(|device| {
            json!({
                "id": device.id.to_string(),
                "name": device.name,
                "kind": device.kind.as_str(),
                "default": device.is_default,
                "channels": device.channels,
            })
        })
        .collect();
    json!({"backend": engine.backend_name(), "devices": devices})
}

/// The prototype's list, reduced to the keys `listing` has; `None` without
/// `SONARA_PROTOTYPE_DEVICES`.
fn prototype_listing() -> Option<Value> {
    let path = std::env::var_os("SONARA_PROTOTYPE_DEVICES")?;
    let text = std::fs::read_to_string(path).expect("the prototype's device list");
    let prototype: Value = serde_json::from_str(&text).expect("JSON");
    // `sonara devices` names the backend "native/<name>".
    let backend = prototype["backend"].as_str().expect("a backend name");
    let backend = backend.strip_prefix("native/").unwrap_or(backend);
    let devices: Vec<Value> = prototype["devices"]
        .as_array()
        .expect("a device array")
        .iter()
        .map(|device| {
            json!({
                "id": device["id"],
                "name": device["name"],
                "kind": device["kind"],
                "default": device["default"],
                "channels": device["channels"],
            })
        })
        .collect();
    Some(json!({"backend": backend, "devices": devices}))
}

#[test]
#[ignore = "hardware: lists the host's audio devices (HW-01)"]
fn real_devices_match_the_prototype() {
    let mut engine = system_engine();
    let listing = listing(&mut engine);
    println!("{}", serde_json::to_string_pretty(&listing).unwrap());
    if cfg!(target_os = "macos") {
        assert_eq!(engine.backend_name(), "Core Audio");
    }
    assert_ne!(listing["devices"], json!([]), "no device listed");
    // Every listed id survives its saved form.
    for device in engine.devices().unwrap() {
        assert_eq!(device.id.to_string().parse(), Ok(device.id));
    }
    match prototype_listing() {
        Some(prototype) => assert_eq!(listing, prototype),
        None => println!("SONARA_PROTOTYPE_DEVICES is not set: nothing compared"),
    }
}

fn playback(output_device: Option<String>) -> DeviceConfig {
    DeviceConfig {
        sample_rate: 48_000,
        buffer_frames: 256,
        stream: StreamConfig {
            output_channels: 2,
            input_channels: 0,
            block_frames: BLOCK_FRAMES,
            ring_blocks: 4,
            capture_frames: 0,
        },
        output_device,
        input_device: None,
    }
}

/// Opens the default output (it plays silence: nothing is pushed): (48000, 256, 3, …, 2) and a
/// latency estimate of 768 frames, 16 ms at 48 kHz, as the prototype on 2026-10-22. Then opens
/// each listed playback device by its id and prints what it obtained. An unknown id fails with
/// the prototype's text.
#[test]
#[ignore = "hardware: opens the host's audio devices (HW-01)"]
fn real_output_latency_is_768_frames() {
    let mut engine = system_engine();
    let StreamHandles {
        format, control, ..
    } = engine.start(&playback(None)).unwrap();
    println!("default output: {format:?}");
    assert_eq!(
        (format.sample_rate, format.buffer_frames, format.periods),
        (48_000, 256, 3)
    );
    assert_eq!(format.output_channels, 2);
    assert_eq!(assert_no_alloc(|| control.latency()).0.get(), 768);
    let end = Instant::now() + Duration::from_secs(3);
    while assert_no_alloc(|| control.stats()).callbacks < 8 && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(10));
    }
    let stats = assert_no_alloc(|| control.stats());
    println!(
        "callbacks {}, callback_frames {}, load {}",
        stats.callbacks, stats.callback_frames, stats.callback_load
    );
    assert!(stats.callbacks >= 8);
    assert_eq!(stats.callback_frames, stats.callbacks * 256);
    engine.stop();
    assert_eq!(assert_no_alloc(|| control.stats()).device_errors, 0);

    let outputs: Vec<_> = engine
        .devices()
        .unwrap()
        .into_iter()
        .filter(|device| device.kind == sonara_io::DeviceKind::Playback)
        .collect();
    for device in outputs {
        let StreamHandles {
            format, control, ..
        } = engine
            .start(&playback(Some(device.id.to_string())))
            .unwrap();
        // Compared by hand with the prototype's `start` on the same id: other devices may
        // obtain other periods.
        println!(
            "{} ({}): {format:?}, latency {}",
            device.name,
            device.id,
            assert_no_alloc(|| control.latency()).0.get()
        );
        engine.stop();
    }

    let error = engine
        .start(&playback(Some("0123".to_owned())))
        .err()
        .unwrap();
    assert_eq!(error.to_string(), "cannot open audio device: No device");
}
