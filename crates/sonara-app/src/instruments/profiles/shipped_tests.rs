//! Tests of what the shipped profiles say. The libraries are not installed where the tests run
//! (TST-08), so each profile is checked against the published file list of its revision, against
//! the orchestration metadata, and against expectations written from the patch files by hand.

use std::collections::BTreeSet;

use serde_json::Value;
use sonara_midi::orchestration::{InstrumentType, Mute, RangeCheck, SoundingPitch, Vibrato};

use super::{Profile, SHIPPED_SOURCES, shipped};
use crate::instruments::manifest::{
    ArticulationEntry, ArticulationList, ArticulationVia, CheckMethod, Claimant, DynamicsMode,
    EnsembleKind, EntryTarget, Field, KeymapEntry, ManifestEntry, VerdictState,
};

/// The profile with library ID `id`.
fn profile(id: &str) -> &'static Profile {
    shipped()
        .iter()
        .find(|profile| profile.id.as_str() == id)
        .unwrap_or_else(|| panic!("no shipped profile {id}"))
}

/// The entry of `profile` for the patch `path` (exact spelling) or the set `@slug`.
fn entry<'a>(profile: &'a Profile, name: &str) -> &'a ManifestEntry {
    profile
        .claims
        .entries
        .iter()
        .find(|entry| match &entry.target {
            EntryTarget::Path(path) => path == name,
            EntryTarget::Set(slug) => name.strip_prefix('@') == Some(slug.as_str()),
        })
        .unwrap_or_else(|| panic!("{}: no entry {name}", profile.id))
}

fn paths(profile: &Profile) -> Vec<&str> {
    profile
        .claims
        .entries
        .iter()
        .filter_map(|entry| match &entry.target {
            EntryTarget::Path(path) => Some(path.as_str()),
            EntryTarget::Set(_) => None,
        })
        .collect()
}

/// The mapped articulations of `entry`; empty when it maps none.
fn mapped(entry: &ManifestEntry) -> &[ArticulationEntry] {
    match entry.articulations.value() {
        Some(ArticulationList::Mapped(list)) => list,
        Some(ArticulationList::Names(names)) => panic!("version 1 names in a profile: {names:?}"),
        None => &[],
    }
}

/// An articulation in words: how it is reached, its ID, its mute and vibrato when they are not
/// the ordinary ones, and `(default)`.
fn describe(articulation: &ArticulationEntry) -> String {
    let mut words = vec![match (articulation.via, &articulation.member) {
        (ArticulationVia::Keyswitch { key }, _) => format!("key {key}"),
        (ArticulationVia::Patch, Some(member)) => member.clone(),
        (ArticulationVia::Patch, None) => "patch".to_owned(),
        (via, _) => format!("{via:?}"),
    }];
    words.push(articulation.name.id().to_owned());
    if let Some(mute) = articulation.mute.articulation() {
        words.push(mute.id().to_owned());
    }
    if let Some(vibrato) = articulation.vibrato.articulation() {
        words.push(vibrato.id().to_owned());
    }
    if articulation.is_default {
        words.push("(default)".to_owned());
    }
    words.join(" ")
}

fn articulations(entry: &ManifestEntry) -> Vec<String> {
    mapped(entry).iter().map(describe).collect()
}

/// A key map row in words: its keys, its instrument and its articulation when it has one.
fn keymap(entry: &ManifestEntry) -> Vec<String> {
    entry
        .keymap
        .value()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|row: &KeymapEntry| {
            let mut words: Vec<String> = row.keys.iter().map(u8::to_string).collect();
            words.push(row.instrument.id().to_owned());
            if let Some(articulation) = row.articulation {
                words.push(articulation.id().to_owned());
            }
            words.join(" ")
        })
        .collect()
}

