//! Real-time: holds the `extern "C"` trampolines that the audio device thread calls (see
//! docs/audio-engine.md).
//!
//! The miniaudio FFI, the only module of `sonara-io` with `unsafe` (RT-10). It binds Sonara's C
//! shim (`csrc/shim.h`), whose types do not depend on the platform, and wraps it in
//! two safe owners:
//!
//! - [`Context`] owns the shim's context and at most one device. Its `Drop` uninitializes the
//!   device before the context, so the context always outlives its device (MEM-07).
//! - The device owns the two callback states the trampolines use: the [`DeviceProcess`],
//!   used only by the data trampoline, and a [`StreamControl`], shared with the notification
//!   trampoline. They are freed only after `sonara_device_uninit` has returned, on the control
//!   thread (RT-09).
//!
//! Two thread functions wrap the shim's: [`raise_thread_qos`] requests the render thread's
//! `QoS` class, and [`join_audio_workgroup`] joins the render thread to the output device's
//! audio workgroup with a time-constraint policy, returning a [`WorkgroupMembership`] that
//! leaves it on drop. Both are setup code, called on the render thread before its loop; the
//! membership drops after the loop, on the same thread.
//!
//! The trampolines turn the shim's raw pointers into references and slices and call the
//! callback module. They cannot panic: the real-time lint block below rules out every
//! panicking construct, and the lengths are computed with checked arithmetic, so no
//! `catch_unwind` is needed (RT-05). Everything else here is setup and control code, which runs
//! on non-real-time threads.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod shim;

use std::ffi::{CStr, c_char, c_void};
use std::num::NonZeroU32;
use std::ptr::{self, NonNull};
use std::slice;

use sonara_rt::StreamControl;

use crate::callback::{self, DeviceProcess, Notification, Period};
use crate::device::{DeviceId, DeviceInfo, DeviceKind};
use crate::engine::ContextKind;
use crate::error::{AudioWorkgroupError, MaError, ThreadQosError};

pub(crate) use shim::DEVICE_ID_BYTES;

// The trampolines hand the callback states to other threads: the device thread and the
// notification thread.
const _: () = {
    const fn send<T: Send>() {}
    const fn sync<T: Sync>() {}
    send::<DeviceProcess>();
    send::<StreamControl>();
    sync::<StreamControl>();
};

/// Version of the vendored miniaudio library.
pub(crate) fn version() -> String {
    // SAFETY: the shim returns `MA_VERSION_STRING`, a static NUL-terminated literal.
    unsafe { static_text(shim::sonara_miniaudio_version()) }
}

/// Copies a NUL-terminated C string; empty for NULL.
///
/// # Safety
///
/// `text` is NULL or points to a NUL-terminated string that stays valid during the call.
unsafe fn static_text(text: *const c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    // SAFETY: the caller guarantees a valid NUL-terminated string.
    unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned()
}

/// Turns a shim result code into a `Result`.
fn check(code: i32) -> Result<(), MaError> {
    if code == shim::RESULT_SUCCESS {
        Ok(())
    } else {
        Err(error(code))
    }
}

/// The error of a failed miniaudio call, with miniaudio's description.
fn error(code: i32) -> MaError {
    // SAFETY: the shim returns a static NUL-terminated description for every code.
    let description = unsafe { static_text(shim::sonara_result_description(code)) };
    MaError::new(code, description)
}

/// Raises the calling thread to the user-interactive `QoS` class (see
/// [`raise_render_qos`](crate::thread::raise_render_qos)).
///
/// Setup: a system call, made once on a thread before its real-time loop starts, never inside
/// it (RT-03).
pub(crate) fn raise_thread_qos() -> Result<(), ThreadQosError> {
    // Declared safe: the `// SAFETY:` comment on its declaration (`shim.rs`) says why.
    match shim::sonara_thread_set_qos_user_interactive() {
        shim::RESULT_SUCCESS => Ok(()),
        shim::RESULT_NOT_IMPLEMENTED => Err(ThreadQosError::Unsupported),
        errno => Err(ThreadQosError::Refused { errno }),
    }
}

