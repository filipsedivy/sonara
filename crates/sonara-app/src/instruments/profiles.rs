//! The library profiles Sonara ships for free sample libraries: Sonara's own data under the
//! repository's licence, holding file names and facts, never samples or SFZ text (LIC-01).
//!
//! The profiles live in `crates/sonara-app/profiles/` in the shape of a library manifest of
//! version 2 and are embedded in the binary, so nothing is read at run time and the tests pin
//! them. This build reads their **identity**: the library `id`, its `name`, and the `detect`
//! paths that recognize a copy of the library. A library whose manifest declares no `id` (or
//! that has no manifest) and that holds every `detect` path as a regular file takes the
//! profile's `id` and `name`. Its preset IDs then no longer depend on the name of its folder: a
//! checkout of VSCO 2 CE in `VSCO-2-CE` or anywhere else has the IDs `vsco2ce/…`. A library
//! that two profiles match takes neither ([`Detected::Ambiguous`]).

use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use serde::Deserialize;

use super::manifest::validate_relative_path;
use super::model::{InvalidLibraryId, LibraryId};

/// The embedded profiles, by file name.
const SHIPPED_SOURCES: [(&str, &str); 2] = [
    ("vsco2ce.json", include_str!("../../profiles/vsco2ce.json")),
    ("vcsl.json", include_str!("../../profiles/vcsl.json")),
];

/// The format version of a profile: that of the library manifest whose shape it shares.
const PROFILE_VERSION: u32 = 2;

/// The identity a shipped profile gives a library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProfileIdentity {
    /// The library ID its presets take.
    pub id: LibraryId,
    /// The library name to show.
    pub name: String,
    /// Paths below the library root, `/`-separated, that must all be regular files for the
    /// profile to apply to a library without a declared ID. Never empty.
    pub detect: Vec<String>,
}

/// The fields of a profile file this build reads; the others are left for later readers.
#[derive(Deserialize)]
struct RawIdentity {
    version: u32,
    id: String,
    name: String,
    detect: Vec<String>,
}

/// Why a profile file is not a valid identity.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ProfileError {
    #[error("the profile is not valid: {0}")]
    Parse(#[source] serde_json::Error),
    #[error("profile version {0} is not {PROFILE_VERSION}")]
    Version(u32),
    #[error(transparent)]
    Id(#[from] InvalidLibraryId),
    #[error("the profile has no name")]
    NoName,
    #[error("the profile has no detect path")]
    NoDetect,
    #[error("detect path {path:?}: {reason}")]
    Detect { path: String, reason: String },
}

/// Reads the identity of a profile file (TYP-06: the same checks as a manifest's ID and paths).
pub(crate) fn parse(text: &str) -> Result<ProfileIdentity, ProfileError> {
    let raw: RawIdentity = serde_json::from_str(text).map_err(ProfileError::Parse)?;
    if raw.version != PROFILE_VERSION {
        return Err(ProfileError::Version(raw.version));
    }
    let id = LibraryId::parse(&raw.id)?;
    let name = raw.name.trim().to_owned();
    if name.is_empty() {
        return Err(ProfileError::NoName);
    }
    if raw.detect.is_empty() {
        return Err(ProfileError::NoDetect);
    }
    for path in &raw.detect {
        validate_relative_path(path).map_err(|reason| ProfileError::Detect {
            path: path.clone(),
            reason,
        })?;
    }
    Ok(ProfileIdentity {
        id,
        name,
        detect: raw.detect,
    })
}

/// The shipped profiles. A file that does not parse is logged and left out; the tests check
/// that every one parses, so that never happens in a tested build.
static SHIPPED: LazyLock<Vec<ProfileIdentity>> = LazyLock::new(|| {
    SHIPPED_SOURCES
        .iter()
        .filter_map(|(file, text)| {
            parse(text)
                .inspect_err(|err| {
                    log::error!("the shipped library profile {file} is ignored: {err}");
                })
                .ok()
        })
        .collect()
});

/// The profiles Sonara ships.
pub(crate) fn shipped() -> &'static [ProfileIdentity] {
    &SHIPPED
}

/// Which of a set of profiles a library directory holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Detected<'a> {
    /// No profile matches.
    None,
    /// Exactly one profile matches.
    One(&'a ProfileIdentity),
    /// Several profiles match, so none applies; their IDs, in the order given.
    Ambiguous(Vec<LibraryId>),
}

