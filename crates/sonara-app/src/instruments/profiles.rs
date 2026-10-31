//! The library profiles Sonara ships for free sample libraries: Sonara's own data under the
//! repository's licence, holding file names and facts, never samples or SFZ text (LIC-01).
//!
//! The profiles live in `crates/sonara-app/profiles/` as library manifests of version 2 and are
//! embedded in the binary, so nothing is read at run time and the tests pin them. Each is read
//! with the manifest reader ([`super::manifest`]) the first time a profile is needed, and every
//! claim it makes is marked as the profile's ([`Claimant::Profile`]), never as the library's
//! own.
//!
//! Two profiles ship: VSCO 2 Community Edition (branch `SFZ`) and the Versilian Community Sample
//! Library (tag `v1.2.2-RC`), both CC0. Each lists every patch of its revision with the instrument
//! type it plays, solo or section, how each articulation is reached (a key switch, or the whole
//! patch), what drives dynamics, a key offset for patches keyed an octave below sounding pitch, a
//! key map for percussion patches with one sound per key, and a verdict on how the sampler plays
//! it; and it groups an instrument's single-articulation patches into multi-patch sets. Patches of
//! instruments the orchestration metadata does not know claim only a family, and those whose
//! names read as an instrument it knows also write `instrument: null`, so that none is guessed
//! from the name.
//!
//! **Identity.** A profile names the library `id`, its `name`, and the `detect` paths that
//! recognize a copy of the library. A library whose manifest declares no `id` (or that has no
//! manifest) and that holds every `detect` path as a regular file takes the profile's `id`. Its
//! preset IDs then no longer depend on the name of its folder: a checkout of VSCO 2 CE in
//! `VSCO-2-CE` or anywhere else has the IDs `vsco2ce/…`. A library that two profiles match takes
//! neither ([`Detected::Ambiguous`]). A library whose manifest declares a profile's `id` is that
//! library, so the profile applies to it as well.
//!
//! **Claims.** The profile's claims about the library and its patches lie below those of the
//! library's own manifest ([`merge`]): a field the manifest writes replaces the profile's, `null`
//! clears it, an entry only in the profile stays and an entry only in the manifest is added.

use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use super::manifest::{self, Claimant, LibraryClaims, ManifestError};
use super::model::LibraryId;

/// The embedded profiles, by file name.
const SHIPPED_SOURCES: [(&str, &str); 2] = [
    ("vsco2ce.json", include_str!("../../profiles/vsco2ce.json")),
    ("vcsl.json", include_str!("../../profiles/vcsl.json")),
];

/// The manifest version a profile is written in.
const PROFILE_VERSION: u32 = 2;

/// A library profile Sonara ships: the identity it gives a library, and its claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Profile {
    /// The library ID its presets take.
    pub id: LibraryId,
    /// Paths below the library root, `/`-separated, that must all be regular files for the
    /// profile to apply to a library without a declared ID. Never empty.
    pub detect: Vec<String>,
    /// What the profile claims about the library and its patches, all as the profile's. The
    /// library name is always set.
    pub claims: LibraryClaims,
}

/// Why a profile file is not a valid profile.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ProfileError {
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("profile version {0} is not {PROFILE_VERSION}")]
    Version(u32),
    #[error("the profile has no id")]
    NoId,
    #[error("the profile has no name")]
    NoName,
    #[error("the profile has no detect path")]
    NoDetect,
    #[error("the profile's entry {entry:?} is not valid: {reason}")]
    Entry { entry: String, reason: String },
}