/// The fields a version 2 manifest has, by the object that holds them. A field the reader does
/// not know is ignored without a word, so a misspelt one would silently drop its claim.
const KNOWN_FIELDS: [(&str, &[&str]); 9] = [
    (
        "library",
        &[
            "version",
            "id",
            "name",
            "license",
            "attribution",
            "source",
            "detect",
            "profile",
            "instruments",
        ],
    ),
    ("source", &["url", "revision"]),
    ("profile", &["checked", "method"]),
    (
        "entry",
        &[
            "path",
            "set",
            "name",
            "family",
            "instrument",
            "ensemble",
            "articulations",
            "keymap",
            "key_offset",
            "dynamics",
            "legato",
            "in_situ",
            "verdict",
            "license",
        ],
    ),
    ("ensemble", &["kind", "players"]),
    (
        "articulation",
        &[
            "id",
            "via",
            "key",
            "lo",
            "hi",
            "cc",
            "value",
            "path",
            "label",
            "default",
            "mute",
            "vibrato",
            "attack_ms",
        ],
    ),
    ("keymap", &["keys", "instrument", "articulation", "name"]),
    ("dynamics", &["via", "cc", "velocity"]),
    ("verdict", &["state", "lost"]),
];

/// Checks that `value`, an object of kind `kind`, holds only known fields.
fn check_fields(file: &str, kind: &str, value: &Value) {
    let Some((_, known)) = KNOWN_FIELDS.iter().find(|(name, _)| *name == kind) else {
        panic!("no field list for {kind}");
    };
    let object = value
        .as_object()
        .unwrap_or_else(|| panic!("{file}: {kind} is not an object"));
    for field in object.keys() {
        assert!(
            known.contains(&field.as_str()),
            "{file}: {kind} field {field:?}"
        );
    }
}

#[test]
fn the_profiles_hold_only_fields_the_reader_knows() {
    for (file, text) in SHIPPED_SOURCES {
        let library: Value = serde_json::from_str(text).expect("JSON");
        check_fields(file, "library", &library);
        check_fields(file, "source", &library["source"]);
        check_fields(file, "profile", &library["profile"]);
        let entries = library["instruments"].as_array().expect("a list");
        for entry in entries {
            check_fields(file, "entry", entry);
            for (field, kind) in [("ensemble", "ensemble"), ("dynamics", "dynamics")] {
                if let Some(value) = entry.get(field) {
                    check_fields(file, kind, value);
                }
            }
            if let Some(verdict) = entry.get("verdict") {
                check_fields(file, "verdict", verdict);
            }
            for (field, kind) in [("articulations", "articulation"), ("keymap", "keymap")] {
                for item in entry
                    .get(field)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    check_fields(file, kind, item);
                }
            }
            // The fields this profile leaves to the library or to a later check.
            for unclaimed in ["legato", "in_situ", "license"] {
                assert!(entry.get(unclaimed).is_none(), "{file}: {unclaimed}");
            }
        }
    }
}

#[test]
fn each_profile_lists_every_patch_of_its_revision_once() {
    // The published trees: 75 patches at the root of VSCO 2 CE's branch `SFZ`, 183 in VCSL's tag
    // `v1.2.2-RC`.
    for (id, count) in [("vsco2ce", 75), ("vcsl", 183)] {
        let profile = profile(id);
        let listed = paths(profile);
        assert_eq!(listed.len(), count, "{id}");
        let unique: BTreeSet<String> = listed.iter().map(|p| p.to_lowercase()).collect();
        assert_eq!(unique.len(), count, "{id}: a patch is listed twice");
        for detect in &profile.detect {
            assert!(listed.contains(&detect.as_str()), "{id}: {detect}");
        }
    }
    assert!(
        paths(profile("vsco2ce")).iter().all(|p| !p.contains('/')),
        "every VSCO 2 CE patch lies at the root"
    );
}

