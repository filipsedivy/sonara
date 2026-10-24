//! Errors of the media layer, and their mapping onto the prototype's error codes.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::pyfmt::str_repr;

use crate::codec_ffi::CodecError;

/// Why libsndfile refuses to open a file, for the readers that follow its rules (CAF and Ogg
/// Opus). The texts are libsndfile 1.2.2's (`sf_error_number`), because the importer's
/// `UNSUPPORTED` message embeds them (CTR-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// `SFE_MALFORMED_FILE`.
    Malformed,
    /// `SFE_UNSUPPORTED_ENCODING`.
    UnsupportedEncoding,
    /// `SFE_UNIMPLEMENTED`.
    Unimplemented,
    /// `SFE_INTERNAL`.
    Internal,
    /// `SFE_CAF_NOT_CAF`.
    CafNotCaf,
    /// `SFE_CAF_NO_DESC`.
    CafNoDesc,
    /// `SFE_CAF_BAD_PEAK`.
    CafBadPeak,
}

impl Rejection {
    /// libsndfile's message.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::Malformed => "Supported file format but file is malformed.",
            Self::UnsupportedEncoding => "Supported file format but unsupported encoding.",
            Self::Unimplemented => "File contains data in an unimplemented format.",
            Self::Internal => "Unspecified internal error.",
            Self::CafNotCaf => "Error : Not a CAF file.",
            Self::CafNoDesc => "Error : No 'desc' marker in CAF file.",
            Self::CafBadPeak => "Error : Bad 'PEAK' chunk in CAF file.",
        }
    }
}

/// A media operation failed.
#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    /// A file operation failed.
    #[error("cannot {operation} {}: {source}", path.display())]
    Io {
        /// What was attempted, for example `"open"`.
        operation: &'static str,
        /// The file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// No reader recognises the file. The text is libsndfile's, as soundfile reports it, because
    /// the importer's `UNSUPPORTED` message embeds it (golden coverage transcript, CTR-01).
    #[error("Error opening {}: Format not recognised.", str_repr(&path.display().to_string()))]
    NotRecognised {
        /// The file.
        path: PathBuf,
    },
    /// libsndfile refuses to open the file, for the reason given; the text is libsndfile's, as
    /// soundfile reports it.
    #[error("Error opening {}: {}", str_repr(&path.display().to_string()), reason.message())]
    Rejected {
        /// The file.
        path: PathBuf,
        /// libsndfile's reason.
        reason: Rejection,
    },
    /// The container is recognised, but one of its features is not supported.
    #[error("{}: unsupported {what}", path.display())]
    Unsupported {
        /// The file.
        path: PathBuf,
        /// The unsupported feature, for example `"codec Opus"`.
        what: String,
    },
    /// The file is malformed.
    #[error("{}: {message}", path.display())]
    Malformed {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        message: String,
    },
    /// The decoder failed.
    #[error("cannot decode {}: {source}", path.display())]
    Decode {
        /// The file.
        path: PathBuf,
        /// The decoder's error.
        #[source]
        source: symphonia::core::errors::Error,
    },
    /// A codec library is missing or reported an error.
    #[error(transparent)]
    Codec(#[from] CodecError),
    /// A decode-cache entry cannot be used.
    #[error("invalid decode cache {}: {message}", path.display())]
    Cache {
        /// The sidecar or data file.
        path: PathBuf,
        /// What is wrong.
        message: String,
    },
    /// The asset's file does not exist (`MEDIA_MISSING`).
    #[error("media file for {} is missing", str_repr(name))]
    Missing {
        /// The asset's display name.
        name: String,
        /// The resolved path.
        path: PathBuf,
    },
    /// No cache directory is configured (`INTERNAL`).
    #[error("no media cache directory configured")]
    NoCacheDir,
    /// The caller cancelled the operation (`CANCELLED`).
    #[error("media decoding cancelled")]
    Cancelled,
    /// An argument is out of range, for example a channel count of zero.
    #[error("{0}")]
    InvalidArgument(String),
}

impl MediaError {
    /// The prototype's error code for this failure.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Io { .. } | Self::Cache { .. } => ErrorCode::IoError,
            Self::NotRecognised { .. } | Self::Unsupported { .. } => ErrorCode::Unsupported,
            Self::Rejected { reason, .. } => match reason {
                Rejection::UnsupportedEncoding | Rejection::Unimplemented => ErrorCode::Unsupported,
                _ => ErrorCode::FormatError,
            },
            Self::Malformed { .. } | Self::Decode { .. } => ErrorCode::FormatError,
            Self::Codec(_) | Self::NoCacheDir => ErrorCode::Internal,
            Self::Missing { .. } => ErrorCode::MediaMissing,
            Self::Cancelled => ErrorCode::Cancelled,
            Self::InvalidArgument(_) => ErrorCode::InvalidArgument,
        }
    }

    /// The error as the prototype's `SonaraError`: code, message and details. Only
    /// [`MediaError::Missing`] carries details (`path`), as in `MediaProvider.prepare`.
    #[must_use]
    pub fn to_domain_error(&self) -> DomainError {
        let mut details = Map::new();
        if let Self::Missing { path, .. } = self {
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

    pub(crate) fn malformed(path: &Path, message: impl Into<String>) -> Self {
        Self::Malformed {
            path: path.to_owned(),
            message: message.into(),
        }
    }

    pub(crate) fn rejected(path: &Path, reason: Rejection) -> Self {
        Self::Rejected {
            path: path.to_owned(),
            reason,
        }
    }

    pub(crate) fn unsupported(path: &Path, what: impl Into<String>) -> Self {
        Self::Unsupported {
            path: path.to_owned(),
            what: what.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use sonara_core::error::ErrorCode;

    use super::{MediaError, Rejection};

    #[test]
    fn a_refused_file_reads_like_libsndfile() {
        let err = MediaError::rejected(std::path::Path::new("/in/cut.caf"), Rejection::Malformed);
        assert_eq!(
            err.to_string(),
            "Error opening '/in/cut.caf': Supported file format but file is malformed."
        );
        assert_eq!(err.code(), ErrorCode::FormatError);
        let err = MediaError::rejected(
            std::path::Path::new("/in/x.caf"),
            Rejection::UnsupportedEncoding,
        );
        assert_eq!(err.code(), ErrorCode::Unsupported);
    }

    #[test]
    fn an_unrecognised_file_reads_like_libsndfile() {
        let err = MediaError::NotRecognised {
            path: PathBuf::from("/tmp/in/not-media.wav"),
        };
        assert_eq!(
            err.to_string(),
            "Error opening '/tmp/in/not-media.wav': Format not recognised."
        );
        assert_eq!(err.code(), ErrorCode::Unsupported);
    }

    #[test]
    fn a_missing_file_keeps_the_prototypes_code_message_and_path() {
        let err = MediaError::Missing {
            name: "Kick".to_owned(),
            path: PathBuf::from("/p/media/kick.wav"),
        };
        let domain = err.to_domain_error();
        assert_eq!(domain.code, ErrorCode::MediaMissing);
        assert_eq!(domain.message, "media file for 'Kick' is missing");
        assert_eq!(domain.details["path"], "/p/media/kick.wav");
        assert_eq!(MediaError::Cancelled.code(), ErrorCode::Cancelled);
        assert_eq!(
            MediaError::NoCacheDir.to_domain_error().message,
            "no media cache directory configured"
        );
    }
}
