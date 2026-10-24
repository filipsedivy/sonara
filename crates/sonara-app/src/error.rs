//! Errors of the application services (ERR-03, ERR-04).
//!
//! The prototype's `SonaraError`s are [`AppError::Domain`], with their code, message and
//! details. Failures the prototype let escape as raw Python exceptions, which its tool registry
//! reports as `INTERNAL`, are the other variants: they name the operation and keep the cause.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::model::JsonMap;
use sonara_engine::EngineError;
use sonara_media::export::ExportError;
use sonara_project::store::StoreError;

use crate::paths::NoHomeDirectory;

/// Errors of the application services.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// An error the prototype reports with its code, message and details.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// A file-system or ZIP failure of the project store.
    #[error(transparent)]
    Store(StoreError),
    /// A file-system operation of a service failed.
    #[error("cannot {operation} {}: {source}", path.display())]
    Io {
        /// What was attempted, for example `"copy"`.
        operation: &'static str,
        /// The file or directory.
        path: PathBuf,
        /// The cause.
        source: io::Error,
    },
    /// The application directories are unknown.
    #[error(transparent)]
    NoHome(#[from] NoHomeDirectory),
    /// The job worker threads could not be started.
    #[error("cannot start the job workers: {0}")]
    Workers(#[source] io::Error),
    /// A render failed outside the project's rules: a clip's media could not be read
    /// (`IO_ERROR`), or a device's stored parameters do not convert (`INTERNAL`). The project's
    /// own rules (an empty range, an unknown track, cancellation) are [`AppError::Domain`].
    #[error(transparent)]
    Render(EngineError),
    /// Writing, encoding or reading back an export, or reading a file to analyze, failed:
    /// I/O (`IO_ERROR`), a codec or encoder (`INTERNAL`), or the media layer (its own code).
    /// The export's rules (arguments, formats, conflicts, cancellation) are
    /// [`AppError::Domain`].
    #[error(transparent)]
    Export(ExportError),
}

impl From<EngineError> for AppError {
    /// The render's domain errors become [`AppError::Domain`]; the others keep their cause.
    fn from(err: EngineError) -> Self {
        match err {
            EngineError::Domain(err) => Self::Domain(err),
            other => Self::Render(other),
        }
    }
}

impl From<ExportError> for AppError {
    /// The export's rule violations become [`AppError::Domain`] with the prototype's code,
    /// message and details; failures with a cause keep it.
    fn from(err: ExportError) -> Self {
        match err {
            ExportError::Io { .. }
            | ExportError::Media(_)
            | ExportError::Codec(_)
            | ExportError::Encoder { .. } => Self::Export(err),
            other => Self::Domain(other.to_domain_error()),
        }
    }
}

impl From<StoreError> for AppError {
    /// The store's domain errors become [`AppError::Domain`], so that callers match one variant.
    fn from(err: StoreError) -> Self {
        match err {
            StoreError::Domain(err) => Self::Domain(err),
            other => Self::Store(other),
        }
    }
}

impl AppError {
    /// The code a tool result reports: the domain error's code, or [`ErrorCode::Internal`] for
    /// every other failure, as the prototype's tool registry reports escaped exceptions.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Domain(err) => err.code,
            Self::Render(err) => err.code(),
            Self::Export(err) => err.code(),
            Self::Store(_) | Self::Io { .. } | Self::NoHome(_) | Self::Workers(_) => {
                ErrorCode::Internal
            }
        }
    }

    /// The domain error, if this is one.
    #[must_use]
    pub fn as_domain(&self) -> Option<&DomainError> {
        match self {
            Self::Domain(err) => Some(err),
            _ => None,
        }
    }

    /// What a tool result reports about this error: the domain error's code, message and
    /// details; a render or export failure's own code with its text (and details); or
    /// [`ErrorCode::Internal`] with this error's text.
    #[must_use]
    pub fn report(&self) -> ErrorReport {
        match self {
            Self::Domain(err) => ErrorReport {
                code: err.code,
                message: err.message.clone(),
                details: err.details.clone(),
            },
            Self::Render(err) => ErrorReport {
                code: err.code(),
                message: err.to_string(),
                details: JsonMap::new(),
            },
            Self::Export(err) => {
                let err = err.to_domain_error();
                ErrorReport {
                    code: err.code,
                    message: err.message,
                    details: err.details,
                }
            }
            other => ErrorReport {
                code: ErrorCode::Internal,
                message: other.to_string(),
                details: JsonMap::new(),
            },
        }
    }

    /// The prototype's `SonaraError.to_dict()` of this error (see [`ErrorReport::to_json`]).
    #[must_use]
    pub fn to_json(&self) -> Value {
        self.report().to_json()
    }
}

/// A reported error as plain data, for example the error a failed job keeps.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorReport {
    /// The error code.
    pub code: ErrorCode,
    /// The message, worded as in the prototype.
    pub message: String,
    /// Structured details.
    pub details: JsonMap,
}

impl ErrorReport {
    /// The prototype's `SonaraError.to_dict()`: `code`, `message`, and `details` when there are
    /// any.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut out = json!({"code": self.code.as_str(), "message": self.message});
        if !self.details.is_empty() {
            out["details"] = Value::Object(self.details.clone());
        }
        out
    }
}

/// Results of the application services.
pub type AppResult<T> = Result<T, AppError>;

/// Adds the operation and path to an I/O error.
pub(crate) trait IoContext<T> {
    fn io(self, operation: &'static str, path: &Path) -> AppResult<T>;
}

impl<T> IoContext<T> for io::Result<T> {
    fn io(self, operation: &'static str, path: &Path) -> AppResult<T> {
        self.map_err(|source| AppError::Io {
            operation,
            path: path.to_path_buf(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::PathBuf;

    use serde_json::json;
    use sonara_core::error::{DomainError, ErrorCode};
    use sonara_project::store::StoreError;

    use super::AppError;

    #[test]
    fn domain_errors_keep_code_message_and_details() {
        let err = AppError::from(StoreError::Domain(
            DomainError::new(ErrorCode::MediaMissing, "gone").with_detail("missing", ["a"]),
        ));
        assert_eq!(err.code(), ErrorCode::MediaMissing);
        assert_eq!(
            err.to_json(),
            json!({"code": "MEDIA_MISSING", "message": "gone", "details": {"missing": ["a"]}})
        );
        let plain = AppError::from(DomainError::new(
            ErrorCode::Cancelled,
            "operation cancelled",
        ));
        assert_eq!(
            plain.to_json(),
            json!({"code": "CANCELLED", "message": "operation cancelled"})
        );
    }

    #[test]
    fn other_failures_are_internal_and_keep_the_cause() {
        let err = AppError::Io {
            operation: "copy",
            path: PathBuf::from("/x/a.wav"),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        };
        assert_eq!(err.code(), ErrorCode::Internal);
        assert_eq!(err.to_string(), "cannot copy /x/a.wav: denied");
        assert!(std::error::Error::source(&err).is_some());
        assert_eq!(err.to_json()["code"], "INTERNAL");
        assert!(err.as_domain().is_none());
    }
}