#[test]
fn the_library_claims_are_cc0_with_their_source() {
    for (id, url, revision) in [
        (
            "vsco2ce",
            "https://github.com/sgossner/VSCO-2-CE",
            "branch SFZ",
        ),
        ("vcsl", "https://github.com/sgossner/VCSL", "tag v1.2.2-RC"),
    ] {
        let claims = &profile(id).claims;
        assert_eq!(
            claims.license,
            Field::Set("CC0 1.0".to_owned(), Claimant::Profile),
            "{id}"
        );
        let source = claims.source.value().expect("a source");
        assert_eq!(
            (source.url.as_str(), source.revision.as_deref()),
            (url, Some(revision))
        );
        let check = claims.checked.value().expect("a check date");
        assert_eq!(check.checked, jiff::civil::date(2026, 10, 30), "{id}");
        assert_eq!(check.method, CheckMethod::CodeReading, "{id}");
        assert_eq!(claims.attribution, Field::Unset, "CC0 asks for no credit");
    }
}

#[test]
fn every_entry_says_what_it_plays_and_how_well() {
    for profile in shipped() {
        for entry in &profile.claims.entries {
            let what = format!("{}: {:?}", profile.id, entry.target);
            // An instrument implies its family, so a family is claimed only without one.
            assert!(
                entry.instrument.value().is_some() != entry.family.value().is_some(),
                "{what}: an instrument or a family"
            );
            if entry.instrument.value().is_some() {
                assert!(entry.ensemble.value().is_some(), "{what}: ensemble");
                assert_eq!(
                    entry.dynamics.value(),
                    Some(&DynamicsMode::Velocity),
                    "{what}"
                );
            }
            match &entry.target {
                EntryTarget::Path(_) => {
                    assert!(entry.verdict.value().is_some(), "{what}: verdict");
                }
                EntryTarget::Set(_) => {
                    assert!(
                        entry.instrument.value().is_some(),
                        "{what}: a set's instrument"
                    );
                    assert!(entry.name.value().is_some(), "{what}: a set's name");
                }
            }
            // A mapped entry names exactly one ordinary sound.
            if !mapped(entry).is_empty() {
                let defaults = mapped(entry).iter().filter(|a| a.is_default).count();
                assert_eq!(defaults, 1, "{what}");
            }
            // A verdict with something audible lost is `partly`; `ampeg_dynamic` alone changes
            // nothing audible, as no controller changes the envelope.
            if let Some(verdict) = entry.verdict.value() {
                let audible = verdict.lost.iter().any(|line| line != "ampeg_dynamic");
                let expected = if audible {
                    VerdictState::Partly
                } else {
                    VerdictState::AsIs
                };
                assert_eq!(verdict.state, expected, "{what}");
            }
        }
    }
}

#[test]
fn every_set_member_is_a_listed_patch_that_claims_the_same() {
    for profile in shipped() {
        for set in &profile.claims.entries {
            let EntryTarget::Set(slug) = &set.target else {
                continue;
            };
            for articulation in mapped(set) {
                let member = articulation.member.as_deref().expect("a member path");
                let patch = entry(profile, member);
                let what = format!("{}: @{slug} {member}", profile.id);
                assert_eq!(patch.instrument, set.instrument, "{what}");
                assert_eq!(patch.ensemble, set.ensemble, "{what}");
                assert_eq!(patch.key_offset, set.key_offset, "{what}");
                // The member's own entry maps the same articulation, as its whole patch.
                let own: Vec<_> = mapped(patch)
                    .iter()
                    .map(|a| (a.name, a.mute, a.vibrato, a.via))
                    .collect();
                assert_eq!(
                    own,
                    [(
                        articulation.name,
                        articulation.mute,
                        articulation.vibrato,
                        ArticulationVia::Patch
                    )],
                    "{what}"
                );
            }
        }
    }
}

#[test]
fn the_names_a_library_shows_are_unique() {
    // The device editor labels a preset by its name, and two presets of one library that share a
    // name ignoring case get their paths added; the profile's names never need that.
    for profile in shipped() {
        let mut names = BTreeSet::new();
        for entry in &profile.claims.entries {
            let name = match (&entry.target, entry.name.value()) {
                (_, Some(name)) => name.clone(),
                (EntryTarget::Path(path), None) => {
                    let file = path.rsplit('/').next().unwrap_or(path);
                    file.strip_suffix(".sfz").unwrap_or(file).to_owned()
                }
                (EntryTarget::Set(slug), None) => panic!("@{slug} has no name"),
            };
            assert!(
                names.insert(name.to_lowercase()),
                "{}: {name:?} twice",
                profile.id
            );
        }
    }
}