/// The calling thread's membership of an audio device's IO workgroup (see
/// [`join_audio_workgroup`](crate::thread::join_audio_workgroup)). Its `Drop` leaves the
/// workgroup.
///
/// Neither `Send` nor `Sync` (its `NonNull` field sees to it): `os_workgroup_leave` acts on the
/// calling thread, so the membership must drop on the thread that joined.
#[derive(Debug)]
pub(crate) struct WorkgroupMembership {
    raw: NonNull<shim::RawWorkgroup>,
}

impl Drop for WorkgroupMembership {
    fn drop(&mut self) {
        // SAFETY: `self.raw` is the membership `sonara_thread_join_audio_workgroup` stored, left
        // only here, once. It is not `Send`, so this runs on the thread that joined.
        unsafe { shim::sonara_thread_leave_audio_workgroup(self.raw.as_ptr()) };
    }
}

/// Joins the calling thread to the IO workgroup of `device` (the default output device for
/// `None`) with a time-constraint policy of `period_frames` at the device's nominal rate (see
/// [`join_audio_workgroup`](crate::thread::join_audio_workgroup)).
///
/// Setup: system calls and an allocation in the shim, made once on a thread before its
/// real-time loop starts, never inside it (RT-03).
pub(crate) fn join_audio_workgroup(
    device: Option<&DeviceId>,
    period_frames: NonZeroU32,
) -> Result<WorkgroupMembership, AudioWorkgroupError> {
    let mut raw = ptr::null_mut();
    let mut detail = 0;
    // SAFETY: the id is NULL or `DEVICE_ID_BYTES` readable bytes, which the shim reads during
    // the call only; `raw` and `detail` are valid places for the shim to store its results.
    let result = unsafe {
        shim::sonara_thread_join_audio_workgroup(
            device.map_or(ptr::null(), |id| id.raw().as_ptr()),
            period_frames.get(),
            &raw mut raw,
            &raw mut detail,
        )
    };
    match result {
        // The shim stores a membership whenever it joins; without one the join could not be
        // left, which only an allocation failure could cause.
        shim::WORKGROUP_JOINED => NonNull::new(raw)
            .map(|raw| WorkgroupMembership { raw })
            .ok_or(AudioWorkgroupError::OutOfMemory),
        shim::WORKGROUP_NO_DEVICE => Err(AudioWorkgroupError::NoDevice { status: detail }),
        shim::WORKGROUP_NO_WORKGROUP => Err(AudioWorkgroupError::NoWorkgroup { status: detail }),
        shim::WORKGROUP_CANCELLED => Err(AudioWorkgroupError::Cancelled),
        shim::WORKGROUP_POLICY_REFUSED => Err(AudioWorkgroupError::PolicyRefused { code: detail }),
        shim::WORKGROUP_JOIN_REFUSED => Err(AudioWorkgroupError::JoinRefused { errno: detail }),
        shim::WORKGROUP_OUT_OF_MEMORY => Err(AudioWorkgroupError::OutOfMemory),
        // `SONARA_WORKGROUP_UNSUPPORTED`, and any code a newer shim might add.
        _ => Err(AudioWorkgroupError::Unsupported),
    }
}

/// What [`Context::open_device`] opens. The ids are raw device ids; `None` selects the default
/// device.
pub(crate) struct DeviceRequest<'a> {
    pub(crate) sample_rate: u32,
    pub(crate) period_frames: u32,
    pub(crate) output_channels: u32,
    pub(crate) input_channels: u32,
    pub(crate) output_id: Option<&'a DeviceId>,
    pub(crate) input_id: Option<&'a DeviceId>,
}

/// What an opened device obtained (`sonara_device_get_format`).
pub(crate) type DeviceFormat = shim::RawDeviceFormat;

/// A miniaudio context and the device opened on it.
pub(crate) struct Context {
    raw: NonNull<shim::RawContext>,
    backend_name: String,
    device: Option<Device>,
}

// SAFETY: miniaudio's context and device are not bound to the thread that created them on
// Sonara's platforms (macOS; Linux in the container): miniaudio runs the device on its own
// threads and synchronizes with them inside its calls. `Context` owns both exclusively and calls
// the shim only through `&mut self`, so no two threads use them at once. The callback states it
// hands to other threads are `Send` and, where shared, `Sync` (checked above).
unsafe impl Send for Context {}

