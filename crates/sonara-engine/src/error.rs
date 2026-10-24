//! Errors of building and running a render (the prototype raises `SonaraError` for the same
//! cases, so each variant maps to its error code).

use sonara_core::error::{DomainError, ErrorCode};
use sonara_dsp::processors::ParamSlotsError;
use sonara_media::MediaError;

/// Why a render graph could not be built, or a render could not run.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// A rule of the project or the request, with the prototype's code and message: an empty
    /// range (`INVALID_ARGUMENT`), a render over the length limit (`LIMIT_EXCEEDED`), an unknown
    /// target track (`NOT_FOUND`), a routing cycle (`ROUTING_CYCLE`), an automation lane on a
    /// parameter the plugin does not have (`INVALID_ARGUMENT`) or a cancelled render
    /// (`CANCELLED`).
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// A device's stored parameter values do not convert into its processor's parameter set.
    #[error("cannot set up device {device}: {source}")]
    Params {
        /// The device ID.
        device: String,
        /// The conversion error.
        #[source]
        source: ParamSlotsError,
    },
    /// Media could not be read: an audio clip during the render, or an impulse response while
    /// the graph is built.
    #[error("cannot read the media of {asset}: {source}")]
    Media {
        /// The asset ID.
        asset: String,
        /// The read error.
        #[source]
        source: MediaError,
    },
}

impl EngineError {
    /// The error code the agent interface reports: the domain error's own code, `INTERNAL` for
    /// a parameter set that does not convert (the project's values were validated when they
    /// were stored), and `IO_ERROR` for unreadable media.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Domain(err) => err.code,
            Self::Params { .. } => ErrorCode::Internal,
            Self::Media { .. } => ErrorCode::IoError,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use sonara_core::error::{DomainError, ErrorCode};
    use sonara_media::MediaError;

    use super::EngineError;

    #[test]
    fn codes_follow_the_cause() {
        let domain = EngineError::from(DomainError::new(ErrorCode::Cancelled, "render cancelled"));
        assert_eq!(domain.code(), ErrorCode::Cancelled);
        assert_eq!(domain.to_string(), "[CANCELLED] render cancelled");
        let media = EngineError::Media {
            asset: "med_1".to_owned(),
            source: MediaError::InvalidArgument("a partial frame".to_owned()),
        };
        assert_eq!(media.code(), ErrorCode::IoError);
        assert!(media.source().is_some(), "the cause is kept (ERR-04)");
    }
}
