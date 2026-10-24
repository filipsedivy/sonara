//! The metadata strings of an export, as libsndfile 1.2.2 stores them (`sf_set_string`,
//! `src/strings.c`): which fields a container accepts, in which order they are kept, and how
//! the software string is rewritten. The prototype sets each field with `setattr` on the open
//! file and reports the ones libsndfile accepted as `metadata_written`.

use super::Container;

/// The libsndfile version the software string names (the golden data's writer, CTR-01).
pub(crate) const LIBSNDFILE_VERSION: &str = "libsndfile-1.2.2";

/// libsndfile's `new_str [128]`: the rewritten software string keeps at most 127 bytes.
const SOFTWARE_MAX_BYTES: usize = 127;

/// A metadata field (`SF_STR_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StrKind {
    Title,
    Copyright,
    Software,
    Artist,
    Comment,
    Date,
    Album,
    License,
    TrackNumber,
    Genre,
}

impl StrKind {
    /// The field for one of the exporter's metadata keys (`_metadata_fields`).
    fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "title" => Self::Title,
            "copyright" => Self::Copyright,
            "software" => Self::Software,
            "artist" => Self::Artist,
            "comment" => Self::Comment,
            "date" => Self::Date,
            "album" => Self::Album,
            "license" => Self::License,
            "tracknumber" => Self::TrackNumber,
            "genre" => Self::Genre,
            _ => return None,
        })
    }

    /// The lower-case key, which is also the FLAC and CAF field name.
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Copyright => "copyright",
            Self::Software => "software",
            Self::Artist => "artist",
            Self::Comment => "comment",
            Self::Date => "date",
            Self::Album => "album",
            Self::License => "license",
            Self::TrackNumber => "tracknumber",
            Self::Genre => "genre",
        }
    }
}

/// libsndfile's string table: accepted fields in the order they were set, each the bytes of
/// the C string (without its terminator).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Strings {
    entries: Vec<(StrKind, Vec<u8>)>,
}

impl Strings {
    /// The accepted fields in order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (StrKind, &[u8])> {
        self.entries
            .iter()
            .map(|(kind, text)| (*kind, text.as_slice()))
    }

    /// Whether no field was accepted.
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Sets `metadata` on a file of `container` as the prototype does: the keys are merged like
    /// a Python `dict` (a repeated key keeps its first position and takes the last value),
    /// unknown keys and empty values are dropped (`_metadata_fields`), and each remaining field
    /// is offered to libsndfile in order. Returns the table and the keys libsndfile accepted
    /// (`metadata_written`).
    pub(crate) fn set(container: Container, metadata: &[(String, String)]) -> (Self, Vec<String>) {
        let mut merged: Vec<(&str, &str)> = Vec::new();
        for (key, value) in metadata {
            match merged.iter_mut().find(|(k, _)| *k == key.as_str()) {
                Some(entry) => entry.1 = value.as_str(),
                None => merged.push((key.as_str(), value.as_str())),
            }
        }
        let mut strings = Self::default();
        let mut written = Vec::new();
        for (key, value) in merged {
            let Some(kind) = StrKind::from_key(key) else {
                continue;
            };
            if value.is_empty() {
                continue;
            }
            if strings.store(container, kind, value.as_bytes()) {
                written.push(key.to_owned());
            }
        }
        (strings, written)
    }

    /// `psf_store_string` in write mode before any audio is written; false when libsndfile
    /// refuses the field.
    fn store(&mut self, container: Container, kind: StrKind, value: &[u8]) -> bool {
        // Wave64 sets no string flags: every field is `SFE_STR_NO_SUPPORT`.
        if container == Container::W64 {
            return false;
        }
        // The value reaches C as a NUL-terminated string.
        let end = value.iter().position(|&b| b == 0).unwrap_or(value.len());
        let text = value.get(..end).unwrap_or_default();
        if kind != StrKind::Software && text.is_empty() {
            return false;
        }
        let stored = if kind == StrKind::Software {
            software_string(text)
        } else {
            text.to_vec()
        };
        self.entries.retain(|(k, _)| *k != kind);
        self.entries.push((kind, stored));
        true
    }
}

/// The software string libsndfile stores in write mode: the libsndfile version is appended
/// unless the text already names libsndfile, and the result is cut to 127 bytes.
fn software_string(text: &[u8]) -> Vec<u8> {
    let mut out = if contains(text, b"libsndfile") {
        text.to_vec()
    } else if text.is_empty() {
        LIBSNDFILE_VERSION.as_bytes().to_vec()
    } else {
        let mut s = text.to_vec();
        s.extend_from_slice(b" (");
        s.extend_from_slice(LIBSNDFILE_VERSION.as_bytes());
        s.push(b')');
        s
    };
    out.truncate(SOFTWARE_MAX_BYTES);
    out
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::{Container, StrKind, Strings};

    fn meta(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn fields_keep_their_order_and_unknown_or_empty_ones_are_dropped() {
        let (strings, written) = Strings::set(
            Container::Wav,
            &meta(&[
                ("genre", "Ambient"),
                ("label", "x"),
                ("title", "T"),
                ("album", ""),
                ("software", "Sonara"),
            ]),
        );
        assert_eq!(written, ["genre", "title", "software"]);
        let kinds: Vec<StrKind> = strings.iter().map(|(k, _)| k).collect();
        assert_eq!(kinds, [StrKind::Genre, StrKind::Title, StrKind::Software]);
        let software = strings.iter().last().map(|(_, v)| v.to_vec()).unwrap();
        assert_eq!(software, b"Sonara (libsndfile-1.2.2)");
    }

    #[test]
    fn a_repeated_key_keeps_its_first_position_like_a_dict() {
        let (_, written) = Strings::set(
            Container::Flac,
            &meta(&[("title", "a"), ("artist", "b"), ("title", "c")]),
        );
        assert_eq!(written, ["title", "artist"]);
        let (strings, _) = Strings::set(Container::Flac, &meta(&[("title", "a"), ("title", "")]));
        assert!(
            strings.is_empty(),
            "the last value is empty, so the field is dropped"
        );
    }

    #[test]
    fn wave64_refuses_every_field_and_nul_cuts_a_value() {
        let (strings, written) = Strings::set(Container::W64, &meta(&[("title", "T")]));
        assert!(strings.is_empty() && written.is_empty());
        let (_, written) = Strings::set(
            Container::Wav,
            &meta(&[("title", "\0x"), ("artist", "a\0b")]),
        );
        assert_eq!(written, ["artist"]);
    }

    #[test]
    fn the_software_string_names_libsndfile_once_and_fits_127_bytes() {
        let (strings, _) = Strings::set(Container::Wav, &meta(&[("software", "my libsndfile")]));
        assert_eq!(strings.iter().next().unwrap().1, b"my libsndfile");
        let long = "x".repeat(200);
        let (strings, _) = Strings::set(Container::Wav, &meta(&[("software", &long)]));
        assert_eq!(strings.iter().next().unwrap().1.len(), 127);
    }
}
