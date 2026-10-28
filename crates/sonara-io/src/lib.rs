//! Sonara's device and MIDI I/O: the miniaudio device layer and [`Engine`]
//! and MIDI through midir.
//!
//! # The device layer
//!
//! - **The C shim** (`csrc/shim.{h,c}`) owns miniaudio's context and device behind opaque
//!   handles, with types that do not depend on the platform. It reproduces the
//!   prototype's miniaudio configuration (`native/src/engine.cpp`).
//! - **The FFI module** (`ffi`, the only `unsafe` of the crate, RT-10) binds the shim and holds
//!   the `extern "C"` trampolines of the data and notification callbacks.
//! - **The callback module** (`callback`, real-time) runs on the device thread: the
//!   `sonara-rt` stream's [`fill`](sonara_rt::DeviceCallback::fill), the direct monitor, the
//!   meters, the capture and the load, in the prototype's order.
//! - **[`Engine`]** (not real-time) creates the context, lists the [devices](DeviceInfo), and
//!   starts and stops a stream, returning its per-thread [`StreamHandles`].
//! - **The thread module** ([`thread`], not real-time) joins the player's render thread to the
//!   output device's audio workgroup with a time-constraint policy on macOS, or raises it to the
//!   user-interactive `QoS` class, through the shim.
//!
//! - **[`AudioBackend`]** is the engine-agnostic seam over the lifecycle: [`Engine`] implements it,
//!   and [`MockBackend`] is a scripted stand-in for tests of other crates.
//!
//! # MIDI
//!
//! - **The MIDI module** ([`midi`], not real-time) lists the ports and opens inputs and outputs,
//!   virtual ports among them, through midir (CoreMIDI on macOS, ALSA on Linux).
//! - **The MIDI callback module** (`midi_callback`, real-time) runs on the MIDI input thread: it
//!   pushes each message as a fixed-size [`MidiEvent`](sonara_rt::MidiEvent) into the input's
//!   ring, which the render thread drains.
//!
//! Real-time code here never depends on application, agent or GUI crates (RT-01).

mod backend;
mod callback;
mod device;
mod engine;
mod error;
#[expect(
    unsafe_code,
    reason = "RT-10: the miniaudio FFI is one of the three modules allowed to use unsafe"
)]
mod ffi;
pub mod midi;
mod midi_callback;
pub mod thread;

#[cfg(test)]
mod callback_tests;
#[cfg(test)]
mod midi_callback_tests;

pub use backend::{AudioBackend, MockBackend};
pub use device::{DeviceId, DeviceInfo, DeviceKind, InvalidDeviceId};
pub use engine::{ContextKind, DeviceConfig, Engine, StreamFormat, StreamHandles};
pub use error::{AudioWorkgroupError, EngineError, MaError, MidiError, ThreadQosError};

/// Version of the vendored miniaudio library, for example `"0.11.25"`.
#[must_use]
pub fn miniaudio_version() -> String {
    ffi::version()
}

#[cfg(test)]
mod tests {
    use super::miniaudio_version;

    #[test]
    fn reports_the_vendored_miniaudio_version() {
        // third_party/miniaudio is 0.11.25 (docs/dependencies.md).
        assert_eq!(miniaudio_version(), "0.11.25");
    }
}
