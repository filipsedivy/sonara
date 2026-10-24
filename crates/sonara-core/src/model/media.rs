//! Media assets referenced by clips and devices (`sonara.domain.media`).
//!
//! Embedded assets live inside the project bundle at a relative POSIX path under `media/`.
//! Linked assets live outside it; their `path` is absolute and `relative_hint` records the path
//! relative to the bundle at save time, which relinking uses.

use super::{JsonMap, Valid, Validate};
use crate::error::{Result, require};
use crate::ids::{self, IdGenerator};
use crate::pyfmt::str_repr;

/// What an asset contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaKind {
    /// Audio for clips.
    Audio,
    /// A MIDI file.
    Midi,
    /// An impulse response for the convolver.
    Impulse,
}

impl MediaKind {
    /// The kind as stored in project files.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Midi => "midi",
            Self::Impulse => "impulse",
        }
    }

    /// Parses the stored form.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "audio" => Some(Self::Audio),
            "midi" => Some(Self::Midi),
            "impulse" => Some(Self::Impulse),
            _ => None,
        }
    }
}

/// Where an asset's file lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaLocation {
    /// Inside the bundle, under `media/`.
    Embedded,
    /// Outside the bundle, at an absolute path.
    Linked,
}

impl MediaLocation {
    /// The location as stored in project files.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::Linked => "linked",
        }
    }

    /// Parses the stored form.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "embedded" => Some(Self::Embedded),
            "linked" => Some(Self::Linked),
            _ => None,
        }
    }
}

/// The data of a [`MediaAsset`].
#[derive(Debug, Clone, PartialEq)]
pub struct MediaAssetData {
    /// Stable ID (`med_…`).
    pub id: String,
    /// What the file contains.
    pub kind: MediaKind,
    /// Display name.
    pub name: String,
    /// Bundle-relative POSIX path (embedded) or absolute path (linked).
    pub path: String,
    /// Where the file lives.
    pub location: MediaLocation,
    /// File size in bytes.
    pub size_bytes: u64,
    /// SHA-256 of the file, lowercase hex.
    pub sha256: Option<String>,
    /// Sample rate of audio, 0 otherwise.
    pub sample_rate: u32,
    /// Channel count of audio, 0 otherwise.
    pub channels: u32,
    /// Frame count of audio.
    pub frames: u64,
    /// Container format (`WAV`, `FLAC`, …).
    pub format: String,
    /// Sample encoding (`PCM_24`, `FLOAT`, …).
    pub subtype: String,
    /// The path the file was imported from.
    pub original_path: Option<String>,
    /// A linked file's path relative to the bundle at save time.
    pub relative_hint: Option<String>,
    /// Analysis and tag data.
    pub metadata: JsonMap,
    /// Unknown keys preserved from newer project files.
    pub extra: JsonMap,
}

impl Validate for MediaAssetData {
    fn validate(&mut self) -> Result<()> {
        require(!self.path.is_empty(), || {
            "asset path must not be empty".to_owned()
        })?;
        if self.location == MediaLocation::Embedded {
            // PurePosixPath.parts: empty and "." segments vanish, ".." stays.
            let mut parts = self.path.split('/').filter(|p| !p.is_empty() && *p != ".");
            let inside_media = !self.path.starts_with('/')
                && parts.next() == Some("media")
                && parts.all(|p| p != "..");
            require(inside_media, || {
                format!(
                    "embedded asset path must be relative and inside media/: {}",
                    str_repr(&self.path)
                )
            })?;
        }
        if matches!(self.kind, MediaKind::Audio | MediaKind::Impulse) {
            require(self.sample_rate > 0, || {
                "audio asset needs a sample rate".to_owned()
            })?;
            require(self.channels > 0, || {
                "audio asset needs a channel count".to_owned()
            })?;
        }
        Ok(())
    }
}

/// A media file used by the project.
pub type MediaAsset = Valid<MediaAssetData>;

impl MediaAsset {
    /// A fresh asset ID.
    #[must_use]
    pub fn new_id(ids: &mut dyn IdGenerator) -> String {
        ids.new_id(ids::ASSET)
    }

    /// Duration of audio in seconds (0 without a sample rate).
    #[must_use]
    pub fn duration_seconds(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            #[expect(
                clippy::cast_precision_loss,
                reason = "Python's frames / sample_rate rounds the same way"
            )]
            let frames = self.frames as f64;
            frames / f64::from(self.sample_rate)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MediaAsset, MediaAssetData, MediaKind, MediaLocation};
    use crate::model::JsonMap;

    fn asset(path: &str) -> MediaAssetData {
        MediaAssetData {
            id: "med_1".into(),
            kind: MediaKind::Audio,
            name: "Kick".into(),
            path: path.into(),
            location: MediaLocation::Embedded,
            size_bytes: 10,
            sha256: None,
            sample_rate: 48_000,
            channels: 2,
            frames: 96_000,
            format: "WAV".into(),
            subtype: "PCM_24".into(),
            original_path: None,
            relative_hint: None,
            metadata: JsonMap::new(),
            extra: JsonMap::new(),
        }
    }

    #[test]
    fn embedded_paths_stay_inside_media() {
        let ok = MediaAsset::new(asset("./media//kick.wav")).expect("inside media");
        assert!((ok.duration_seconds() - 2.0).abs() < f64::EPSILON);
        for bad in ["/media/kick.wav", "media/../kick.wav", "audio/kick.wav"] {
            let err = MediaAsset::new(asset(bad)).expect_err(bad);
            assert!(
                err.message.starts_with("embedded asset path must be"),
                "{bad}"
            );
        }
    }
}