#[test]
fn keys_lie_in_midi_range_and_apart() {
    for profile in shipped() {
        for entry in &profile.claims.entries {
            let switches: Vec<u8> = mapped(entry)
                .iter()
                .filter_map(|a| match a.via {
                    ArticulationVia::Keyswitch { key } => Some(key),
                    _ => None,
                })
                .collect();
            let mapped_keys: Vec<u8> = entry
                .keymap
                .value()
                .into_iter()
                .flatten()
                .flat_map(|row| row.keys.iter().copied())
                .collect();
            for key in switches.iter().chain(&mapped_keys) {
                assert!(*key <= 127, "{}: {:?} key {key}", profile.id, entry.target);
            }
            // A key switch never doubles as a pad of the key map.
            assert!(
                switches.iter().all(|key| !mapped_keys.contains(key)),
                "{}: {:?}",
                profile.id,
                entry.target
            );
        }
    }
}

/// The key switches of the shipped profiles that lie inside their instrument's playable range,
/// once the entry's key offset is undone: a note of a part there would switch the articulation
/// instead of sounding.
#[test]
fn switch_keys_inside_the_playable_range_are_known_and_flagged() {
    let mut inside = Vec::new();
    for profile in shipped() {
        for entry in &profile.claims.entries {
            let Some(instrument) = entry.instrument.value() else {
                continue;
            };
            let offset = entry.key_offset.value().copied().unwrap_or(0);
            let keys: Vec<u8> = mapped(entry)
                .iter()
                .filter_map(|a| match a.via {
                    ArticulationVia::Keyswitch { key } => Some(key),
                    _ => None,
                })
                .filter(|&key| {
                    // The note that lands on `key` after the offset is added.
                    SoundingPitch::new(i64::from(key) - i64::from(offset)).is_some_and(|pitch| {
                        matches!(
                            instrument.get().check_sounding(pitch),
                            Ok(RangeCheck::Within | RangeCheck::OutsideComfortable)
                        )
                    })
                })
                .collect();
            if !keys.is_empty() {
                inside.push((profile.id.as_str(), format!("{:?}", entry.target), keys));
                let verdict = entry.verdict.value().expect("a verdict");
                assert_eq!(verdict.state, VerdictState::Partly, "{:?}", entry.target);
            }
        }
    }
    assert_eq!(
        inside,
        [
            (
                "vsco2ce",
                format!("{:?}", EntryTarget::Path("CelloEns-KS.sfz".to_owned())),
                vec![84, 85, 86, 87]
            ),
            (
                "vcsl",
                format!(
                    "{:?}",
                    EntryTarget::Path(
                        "Aerophones/Edge-blown Aerophones/Renaissance Organ - Keyswitch.sfz"
                            .to_owned()
                    )
                ),
                vec![94]
            ),
            (
                "vcsl",
                format!(
                    "{:?}",
                    EntryTarget::Path(
                        "Idiophones/Struck Idiophones/Xylophone - Keyswitch.sfz".to_owned()
                    )
                ),
                vec![48]
            ),
            (
                "vcsl",
                format!(
                    "{:?}",
                    EntryTarget::Path(
                        "Membranophones/Struck Membranophones/Timpani 1 - Keyswitch.sfz".to_owned()
                    )
                ),
                vec![36, 37]
            ),
        ]
    );
    // The timpani's separate patches form a set that switches no key.
    let set = entry(profile("vcsl"), "@timpani-1");
    assert!(mapped(set).iter().all(|a| a.via == ArticulationVia::Patch));
}

// The expectations below were written from the patch files of each revision (their sw_last,
// sw_label and sw_default opcodes, key ranges and sample names) and from measuring the pitch of
// one sample per mallet instrument, not from the profiles.