impl Context {
    /// Creates a context: miniaudio's default backends, or the null backend only.
    ///
    /// Setup: runs on a control thread.
    pub(crate) fn init(kind: ContextKind) -> Result<Self, MaError> {
        let kind = match kind {
            ContextKind::Default => shim::CONTEXT_DEFAULT,
            ContextKind::Null => shim::CONTEXT_NULL,
        };
        let mut raw = ptr::null_mut();
        // SAFETY: `raw` is a valid place for the shim to store the new context.
        check(unsafe { shim::sonara_context_init(kind, &raw mut raw) })?;
        // The shim stores a context whenever it succeeds.
        let raw = NonNull::new(raw).ok_or_else(|| error(shim::RESULT_ERROR))?;
        // SAFETY: `raw` is a live context; the backend name is a static NUL-terminated literal.
        let backend_name = unsafe { static_text(shim::sonara_context_backend_name(raw.as_ptr())) };
        Ok(Self {
            raw,
            backend_name,
            device: None,
        })
    }

    /// miniaudio's name of the context's backend, for example `"Core Audio"` or `"Null"`.
    pub(crate) fn backend_name(&self) -> &str {
        &self.backend_name
    }

    /// Lists the devices: playback devices first, then capture devices.
    ///
    /// Setup: runs on a control thread.
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs on a control thread, never on the real-time thread"
    )]
    pub(crate) fn devices(&mut self) -> Result<Vec<DeviceInfo>, MaError> {
        let mut capacity: u32 = 16;
        loop {
            let mut infos =
                vec![shim::RawDeviceInfo::EMPTY; usize::try_from(capacity).unwrap_or(0)];
            let mut count = 0;
            // SAFETY: `self.raw` is a live context, used by this thread only (`&mut self`).
            // `infos` has room for `capacity` entries, and `count` is a valid place to store the
            // total.
            let code = unsafe {
                shim::sonara_context_devices(
                    self.raw.as_ptr(),
                    infos.as_mut_ptr(),
                    capacity,
                    &raw mut count,
                )
            };
            check(code)?;
            if count <= capacity {
                infos.truncate(usize::try_from(count).unwrap_or(0));
                return Ok(infos.iter().map(device_info).collect());
            }
            // A device appeared between the calls: list again with room for all of them.
            capacity = count.saturating_add(8);
        }
    }

    /// Opens a device with the prototype's configuration, closing the open one first. The
    /// device calls `process` for its data and `control` for its notifications from
    /// [`start_device`](Self::start_device) until it is closed.
    ///
    /// Setup: runs on a control thread; it blocks while miniaudio opens the device.
    pub(crate) fn open_device(
        &mut self,
        request: &DeviceRequest<'_>,
        process: Box<DeviceProcess>,
        control: Box<StreamControl>,
    ) -> Result<DeviceFormat, MaError> {
        self.close_device();
        let callbacks = Callbacks::new(process, control);
        let config = shim::RawDeviceConfig {
            sample_rate: request.sample_rate,
            period_frames: request.period_frames,
            output_channels: request.output_channels,
            input_channels: request.input_channels,
            output_id: request
                .output_id
                .map_or(ptr::null(), |id| id.raw().as_ptr()),
            input_id: request.input_id.map_or(ptr::null(), |id| id.raw().as_ptr()),
            on_data: Some(on_data),
            data_user: callbacks.process.as_ptr().cast(),
            on_notification: Some(on_notification),
            notification_user: callbacks.control.as_ptr().cast(),
        };
        let mut raw = ptr::null_mut();
        // SAFETY: `self.raw` is a live context, used by this thread only. `config` and the ids
        // it points to outlive the call; the shim copies the ids. The user pointers stay valid
        // until the device is uninitialized: `Device` owns `callbacks` and frees them only after
        // `sonara_device_uninit` (and on failure no device exists that could call them).
        let code =
            unsafe { shim::sonara_device_init(self.raw.as_ptr(), &raw const config, &raw mut raw) };
        check(code)?;
        let raw = NonNull::new(raw).ok_or_else(|| error(shim::RESULT_ERROR))?;
        let device = Device {
            raw,
            _callbacks: callbacks,
        };
        let mut format = DeviceFormat::default();
        // SAFETY: `device.raw` is a live, initialized device; `format` is a valid place to store
        // its format.
        unsafe { shim::sonara_device_get_format(device.raw.as_ptr(), &raw mut format) };
        self.device = Some(device);
        Ok(format)
    }

    /// Starts the open device; on failure, closes it.
    ///
    /// Setup: runs on a control thread.
    pub(crate) fn start_device(&mut self) -> Result<(), MaError> {
        let Some(device) = self.device.as_mut() else {
            return Err(error(shim::RESULT_INVALID_OPERATION));
        };
        // SAFETY: `device.raw` is a live, initialized device, used by this thread only.
        let started = check(unsafe { shim::sonara_device_start(device.raw.as_ptr()) });
        if started.is_err() {
            self.close_device();
        }
        started
    }

    /// Whether a device is open.
    pub(crate) fn has_device(&self) -> bool {
        self.device.is_some()
    }

    /// Stops and closes the open device, if any. Blocks until its callbacks have returned
    /// (`ma_device_uninit`), so it runs on a control thread, never on the device thread (RT-06).
    pub(crate) fn close_device(&mut self) {
        self.device = None;
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // The device first: it refers to the context.
        self.close_device();
        // SAFETY: `self.raw` is a live context without a device, and it is not used again.
        unsafe { shim::sonara_context_uninit(self.raw.as_ptr()) };
    }
}

