//! The Rust declarations of `csrc/shim.h`.
//!
//! Written by hand, not by bindgen: the header holds fourteen functions over
//! fixed-width integers, byte arrays, `float` and pointers, so the declarations stay short, use
//! Rust names, and need neither libclang nor a module-wide lint exception for generated names
//! (`quality-gate.md`). They change together with the header. `sonara_struct_size` checks the
//! struct sizes in a test, and the null-device tests check every field's meaning.

use std::ffi::{c_char, c_void};

/// `SONARA_DEVICE_ID_BYTES`: the bytes of a raw device id (`ma_device_id`).
pub(crate) const DEVICE_ID_BYTES: usize = 256;
/// `SONARA_DEVICE_NAME_BYTES`: the bytes of a device name, NUL terminator included.
pub(crate) const DEVICE_NAME_BYTES: usize = 256;

pub(crate) const CONTEXT_DEFAULT: u32 = 0;
pub(crate) const CONTEXT_NULL: u32 = 1;

pub(crate) const DEVICE_CAPTURE: u32 = 1;

pub(crate) const NOTIFICATION_STARTED: u32 = 0;
pub(crate) const NOTIFICATION_STOPPED: u32 = 1;
pub(crate) const NOTIFICATION_REROUTED: u32 = 2;
pub(crate) const NOTIFICATION_INTERRUPTION_BEGAN: u32 = 3;
pub(crate) const NOTIFICATION_INTERRUPTION_ENDED: u32 = 4;
pub(crate) const NOTIFICATION_UNLOCKED: u32 = 5;

#[cfg(test)]
pub(crate) const STRUCT_DEVICE_INFO: u32 = 0;
#[cfg(test)]
pub(crate) const STRUCT_DEVICE_CONFIG: u32 = 1;
#[cfg(test)]
pub(crate) const STRUCT_DEVICE_FORMAT: u32 = 2;

/// miniaudio's `MA_SUCCESS`.
pub(crate) const RESULT_SUCCESS: i32 = 0;
/// miniaudio's `MA_ERROR`, a generic error.
pub(crate) const RESULT_ERROR: i32 = -1;
/// miniaudio's `MA_INVALID_OPERATION`.
pub(crate) const RESULT_INVALID_OPERATION: i32 = -3;
/// miniaudio's `MA_NOT_IMPLEMENTED`: `sonara_thread_set_qos_user_interactive` off macOS.
pub(crate) const RESULT_NOT_IMPLEMENTED: i32 = -29;

/// The results of `sonara_thread_join_audio_workgroup` (`SONARA_WORKGROUP_*`). The FFI module
/// reads every other code, `SONARA_WORKGROUP_UNSUPPORTED` (1) among them, as unsupported.
pub(crate) const WORKGROUP_JOINED: u32 = 0;
pub(crate) const WORKGROUP_NO_DEVICE: u32 = 2;
pub(crate) const WORKGROUP_NO_WORKGROUP: u32 = 3;
pub(crate) const WORKGROUP_CANCELLED: u32 = 4;
pub(crate) const WORKGROUP_POLICY_REFUSED: u32 = 5;
pub(crate) const WORKGROUP_JOIN_REFUSED: u32 = 6;
pub(crate) const WORKGROUP_OUT_OF_MEMORY: u32 = 7;

/// The opaque `sonara_context`.
#[repr(C)]
pub(crate) struct RawContext {
    _opaque: [u8; 0],
}

/// The opaque `sonara_device`.
#[repr(C)]
pub(crate) struct RawDevice {
    _opaque: [u8; 0],
}

/// The opaque `sonara_workgroup`: a thread's membership of a device's IO workgroup.
#[repr(C)]
pub(crate) struct RawWorkgroup {
    _opaque: [u8; 0],
}

/// `sonara_device_info`. The name is C `char`, read as bytes: `char` and `uint8_t` have the same
/// layout whatever the signedness of `char`.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct RawDeviceInfo {
    pub(crate) id: [u8; DEVICE_ID_BYTES],
    pub(crate) name: [u8; DEVICE_NAME_BYTES],
    pub(crate) kind: u32,
    pub(crate) is_default: u32,
    pub(crate) channels: u32,
}

