//! Errors of the export, and their mapping onto the prototype's error codes and messages.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use sonara_core::error::{DomainError, ErrorCode};

use crate::codec_ffi::CodecError;
use crate::error::MediaError;

/// An export failed.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// An argument is invalid (`INVALID_ARGUMENT`), for example no outputs.
    #[error("{0}")]
    InvalidArgument(String),
    /// The output cannot be written in this format (`UNSUPPORTED`).
    #[error("{0}")]
    Unsupported(String),
    /// The output path conflicts with an existing file or directory (`CONFLICT`).
    #[error("{message}")]
    Conflict {
        /// The prototype's message.
        message: String,
        /// The path reported in the details (only for an existing file).
        path: Option<PathBuf>,
    },
    /// The caller cancelled the export (`CANCELLED`).
    #[error("export cancelled")]
    Cancelled,
    /// A written file does not read back as written (`IO_ERROR`).
    #[error("{0}")]
    Verification(String),
    /// A file operation failed (`IO_ERROR`).
    #[error("cannot {operation} {}: {source}", path.display())]
    Io {
        /// What was attempted, for example `"write"`.
        operation: &'static str,
        /// The file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// A file has the wrong layout (`FORMAT_ERROR`), for example a BWF target that is not
    /// RIFF WAVE.
    #[error("{0}")]
    Format(String),
    /// Reading a written file back failed.
    #[error(transparent)]
    Media(#[from] MediaError),
    /// A codec library is missing or failed (`INTERNAL`).
    #[error(transparent)]
    Codec(#[from] CodecError),
    /// An encoder failed (`INTERNAL`).
    #[error("{format} encoder: {message}")]
    Encoder {
        /// The output format, for example `"flac"`.
        format: &'static str,
        /// The encoder's message.
        message: String,
    },
}

impl ExportError {
    /// The prototype's error code for this failure.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidArgument(_) => ErrorCode::InvalidArgument,
            Self::Unsupported(_) => ErrorCode::Unsupported,
            Self::Conflict { .. } => ErrorCode::Conflict,
            Self::Cancelled => ErrorCode::Cancelled,
            Self::Verification(_) | Self::Io { .. } => ErrorCode::IoError,
            Self::Format(_) => ErrorCode::FormatError,
            Self::Media(e) => e.code(),
            Self::Codec(_) | Self::Encoder { .. } => ErrorCode::Internal,
        }
    }

    /// The error as the prototype's `SonaraError`: code, message and details (`path` for an
    /// existing output file).
    #[must_use]
    pub fn to_domain_error(&self) -> DomainError {
        if let Self::Media(e) = self {
            return e.to_domain_error();
        }
        let mut details = Map::new();
        if let Self::Conflict {
            path: Some(path), ..
        } = self
        {
            details.insert("path".to_owned(), Value::String(path.display().to_string()));
        }
        DomainError {
            code: self.code(),
            message: self.to_string(),
            details,
        }
    }

    pub(crate) fn io(operation: &'static str, path: &Path, source: io::Error) -> Self {
        Self::Io {
            operation,
            path: path.to_owned(),
            source,
        }
    }
}