/// The profiles of `profiles` whose `detect` paths are all regular files below `root`. Symbolic
/// links do not count, as discovery never follows them.
pub(crate) fn detect<'a>(root: &Path, profiles: &'a [ProfileIdentity]) -> Detected<'a> {
    let matching: Vec<&ProfileIdentity> = profiles
        .iter()
        .filter(|profile| {
            profile.detect.iter().all(|path| {
                fs::symlink_metadata(root.join(path)).is_ok_and(|meta| meta.file_type().is_file())
            })
        })
        .collect();
    match matching.as_slice() {
        [] => Detected::None,
        [one] => Detected::One(one),
        several => Detected::Ambiguous(several.iter().map(|p| p.id.clone()).collect()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{Detected, ProfileError, ProfileIdentity, SHIPPED_SOURCES, detect, parse, shipped};
    use crate::instruments::model::LibraryId;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
        fs::write(path, "<region> sample=a.wav\n").expect("write");
    }

    fn profile(id: &str, detect: &[&str]) -> ProfileIdentity {
        ProfileIdentity {
            id: LibraryId::parse(id).expect("a valid id"),
            name: id.to_uppercase(),
            detect: detect.iter().map(|path| (*path).to_owned()).collect(),
        }
    }

    #[test]
    fn every_shipped_profile_is_valid() {
        for (file, text) in SHIPPED_SOURCES {
            parse(text).unwrap_or_else(|err| panic!("{file}: {err}"));
        }
        let ids: Vec<&str> = shipped().iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["vsco2ce", "vcsl"]);
        let names: Vec<&str> = shipped().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "VSCO 2 Community Edition",
                "Versilian Community Sample Library"
            ]
        );
    }

    #[test]
    fn no_copy_of_one_shipped_library_matches_another_profile() {
        // Each library's own detect files never match another profile, so a real checkout is
        // never ambiguous (TST-07).
        for own in shipped() {
            let dir = tempfile::tempdir().expect("a tempdir");
            for path in &own.detect {
                touch(&dir.path().join(path));
            }
            assert_eq!(
                detect(dir.path(), shipped()),
                Detected::One(own),
                "{}",
                own.id
            );
        }
    }

    #[test]
    fn a_profile_matches_only_when_every_detect_file_is_there() {
        let profiles = [profile("lib-a", &["One.sfz", "Sub/Two.sfz"])];
        let dir = tempfile::tempdir().expect("a tempdir");
        assert_eq!(detect(dir.path(), &profiles), Detected::None);
        touch(&dir.path().join("One.sfz"));
        assert_eq!(
            detect(dir.path(), &profiles),
            Detected::None,
            "one is missing"
        );
        fs::create_dir_all(dir.path().join("Sub/Two.sfz")).expect("a folder");
        assert_eq!(
            detect(dir.path(), &profiles),
            Detected::None,
            "a folder is not a file"
        );
        fs::remove_dir(dir.path().join("Sub/Two.sfz")).expect("rmdir");
        touch(&dir.path().join("Sub/Two.sfz"));
        assert_eq!(detect(dir.path(), &profiles), Detected::One(&profiles[0]));
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_detect_file_does_not_count() {
        let profiles = [profile("lib-a", &["One.sfz"])];
        let dir = tempfile::tempdir().expect("a tempdir");
        touch(&dir.path().join("elsewhere/One.sfz"));
        std::os::unix::fs::symlink(
            dir.path().join("elsewhere/One.sfz"),
            dir.path().join("One.sfz"),
        )
        .expect("a link");
        assert_eq!(detect(dir.path(), &profiles), Detected::None);
    }

    #[test]
    fn two_matching_profiles_make_neither_apply() {
        let profiles = [
            profile("lib-a", &["One.sfz"]),
            profile("lib-b", &["Two.sfz"]),
            profile("lib-c", &["Three.sfz"]),
        ];
        let dir = tempfile::tempdir().expect("a tempdir");
        touch(&dir.path().join("One.sfz"));
        touch(&dir.path().join("Two.sfz"));
        assert_eq!(
            detect(dir.path(), &profiles),
            Detected::Ambiguous(vec![
                LibraryId::parse("lib-a").expect("valid"),
                LibraryId::parse("lib-b").expect("valid"),
            ])
        );
    }

    #[test]
    fn invalid_profiles_are_rejected() {
        let check = |text: &str| parse(text).expect_err(text);
        assert!(matches!(check("{"), ProfileError::Parse(_)));
        assert!(matches!(
            check(r#"{"version":1,"id":"a","name":"A","detect":["a.sfz"]}"#),
            ProfileError::Version(1)
        ));
        assert!(matches!(
            check(r#"{"version":2,"id":"Bad Id","name":"A","detect":["a.sfz"]}"#),
            ProfileError::Id(_)
        ));
        assert!(matches!(
            check(r#"{"version":2,"id":"a","name":" ","detect":["a.sfz"]}"#),
            ProfileError::NoName
        ));
        assert!(matches!(
            check(r#"{"version":2,"id":"a","name":"A","detect":[]}"#),
            ProfileError::NoDetect
        ));
        for path in ["../a.sfz", "/a.sfz", "a.txt", "a//b.sfz"] {
            let text = format!(r#"{{"version":2,"id":"a","name":"A","detect":["{path}"]}}"#);
            assert!(
                matches!(check(&text), ProfileError::Detect { .. }),
                "{path}"
            );
        }
    }
}