/// An initialized device and the callback states its callbacks use.
struct Device {
    raw: NonNull<shim::RawDevice>,
    /// Held for its `Drop`, which frees the callback states after the device.
    _callbacks: Callbacks,
}

impl Drop for Device {
    fn drop(&mut self) {
        // SAFETY: `self.raw` is a live device and is not used again. When this returns, no
        // callback runs any more; only then do the fields drop and free `_callbacks`.
        unsafe { shim::sonara_device_uninit(self.raw.as_ptr()) };
    }
}

/// The callback states registered with a device. The trampolines use them through the raw
/// pointers; this owner frees them.
struct Callbacks {
    /// Used only by the data trampoline, as `&mut`.
    process: NonNull<DeviceProcess>,
    /// Shared with the notification trampoline, as `&`.
    control: NonNull<StreamControl>,
}

impl Callbacks {
    fn new(process: Box<DeviceProcess>, control: Box<StreamControl>) -> Self {
        Self {
            process: NonNull::from(Box::leak(process)),
            control: NonNull::from(Box::leak(control)),
        }
    }
}

impl Drop for Callbacks {
    fn drop(&mut self) {
        // SAFETY: both pointers come from `Box::leak` in `new` and are freed only here. Their
        // device has been uninitialized (`Device::drop` runs before its fields drop) or was never
        // initialized, so no trampoline uses them any more.
        unsafe {
            drop(Box::from_raw(self.process.as_ptr()));
            drop(Box::from_raw(self.control.as_ptr()));
        }
    }
}

