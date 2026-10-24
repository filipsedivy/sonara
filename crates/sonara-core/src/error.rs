//! Typed errors with the prototype's error codes (`sonara.domain.errors`).

use std::fmt;

use serde_json::{Map, Value};

use crate::pyfmt::str_repr;

/// Error codes shared by the domain, the application and the agent interface. The string forms
/// are part of the agent contract (CTR-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// An argument is malformed or out of range.
    InvalidArgument,
    /// The referenced object does not exist.
    NotFound,
    /// The edit conflicts with the current state.
    Conflict,
    /// The routing change would create a cycle.
    RoutingCycle,
    /// The routing change is not allowed.
    InvalidRouting,
    /// The operation is not supported.
    Unsupported,
    /// No project is open.
    NoProject,
    /// The undo history is empty.
    NothingToUndo,
    /// The redo history is empty.
    NothingToRedo,
    /// A file operation failed.
    IoError,
    /// A file has the wrong format.
    FormatError,
    /// Referenced media is missing.
    MediaMissing,
    /// The path is outside the allowed locations.
    PermissionDenied,
    /// The action needs a confirmation token.
    ConfirmationRequired,
    /// A configured limit was exceeded.
    LimitExceeded,
    /// The operation was cancelled.
    Cancelled,
    /// An audio or MIDI device failed.
    DeviceError,
    /// The real-time engine is not available.
    EngineUnavailable,
    /// The operation timed out.
    Timeout,
    /// An unexpected internal failure.
    Internal,
}

impl ErrorCode {
    /// Every code, in the prototype's declaration order.
    pub const ALL: [Self; 20] = [
        Self::InvalidArgument,
        Self::NotFound,
        Self::Conflict,
        Self::RoutingCycle,
        Self::InvalidRouting,
        Self::Unsupported,
        Self::NoProject,
        Self::NothingToUndo,
        Self::NothingToRedo,
        Self::IoError,
        Self::FormatError,
        Self::MediaMissing,
        Self::PermissionDenied,
        Self::ConfirmationRequired,
        Self::LimitExceeded,
        Self::Cancelled,
        Self::DeviceError,
        Self::EngineUnavailable,
        Self::Timeout,
        Self::Internal,
    ];

    /// The code as it appears in tool results, for example `"NOT_FOUND"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidArgument => "INVALID_ARGUMENT",
            Self::NotFound => "NOT_FOUND",
            Self::Conflict => "CONFLICT",
            Self::RoutingCycle => "ROUTING_CYCLE",
            Self::InvalidRouting => "INVALID_ROUTING",
            Self::Unsupported => "UNSUPPORTED",
            Self::NoProject => "NO_PROJECT",
            Self::NothingToUndo => "NOTHING_TO_UNDO",
            Self::NothingToRedo => "NOTHING_TO_REDO",
            Self::IoError => "IO_ERROR",
            Self::FormatError => "FORMAT_ERROR",
            Self::MediaMissing => "MEDIA_MISSING",
            Self::PermissionDenied => "PERMISSION_DENIED",
            Self::ConfirmationRequired => "CONFIRMATION_REQUIRED",
            Self::LimitExceeded => "LIMIT_EXCEEDED",
            Self::Cancelled => "CANCELLED",
            Self::DeviceError => "DEVICE_ERROR",
            Self::EngineUnavailable => "ENGINE_UNAVAILABLE",
            Self::Timeout => "TIMEOUT",
            Self::Internal => "INTERNAL",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An expected, reportable error: a broken rule of the project model, a missing object, a
/// failed file operation. It carries a code, a human-readable message and structured details.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("[{code}] {message}")]
pub struct DomainError {
    /// What kind of failure this is.
    pub code: ErrorCode,
    /// The message, worded as in the prototype.
    pub message: String,
    /// Structured details, for example the allowed values.
    pub details: Map<String, Value>,
}

impl DomainError {
    /// An error with no details.
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: Map::new(),
        }
    }

    /// An [`ErrorCode::InvalidArgument`] error.
    #[must_use]
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidArgument, message)
    }

    /// The prototype's `NotFoundError`: `"<kind> '<id>' not found"` with `kind` and `id`
    /// details.
    #[must_use]
    pub fn not_found(kind: &str, id: &str) -> Self {
        Self::new(
            ErrorCode::NotFound,
            format!("{kind} {} not found", str_repr(id)),
        )
        .with_detail("kind", kind)
        .with_detail("id", id)
    }

    /// Adds one detail.
    #[must_use]
    pub fn with_detail(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.details.insert(key.to_owned(), value.into());
        self
    }
}

/// Results of domain operations.
pub type Result<T, E = DomainError> = std::result::Result<T, E>;

/// Fails with an [`ErrorCode::InvalidArgument`] error when `condition` is false (the prototype's
/// `require`).
///
/// # Errors
///
/// [`ErrorCode::InvalidArgument`] with `message`.
pub fn require(condition: bool, message: impl FnOnce() -> String) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(DomainError::invalid(message()))
    }
}

/// Fails unless `value` is finite (the prototype's `require_finite`).
///
/// # Errors
///
/// [`ErrorCode::InvalidArgument`]: `"<name> must be a finite number"`.
pub fn require_finite(value: f64, name: &str) -> Result<()> {
    require(value.is_finite(), || {
        format!("{name} must be a finite number")
    })
}

#[cfg(test)]
mod tests {
    use super::{DomainError, ErrorCode};

    #[test]
    fn not_found_matches_the_prototype_message() {
        let err = DomainError::not_found("track", "trk_1");
        assert_eq!(err.message, "track 'trk_1' not found");
        assert_eq!(err.to_string(), "[NOT_FOUND] track 'trk_1' not found");
        assert_eq!(err.details["kind"], "track");
    }

    #[test]
    fn codes_are_unique_strings() {
        let mut names: Vec<&str> = ErrorCode::ALL.iter().map(|c| c.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ErrorCode::ALL.len());
    }
}
