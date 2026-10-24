//! Wall-clock time stamps of the application services.
//!
//! The prototype reads `datetime.now()`, `time.time()` and `time.strftime()`; the golden
//! recorder freezes the first and the last at 2026-01-01T12:00:00Z. A [`WallClock`] makes the
//! same freeze possible here.
//!
//! **Local time.** `time.strftime` formats the local time of the process (preview file names).
//! The recorder replaces it with a formatter of its frozen UTC moment, so golden runs see UTC
//! whatever the machine's time zone. [`LocalZone`] chooses between the two without touching the
//! process environment: [`LocalZone::System`] (the default) reads the system time zone as libc
//! does (`TZ`, else `/etc/localtime`), and [`LocalZone::Utc`] is the golden runs' choice.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use jiff::Timestamp;
use jiff::civil::DateTime;
use jiff::tz::{Offset, TimeZone};

/// A source of the current wall-clock time.
pub type WallClock = Arc<dyn Fn() -> SystemTime + Send + Sync>;

/// The system clock.
#[must_use]
pub fn system_clock() -> WallClock {
    Arc::new(SystemTime::now)
}

/// The time zone of local time stamps (`time.strftime`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LocalZone {
    /// The system's time zone: `TZ`, else the system setting (`/etc/localtime`); UTC when
    /// neither can be read. On Windows the setting maps to an IANA zone that needs a time zone
    /// database, which this build does not bundle, so the stamp is UTC there unless `TZ` names
    /// a POSIX rule.
    #[default]
    System,
    /// UTC, as the golden recorder's frozen `time.strftime` formats.
    Utc,
}

/// `time` as a timestamp; times beyond jiff's range (years ±9999) are clamped, as Python's
/// `datetime` cannot represent them either.
fn timestamp(time: SystemTime) -> Timestamp {
    Timestamp::try_from(time).unwrap_or(if time < UNIX_EPOCH {
        Timestamp::MIN
    } else {
        Timestamp::MAX
    })
}

/// `time.strftime("%Y%m%d-%H%M%S")` of `time` in `zone`: the stamp of a preview file name.
#[must_use]
pub fn local_stamp(time: SystemTime, zone: LocalZone) -> String {
    let t: DateTime = match zone {
        LocalZone::System => TimeZone::system().to_datetime(timestamp(time)),
        LocalZone::Utc => Offset::UTC.to_datetime(timestamp(time)),
    };
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        t.year(),
        t.month(),
        t.day(),
        t.hour(),
        t.minute(),
        t.second()
    )
}

/// `strftime("%Y-%m-%dT%H:%M:%SZ")` of `time` in UTC, as the prototype stamps a new project's
/// `created` metadata. Times beyond jiff's range (years ±9999) are clamped; Python's `datetime`
/// cannot represent them either.
#[must_use]
pub fn iso_seconds(time: SystemTime) -> String {
    let t = Offset::UTC.to_datetime(timestamp(time));
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        t.year(),
        t.month(),
        t.day(),
        t.hour(),
        t.minute(),
        t.second()
    )
}

/// Seconds since the Unix epoch, as Python's `time.time()` (negative before 1970).
#[must_use]
pub fn unix_seconds(time: SystemTime) -> f64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => after.as_secs_f64(),
        Err(before) => -before.duration().as_secs_f64(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::{LocalZone, iso_seconds, local_stamp, unix_seconds};

    #[test]
    fn stamps_match_python() {
        // 2026-01-01T12:00:00Z plus 0.25 s.
        let time = UNIX_EPOCH + Duration::new(1_767_268_800, 250_000_000);
        assert_eq!(iso_seconds(time), "2026-01-01T12:00:00Z");
        // The recorder's frozen `time.strftime("%Y%m%d-%H%M%S")`.
        assert_eq!(local_stamp(time, LocalZone::Utc), "20260101-120000");
        // The system zone is whatever the machine has; the stamp keeps its shape, and its
        // seconds match UTC's (every zone's offset is whole minutes).
        let local = local_stamp(time, LocalZone::System);
        assert_eq!(local.len(), 15);
        assert_eq!(local.as_bytes()[8], b'-');
        assert!(local.ends_with("00"), "{local}");
        assert!((unix_seconds(time) - 1_767_268_800.25).abs() < 1e-6);
        assert!((unix_seconds(UNIX_EPOCH - Duration::from_secs(2)) + 2.0).abs() < 1e-9);
    }
}