#[test]
fn the_vsco_key_switch_patches_map_their_switches() {
    let vsco = profile("vsco2ce");
    let cases: [(&str, InstrumentType, EnsembleKind, &[&str]); 7] = [
        (
            "ViolinEns-KS.sfz",
            InstrumentType::Violin,
            EnsembleKind::Section,
            &[
                "key 36 sustain (default)",
                "key 37 tremolo",
                "key 38 spiccato",
                "key 39 pizzicato",
            ],
        ),
        (
            "SViolin-KS.sfz",
            InstrumentType::Violin,
            EnsembleKind::Solo,
            &[
                "key 36 sustain (default)",
                "key 37 tremolo",
                "key 38 spiccato",
                "key 39 pizzicato",
            ],
        ),
        (
            "CelloEns-KS.sfz",
            InstrumentType::Cello,
            EnsembleKind::Section,
            &[
                "key 84 sustain (default)",
                "key 85 tremolo",
                "key 86 spiccato",
                "key 87 pizzicato",
            ],
        ),
        (
            "Contrabass-KS.sfz",
            InstrumentType::DoubleBass,
            EnsembleKind::Solo,
            &[
                "key 84 sustain non_vibrato (default)",
                "key 85 sustain",
                "key 86 tremolo",
                "key 87 spiccato",
                "key 88 pizzicato",
            ],
        ),
        (
            "Flute-KS.sfz",
            InstrumentType::Flute,
            EnsembleKind::Solo,
            &[
                "key 36 sustain non_vibrato (default)",
                "key 37 sustain",
                "key 38 sustain vibrato_on",
                "key 39 staccato",
            ],
        ),
        (
            "Clarinet-KS.sfz",
            InstrumentType::ClarinetBb,
            EnsembleKind::Solo,
            &["key 36 sustain (default)", "key 37 staccato"],
        ),
        (
            "Tuba-KS.sfz",
            InstrumentType::Tuba,
            EnsembleKind::Solo,
            &["key 84 sustain (default)", "key 85 staccato"],
        ),
    ];
    for (path, instrument, kind, expected) in cases {
        let entry = entry(vsco, path);
        assert_eq!(entry.instrument.value(), Some(&instrument), "{path}");
        assert_eq!(entry.ensemble.value().map(|e| e.kind), Some(kind), "{path}");
        assert_eq!(articulations(entry), expected, "{path}");
    }
    // The violin section's labels are the library's own.
    let labels: Vec<Option<&str>> = mapped(entry(vsco, "ViolinEns-KS.sfz"))
        .iter()
        .map(|a| a.label.as_deref())
        .collect();
    assert_eq!(
        labels,
        [
            Some("Sustain Vibrato"),
            Some("Tremolo"),
            Some("Spiccato"),
            Some("Pizzicato")
        ]
    );
}

#[test]
fn the_vsco_single_patches_and_sets_map_what_they_record() {
    let vsco = profile("vsco2ce");
    for (path, expected) in [
        ("FHornMute.sfz", "patch sustain con_sordino (default)"),
        (
            "TrumpetStraightMuteSus.sfz",
            "patch sustain con_sordino (default)",
        ),
        (
            "TrumpetHarmonMuteSus.sfz",
            "patch sustain mute_harmon (default)",
        ),
        ("TrumpetSusVib.sfz", "patch sustain vibrato_on (default)"),
        ("OboeSusNV.sfz", "patch sustain non_vibrato (default)"),
        ("ViolinEnsSusVib-Quiet.sfz", "patch sustain (default)"),
        ("CelloEnsPizz.sfz", "patch pizzicato (default)"),
        ("TimpaniRolls.sfz", "patch roll (default)"),
        ("OrganLoudPedal.sfz", "patch ordinario (default)"),
    ] {
        assert_eq!(articulations(entry(vsco, path)), [expected], "{path}");
    }
    assert_eq!(
        articulations(entry(vsco, "@horn")),
        [
            "FHornSus.sfz sustain (default)",
            "FHornStac.sfz staccato",
            "FHornMute.sfz sustain con_sordino",
        ]
    );
    assert_eq!(
        articulations(entry(vsco, "@double-bass")),
        [
            "ContrabassSusNV.sfz sustain non_vibrato (default)",
            "ContrabassSusVB.sfz sustain",
            "ContrabassTrem.sfz tremolo",
            "ContrabassSpic.sfz spiccato",
            "ContrabassPizz.sfz pizzicato",
        ]
    );
    let slugs: Vec<&str> = vsco
        .claims
        .entries
        .iter()
        .filter_map(|e| match &e.target {
            EntryTarget::Set(slug) => Some(slug.as_str()),
            EntryTarget::Path(_) => None,
        })
        .collect();
    assert_eq!(
        slugs,
        [
            "piccolo",
            "flute",
            "oboe",
            "clarinet",
            "bassoon",
            "horn",
            "trumpet",
            "trombone",
            "tuba",
            "timpani",
            "violin-solo",
            "violin-section",
            "viola-section",
            "cello-section",
            "double-bass",
        ]
    );
}