/// Reads a profile file: a version 2 manifest with an `id`, a `name` and `detect` paths, whose
/// every entry is valid (TYP-06: the manifest's own checks, and stricter, because the profile is
/// Sonara's data and a rejected entry is a defect).
pub(crate) fn parse(text: &str) -> Result<Profile, ProfileError> {
    let manifest = manifest::parse(text.as_bytes(), Claimant::Profile)?;
    if manifest.version != PROFILE_VERSION {
        return Err(ProfileError::Version(manifest.version));
    }
    let id = manifest.id.ok_or(ProfileError::NoId)?;
    if manifest.claims.name.value().is_none() {
        return Err(ProfileError::NoName);
    }
    if manifest.detect.is_empty() {
        return Err(ProfileError::NoDetect);
    }
    if let Some((entry, reason)) = manifest.rejected.into_iter().next() {
        return Err(ProfileError::Entry { entry, reason });
    }
    Ok(Profile {
        id,
        detect: manifest.detect,
        claims: manifest.claims,
    })
}

/// The shipped profiles. A file that does not parse is logged and left out; the tests check
/// that every one parses, so that never happens in a tested build.
static SHIPPED: LazyLock<Vec<Profile>> = LazyLock::new(|| {
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
pub(crate) fn shipped() -> &'static [Profile] {
    &SHIPPED
}

/// Which of a set of profiles a library directory holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Detected<'a> {
    /// No profile matches.
    None,
    /// Exactly one profile matches.
    One(&'a Profile),
    /// Several profiles match, so none applies; their IDs, in the order given.
    Ambiguous(Vec<LibraryId>),
}

/// The profiles of `profiles` whose `detect` paths are all regular files below `root`. Symbolic
/// links do not count, as discovery never follows them.
pub(crate) fn detect<'a>(root: &Path, profiles: &'a [Profile]) -> Detected<'a> {
    let matching: Vec<&Profile> = profiles
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

/// The claims of a library: those of its own manifest (`user`) over those of the shipped profile
/// that applies to it (`shipped`), field by field and entry by entry
/// ([`LibraryClaims::over`]). Without either, the other's claims; without both, none.
pub(crate) fn merge(
    shipped: Option<&LibraryClaims>,
    user: Option<&LibraryClaims>,
) -> LibraryClaims {
    match (shipped, user) {
        (Some(shipped), Some(user)) => user.over(shipped),
        (Some(only), None) | (None, Some(only)) => only.clone(),
        (None, None) => LibraryClaims::default(),
    }
}