impl RawDeviceInfo {
    /// An all-zero entry, for the enumeration buffer.
    pub(crate) const EMPTY: Self = Self {
        id: [0; DEVICE_ID_BYTES],
        name: [0; DEVICE_NAME_BYTES],
        kind: 0,
        is_default: 0,
        channels: 0,
    };
}

/// `sonara_data_fn`.
pub(crate) type DataFn = unsafe extern "C" fn(
    user: *mut c_void,
    output: *mut f32,
    output_channels: u32,
    input: *const f32,
    input_channels: u32,
    frames: u32,
    sample_rate: u32,
);

/// `sonara_notification_fn`.
pub(crate) type NotificationFn = unsafe extern "C" fn(user: *mut c_void, notification: u32);

/// `sonara_device_config`.
#[repr(C)]
pub(crate) struct RawDeviceConfig {
    pub(crate) sample_rate: u32,
    pub(crate) period_frames: u32,
    pub(crate) output_channels: u32,
    pub(crate) input_channels: u32,
    pub(crate) output_id: *const u8,
    pub(crate) input_id: *const u8,
    pub(crate) on_data: Option<DataFn>,
    pub(crate) data_user: *mut c_void,
    pub(crate) on_notification: Option<NotificationFn>,
    pub(crate) notification_user: *mut c_void,
}

/// `sonara_device_format`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RawDeviceFormat {
    pub(crate) sample_rate: u32,
    pub(crate) period_frames: u32,
    pub(crate) periods: u32,
    pub(crate) internal_sample_rate: u32,
    pub(crate) output_channels: u32,
    pub(crate) capture_period_frames: u32,
    pub(crate) capture_periods: u32,
}

unsafe extern "C" {
    /// `sonara_miniaudio_version`: a static, NUL-terminated string.
    pub(crate) safe fn sonara_miniaudio_version() -> *const c_char;

    /// `sonara_result_description`: a static, NUL-terminated string for every code.
    pub(crate) safe fn sonara_result_description(result: i32) -> *const c_char;

    /// `sonara_struct_size`.
    #[cfg(test)]
    pub(crate) safe fn sonara_struct_size(which: u32) -> u32;

    /// `sonara_context_init`.
    pub(crate) fn sonara_context_init(kind: u32, out: *mut *mut RawContext) -> i32;

    /// `sonara_context_uninit`.
    pub(crate) fn sonara_context_uninit(context: *mut RawContext);

    /// `sonara_context_backend_name`: a static, NUL-terminated string.
    pub(crate) fn sonara_context_backend_name(context: *const RawContext) -> *const c_char;

    /// `sonara_context_devices`.
    pub(crate) fn sonara_context_devices(
        context: *mut RawContext,
        out: *mut RawDeviceInfo,
        capacity: u32,
        count: *mut u32,
    ) -> i32;

    /// `sonara_device_init`.
    pub(crate) fn sonara_device_init(
        context: *mut RawContext,
        config: *const RawDeviceConfig,
        out: *mut *mut RawDevice,
    ) -> i32;

    /// `sonara_device_start`.
    pub(crate) fn sonara_device_start(device: *mut RawDevice) -> i32;

    /// `sonara_device_get_format`.
    pub(crate) fn sonara_device_get_format(device: *const RawDevice, out: *mut RawDeviceFormat);

    /// `sonara_device_uninit`: blocks until the device's callbacks have returned.
    pub(crate) fn sonara_device_uninit(device: *mut RawDevice);

    /// `sonara_thread_set_qos_user_interactive`: 0, an error number, or `MA_NOT_IMPLEMENTED`.
    // SAFETY: declared safe because it has no preconditions: it takes no arguments, touches no
    // memory of the caller's, and changes only the calling thread's scheduling class, which any
    // thread may do at any time (`pthread_set_qos_class_self_np`; a no-op off macOS).
    pub(crate) safe fn sonara_thread_set_qos_user_interactive() -> i32;

    /// `sonara_thread_join_audio_workgroup`: one of the `WORKGROUP_*` results.
    pub(crate) fn sonara_thread_join_audio_workgroup(
        device_id: *const u8,
        period_frames: u32,
        out: *mut *mut RawWorkgroup,
        detail: *mut i32,
    ) -> u32;

    /// `sonara_thread_leave_audio_workgroup`: on the thread that joined.
    pub(crate) fn sonara_thread_leave_audio_workgroup(membership: *mut RawWorkgroup);
}