#[test]
fn patches_keyed_an_octave_below_sounding_pitch_carry_an_offset() {
    // Measured: VSCO's piccolo G4 sample on key 67 sounds G5, its glockenspiel C5 sample on key
    // 72 sounds C6, and both libraries' xylophone C4 samples on key 60 sound C5; the marimbas,
    // the vibraphone, the harps and the tubular bells sound at their keys.
    let offsets = |id: &str| -> Vec<(String, i8)> {
        profile(id)
            .claims
            .entries
            .iter()
            .filter_map(|e| {
                e.key_offset
                    .value()
                    .map(|offset| (format!("{:?}", e.target), *offset))
            })
            .collect()
    };
    let path = |p: &str| format!("{:?}", EntryTarget::Path(p.to_owned()));
    let set = |s: &str| format!("{:?}", EntryTarget::Set(s.to_owned()));
    assert_eq!(
        offsets("vsco2ce"),
        [
            (path("PiccoloSus.sfz"), -12),
            (path("PiccoloStac.sfz"), -12),
            (set("piccolo"), -12),
            (path("Glockenspiel.sfz"), -12),
            (path("Xylophone.sfz"), -12),
        ]
    );
    let struck = |p: &str| path(&format!("Idiophones/Struck Idiophones/{p}.sfz"));
    assert_eq!(
        offsets("vcsl"),
        [
            (struck("Glockenspiel"), -12),
            (struck("Xylophone - Hard Mallets"), -12),
            (struck("Xylophone - Keyswitch"), -12),
            (struck("Xylophone - Medium Mallets"), -12),
            (struck("Xylophone - Soft Mallets"), -12),
        ]
    );
}

#[test]
fn the_percussion_patches_map_their_keys() {
    let perc = entry(profile("vsco2ce"), "GM-StylePerc.sfz");
    assert_eq!(perc.instrument, Field::Unset, "several instruments");
    assert_eq!(
        keymap(perc),
        [
            "36 bass_drum",
            "38 snare_drum",
            "39 snare_drum roll",
            "40 snare_drum snares_off",
            "42 tam_tam scrape",
            "46 tam_tam",
            "47 48 50 suspended_cymbal crescendo_swell",
            "49 clash_cymbals",
            "51 59 suspended_cymbal",
            "53 tambourine shake_stroke",
            "54 tambourine",
            "55 tambourine roll",
            "79 81 triangle",
            "82 sleigh_bells",
            "101 102 103 104 suspended_cymbal bowed",
        ]
    );
    let vcsl = profile("vcsl");
    for (path, instrument, expected) in [
        (
            "Membranophones/Struck Membranophones/Snare Drum, Modern 1.sfz",
            InstrumentType::SnareDrum,
            &[
                "61 snare_drum",
                "63 snare_drum roll",
                "60 snare_drum snares_off",
            ][..],
        ),
        (
            "Idiophones/Struck Idiophones/Triangles.sfz",
            InstrumentType::Triangle,
            &["60 65 69 triangle", "63 68 72 triangle roll"][..],
        ),
        (
            "Idiophones/Struck Idiophones/Clash Cymbals 1.sfz",
            InstrumentType::ClashCymbals,
            &["60 clash_cymbals", "61 62 clash_cymbals choke"][..],
        ),
        (
            "Idiophones/Struck Idiophones/Suspended Cymbal 1.sfz",
            InstrumentType::SuspendedCymbal,
            &[
                "68 suspended_cymbal",
                "71 suspended_cymbal roll",
                "64 65 66 67 suspended_cymbal crescendo_swell",
                "60 61 62 63 suspended_cymbal bowed",
                "72 suspended_cymbal scrape",
            ][..],
        ),
    ] {
        let entry = entry(vcsl, path);
        assert_eq!(entry.instrument.value(), Some(&instrument), "{path}");
        assert_eq!(keymap(entry), expected, "{path}");
    }
    // Every key map row of a single-instrument patch plays that instrument, and a pitched
    // instrument has no key map.
    for profile in shipped() {
        for entry in &profile.claims.entries {
            let rows = entry.keymap.value().into_iter().flatten();
            if let Some(instrument) = entry.instrument.value() {
                for row in rows {
                    assert_eq!(row.instrument, *instrument, "{:?}", entry.target);
                }
            }
        }
    }
}

