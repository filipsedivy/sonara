//! The importer's naming rules (`import_export/audio_import.py`): `safe_stem`, the paths of
//! imported files and recordings inside the project, and the import warnings.

use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::model::media::MediaKind;
use unicode_normalization::UnicodeNormalization;

use crate::inspect::AudioFileInfo;

/// The largest file the importer accepts (`MAX_IMPORT_BYTES`, 8 GiB).
pub const MAX_IMPORT_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// `safe_stem(name, limit)`: NFKD-normalized, non-ASCII dropped, every run of characters other
/// than `A-Z a-z 0-9 . _ -` replaced by one `-`, leading and trailing `-` and `.` stripped,
/// `"audio"` when nothing is left, then cut to `limit` characters.
///
/// The NFKD tables are `unicode-normalization`'s, which may track a newer Unicode version than
/// the prototype's Python; characters added in between can normalize differently.
#[must_use]
pub fn safe_stem(name: &str, limit: usize) -> String {
    let mut text = String::with_capacity(name.len());
    let mut in_run = false;
    for c in name.nfkd().filter(char::is_ascii) {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
            text.push(c);
            in_run = false;
        } else if !in_run {
            text.push('-');
            in_run = true;
        }
    }
    let text = text.trim_matches(['-', '.']);
    let text = if text.is_empty() { "audio" } else { text };
    text.chars().take(limit).collect()
}

/// The last component of a `/`-separated path.
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The position of the suffix dot by Python's `PurePath.suffix` rule (3.12 and 3.13): the last
/// `.` that is neither the first nor the last character.
fn suffix_dot(name: &str) -> Option<usize> {
    name.rfind('.').filter(|&i| i > 0 && i + 1 < name.len())
}

/// Python's `PurePath(path).stem`.
#[must_use]
pub fn path_stem(path: &str) -> &str {
    let name = file_name(path);
    suffix_dot(name).map_or(name, |i| &name[..i])
}

/// Python's `PurePath(path).suffix`.
#[must_use]
pub fn path_suffix(path: &str) -> &str {
    let name = file_name(path);
    suffix_dot(name).map_or("", |i| &name[i..])
}

/// The media folder of an asset kind: `impulses` for impulse responses, `audio` otherwise.
fn folder(kind: MediaKind) -> &'static str {
    if kind == MediaKind::Impulse {
        "impulses"
    } else {
        "audio"
    }
}

/// Where an imported copy lives in the project: `media/{folder}/{asset_id}-{safe_stem(stem)}`
/// followed by the source's lower-cased suffix.
#[must_use]
pub fn import_path(asset_id: &str, kind: MediaKind, source: &str) -> String {
    format!(
        "media/{}/{asset_id}-{}{}",
        folder(kind),
        safe_stem(path_stem(source), 48),
        path_suffix(source).to_lowercase()
    )
}

/// Where the `FFmpeg` fallback puts an import in the project (`import_audio`'s `decoder ==
/// "ffmpeg"` branch): the FLAC transcode at `media/{folder}/{asset_id}-{safe_stem(stem)}.flac`,
/// and the original at `media/{folder}/originals/{asset_id}-{safe_stem(stem)}` with the source's
/// lower-cased suffix, as `(transcode, original)`.
#[must_use]
pub fn transcode_paths(asset_id: &str, kind: MediaKind, source: &str) -> (String, String) {
    let folder = folder(kind);
    let stem = safe_stem(path_stem(source), 48);
    (
        format!("media/{folder}/{asset_id}-{stem}.flac"),
        format!(
            "media/{folder}/originals/{asset_id}-{stem}{}",
            path_suffix(source).to_lowercase()
        ),
    )
}

/// The warning of an import the `FFmpeg` fallback transcoded.
#[must_use]
pub fn transcode_warning(source_name: &str) -> String {
    format!(
        "{source_name} was decoded with FFmpeg into lossless FLAC; the original is kept in the \
         project"
    )
}

