//! The player's two seams over the application (DES-06): the
//! [`AudioDevice`] over `sonara-io`'s [`Engine`], and the [`ProjectSource`] over the session.
//! Not real-time code.

use std::any::Any;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use sonara_engine::player::{
    AudioDevice, DeviceError, DeviceStream, ProjectSnapshot, ProjectSource, RenderThreadStart,
    StreamRequest,
};
use sonara_io::{
    AudioWorkgroupError, ContextKind, DeviceConfig, DeviceId, DeviceInfo, Engine, ThreadQosError,
};
use sonara_rt::{BLOCK_FRAMES, StreamConfig};

use crate::media::MediaService;
use crate::session::SharedSession;
use crate::settings::AudioSettings;
use crate::sync::lock;

/// `create_backend(name)`: the engine of an audio backend, `Ok(None)` for `null`.
///
/// `native-null` opens miniaudio's null context (a simulated real-time device), every other
/// name the system's default context (`auto`, `native`).
///
/// # Errors
///
/// The context's error message when it cannot be created: an error for `native` and
/// `native-null` (`ENGINE_UNAVAILABLE` with that reason). For `auto` the failure is logged and
/// the transport runs without a backend, as in the prototype.
pub(crate) fn create_backend(name: &str) -> Result<Option<Engine>, String> {
    if name == "null" {
        return Ok(None);
    }
    let kind = if name == "native-null" {
        ContextKind::Null
    } else {
        ContextKind::Default
    };
    let mut engine = Engine::new();
    match engine.init_context(kind) {
        Ok(()) => Ok(Some(engine)),
        Err(error) if matches!(name, "native" | "native-null") => Err(error.to_string()),
        Err(error) => {
            log::info!("native audio backend unavailable: {error}");
            Ok(None)
        }
    }
}

/// A device as `sonara devices` and the settings dialog list it (`DeviceInfo.to_json`). The
/// device layer reports no sample rates, as the prototype's engine reported none.
#[must_use]
pub fn device_json(device: &DeviceInfo) -> Value {
    let rates: [u32; 0] = [];
    json!({
        "id": device.id.to_string(),
        "name": device.name,
        "kind": device.kind.as_str(),
        "default": device.is_default,
        "channels": device.channels,
        "sample_rates": rates,
    })
}

/// miniaudio's name of its null backend: a simulated device, opened by `native-null`, or by
/// `auto` on a machine without audio.
const NULL_BACKEND: &str = "Null";

/// The hook the player's render thread runs once before its loop, on a real device:
/// `None` on the null backend, whose simulated device has no deadline to meet.
///
/// The hook joins the thread to the output device's audio workgroup with a time-constraint
/// policy of one device period (`buffer_frames`), as Apple advises for an auxiliary audio
/// thread, and hands the membership back to the thread, which leaves the workgroup after its
/// loop ([`join_audio_workgroup`](sonara_io::thread::join_audio_workgroup)). The device is the
/// saved output device (`output_device`), or the system's default output device; the
/// workgroup belongs to the device, so the thread joins it before the stream opens.
///
/// Where it joins no workgroup (off macOS, without a device period or a valid device id, or when
/// the join fails), it requests the user-interactive `QoS` class instead, the class of work the
/// user waits on. macOS honours that class only when the process has an application role (the
/// app launched by Launch Services); a process started from a terminal keeps the default class
/// ([`raise_render_qos`](sonara_io::thread::raise_render_qos)).
///
/// A failure is logged and playback goes on at the thread's scheduling (ERR-06, RT-12): off
/// macOS at debug level, because no other system has workgroups or `QoS` classes.
pub(crate) fn render_thread_start(
    engine: &Engine,
    audio: &AudioSettings,
) -> Option<RenderThreadStart> {
    if engine.backend_name() == NULL_BACKEND {
        return None;
    }
    let target = WorkgroupTarget::of(audio);
    Some(Box::new(move || -> Option<Box<dyn Any>> {
        schedule_render_thread(target.as_ref())
    }))
}

/// The workgroup the render thread joins: the output device's, with its period.
struct WorkgroupTarget {
    /// `None`: the system's default output device.
    device: Option<DeviceId>,
    period_frames: NonZeroU32,
}

impl WorkgroupTarget {
    /// The engine's output device: an empty or missing id is the default device. `None` for an
    /// invalid id, which fails the stream with its own error, and without a device period.
    fn of(audio: &AudioSettings) -> Option<Self> {
        let device = match audio.output_device.as_deref() {
            None | Some("") => None,
            Some(id) => Some(id.parse::<DeviceId>().ok()?),
        };
        let period_frames = NonZeroU32::new(u32::try_from(audio.buffer_frames).ok()?)?;
        Some(Self {
            device,
            period_frames,
        })
    }
}