#[test]
fn the_vcsl_saxophones_and_timpani_map_their_switches() {
    let vcsl = profile("vcsl");
    let reed = |p: &str| format!("Aerophones/Reed Aerophones/{p}.sfz");
    let tenor = entry(vcsl, &reed("Tenor Saxophone - Keyswitch"));
    assert_eq!(
        tenor.instrument.value(),
        Some(&InstrumentType::SaxophoneTenor)
    );
    assert_eq!(
        articulations(tenor),
        [
            "key 36 sustain (default)",
            "key 37 sustain non_vibrato",
            "key 38 staccato"
        ]
    );
    let saxello = entry(vcsl, &reed("Saxello - Keyswitch"));
    assert_eq!(
        saxello.instrument.value(),
        Some(&InstrumentType::SaxophoneSoprano)
    );
    assert_eq!(
        articulations(saxello),
        [
            "key 48 sustain (default)",
            "key 49 sustain non_vibrato",
            "key 50 staccato"
        ]
    );
    let timpani = |p: &str| format!("Membranophones/Struck Membranophones/{p}.sfz");
    assert_eq!(
        articulations(entry(vcsl, &timpani("Timpani 1 - Keyswitch"))),
        ["key 36 ordinario (default)", "key 37 roll"]
    );
    assert_eq!(
        articulations(entry(vcsl, "@timpani-1")),
        [
            format!("{} ordinario (default)", timpani("Timpani 1 - Hit")),
            format!("{} roll", timpani("Timpani 1 - Roll")),
        ]
    );
    // One recording per key from C4 up is not a timpani played by pitch.
    let all = entry(vcsl, &timpani("Timpani 2 - All Samples"));
    assert_eq!(all.instrument, Field::Unset);
    // The vibraphone key-switch patch's own default is the bow; its ordinary sound is the soft
    // mallets, and the hard mallets have no ID of their own.
    assert_eq!(
        articulations(entry(
            vcsl,
            "Idiophones/Struck Idiophones/Vibraphone - Keyswitch.sfz"
        )),
        ["key 36 bowed", "key 38 ordinario (default)"]
    );
}

