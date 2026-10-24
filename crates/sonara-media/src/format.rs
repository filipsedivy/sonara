//! libsndfile's names for containers and sample encodings (`sf.info().format` and `.subtype`).
//!
//! Project assets store these names (`MediaAsset::format`, `MediaAsset::subtype`), and the
//! prototype chooses between the direct reader and the decode cache by them
//! (`audio/engine/media.py`, `import_export/audio_import.py`). Stored names are plain strings, so
//! the lookups here take strings too; [`MajorFormat`] and [`Subtype`] are what inspection
//! produces.

/// A container ("major format"), with libsndfile's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MajorFormat {
    /// RIFF WAVE with a plain `fmt ` chunk.
    Wav,
    /// RIFF WAVE with `WAVE_FORMAT_EXTENSIBLE`.
    Wavex,
    /// RF64, the 64-bit RIFF WAVE variant (EBU Tech 3306).
    Rf64,
    /// Sony Wave64.
    W64,
    /// AIFF and AIFF-C.
    Aiff,
    /// Apple Core Audio Format.
    Caf,
    /// Native FLAC.
    Flac,
    /// Ogg (Vorbis or Opus).
    Ogg,
    /// MPEG audio (layer I, II or III).
    Mp3,
}

impl MajorFormat {
    /// libsndfile's name, as stored in project assets.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Wav => "WAV",
            Self::Wavex => "WAVEX",
            Self::Rf64 => "RF64",
            Self::W64 => "W64",
            Self::Aiff => "AIFF",
            Self::Caf => "CAF",
            Self::Flac => "FLAC",
            Self::Ogg => "OGG",
            Self::Mp3 => "MP3",
        }
    }
}

/// A sample encoding ("subtype"), with libsndfile's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Subtype {
    /// Signed 8-bit integer PCM.
    PcmS8,
    /// Unsigned 8-bit integer PCM.
    PcmU8,
    /// Signed 16-bit integer PCM.
    Pcm16,
    /// Signed 24-bit integer PCM.
    Pcm24,
    /// Signed 32-bit integer PCM.
    Pcm32,
    /// 32-bit IEEE float.
    Float,
    /// 64-bit IEEE float.
    Double,
    /// G.711 µ-law.
    Ulaw,
    /// G.711 A-law.
    Alaw,
    /// Ogg Vorbis.
    Vorbis,
    /// Ogg Opus.
    Opus,
    /// MPEG-1/2 audio layer I.
    MpegLayerI,
    /// MPEG-1/2 audio layer II.
    MpegLayerII,
    /// MPEG-1/2 audio layer III (MP3).
    MpegLayerIII,
    /// Apple Lossless, 16-bit.
    Alac16,
    /// Apple Lossless, 20-bit.
    Alac20,
    /// Apple Lossless, 24-bit.
    Alac24,
    /// Apple Lossless, 32-bit.
    Alac32,
}

impl Subtype {
    /// libsndfile's name, as stored in project assets.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::PcmS8 => "PCM_S8",
            Self::PcmU8 => "PCM_U8",
            Self::Pcm16 => "PCM_16",
            Self::Pcm24 => "PCM_24",
            Self::Pcm32 => "PCM_32",
            Self::Float => "FLOAT",
            Self::Double => "DOUBLE",
            Self::Ulaw => "ULAW",
            Self::Alaw => "ALAW",
            Self::Vorbis => "VORBIS",
            Self::Opus => "OPUS",
            Self::MpegLayerI => "MPEG_LAYER_I",
            Self::MpegLayerII => "MPEG_LAYER_II",
            Self::MpegLayerIII => "MPEG_LAYER_III",
            Self::Alac16 => "ALAC_16",
            Self::Alac20 => "ALAC_20",
            Self::Alac24 => "ALAC_24",
            Self::Alac32 => "ALAC_32",
        }
    }
}

/// Containers the prototype reads directly when the asset is at the project rate
/// (`media.DIRECT_FORMATS`).
pub const DIRECT_FORMATS: [&str; 6] = ["WAV", "WAVEX", "AIFF", "W64", "RF64", "CAF"];