/// Where a new recording lives in the project (`write_recording`):
/// `media/audio/{asset_id}-{safe_stem(name)}.wav`.
#[must_use]
pub fn recording_path(asset_id: &str, name: &str) -> String {
    format!("media/audio/{asset_id}-{}.wav", safe_stem(name, 48))
}

/// The importer's warnings for a file named `source_name`: more than two channels, and a lossy
/// encoding.
#[must_use]
pub fn import_warnings(source_name: &str, info: &AudioFileInfo) -> Vec<String> {
    let mut warnings = Vec::new();
    if info.channels > 2 {
        warnings.push(format!(
            "{source_name} has {} channels; Sonara's mixer is stereo, so playback folds them to \
             stereo (even channels left, odd right). The original file is preserved.",
            info.channels
        ));
    }
    if info.lossy {
        warnings.push(format!(
            "{source_name} is lossy ({}); it is kept as-is and decoded for playback",
            info.subtype
        ));
    }
    warnings
}

/// Fails with the importer's `LIMIT_EXCEEDED` error when a file is larger than
/// [`MAX_IMPORT_BYTES`].
///
/// # Errors
///
/// [`ErrorCode::LimitExceeded`]: `"{name} is larger than 8 GB"`.
pub fn check_import_size(source_name: &str, size_bytes: u64) -> Result<(), DomainError> {
    if size_bytes > MAX_IMPORT_BYTES {
        return Err(DomainError::new(
            ErrorCode::LimitExceeded,
            format!(
                "{source_name} is larger than {} GB",
                MAX_IMPORT_BYTES / (1024 * 1024 * 1024)
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use sonara_core::error::ErrorCode;
    use sonara_core::model::media::MediaKind;

    use super::{
        MAX_IMPORT_BYTES, check_import_size, import_path, path_stem, path_suffix, safe_stem,
        transcode_paths,
    };

    #[test]
    fn safe_stem_keeps_ascii_and_collapses_the_rest() {
        assert_eq!(safe_stem("Kick Drum", 48), "Kick-Drum");
        assert_eq!(safe_stem("Příliš žluťoučký", 48), "Prilis-zlutoucky");
        assert_eq!(safe_stem("...--x--...", 48), "x");
        assert_eq!(safe_stem("日本語", 48), "audio");
        assert_eq!(safe_stem(&"x".repeat(60), 48).len(), 48);
    }

    #[test]
    fn stems_and_suffixes_follow_python() {
        assert_eq!(
            (path_stem("a.b.c.Aiff"), path_suffix("a.b.c.Aiff")),
            ("a.b.c", ".Aiff")
        );
        assert_eq!(
            (path_stem(".hidden"), path_suffix(".hidden")),
            (".hidden", "")
        );
        assert_eq!(
            (path_stem("trailing."), path_suffix("trailing.")),
            ("trailing.", "")
        );
        assert_eq!(
            (path_stem("dir/no-ext"), path_suffix("dir/no-ext")),
            ("no-ext", "")
        );
        assert_eq!(
            import_path("med_1", MediaKind::Impulse, "/in/Hall IR.WAV"),
            "media/impulses/med_1-Hall-IR.wav"
        );
    }

    #[test]
    fn transcodes_and_originals_have_their_own_paths() {
        assert_eq!(
            transcode_paths("med_1", MediaKind::Audio, "/in/Song Take 1.M4A"),
            (
                "media/audio/med_1-Song-Take-1.flac".to_owned(),
                "media/audio/originals/med_1-Song-Take-1.m4a".to_owned()
            )
        );
        assert_eq!(
            transcode_paths("med_2", MediaKind::Impulse, "/in/Hall"),
            (
                "media/impulses/med_2-Hall.flac".to_owned(),
                "media/impulses/originals/med_2-Hall".to_owned()
            )
        );
    }

    #[test]
    fn oversized_imports_are_refused() {
        assert!(check_import_size("a.wav", MAX_IMPORT_BYTES).is_ok());
        let err = check_import_size("a.wav", MAX_IMPORT_BYTES + 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::LimitExceeded);
        assert_eq!(err.message, "a.wav is larger than 8 GB");
    }
}