#[test]
fn the_verdicts_name_what_the_sampler_loses() {
    let vsco = profile("vsco2ce");
    // Every VSCO 2 CE patch sets ampeg_dynamic, which changes nothing audible here.
    for entry in &vsco.claims.entries {
        if let Some(verdict) = entry.verdict.value() {
            assert_eq!(
                verdict.lost.first().map(String::as_str),
                Some("ampeg_dynamic")
            );
        }
    }
    let vcsl = profile("vcsl");
    let partly: Vec<&str> = vcsl
        .claims
        .entries
        .iter()
        .filter(|e| e.verdict.value().map(|v| v.state) == Some(VerdictState::Partly))
        .filter_map(|e| match &e.target {
            EntryTarget::Path(path) => Some(path.as_str()),
            EntryTarget::Set(_) => None,
        })
        .collect();
    // The patches that set envelope decay, release decay, sustain-pedal gates or controller
    // triggers, and the organ, xylophone and timpani whose switch keys lie in their range.
    assert_eq!(
        partly,
        [
            "Aerophones/Edge-blown Aerophones/Renaissance Organ - Keyswitch.sfz",
            "Aerophones/Free Aerophones/Harmonica-Hohner-Special20-C - Keyswitch.sfz",
            "Aerophones/Free Aerophones/Harmonica-Hohner-Special20-C - Normal.sfz",
            "Aerophones/Free Aerophones/Harmonica-Hohner-Special20-F - HandVib.sfz",
            "Aerophones/Free Aerophones/Harmonica-Hohner-Special20-F - Keyswitch.sfz",
            "Aerophones/Free Aerophones/Harmonica-Hohner-Super64 - Keyswitch.sfz",
            "Aerophones/Free Aerophones/Harmonica-Hohner-Super64 - Vib.sfz",
            "Chordophones/Zithers/Grand Piano, Kawai - Legacy.sfz",
            "Chordophones/Zithers/Grand Piano, Kawai.sfz",
            "Chordophones/Zithers/Grand Piano, Steinway B.sfz",
            "Chordophones/Zithers/Harpsichord, Flemish - 8'.sfz",
            "Chordophones/Zithers/Harpsichord, Flemish - Full.sfz",
            "Chordophones/Zithers/Harpsichord, Flemish - Keyswitch.sfz",
            "Chordophones/Zithers/Harpsichord, Unk.sfz",
            "Chordophones/Zithers/Upright Piano, Knight.sfz",
            "Idiophones/Struck Idiophones/Tubular Glockenspiel.sfz",
            "Idiophones/Struck Idiophones/Xylophone - Keyswitch.sfz",
            "Membranophones/Struck Membranophones/Timpani 1 - Keyswitch.sfz",
        ]
    );
    let knight = entry(vcsl, "Chordophones/Zithers/Upright Piano, Knight.sfz");
    let lost = &knight.verdict.value().expect("a verdict").lost;
    assert!(
        lost.iter().any(|line| line.starts_with("on_locc64")),
        "{lost:?}"
    );
    assert!(
        lost.iter().any(|line| line.starts_with("rt_decay")),
        "{lost:?}"
    );
}

#[test]
fn qualifiers_name_only_departures_from_the_ordinary_sound() {
    // A recording is marked with vibrato only beside a plain sustain of the same instrument, and
    // without vibrato only beside one with it, so each instrument has one plain sustain.
    for profile in shipped() {
        for entry in &profile.claims.entries {
            let sustains: Vec<(Mute, Vibrato)> = mapped(entry)
                .iter()
                .filter(|a| a.name.id() == "sustain")
                .map(|a| (a.mute, a.vibrato))
                .collect();
            if sustains.len() > 1 {
                assert!(
                    sustains.contains(&(Mute::Open, Vibrato::Ordinary)),
                    "{}: {:?}",
                    profile.id,
                    entry.target
                );
            }
        }
    }
}

#[test]
fn one_bad_entry_in_a_shipped_profile_rejects_the_profile() {
    // A profile is Sonara's data, so a mistake in it is a defect, not an entry to skip: the whole
    // profile fails to read and this test, rather than the user, finds it (TST-03).
    let (_, text) = SHIPPED_SOURCES
        .iter()
        .find(|(file, _)| *file == "vsco2ce.json")
        .expect("the VSCO 2 CE profile");
    let broken = text.replacen(r#""id": "tremolo""#, r#""id": "tremolando""#, 1);
    assert_ne!(&broken, text);
    assert!(matches!(
        super::parse(&broken),
        Err(super::ProfileError::Entry { entry, reason })
            if entry == "SViolin-KS.sfz" && reason.contains("tremolando")
    ));
}