#[cfg(test)]
mod shipped_tests;

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use proptest::prelude::*;

    use super::{Detected, Profile, ProfileError, SHIPPED_SOURCES, detect, merge, parse, shipped};
    use crate::instruments::manifest::{
        self, ArticulationList, Claimant, EntryTarget, Field, LibraryClaims, ManifestEntry,
        ManifestError,
    };
    use crate::instruments::model::LibraryId;
    use crate::instruments::{ArticulationName, InstrumentFamily};
    use sonara_midi::orchestration::{Articulation, InstrumentType};

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
        fs::write(path, "<region> sample=a.wav\n").expect("write");
    }

    fn profile(id: &str, detect: &[&str]) -> Profile {
        Profile {
            id: LibraryId::parse(id).expect("a valid id"),
            detect: detect.iter().map(|path| (*path).to_owned()).collect(),
            claims: LibraryClaims::default(),
        }
    }

    /// The claims of `json`, a manifest of the library (`Manifest`) or of a profile (`Profile`).
    fn claims(json: &serde_json::Value, source: Claimant) -> LibraryClaims {
        let manifest = manifest::parse(json.to_string().as_bytes(), source).expect("valid");
        assert_eq!(manifest.rejected, [], "{json}");
        manifest.claims
    }

    fn entry<'a>(claims: &'a LibraryClaims, path: &str) -> &'a ManifestEntry {
        claims
            .entries
            .iter()
            .find(|entry| matches!(&entry.target, EntryTarget::Path(p) if p == path))
            .unwrap_or_else(|| panic!("no entry {path}"))
    }

    #[test]
    fn every_shipped_profile_is_valid() {
        for (file, text) in SHIPPED_SOURCES {
            let profile = parse(text).unwrap_or_else(|err| panic!("{file}: {err}"));
            // Every claim of a profile is the profile's, never the library's own word.
            assert_eq!(profile.claims.name.declared(), None, "{file}");
            assert!(
                matches!(profile.claims.name, Field::Set(_, Claimant::Profile)),
                "{file}"
            );
        }
        let ids: Vec<&str> = shipped().iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["vsco2ce", "vcsl"]);
        let names: Vec<&str> = shipped()
            .iter()
            .filter_map(|p| p.claims.name.value().map(String::as_str))
            .collect();
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
        assert!(matches!(
            check("{"),
            ProfileError::Manifest(ManifestError::Parse(_))
        ));
        assert!(matches!(
            check(r#"{"version":1,"id":"a","name":"A","detect":["a.sfz"]}"#),
            ProfileError::Version(1)
        ));
        assert!(matches!(
            check(r#"{"version":3,"id":"a","name":"A","detect":["a.sfz"]}"#),
            ProfileError::Manifest(ManifestError::Version { found: 3 })
        ));
        assert!(matches!(
            check(r#"{"version":2,"id":"Bad Id","name":"A","detect":["a.sfz"]}"#),
            ProfileError::Manifest(ManifestError::LibraryId(_))
        ));
        assert!(matches!(
            check(r#"{"version":2,"name":"A","detect":["a.sfz"]}"#),
            ProfileError::NoId
        ));
        for name in [r#"" ""#, "null"] {
            let text = format!(r#"{{"version":2,"id":"a","name":{name},"detect":["a.sfz"]}}"#);
            assert!(matches!(check(&text), ProfileError::NoName), "{name}");
        }
        assert!(matches!(
            check(r#"{"version":2,"id":"a","name":"A","detect":[]}"#),
            ProfileError::NoDetect
        ));
        for path in ["../a.sfz", "/a.sfz", "a.txt", "a//b.sfz"] {
            let text = format!(r#"{{"version":2,"id":"a","name":"A","detect":["{path}"]}}"#);
            assert!(
                matches!(
                    check(&text),
                    ProfileError::Manifest(ManifestError::Library {
                        field: "detect",
                        ..
                    })
                ),
                "{path}"
            );
        }
        // A profile is Sonara's own data: one bad entry rejects the whole profile.
        assert!(matches!(
            check(
                r#"{"version":2,"id":"a","name":"A","detect":["a.sfz"],
                "instruments":[{"path":"a.sfz"},{"path":"b.sfz","instrument":"kazoo"}]}"#
            ),
            ProfileError::Entry { entry, .. } if entry == "b.sfz"
        ));
    }

    /// A profile entry with a value of every kind, and two more entries.
    fn shipped_claims() -> LibraryClaims {
        claims(
            &serde_json::json!({
                "version": 2, "id": "lib", "name": "Shipped name", "license": "CC0 1.0",
                "attribution": "Shipped credit", "source": {"url": "https://example.org/lib"},
                "detect": ["Violin.sfz"],
                "instruments": [
                    {"path": "Strings/Violin.sfz", "name": "Violin", "family": "strings",
                     "instrument": "violin", "in_situ": true,
                     "articulations": [
                         {"id": "sustain", "via": "keyswitch", "key": 36, "default": true},
                         {"id": "pizzicato", "via": "keyswitch", "key": 37}]},
                    {"path": "Timpani.sfz", "instrument": "timpani"},
                    {"set": "horn", "instrument": "horn_f",
                     "articulations": [{"id": "sustain", "via": "patch", "path": "HornSus.sfz"}]}
                ]
            }),
            Claimant::Profile,
        )
    }

    #[test]
    fn a_manifest_overrides_a_profile_field_by_field() {
        let shipped = shipped_claims();
        let user = claims(
            &serde_json::json!({
                "version": 2, "name": "My name", "license": null,
                "instruments": [
                    {"path": "strings/VIOLIN.sfz", "name": "My violin", "in_situ": null,
                     "articulations": [{"id": "staccato", "via": "patch"}]},
                    {"set": "horn", "family": "brass"},
                    {"path": "Mine.sfz", "family": "other"}
                ]
            }),
            Claimant::Manifest,
        );
        let merged = merge(Some(&shipped), Some(&user));

        // Library fields: written replaces, `null` clears, left out keeps the profile's.
        assert_eq!(
            merged.name,
            Field::Set("My name".to_owned(), Claimant::Manifest)
        );
        assert_eq!(merged.license, Field::Cleared);
        assert_eq!(
            merged.attribution,
            Field::Set("Shipped credit".to_owned(), Claimant::Profile)
        );
        assert_eq!(merged.source, shipped.source);

        // The entries of both are merged, matched ignoring case; the user's spelling stays.
        let targets: Vec<&EntryTarget> = merged.entries.iter().map(|e| &e.target).collect();
        assert_eq!(
            targets,
            [
                &EntryTarget::Path("strings/VIOLIN.sfz".to_owned()),
                &EntryTarget::Path("Timpani.sfz".to_owned()),
                &EntryTarget::Set("horn".to_owned()),
                &EntryTarget::Path("Mine.sfz".to_owned()),
            ]
        );
        let violin = entry(&merged, "strings/VIOLIN.sfz");
        assert_eq!(
            violin.name,
            Field::Set("My violin".to_owned(), Claimant::Manifest)
        );
        assert_eq!(
            violin.family,
            Field::Set(InstrumentFamily::Strings, Claimant::Profile)
        );
        assert_eq!(
            violin.instrument,
            Field::Set(InstrumentType::Violin, Claimant::Profile)
        );
        assert_eq!(violin.in_situ, Field::Cleared);
        // The articulation list is replaced whole: the shipped key switches are gone.
        let Field::Set(ArticulationList::Mapped(list), Claimant::Manifest) = &violin.articulations
        else {
            panic!("{:?}", violin.articulations)
        };
        let names: Vec<ArticulationName> = list.iter().map(|a| a.name).collect();
        assert_eq!(
            names,
            [ArticulationName::Vocabulary(Articulation::Staccato)]
        );

        // An entry only in the profile stays as it is; the set merges by its slug.
        assert_eq!(
            entry(&merged, "Timpani.sfz"),
            entry(&shipped, "Timpani.sfz")
        );
        let horn = merged
            .entries
            .iter()
            .find(|e| e.target == EntryTarget::Set("horn".to_owned()))
            .expect("the set");
        assert_eq!(
            horn.family,
            Field::Set(InstrumentFamily::Brass, Claimant::Manifest)
        );
        assert_eq!(
            horn.instrument,
            Field::Set(InstrumentType::HornF, Claimant::Profile)
        );
        // An entry only in the manifest is added.
        assert_eq!(
            entry(&merged, "Mine.sfz").family,
            Field::Set(InstrumentFamily::Other, Claimant::Manifest)
        );
    }

    /// A version 1 manifest over a profile replaces only what it sets: it cannot clear a field,
    /// and its articulation names replace the profile's mapped list whole.
    #[test]
    fn a_version_1_manifest_over_a_profile_keeps_what_it_leaves_out() {
        let shipped = shipped_claims();
        let user = claims(
            &serde_json::json!({
                "version": 1, "license": null,
                "instruments": [
                    {"path": "Strings/Violin.sfz", "articulations": ["pizz"]},
                    {"path": "Timpani.sfz", "name": null, "family": "percussion"}
                ]
            }),
            Claimant::Manifest,
        );
        let merged = merge(Some(&shipped), Some(&user));
        assert_eq!(merged.license, shipped.license);
        let violin = entry(&merged, "Strings/Violin.sfz");
        assert_eq!(
            violin.articulations,
            Field::Set(
                ArticulationList::Names(vec!["pizz".to_owned()]),
                Claimant::Manifest
            )
        );
        assert_eq!(violin.name, entry(&shipped, "Strings/Violin.sfz").name);
        let timpani = entry(&merged, "Timpani.sfz");
        assert_eq!(timpani.name, Field::Unset);
        assert_eq!(
            timpani.family,
            Field::Set(InstrumentFamily::Percussion, Claimant::Manifest)
        );
        assert_eq!(
            timpani.instrument,
            Field::Set(InstrumentType::Timpani, Claimant::Profile)
        );
    }

    #[test]
    fn without_one_side_the_merge_is_the_other() {
        let shipped = shipped_claims();
        assert_eq!(merge(Some(&shipped), None), shipped);
        assert_eq!(merge(None, Some(&shipped)), shipped);
        assert_eq!(merge(None, None), LibraryClaims::default());
        assert_eq!(
            merge(Some(&shipped), Some(&LibraryClaims::default())),
            shipped
        );
    }

    /// Text fields of a generated entry: left out, `null` or a value.
    fn text_field() -> impl Strategy<Value = Option<Option<String>>> {
        prop_oneof![
            Just(None),
            Just(Some(None)),
            "[A-Za-z]{1,6}".prop_map(|text| Some(Some(text))),
        ]
    }

    /// A generated entry: one of a few paths, spelled in either case, with a name and a licence
    /// that may be left out, `null` or set.
    fn generated_entry() -> impl Strategy<Value = serde_json::Value> {
        (
            prop::sample::select(vec!["a.sfz", "A.sfz", "b/c.sfz", "B/C.sfz", "d.sfz"]),
            text_field(),
            text_field(),
        )
            .prop_map(|(path, name, license)| {
                let mut entry = serde_json::Map::new();
                entry.insert("path".to_owned(), path.into());
                for (key, value) in [("name", name), ("license", license)] {
                    if let Some(value) = value {
                        entry.insert(key.to_owned(), value.into());
                    }
                }
                serde_json::Value::Object(entry)
            })
    }

    fn generated_claims(source: Claimant) -> impl Strategy<Value = LibraryClaims> {
        (text_field(), prop::collection::vec(generated_entry(), 0..6)).prop_map(
            move |(name, entries)| {
                let mut json = serde_json::json!({"version": 2, "instruments": entries});
                if let Some(name) = name {
                    json["name"] = name.into();
                }
                // Entries that repeat a path are rejected; the rest is what merges.
                manifest::parse(json.to_string().as_bytes(), source)
                    .expect("valid")
                    .claims
            },
        )
    }

    fn key(target: &EntryTarget) -> String {
        match target {
            EntryTarget::Path(path) => path.to_lowercase(),
            EntryTarget::Set(slug) => format!("@{slug}"),
        }
    }

    proptest! {
        /// The merge laws (TST-09): every entry of either side is in the result once; a field
        /// the user writes wins, one the user leaves out keeps the profile's.
        #[test]
        fn the_merge_keeps_every_entry_and_lets_written_fields_win(
            shipped in generated_claims(Claimant::Profile),
            user in generated_claims(Claimant::Manifest),
        ) {
            let merged = merge(Some(&shipped), Some(&user));
            let mut expected: Vec<String> =
                shipped.entries.iter().chain(&user.entries).map(|e| key(&e.target)).collect();
            expected.sort();
            expected.dedup();
            let mut keys: Vec<String> = merged.entries.iter().map(|e| key(&e.target)).collect();
            keys.sort();
            prop_assert_eq!(keys, expected);
            prop_assert_eq!(&merged.name, &user.name.over(&shipped.name));
            for entry in &merged.entries {
                let find = |claims: &LibraryClaims| {
                    claims.entries.iter().find(|e| key(&e.target) == key(&entry.target)).cloned()
                };
                let (mine, theirs) = (find(&user), find(&shipped));
                let expected_name = match (&mine, &theirs) {
                    (Some(mine), Some(theirs)) => mine.name.over(&theirs.name),
                    (Some(only), None) | (None, Some(only)) => only.name.clone(),
                    (None, None) => panic!("an entry from nowhere"),
                };
                prop_assert_eq!(&entry.name, &expected_name);
                if let Some(mine) = &mine {
                    prop_assert_eq!(&entry.target, &mine.target);
                    if mine.license != Field::Unset {
                        prop_assert_eq!(&entry.license, &mine.license);
                    }
                }
            }
        }
    }
}
