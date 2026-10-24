//! Python's logging level names, as `--log-level` of `sonara`, `sonara mcp` and
//! `sonara-desktop` reads them (`logging.basicConfig(level=LEVEL.upper())` in the prototype).
//!
//! Rust code logs nothing more severe than `ERROR`, so `CRITICAL` silences everything; `NOTSET`
//! lets everything through. The binaries choose their loggers; this module only maps the names.

use log::{Level, LevelFilter};
use sonara_core::pyfmt::str_repr;

/// A level name Python's `logging` does not know.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Unknown level: {}", str_repr(.0))]
pub struct UnknownLevel(
    /// The name, upper-cased as Python reports it.
    pub String,
);

/// Python's level names (`.upper()`) as `log` filters.
///
/// # Errors
///
/// [`UnknownLevel`] (`Unknown level: 'NAME'`, Python's `ValueError`) for any other name.
pub fn level_filter(name: &str) -> Result<LevelFilter, UnknownLevel> {
    let upper = name.to_uppercase();
    match upper.as_str() {
        "CRITICAL" | "FATAL" => Ok(LevelFilter::Off),
        "ERROR" => Ok(LevelFilter::Error),
        "WARNING" | "WARN" => Ok(LevelFilter::Warn),
        "INFO" => Ok(LevelFilter::Info),
        "DEBUG" => Ok(LevelFilter::Debug),
        "NOTSET" => Ok(LevelFilter::Trace),
        _ => Err(UnknownLevel(upper)),
    }
}

/// Python's `levelname` of a record.
#[must_use]
pub fn level_name(level: Level) -> &'static str {
    match level {
        Level::Error => "ERROR",
        Level::Warn => "WARNING",
        Level::Info => "INFO",
        Level::Debug => "DEBUG",
        Level::Trace => "TRACE",
    }
}

#[cfg(test)]
mod tests {
    use log::{Level, LevelFilter};

    use super::{UnknownLevel, level_filter, level_name};

    #[test]
    fn python_level_names() {
        assert_eq!(level_filter("warning"), Ok(LevelFilter::Warn));
        assert_eq!(level_filter("Debug"), Ok(LevelFilter::Debug));
        assert_eq!(level_filter("CRITICAL"), Ok(LevelFilter::Off));
        assert_eq!(level_filter("notset"), Ok(LevelFilter::Trace));
        let error = level_filter("loud").expect_err("unknown");
        assert_eq!(error, UnknownLevel("LOUD".to_owned()));
        assert_eq!(error.to_string(), "Unknown level: 'LOUD'");
        assert_eq!(level_name(Level::Warn), "WARNING");
    }
}