/// The render thread's scheduling (see [`render_thread_start`]): the membership of the output
/// device's audio workgroup, or `None` after the `QoS` fallback. Runs on the render thread.
fn schedule_render_thread(target: Option<&WorkgroupTarget>) -> Option<Box<dyn Any>> {
    let Some(target) = target else {
        log::debug!("the render thread joins no audio workgroup: no valid device id and period");
        return raise_render_qos();
    };
    match sonara_io::thread::join_audio_workgroup(target.device.as_ref(), target.period_frames) {
        Ok(membership) => {
            log::debug!("the render thread joined the output device's audio workgroup");
            return Some(Box::new(membership));
        }
        Err(error @ AudioWorkgroupError::Unsupported) => {
            log::debug!("the render thread joins no audio workgroup: {error}");
        }
        Err(error) => log::warn!("the render thread joins no audio workgroup: {error}"),
    }
    raise_render_qos()
}

/// The fallback: the user-interactive `QoS` class. Holds nothing, so it returns `None`.
fn raise_render_qos() -> Option<Box<dyn Any>> {
    match sonara_io::thread::raise_render_qos() {
        Ok(()) => {}
        Err(error @ ThreadQosError::Unsupported) => {
            log::debug!("the render thread keeps its scheduling class: {error}");
        }
        Err(error) => log::warn!("the render thread keeps its scheduling class: {error}"),
    }
    None
}

/// The player's device: the transport's engine, shared with the device listing.
pub(crate) struct EngineDevice {
    pub(crate) engine: Arc<Mutex<Engine>>,
}

impl AudioDevice for EngineDevice {
    fn start(&mut self, request: &StreamRequest) -> Result<DeviceStream, DeviceError> {
        let config = DeviceConfig {
            sample_rate: request.sample_rate,
            buffer_frames: request.buffer_frames,
            stream: StreamConfig {
                output_channels: request.output_channels,
                input_channels: request.input_channels,
                block_frames: BLOCK_FRAMES,
                ring_blocks: request.ring_blocks,
                capture_frames: request.capture_frames,
            },
            output_device: request.output_device.clone(),
            input_device: request.input_device.clone(),
        };
        let handles = lock(&self.engine)
            .start(&config)
            .map_err(|error| DeviceError(error.to_string()))?;
        Ok(DeviceStream {
            sample_rate: handles.format.sample_rate,
            producer: handles.producer,
            capture: handles.capture,
            control: handles.control,
        })
    }

    fn stop(&mut self) {
        lock(&self.engine).stop();
    }
}

/// The session as the player's maintenance thread polls it for edits (`session.version`), with
/// the media provider of the open workspace.
pub(crate) struct SessionSource {
    pub(crate) session: SharedSession,
    pub(crate) media: Arc<MediaService>,
}

impl ProjectSource for SessionSource {
    fn snapshot(&self) -> Option<ProjectSnapshot> {
        snapshot(&self.session, &self.media)
    }
}

/// The open project with its session version and media provider; `None` without a project.
/// The session is locked only for the copy (the provider locks it again on its own).
pub(crate) fn snapshot(session: &SharedSession, media: &MediaService) -> Option<ProjectSnapshot> {
    let (project, version) = {
        let session = session.lock();
        (session.project()?.clone(), session.version())
    };
    Some(ProjectSnapshot {
        project,
        version,
        media: media.provider(),
    })
}

#[cfg(test)]
mod tests {
    use super::{create_backend, render_thread_start};
    use crate::settings::AudioSettings;

    /// The null backend's simulated device has no deadline: the render thread gets no hook.
    #[test]
    fn the_null_backend_gives_the_render_thread_no_hook() {
        let engine = create_backend("native-null").unwrap().unwrap();
        assert!(render_thread_start(&engine, &AudioSettings::default()).is_none());
    }

    /// On Core Audio the hook joins the default output device's workgroup and hands the
    /// membership to the thread, which drops it there.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "needs an audio output device (HW-01); run with --ignored"]
    fn on_core_audio_the_hook_joins_the_output_devices_workgroup() {
        let engine = create_backend("native").unwrap().unwrap();
        assert_eq!(engine.backend_name(), "Core Audio");
        let hook = render_thread_start(&engine, &AudioSettings::default()).unwrap();
        std::thread::spawn(move || {
            let guard = hook().expect("a workgroup membership");
            assert!(guard.is::<sonara_io::thread::WorkgroupMembership>());
            drop(guard);
        })
        .join()
        .unwrap();
    }

    /// A saved id that is not a device id joins nothing: the hook falls back to the `QoS`
    /// class and holds nothing.
    #[test]
    fn an_invalid_saved_device_joins_no_workgroup() {
        let engine = create_backend("native").unwrap().unwrap();
        let audio = AudioSettings {
            output_device: Some("not hex".to_owned()),
            ..AudioSettings::default()
        };
        match render_thread_start(&engine, &audio) {
            // The null backend: no audio hardware here.
            None => assert_eq!(engine.backend_name(), "Null"),
            Some(hook) => {
                let guard = std::thread::spawn(move || hook().is_some()).join().unwrap();
                assert!(!guard);
            }
        }
    }
}
