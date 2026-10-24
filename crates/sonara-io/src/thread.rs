//! The scheduling of the player's render thread: its membership of the output
//! device's audio workgroup with a time-constraint policy, or its `QoS` class.
//!
//! Not real-time code: [`join_audio_workgroup`] and [`raise_render_qos`] are setup. The render
//! thread calls one of them once, before its loop, and drops a [`WorkgroupMembership`] after the
//! loop; the system calls they make never run inside the loop (RT-03).

use std::num::NonZeroU32;

use crate::device::DeviceId;
use crate::error::{AudioWorkgroupError, ThreadQosError};
use crate::ffi;

/// The calling thread's membership of an audio device's IO workgroup, from
/// [`join_audio_workgroup`]. Dropping it leaves the workgroup; the thread keeps its
/// time-constraint policy until it exits.
///
/// Neither `Send` nor `Sync`: macOS removes the calling thread from a workgroup, so the
/// membership drops on the thread that joined, before that thread exits. A thread leaves its
/// workgroups in the reverse order of joining them.
#[derive(Debug)]
pub struct WorkgroupMembership {
    _membership: ffi::WorkgroupMembership,
}

/// Joins the calling thread to the IO thread workgroup of a Core Audio output device
/// (`kAudioDevicePropertyIOThreadOSWorkgroup`), as Apple advises for an auxiliary audio thread
/// on Apple Silicon: the system then schedules the thread as part of the device's audio
/// deadline. `output_device` is the device's id, `None` for the system's default output device.
///
/// Before it joins, it gives the thread a time-constraint policy (`THREAD_TIME_CONSTRAINT_POLICY`)
/// of one device period: `period_frames` (`settings.audio.buffer_frames`) at the device's nominal
/// sample rate, half of it as computation, the whole period as constraint, preemptible.
///
/// The workgroup belongs to the device, not to an open stream, so the render thread joins it at
/// its start, before miniaudio opens the device. A later reroute to another device leaves the
/// thread in the first device's workgroup.
///
/// Call it on the render thread itself, once, before the loop, and drop the membership on the
/// same thread after the loop.
///
/// # Errors
///
/// - [`AudioWorkgroupError::Unsupported`] on every system but macOS 11 or later.
/// - [`AudioWorkgroupError::NoDevice`] when no Core Audio device has the id or there is no
///   default output device; [`AudioWorkgroupError::NoWorkgroup`] when the device reports no
///   workgroup; [`AudioWorkgroupError::Cancelled`] when the device is going away.
/// - [`AudioWorkgroupError::PolicyRefused`] when macOS refuses the policy, for example for a
///   period whose computation is longer than it allows.
/// - [`AudioWorkgroupError::JoinRefused`] when the join fails after the policy was set; the
///   thread is back on the standard policy.
/// - [`AudioWorkgroupError::OutOfMemory`] when the membership cannot be allocated.
///
/// Every error but `JoinRefused` leaves the thread as it was, so the caller can fall back to
/// [`raise_render_qos`]; macOS refuses a `QoS` class to a thread that has had a time-constraint
/// policy. Either way the thread still works: a caller reports the error and carries on.
pub fn join_audio_workgroup(
    output_device: Option<&DeviceId>,
    period_frames: NonZeroU32,
) -> Result<WorkgroupMembership, AudioWorkgroupError> {
    ffi::join_audio_workgroup(output_device, period_frames).map(|membership| WorkgroupMembership {
        _membership: membership,
    })
}

/// Requests macOS's user-interactive `QoS` class (`QOS_CLASS_USER_INTERACTIVE`), the class of
/// work the user is waiting for, for the calling thread.
///
/// The render thread is light and periodic; the owner chose this class for it. It is the
/// fallback when the thread joins no audio workgroup ([`join_audio_workgroup`]). Call it on the
/// render thread itself, once, before the loop.
///
/// `Ok` means the class was requested, not that it is in effect: macOS honours it only in a task
/// with an application role, such as the app launched by Launch Services. A process without a
/// role, such as the CLI started from a terminal, gets the class squashed to the default class
/// (base priority 31 instead of 46 or 47), and the call still succeeds.
///
/// # Errors
///
/// - [`ThreadQosError::Unsupported`] on every system but macOS, which has no `QoS` classes; the
///   thread is left as it was.
/// - [`ThreadQosError::Refused`] when macOS refuses the class, for example for a thread with a
///   fixed scheduling policy; the thread keeps its class.
///
/// Either way the thread still works: a caller reports the error and carries on.
pub fn raise_render_qos() -> Result<(), ThreadQosError> {
    ffi::raise_thread_qos()
}

#[cfg(test)]
mod tests {
    use crate::{AudioWorkgroupError, ThreadQosError};

    #[test]
    fn a_refused_raise_names_the_system_error() {
        // EPERM is 1 on macOS and Linux.
        let refused = ThreadQosError::Refused { errno: 1 };
        assert_eq!(
            refused.to_string(),
            "cannot raise the thread's QoS class: Operation not permitted (os error 1)"
        );
    }

    #[test]
    fn unsupported_says_so() {
        assert_eq!(
            ThreadQosError::Unsupported.to_string(),
            "thread QoS classes are not supported on this platform"
        );
        assert_eq!(
            AudioWorkgroupError::Unsupported.to_string(),
            "audio workgroups are not supported on this platform"
        );
    }

    #[test]
    fn workgroup_errors_keep_the_systems_code() {
        // EINVAL is 22 on macOS and Linux.
        assert_eq!(
            AudioWorkgroupError::JoinRefused { errno: 22 }.to_string(),
            "cannot join the audio workgroup: Invalid argument (os error 22)"
        );
        assert_eq!(
            AudioWorkgroupError::PolicyRefused { code: 4 }.to_string(),
            "cannot give the thread a time-constraint policy (kern_return_t 4)"
        );
        assert_eq!(
            AudioWorkgroupError::NoDevice { status: 0 }.to_string(),
            "no Core Audio output device to join (status 0)"
        );
    }
}
