//! App Nap opt-out. macOS naps an app whose windows are not frontmost, coalescing its timers:
//! the meter thread's 60 Hz wake-ups then arrive late (measured in the spike while the machine
//! was in use). A DAW keeps its meters and transport live in the background, so the process
//! declares a latency-critical, user-initiated activity for its whole lifetime.

/// The activity token; macOS ends the activity when it is dropped.
#[cfg(target_os = "macos")]
pub type Activity =
    objc2::rc::Retained<objc2::runtime::ProtocolObject<dyn objc2::runtime::NSObjectProtocol>>;

/// Begins the activity; keep the returned token alive for as long as it should last.
#[cfg(target_os = "macos")]
pub fn keep_latency_critical() -> Activity {
    use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};
    let reason = NSString::from_str("Sonara keeps meters and transport live");
    NSProcessInfo::processInfo().beginActivityWithOptions_reason(
        NSActivityOptions::UserInitiatedAllowingIdleSystemSleep
            | NSActivityOptions::LatencyCritical,
        &reason,
    )
}

/// No App Nap outside macOS.
#[cfg(not(target_os = "macos"))]
pub fn keep_latency_critical() {}
