//! The errors of the device engine. Their messages are the prototype's
//! (`native/src/engine.cpp`), so the texts that reach users and agents stay the same.

use std::fmt;

use sonara_rt::ConfigError;

use crate::InvalidDeviceId;

/// A failed miniaudio call: its `ma_result` code and miniaudio's description of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaError {
    code: i32,
    description: String,
}

impl MaError {
    pub(crate) fn new(code: i32, description: String) -> Self {
        Self { code, description }
    }

    /// miniaudio's result code, for example −1 (`MA_ERROR`).
    #[must_use]
    pub fn code(&self) -> i32 {
        self.code
    }
}

impl fmt::Display for MaError {
    /// miniaudio's description of the code, for example `"No device"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.description)
    }
}

impl std::error::Error for MaError {}

/// Why an [`Engine`](crate::Engine) call failed.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The audio context could not be created.
    #[error("audio context init failed: {0}")]
    ContextInit(#[source] MaError),
    /// The call needs a context: call [`init_context`](crate::Engine::init_context) first.
    #[error("audio context not initialised")]
    NoContext,
    /// The devices could not be listed.
    #[error("device enumeration failed: {0}")]
    Enumeration(#[source] MaError),
    /// The stream's channel counts or ring sizes are out of range.
    #[error("{0}")]
    Config(#[from] ConfigError),
    /// The output device id is not a device id.
    #[error("invalid output device id")]
    InvalidOutputDevice(#[source] InvalidDeviceId),
    /// The input device id is not a device id.
    #[error("invalid input device id")]
    InvalidInputDevice(#[source] InvalidDeviceId),
    /// miniaudio could not open the device, for example an unknown device id ("No device").
    #[error("cannot open audio device: {0}")]
    Open(#[source] MaError),
    /// miniaudio opened the device but could not start it.
    #[error("cannot start audio device: {0}")]
    Start(#[source] MaError),
}

/// Why [`raise_render_qos`](crate::thread::raise_render_qos) left the thread's scheduling class
/// as it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ThreadQosError {
    /// The platform has no `QoS` classes: every system but macOS.
    #[error("thread QoS classes are not supported on this platform")]
    Unsupported,
    /// macOS refused the class, for example for a thread with a fixed scheduling policy.
    #[error(
        "cannot raise the thread's QoS class: {}",
        std::io::Error::from_raw_os_error(*errno)
    )]
    Refused {
        /// The error number of `pthread_set_qos_class_self_np`.
        errno: i32,
    },
}

/// Why [`join_audio_workgroup`](crate::thread::join_audio_workgroup) left the thread outside the
/// device's workgroup.
///
/// Every error but [`JoinRefused`](Self::JoinRefused) leaves the thread as it was, so a caller
/// can still request a `QoS` class for it ([`raise_render_qos`](crate::thread::raise_render_qos)).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AudioWorkgroupError {
    /// The platform has no audio workgroups: every system but macOS 11 or later.
    #[error("audio workgroups are not supported on this platform")]
    Unsupported,
    /// No Core Audio device has the id, there is no default output device, or its sample rate
    /// cannot be read.
    #[error("no Core Audio output device to join (status {status})")]
    NoDevice {
        /// The HAL's `OSStatus`; 0 when the id names no device or there is no default device.
        status: i32,
    },
    /// The device reports no IO thread workgroup.
    #[error("the audio device has no IO thread workgroup (status {status})")]
    NoWorkgroup {
        /// The HAL's `OSStatus` for `kAudioDevicePropertyIOThreadOSWorkgroup`.
        status: i32,
    },
    /// The device's workgroup has been cancelled: the device is going away.
    #[error("the audio device's workgroup has been cancelled")]
    Cancelled,
    /// macOS refused the time-constraint policy, or the period does not fit it.
    #[error("cannot give the thread a time-constraint policy (kern_return_t {code})")]
    PolicyRefused {
        /// The `kern_return_t` of `thread_policy_set`, for example 4 (`KERN_INVALID_ARGUMENT`)
        /// for a period whose computation exceeds what macOS allows.
        code: i32,
    },
    /// `os_workgroup_join` refused, after the policy was set. The thread is back on the standard
    /// policy, and macOS refuses it a `QoS` class from now on (EPERM).
    #[error(
        "cannot join the audio workgroup: {}",
        std::io::Error::from_raw_os_error(*errno)
    )]
    JoinRefused {
        /// The error number of `os_workgroup_join`: EINVAL for a workgroup cancelled meanwhile,
        /// EALREADY for a thread in a workgroup this one does not nest with.
        errno: i32,
    },
    /// The membership could not be allocated.
    #[error("out of memory for the audio workgroup membership")]
    OutOfMemory,
}

/// Why a MIDI call failed (`midi`).
#[derive(Debug, thiserror::Error)]
pub enum MidiError {
    /// The MIDI system cannot be used, for example on Linux without an ALSA sequencer. midir's
    /// `InitError` carries no detail beyond this text.
    #[error("MIDI support could not be initialized")]
    Init,
    /// No MIDI input port has this name.
    #[error("MIDI input {name:?} not found")]
    InputNotFound {
        /// The name asked for.
        name: String,
        /// The names of the input ports there are.
        available: Vec<String>,
    },
    /// No MIDI output port has this name.
    #[error("MIDI output {name:?} not found")]
    OutputNotFound {
        /// The name asked for.
        name: String,
        /// The names of the output ports there are.
        available: Vec<String>,
    },
    /// The port could not be connected or created.
    #[error("{reason}")]
    Connect {
        /// The port's name.
        name: String,
        /// midir's reason (its `ConnectErrorKind`, which is only a text).
        reason: String,
    },
    /// A message could not be sent.
    #[error("{0}")]
    Send(#[source] midir::SendError),
    /// Virtual ports exist only on macOS and Linux.
    #[error("virtual MIDI ports are not supported on this platform")]
    VirtualPortsUnsupported,
}