/// Encodings the prototype reads directly (`media.DIRECT_SUBTYPES`).
pub const DIRECT_SUBTYPES: [&str; 7] = [
    "PCM_16", "PCM_24", "PCM_32", "FLOAT", "DOUBLE", "PCM_S8", "PCM_U8",
];

/// Encodings the importer reports as lossy (`audio_import.LOSSY_SUBTYPES`). `ALAC_16` is in the
/// prototype's set although ALAC is lossless; the flag only adds an import warning.
const LOSSY_SUBTYPES: [&str; 6] = [
    "VORBIS",
    "OPUS",
    "MPEG_LAYER_I",
    "MPEG_LAYER_II",
    "MPEG_LAYER_III",
    "ALAC_16",
];

/// The bit depth the importer reports for an encoding (`audio_import.BIT_DEPTHS`), `None` for
/// the others.
#[must_use]
pub fn bit_depth(subtype: &str) -> Option<u32> {
    match subtype {
        "PCM_S8" | "PCM_U8" => Some(8),
        "PCM_16" | "ALAC_16" => Some(16),
        "ALAC_20" => Some(20),
        "PCM_24" | "ALAC_24" => Some(24),
        "PCM_32" | "FLOAT" | "ALAC_32" => Some(32),
        "DOUBLE" => Some(64),
        _ => None,
    }
}

/// Whether the importer reports an encoding as lossy (`subtype in LOSSY_SUBTYPES`).
#[must_use]
pub fn is_lossy(subtype: &str) -> bool {
    LOSSY_SUBTYPES.contains(&subtype)
}

/// Whether an asset must be decoded into the cache before it plays at `sample_rate`
/// (`MediaProvider.needs_cache`): everything except a direct container and encoding at the
/// project rate. The names are compared upper-cased, as the prototype does.
#[must_use]
pub fn needs_cache(format: &str, subtype: &str, asset_rate: u32, sample_rate: u32) -> bool {
    let format = format.to_uppercase();
    let subtype = subtype.to_uppercase();
    !(asset_rate == sample_rate
        && DIRECT_FORMATS.contains(&format.as_str())
        && DIRECT_SUBTYPES.contains(&subtype.as_str()))
}

#[cfg(test)]
mod tests {
    use super::{MajorFormat, Subtype, bit_depth, is_lossy, needs_cache};

    #[test]
    fn names_are_libsndfiles() {
        assert_eq!(MajorFormat::Wavex.name(), "WAVEX");
        assert_eq!(MajorFormat::Rf64.name(), "RF64");
        assert_eq!(Subtype::Pcm24.name(), "PCM_24");
        assert_eq!(Subtype::MpegLayerIII.name(), "MPEG_LAYER_III");
    }

    #[test]
    fn bit_depths_and_lossiness_follow_the_importer_tables() {
        assert_eq!(bit_depth("PCM_U8"), Some(8));
        assert_eq!(bit_depth("FLOAT"), Some(32));
        assert_eq!(bit_depth("DOUBLE"), Some(64));
        assert_eq!(bit_depth("ALAC_20"), Some(20));
        assert_eq!(bit_depth("VORBIS"), None);
        assert_eq!(bit_depth("ULAW"), None);
        assert!(is_lossy("MPEG_LAYER_III"));
        assert!(is_lossy("ALAC_16"));
        assert!(!is_lossy("ALAC_24"));
        assert!(!is_lossy("PCM_16"));
    }

    #[test]
    fn only_direct_encodings_at_the_project_rate_skip_the_cache() {
        assert!(!needs_cache("WAV", "PCM_24", 48_000, 48_000));
        assert!(!needs_cache("wav", "pcm_16", 48_000, 48_000));
        assert!(!needs_cache("RF64", "DOUBLE", 44_100, 44_100));
        assert!(needs_cache("WAV", "PCM_24", 44_100, 48_000));
        assert!(needs_cache("WAV", "ULAW", 48_000, 48_000));
        assert!(needs_cache("FLAC", "PCM_16", 48_000, 48_000));
        assert!(needs_cache("AU", "PCM_16", 48_000, 48_000));
    }
}