/// A device of the enumeration.
fn device_info(raw: &shim::RawDeviceInfo) -> DeviceInfo {
    let name = CStr::from_bytes_until_nul(&raw.name).map_or_else(
        |_| String::from_utf8_lossy(&raw.name).into_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let kind = if raw.kind == shim::DEVICE_CAPTURE {
        DeviceKind::Capture
    } else {
        DeviceKind::Playback
    };
    DeviceInfo::new(
        DeviceId::from_raw(raw.id),
        name,
        kind,
        raw.is_default != 0,
        raw.channels,
    )
}

/// Samples in `frames` frames of `channels` channels; `None` when they do not fit a `usize`.
fn samples(frames: u32, channels: u32) -> Option<usize> {
    usize::try_from(frames)
        .ok()?
        .checked_mul(usize::try_from(channels).ok()?)
}

/// The data trampoline: miniaudio's device thread calls it, through the shim, once per period.
///
/// # Safety
///
/// Only the shim calls it, with what [`Context::open_device`] registered and what miniaudio
/// hands to the callback: `user` is the device's `DeviceProcess`; `output` holds
/// `frames × output_channels` writable samples; `input` is NULL or holds
/// `frames × input_channels` readable samples. The buffers are valid for this call only.
#[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
unsafe extern "C" fn on_data(
    user: *mut c_void,
    output: *mut f32,
    output_channels: u32,
    input: *const f32,
    input_channels: u32,
    frames: u32,
    sample_rate: u32,
) {
    let (Some(mut process), Some(output_ptr)) = (
        NonNull::new(user.cast::<DeviceProcess>()),
        NonNull::new(output),
    ) else {
        return;
    };
    let (Some(output_len), Some(input_len)) = (
        samples(frames, output_channels),
        samples(frames, input_channels),
    ) else {
        return;
    };
    // SAFETY: `user` is the `DeviceProcess` that `open_device` registered; it stays alive until
    // the device is uninitialized. miniaudio never runs two data callbacks of one device at
    // once: synchronous backends (the null device) call it from the one device thread, and for
    // a duplex device on an asynchronous backend (Core Audio) the capture side only fills
    // miniaudio's duplex ring, while the playback side alone calls the data callback
    // (`ma_device_handle_backend_data_callback`). No other code dereferences this pointer while
    // the device exists, so this is the only reference to it.
    let process = unsafe { process.as_mut() };
    // SAFETY: miniaudio's output buffer holds `frames` frames of `output_channels` `f32` samples
    // (the client format the shim requested), aligned, writable and not aliased during this call.
    let output = unsafe { slice::from_raw_parts_mut(output_ptr.as_ptr(), output_len) };
    let input = if input.is_null() || input_len == 0 {
        None
    } else {
        // SAFETY: as for the output: the capture buffer holds `frames` frames of
        // `input_channels` `f32` samples, readable during this call.
        Some(unsafe { slice::from_raw_parts(input, input_len) })
    };
    process.process(Period {
        output,
        output_channels,
        input,
        input_channels,
        frames,
        sample_rate,
    });
}

/// The notification trampoline: miniaudio calls it, through the shim, on the device thread or
/// on a system thread.
///
/// # Safety
///
/// Only the shim calls it, with the `StreamControl` that [`Context::open_device`] registered as
/// `user`.
#[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
unsafe extern "C" fn on_notification(user: *mut c_void, notification: u32) {
    let Some(control) = NonNull::new(user.cast::<StreamControl>()) else {
        return;
    };
    let notification = match notification {
        shim::NOTIFICATION_STARTED => Notification::Started,
        shim::NOTIFICATION_STOPPED => Notification::Stopped,
        shim::NOTIFICATION_REROUTED => Notification::Rerouted,
        shim::NOTIFICATION_INTERRUPTION_BEGAN => Notification::InterruptionBegan,
        shim::NOTIFICATION_INTERRUPTION_ENDED => Notification::InterruptionEnded,
        shim::NOTIFICATION_UNLOCKED => Notification::Unlocked,
        _ => return,
    };
    // SAFETY: `user` is the `StreamControl` that `open_device` registered; it stays alive until
    // the device is uninitialized. Only shared references to it are ever made, and it is `Sync`.
    let control = unsafe { control.as_ref() };
    callback::notify(control, notification);
}

#[cfg(test)]
mod tests {
    use super::shim;

    fn size(which: u32) -> usize {
        usize::try_from(shim::sonara_struct_size(which)).unwrap()
    }

    /// The hand-written declarations have the C header's struct sizes.
    #[test]
    fn declared_structs_match_the_shim_header() {
        assert_eq!(
            size(shim::STRUCT_DEVICE_INFO),
            size_of::<shim::RawDeviceInfo>()
        );
        assert_eq!(
            size(shim::STRUCT_DEVICE_CONFIG),
            size_of::<shim::RawDeviceConfig>()
        );
        assert_eq!(
            size(shim::STRUCT_DEVICE_FORMAT),
            size_of::<shim::RawDeviceFormat>()
        );
        assert_eq!(size(u32::MAX), 0);
    }

    #[test]
    fn describes_result_codes() {
        assert_eq!(super::error(shim::RESULT_INVALID_OPERATION).code(), -3);
        assert_eq!(super::error(-9999).to_string(), "Unknown error");
    }

    #[cfg(target_os = "macos")]
    unsafe extern "C" {
        /// `qos_class_self` of `<pthread/qos.h>`: the calling thread's `QoS` class (`qos_class_t`,
        /// an `unsigned int`).
        // SAFETY: declared safe because it has no preconditions: it takes no arguments and only
        // reads the calling thread's own scheduling class.
        safe fn qos_class_self() -> u32;
    }

    /// `QOS_CLASS_USER_INTERACTIVE` of `<sys/qos.h>`.
    #[cfg(target_os = "macos")]
    const QOS_CLASS_USER_INTERACTIVE: u32 = 0x21;

    /// The raise changes the calling thread's requested class, read back from the system. It
    /// runs on a thread of its own, so that the test harness's threads keep their class. It
    /// checks the requested class (`qos_class_self`), not the effective priority: a test process
    /// started from a terminal has no application role, so macOS squashes the class there.
    #[cfg(target_os = "macos")]
    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "test: a fresh thread, not a real-time one, so the raise touches no other test"
    )]
    fn raises_the_calling_thread_to_user_interactive() {
        std::thread::spawn(|| {
            assert_ne!(qos_class_self(), QOS_CLASS_USER_INTERACTIVE);
            assert_eq!(super::raise_thread_qos(), Ok(()));
            assert_eq!(qos_class_self(), QOS_CLASS_USER_INTERACTIVE);
        })
        .join()
        .unwrap();
    }

    /// Off macOS there are no `QoS` classes: the raise does nothing and says so.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn raising_the_class_is_unsupported_off_macos() {
        assert_eq!(
            super::raise_thread_qos(),
            Err(super::ThreadQosError::Unsupported)
        );
    }

    /// The calling thread's scheduling, read back from the kernel.
    #[cfg(target_os = "macos")]
    mod mach {
        use std::ffi::c_void;

        /// `THREAD_TIME_CONSTRAINT_POLICY` of `<mach/thread_policy.h>`.
        const TIME_CONSTRAINT_POLICY: u32 = 2;

        /// `thread_time_constraint_policy_data_t`: period, computation, constraint, preemptible.
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
        #[repr(C)]
        pub(super) struct TimeConstraint {
            pub(super) period: u32,
            pub(super) computation: u32,
            pub(super) constraint: u32,
            pub(super) preemptible: i32,
        }

        /// `mach_timebase_info_data_t`.
        #[repr(C)]
        struct Timebase {
            numer: u32,
            denom: u32,
        }

        unsafe extern "C" {
            // SAFETY: declared safe because it has no preconditions: it returns the calling
            // thread's own handle.
            safe fn pthread_self() -> *mut c_void;
            fn pthread_mach_thread_np(thread: *mut c_void) -> u32;
            fn thread_policy_get(
                thread: u32,
                flavor: u32,
                info: *mut TimeConstraint,
                count: *mut u32,
                get_default: *mut i32,
            ) -> i32;
            fn mach_timebase_info(info: *mut Timebase) -> i32;
        }

        /// The calling thread's time-constraint policy; `None` while it has none, that is while
        /// the kernel reports the default values (`get_default`).
        pub(super) fn time_constraint() -> Option<TimeConstraint> {
            let mut policy = TimeConstraint::default();
            let mut count = 4;
            let mut get_default = 0;
            // SAFETY: `pthread_self` is a live thread, the calling one. `policy` has room for
            // the `count` (4) integers of the flavor, and the other two are valid places.
            let code = unsafe {
                thread_policy_get(
                    pthread_mach_thread_np(pthread_self()),
                    TIME_CONSTRAINT_POLICY,
                    &raw mut policy,
                    &raw mut count,
                    &raw mut get_default,
                )
            };
            assert_eq!(code, 0, "thread_policy_get failed");
            (get_default == 0).then_some(policy)
        }

        /// The sample rate at which `frames` frames last `ticks` of Mach absolute time.
        pub(super) fn rate_of(frames: u32, ticks: u32) -> f64 {
            let mut timebase = Timebase { numer: 0, denom: 0 };
            // SAFETY: `timebase` is a valid place for the result.
            assert_eq!(unsafe { mach_timebase_info(&raw mut timebase) }, 0);
            let seconds =
                f64::from(ticks) * f64::from(timebase.numer) / f64::from(timebase.denom) / 1e9;
            f64::from(frames) / seconds
        }
    }

    /// A device id that no Core Audio device has: the UID `sonara-no-such-device`, in hex.
    #[cfg(target_os = "macos")]
    fn unknown_device() -> crate::DeviceId {
        "736f6e6172612d6e6f2d737563682d646576696365"
            .parse()
            .unwrap()
    }

    const PERIOD: std::num::NonZeroU32 = std::num::NonZeroU32::new(256).unwrap();

    /// An id that names no device is refused before the thread is touched: it keeps the default
    /// policy and can still take a `QoS` class, the transport's fallback. Needs no audio device.
    #[cfg(target_os = "macos")]
    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "test: a fresh thread, not a real-time one, so the policy touches no other test"
    )]
    fn an_unknown_device_is_refused_and_the_thread_keeps_its_policy() {
        std::thread::spawn(|| {
            let refused = super::join_audio_workgroup(Some(&unknown_device()), PERIOD);
            // The status is 0 here (an unknown UID translates to no device); a machine without
            // audio devices may answer otherwise, but never joins.
            assert!(matches!(
                refused,
                Err(super::AudioWorkgroupError::NoDevice { .. })
            ));
            assert_eq!(mach::time_constraint(), None);
            assert_eq!(super::raise_thread_qos(), Ok(()));
        })
        .join()
        .unwrap();
    }

    /// On the default output device: the join gives the thread a time-constraint policy of one
    /// 256-frame period at the device's rate; a second join is refused (EALREADY) and puts the
    /// thread back on the standard policy; the first membership still leaves, after which the
    /// thread joins again. A period too long for the policy is refused before the thread is
    /// touched.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "needs an audio output device (HW-01); run with --ignored"]
    #[expect(
        clippy::disallowed_methods,
        reason = "test: a fresh thread, not a real-time one, so the policy touches no other test"
    )]
    fn joins_the_default_output_devices_workgroup_with_a_time_constraint_policy() {
        std::thread::spawn(|| {
            let too_long = std::num::NonZeroU32::new(1 << 20).unwrap();
            assert_eq!(
                super::join_audio_workgroup(None, too_long).unwrap_err(),
                // KERN_INVALID_ARGUMENT: the computation exceeds what macOS allows.
                super::AudioWorkgroupError::PolicyRefused { code: 4 }
            );
            assert_eq!(mach::time_constraint(), None);

            let membership = super::join_audio_workgroup(None, PERIOD).unwrap();
            let policy = mach::time_constraint().expect("a time-constraint policy");
            assert_eq!(policy.computation, policy.period / 2);
            assert_eq!(policy.constraint, policy.period);
            assert_ne!(policy.preemptible, 0);
            let rate = mach::rate_of(256, policy.period);
            assert!(
                [44_100.0, 48_000.0, 88_200.0, 96_000.0, 176_400.0, 192_000.0]
                    .iter()
                    .any(|nominal| (rate - nominal).abs() < 1.0),
                "256 frames at {rate} Hz"
            );

            // EALREADY: the thread is already in the workgroup.
            assert_eq!(
                super::join_audio_workgroup(None, PERIOD).unwrap_err(),
                super::AudioWorkgroupError::JoinRefused { errno: 37 }
            );
            assert_eq!(mach::time_constraint(), None);
            drop(membership);

            let again = super::join_audio_workgroup(None, PERIOD).unwrap();
            assert!(mach::time_constraint().is_some());
            drop(again);
        })
        .join()
        .unwrap();
    }

    /// Off macOS there are no audio workgroups: the join does nothing and says so.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn joining_a_workgroup_is_unsupported_off_macos() {
        let id: crate::DeviceId = "6869".parse().unwrap();
        for device in [None, Some(&id)] {
            assert_eq!(
                super::join_audio_workgroup(device, PERIOD).unwrap_err(),
                super::AudioWorkgroupError::Unsupported
            );
        }
    }
}
