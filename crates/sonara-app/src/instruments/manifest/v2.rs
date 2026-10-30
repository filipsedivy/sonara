//! Version 2 of the manifest. The library fields are read in one pass; a value there that does
//! not validate rejects the whole manifest. The entries are read as JSON values and each is
//! deserialized and validated on its own, so a bad entry is rejected alone, with its reason,
//! and the rest is kept.

use std::collections::BTreeSet;
use std::fmt;
use std::ops::RangeInclusive;

use serde::{Deserialize, Deserializer};
use serde_json::Value;
use sonara_midi::orchestration::{InstrumentType, Mute, Vibrato};

use super::claims::{
    ArticulationEntry, ArticulationList, ArticulationName, ArticulationVia, CheckMethod,
    DynamicsMode, EnsembleClaim, EnsembleKind, KeymapEntry, LegatoMode, LibrarySource,
    ProfileCheck, Verdict, VerdictState,
};
use super::{
    ClaimSource, EntryTarget, Field, LibraryClaims, Manifest, ManifestEntry, ManifestError,
    parse_family, validate_relative_path,
};
use crate::instruments::model::LibraryId;

/// The longest set slug.
const MAX_SET_LEN: usize = 64;
/// The most players an ensemble claims.
const MAX_PLAYERS: u8 = 64;
/// MIDI keys, controller numbers and controller values.
const MIDI_DATA: RangeInclusive<u8> = 0..=127;
/// Velocities that sound; 0 is a note-off.
const VELOCITY: RangeInclusive<u8> = 1..=127;
/// The key offsets an entry may declare, in semitones.
const KEY_OFFSET: RangeInclusive<i8> = -48..=48;
/// The legato crossfade, start offset and gap, in milliseconds.
const CROSSFADE_MS: RangeInclusive<u16> = 10..=500;
const START_OFFSET_MS: RangeInclusive<u16> = 0..=500;
const GAP_MS: RangeInclusive<u16> = 0..=100;
/// The onset an articulation may declare, in milliseconds.
const ATTACK_MS: RangeInclusive<u16> = 0..=2000;

/// A field as written: left out (with `#[serde(default)]`), `null`, or a value.
#[derive(Default)]
enum Written<T> {
    #[default]
    Absent,
    Null,
    Value(T),
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Written<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(deserializer).map(|value| value.map_or(Self::Null, Self::Value))
    }
}

#[derive(Deserialize)]
#[serde(expecting = "a manifest object")]
struct RawManifest {
    id: Option<String>,
    #[serde(default)]
    name: Written<String>,
    #[serde(default)]
    license: Written<String>,
    #[serde(default)]
    attribution: Written<String>,
    #[serde(default)]
    source: Written<RawSource>,
    #[serde(default)]
    detect: Vec<String>,
    #[serde(default)]
    profile: Written<RawCheck>,
    #[serde(default)]
    instruments: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(expecting = "a source object with a url")]
struct RawSource {
    url: String,
    revision: Option<String>,
}

#[derive(Deserialize)]
#[serde(expecting = "a profile object with checked and method")]
struct RawCheck {
    checked: String,
    method: CheckMethod,
}

#[derive(Deserialize)]
#[serde(expecting = "an entry object")]
struct RawEntry {
    path: Option<String>,
    set: Option<String>,
    #[serde(default)]
    name: Written<String>,
    #[serde(default)]
    family: Written<String>,
    #[serde(default)]
    instrument: Written<String>,
    #[serde(default)]
    ensemble: Written<RawEnsemble>,
    #[serde(default)]
    articulations: Written<Vec<RawArticulation>>,
    #[serde(default)]
    keymap: Written<Vec<RawKeymapRow>>,
    #[serde(default)]
    key_offset: Written<i64>,
    #[serde(default)]
    dynamics: Written<RawDynamics>,
    #[serde(default)]
    legato: Written<RawLegato>,
    #[serde(default)]
    in_situ: Written<bool>,
    #[serde(default)]
    verdict: Written<RawVerdict>,
    #[serde(default)]
    license: Written<String>,
}

#[derive(Deserialize)]
#[serde(expecting = "an ensemble object with a kind")]
struct RawEnsemble {
    kind: EnsembleKind,
    players: Option<i64>,
}

#[derive(Deserialize)]
#[serde(expecting = "an articulation object with an id and a via")]
struct RawArticulation {
    id: String,
    via: RawVia,
    key: Option<i64>,
    lo: Option<i64>,
    hi: Option<i64>,
    cc: Option<i64>,
    value: Option<i64>,
    path: Option<String>,
    label: Option<String>,
    #[serde(default, rename = "default")]
    is_default: bool,
    mute: Option<String>,
    vibrato: Option<String>,
    attack_ms: Option<i64>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawVia {
    Patch,
    Keyswitch,
    Velocity,
    Cc,
}

#[derive(Deserialize)]
#[serde(expecting = "a key map object with keys and an instrument")]
struct RawKeymapRow {
    keys: Vec<i64>,
    instrument: String,
    articulation: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(expecting = "a dynamics object with a via")]
struct RawDynamics {
    via: RawDynamicsVia,
    cc: Option<i64>,
    velocity: Option<i64>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawDynamicsVia {
    Velocity,
    Cc,
    Fixed,
}

#[derive(Deserialize)]
#[serde(expecting = "a legato object with a kind")]
struct RawLegato {
    kind: RawLegatoKind,
    crossfade_ms: Option<i64>,
    start_offset_ms: Option<i64>,
    gap_ms: Option<i64>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawLegatoKind {
    None,
    Overlap,
    Trigger,
}

#[derive(Deserialize)]
#[serde(expecting = "a verdict object with a state")]
struct RawVerdict {
    state: VerdictState,
    #[serde(default)]
    lost: Vec<String>,
}

/// Reads a version 2 manifest; `source` says who claims what it holds.
pub(super) fn parse(bytes: &[u8], source: ClaimSource) -> Result<Manifest, ManifestError> {
    let raw: RawManifest = serde_json::from_slice(bytes).map_err(ManifestError::Parse)?;
    let id = raw.id.as_deref().map(LibraryId::parse).transpose()?;
    let library = |field| move |reason| ManifestError::Library { field, reason };
    for path in &raw.detect {
        validate_relative_path(path)
            .map_err(|reason| format!("{path:?}: {reason}"))
            .map_err(library("detect"))?;
    }
    let library_source = field(raw.source, source, library_source).map_err(library("source"))?;
    let checked = field(raw.profile, source, |raw| check(&raw)).map_err(library("profile"))?;

    let mut entries: Vec<ManifestEntry> = Vec::new();
    let mut rejected = Vec::new();
    for (index, value) in raw.instruments.iter().enumerate() {
        match parse_entry(value, source) {
            Ok(entry)
                if entries
                    .iter()
                    .any(|listed| listed.target.key() == entry.target.key()) =>
            {
                rejected.push((entry.target.label(), "listed twice".to_owned()));
            }
            Ok(entry) => entries.push(entry),
            Err(reason) => rejected.push((label(value, index), reason)),
        }
    }
    Ok(Manifest {
        version: 2,
        id,
        detect: raw.detect,
        claims: LibraryClaims {
            name: text(raw.name, source),
            license: text(raw.license, source),
            attribution: text(raw.attribution, source),
            source: library_source,
            checked,
            entries,
        },
        rejected,
    })
}

/// How a rejected entry is named: its `path`, `@<set>`, or `#<n>`, its position from 1.
fn label(value: &Value, index: usize) -> String {
    if let Some(path) = value.get("path").and_then(Value::as_str) {
        path.to_owned()
    } else if let Some(slug) = value.get("set").and_then(Value::as_str) {
        format!("@{slug}")
    } else {
        format!("#{}", index.saturating_add(1))
    }
}

/// A text field: written as `null` clears it; a blank text is as if it were left out.
fn text(raw: Written<String>, source: ClaimSource) -> Field<String> {
    match raw {
        Written::Absent => Field::Unset,
        Written::Null => Field::Cleared,
        Written::Value(text) if text.trim().is_empty() => Field::Unset,
        Written::Value(text) => Field::Set(text, source),
    }
}

/// A field whose value `validate` checks.
fn field<R, T>(
    raw: Written<R>,
    source: ClaimSource,
    validate: impl FnOnce(R) -> Result<T, String>,
) -> Result<Field<T>, String> {
    Ok(match raw {
        Written::Absent => Field::Unset,
        Written::Null => Field::Cleared,
        Written::Value(raw) => Field::Set(validate(raw)?, source),
    })
}

/// A text that is `None` when blank.
fn optional_text(text: Option<String>) -> Option<String> {
    text.filter(|text| !text.trim().is_empty())
}

/// `value` as a `T` within `range`, or why it is not, naming the value `what`.
fn in_range<T>(what: &str, value: i64, range: &RangeInclusive<T>) -> Result<T, String>
where
    T: TryFrom<i64> + PartialOrd + fmt::Display,
{
    T::try_from(value)
        .ok()
        .filter(|value| range.contains(value))
        .ok_or_else(|| {
            format!(
                "{what} {value} is not between {} and {}",
                range.start(),
                range.end()
            )
        })
}

/// Checks that of `given`, the fields written, only those `takes` names are; `what` names the
/// object that takes them.
fn only(what: &str, given: &[(&str, bool)], takes: &[&str]) -> Result<(), String> {
    match given
        .iter()
        .find(|(name, written)| *written && !takes.contains(name))
    {
        Some((name, _)) => Err(format!("{what} takes no {name}")),
        None => Ok(()),
    }
}

/// `value`, which `what` needs.
fn needs(what: &str, name: &str, value: Option<i64>) -> Result<i64, String> {
    value.ok_or_else(|| format!("{what} needs {name}"))
}

/// Reads and validates one entry.
fn parse_entry(value: &Value, source: ClaimSource) -> Result<ManifestEntry, String> {
    if !value.is_object() {
        return Err("an entry must be a JSON object".to_owned());
    }
    let raw = RawEntry::deserialize(value).map_err(|err| err.to_string())?;
    let target = target(raw.path, raw.set)?;
    let in_set = matches!(target, EntryTarget::Set(_));
    let mut entry = ManifestEntry::new(target);
    entry.name = text(raw.name, source);
    entry.family = field(raw.family, source, |slug| parse_family(&slug))?;
    entry.instrument = field(raw.instrument, source, |id| instrument(&id))?;
    entry.ensemble = field(raw.ensemble, source, |raw| ensemble(&raw))?;
    entry.articulations = field(raw.articulations, source, |list| {
        articulations(list, in_set)
    })?;
    entry.keymap = field(raw.keymap, source, keymap)?;
    entry.key_offset = field(raw.key_offset, source, |offset| {
        in_range("key_offset", offset, &KEY_OFFSET)
    })?;
    entry.dynamics = field(raw.dynamics, source, |raw| dynamics(&raw))?;
    entry.legato = field(raw.legato, source, |raw| legato(&raw))?;
    entry.in_situ = field(raw.in_situ, source, Ok)?;
    entry.verdict = field(raw.verdict, source, |raw| Ok(verdict(raw)))?;
    entry.license = text(raw.license, source);
    Ok(entry)
}

/// The patch or set an entry describes: exactly one of `path` and `set`.
fn target(path: Option<String>, set: Option<String>) -> Result<EntryTarget, String> {
    match (path, set) {
        (Some(_), Some(_)) => Err("an entry names a path or a set, not both".to_owned()),
        (None, None) => Err("an entry names a path or a set".to_owned()),
        (Some(path), None) => {
            if path.starts_with('@') {
                return Err("a patch path must not start with '@'".to_owned());
            }
            validate_relative_path(&path)?;
            Ok(EntryTarget::Path(path))
        }
        (None, Some(slug)) => {
            let mut chars = slug.chars();
            let valid_start = chars
                .next()
                .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
            let valid_rest =
                chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            if valid_start && valid_rest && slug.len() <= MAX_SET_LEN {
                Ok(EntryTarget::Set(slug))
            } else {
                Err(format!(
                    "set {slug:?}: use lower-case letters, digits and '-' (1 to {MAX_SET_LEN}), \
                     starting with a letter or digit"
                ))
            }
        }
    }
}

/// The instrument with orchestration id `id`.
fn instrument(id: &str) -> Result<InstrumentType, String> {
    InstrumentType::from_id(id).ok_or_else(|| format!("unknown instrument {id:?}"))
}

fn ensemble(raw: &RawEnsemble) -> Result<EnsembleClaim, String> {
    let players = raw
        .players
        .map(|players| in_range("players", players, &(1..=MAX_PLAYERS)))
        .transpose()?;
    if raw.kind == EnsembleKind::Solo && players.is_some_and(|players| players != 1) {
        return Err("a solo has one player".to_owned());
    }
    Ok(EnsembleClaim {
        kind: raw.kind,
        players,
    })
}

/// The articulations of an entry: each valid, none twice with the same mute and vibrato, at
/// most one the default.
fn articulations(list: Vec<RawArticulation>, in_set: bool) -> Result<ArticulationList, String> {
    let mut entries: Vec<ArticulationEntry> = Vec::with_capacity(list.len());
    for raw in list {
        let entry = articulation(raw, in_set)?;
        let same = |listed: &&ArticulationEntry| {
            (listed.name, listed.mute, listed.vibrato) == (entry.name, entry.mute, entry.vibrato)
        };
        if entries.iter().any(|listed| same(&listed)) {
            return Err(format!(
                "articulation {:?} is listed twice",
                entry.name.id()
            ));
        }
        entries.push(entry);
    }
    if entries.iter().filter(|entry| entry.is_default).count() > 1 {
        return Err("more than one articulation is the default".to_owned());
    }
    Ok(ArticulationList::Mapped(entries))
}

fn articulation(raw: RawArticulation, in_set: bool) -> Result<ArticulationEntry, String> {
    let name = ArticulationName::parse(&raw.id)
        .ok_or_else(|| format!("unknown articulation {:?}", raw.id))?;
    let about = |reason: String| format!("articulation {:?}: {reason}", raw.id);
    let via = via(&raw).map_err(about)?;
    let member = match (in_set, &raw.path) {
        (true, Some(path)) => {
            validate_relative_path(path).map_err(|reason| about(format!("{path:?}: {reason}")))?;
            Some(path.clone())
        }
        (true, None) => {
            return Err(about(
                "an articulation of a set names its patch with a path".to_owned(),
            ));
        }
        (false, Some(_)) => {
            return Err(about(
                "only an articulation of a set names a path; the entry's own patch plays it"
                    .to_owned(),
            ));
        }
        (false, None) => None,
    };
    let mute = raw
        .mute
        .as_deref()
        .map(|text| mute(text).ok_or_else(|| about(format!("unknown mute {text:?}"))))
        .transpose()?
        .unwrap_or_default();
    let vibrato = raw
        .vibrato
        .as_deref()
        .map(|text| vibrato(text).ok_or_else(|| about(format!("unknown vibrato {text:?}"))))
        .transpose()?
        .unwrap_or_default();
    let attack_ms = raw
        .attack_ms
        .map(|ms| in_range("attack_ms", ms, &ATTACK_MS))
        .transpose()
        .map_err(about)?;
    Ok(ArticulationEntry {
        name,
        via,
        member,
        label: optional_text(raw.label),
        is_default: raw.is_default,
        mute,
        vibrato,
        attack_ms,
    })
}

/// How an articulation is reached, with the fields its `via` takes and no other.
fn via(raw: &RawArticulation) -> Result<ArticulationVia, String> {
    let given = [
        ("key", raw.key.is_some()),
        ("lo", raw.lo.is_some()),
        ("hi", raw.hi.is_some()),
        ("cc", raw.cc.is_some()),
        ("value", raw.value.is_some()),
    ];
    match raw.via {
        RawVia::Patch => {
            only("via patch", &given, &[])?;
            Ok(ArticulationVia::Patch)
        }
        RawVia::Keyswitch => {
            let what = "via keyswitch";
            only(what, &given, &["key"])?;
            let key = in_range("key", needs(what, "a key", raw.key)?, &MIDI_DATA)?;
            Ok(ArticulationVia::Keyswitch { key })
        }
        RawVia::Velocity => {
            let what = "via velocity";
            only(what, &given, &["lo", "hi"])?;
            let lo = in_range("lo", needs(what, "lo", raw.lo)?, &VELOCITY)?;
            let hi = in_range("hi", needs(what, "hi", raw.hi)?, &VELOCITY)?;
            if lo > hi {
                return Err(format!("lo {lo} is above hi {hi}"));
            }
            Ok(ArticulationVia::Velocity { lo, hi })
        }
        RawVia::Cc => {
            let what = "via cc";
            only(what, &given, &["cc", "value"])?;
            let cc = in_range("cc", needs(what, "a cc", raw.cc)?, &MIDI_DATA)?;
            let value = in_range("value", needs(what, "a value", raw.value)?, &MIDI_DATA)?;
            Ok(ArticulationVia::Cc { cc, value })
        }
    }
}

/// The mute that the articulation id `text` names (`con_sordino`, `mute_cup`, …).
fn mute(text: &str) -> Option<Mute> {
    Mute::ALL.into_iter().find(|mute| {
        mute.articulation()
            .is_some_and(|articulation| articulation.id() == text)
    })
}

/// The vibrato that the articulation id `text` names (`vibrato_on`, `non_vibrato`).
fn vibrato(text: &str) -> Option<Vibrato> {
    Vibrato::ALL.into_iter().find(|vibrato| {
        vibrato
            .articulation()
            .is_some_and(|articulation| articulation.id() == text)
    })
}

/// A percussion key map: every row with at least one key, no key in two rows.
fn keymap(rows: Vec<RawKeymapRow>) -> Result<Vec<KeymapEntry>, String> {
    let mut mapped = BTreeSet::new();
    rows.into_iter()
        .map(|row| {
            if row.keys.is_empty() {
                return Err("a key map row needs at least one key".to_owned());
            }
            let keys = row
                .keys
                .iter()
                .map(|&key| in_range("key", key, &MIDI_DATA))
                .collect::<Result<Vec<u8>, String>>()?;
            if let Some(key) = keys.iter().find(|&&key| !mapped.insert(key)) {
                return Err(format!("key {key} is mapped twice"));
            }
            let articulation = row
                .articulation
                .as_deref()
                .map(|id| {
                    ArticulationName::parse(id)
                        .ok_or_else(|| format!("unknown articulation {id:?}"))
                })
                .transpose()?;
            Ok(KeymapEntry {
                keys,
                instrument: instrument(&row.instrument)?,
                articulation,
                name: optional_text(row.name),
            })
        })
        .collect()
}

fn dynamics(raw: &RawDynamics) -> Result<DynamicsMode, String> {
    let given = [
        ("cc", raw.cc.is_some()),
        ("velocity", raw.velocity.is_some()),
    ];
    match raw.via {
        RawDynamicsVia::Velocity => {
            only("dynamics via velocity", &given, &[])?;
            Ok(DynamicsMode::Velocity)
        }
        RawDynamicsVia::Cc => {
            let what = "dynamics via cc";
            only(what, &given, &["cc"])?;
            Ok(DynamicsMode::Cc(in_range(
                "cc",
                needs(what, "a cc", raw.cc)?,
                &MIDI_DATA,
            )?))
        }
        RawDynamicsVia::Fixed => {
            let what = "dynamics via fixed";
            only(what, &given, &["velocity"])?;
            Ok(DynamicsMode::Fixed(in_range(
                "velocity",
                needs(what, "a velocity", raw.velocity)?,
                &VELOCITY,
            )?))
        }
    }
}

fn legato(raw: &RawLegato) -> Result<LegatoMode, String> {
    let given = [
        ("crossfade_ms", raw.crossfade_ms.is_some()),
        ("start_offset_ms", raw.start_offset_ms.is_some()),
        ("gap_ms", raw.gap_ms.is_some()),
    ];
    match raw.kind {
        RawLegatoKind::None => {
            only("legato none", &given, &[])?;
            Ok(LegatoMode::None)
        }
        RawLegatoKind::Trigger => {
            only("legato trigger", &given, &[])?;
            Ok(LegatoMode::Trigger)
        }
        RawLegatoKind::Overlap => {
            let what = "legato overlap";
            Ok(LegatoMode::Overlap {
                crossfade_ms: in_range(
                    "crossfade_ms",
                    needs(what, "crossfade_ms", raw.crossfade_ms)?,
                    &CROSSFADE_MS,
                )?,
                start_offset_ms: in_range(
                    "start_offset_ms",
                    needs(what, "start_offset_ms", raw.start_offset_ms)?,
                    &START_OFFSET_MS,
                )?,
                gap_ms: in_range("gap_ms", needs(what, "gap_ms", raw.gap_ms)?, &GAP_MS)?,
            })
        }
    }
}

fn verdict(raw: RawVerdict) -> Verdict {
    Verdict {
        state: raw.state,
        lost: raw
            .lost
            .into_iter()
            .map(|line| line.trim().to_owned())
            .filter(|line| !line.is_empty())
            .collect(),
    }
}

fn library_source(raw: RawSource) -> Result<LibrarySource, String> {
    if raw.url.trim().is_empty() {
        return Err("the url is empty".to_owned());
    }
    Ok(LibrarySource {
        url: raw.url,
        revision: optional_text(raw.revision),
    })
}

fn check(raw: &RawCheck) -> Result<ProfileCheck, String> {
    let checked = raw.checked.parse::<jiff::civil::Date>().map_err(|err| {
        format!(
            "checked {:?} is not a date (YYYY-MM-DD): {err}",
            raw.checked
        )
    })?;
    Ok(ProfileCheck {
        checked,
        method: raw.method,
    })
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use serde_json::{Value, json};
    use sonara_midi::orchestration::{Articulation, InstrumentType, Mute, Vibrato};

    use super::super::{ClaimSource, EntryTarget, Field, Manifest, ManifestError, parse};
    use super::{
        ArticulationEntry, ArticulationList, ArticulationName, ArticulationVia, CheckMethod,
        DynamicsMode, EnsembleClaim, EnsembleKind, KeymapEntry, LegatoMode, LibrarySource,
        ProfileCheck, Verdict, VerdictState,
    };
    use crate::instruments::InstrumentFamily;
    use crate::instruments::model::LibraryId;

    const MANIFEST: ClaimSource = ClaimSource::Manifest;

    fn read(json: &Value) -> Manifest {
        parse(json.to_string().as_bytes(), MANIFEST).expect("the manifest itself is valid")
    }

    /// The manifest of the given entries.
    fn with_entries(entries: &[Value]) -> Manifest {
        read(&json!({"version": 2, "instruments": entries}))
    }

    /// Why the one entry `entry` is rejected; panics when it is kept.
    fn rejected(entry: &Value) -> String {
        let manifest = with_entries(std::slice::from_ref(entry));
        assert!(manifest.claims.entries.is_empty(), "kept: {entry}");
        match manifest.rejected.as_slice() {
            [(_, reason)] => reason.clone(),
            other => panic!("{other:?}"),
        }
    }

    fn set<T>(value: T) -> Field<T> {
        Field::Set(value, MANIFEST)
    }

    fn named(articulation: Articulation) -> ArticulationName {
        ArticulationName::Vocabulary(articulation)
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one manifest with every field of the format, checked field by field"
    )]
    fn every_field_of_a_version_2_manifest_is_read() {
        let manifest = read(&json!({
            "version": 2, "id": "vsco2ce", "name": "VSCO 2 Community Edition",
            "license": "CC0 1.0", "attribution": null,
            "source": {"url": "https://github.com/sgossner/VSCO-2-CE", "revision": "branch SFZ"},
            "detect": ["ViolinEns-KS.sfz", "Tuba-KS.sfz"],
            "profile": {"checked": "2026-10-30", "method": "code_reading"},
            "instruments": [
                {
                    "path": "ViolinEns-KS.sfz", "name": "Violin Section (key switches)",
                    "family": "strings", "instrument": "violin",
                    "ensemble": {"kind": "section", "players": null},
                    "articulations": [
                        {"id": "sustain", "via": "keyswitch", "key": 36,
                         "label": "Sustain Vibrato", "default": true},
                        {"id": "staccato", "via": "velocity", "lo": 1, "hi": 64,
                         "attack_ms": 12},
                        {"id": "pizzicato", "via": "cc", "cc": 32, "value": 3},
                        {"id": "ordinario", "via": "patch", "vibrato": "non_vibrato"}
                    ],
                    "dynamics": {"via": "velocity"},
                    "legato": {"kind": "overlap", "crossfade_ms": 90, "start_offset_ms": 60,
                               "gap_ms": 0},
                    "in_situ": null, "key_offset": -12,
                    "verdict": {"state": "partly", "lost": [" sw_default ", "", "ampeg_dynamic"]},
                    "license": "CC0 1.0 (this recording)"
                },
                {
                    "set": "horn-solo", "instrument": "horn_f",
                    "ensemble": {"kind": "solo", "players": 1},
                    "articulations": [
                        {"id": "sustain", "via": "patch", "path": "FHornSus.sfz",
                         "default": true},
                        {"id": "sustain", "via": "patch", "path": "FHornMute.sfz",
                         "mute": "con_sordino"},
                        {"id": "staccato", "via": "keyswitch", "key": 24,
                         "path": "FHornStac.sfz"}
                    ],
                    "dynamics": {"via": "cc", "cc": 1}, "legato": {"kind": "trigger"}
                },
                {
                    "path": "GM-StylePerc.sfz", "family": "percussion", "instrument": null,
                    "keymap": [
                        {"keys": [36], "instrument": "bass_drum"},
                        {"keys": [39], "instrument": "snare_drum", "articulation": "roll",
                         "name": " "},
                        {"keys": [78, 79], "instrument": "triangle", "name": "Triangle"}
                    ],
                    "dynamics": {"via": "fixed", "velocity": 100}, "legato": {"kind": "none"},
                    "verdict": {"state": "as_is"}, "in_situ": false
                }
            ]
        }));
        assert_eq!(manifest.rejected, []);
        assert_eq!(manifest.version, 2);
        assert_eq!(manifest.id.as_ref().map(LibraryId::as_str), Some("vsco2ce"));
        assert_eq!(manifest.detect, ["ViolinEns-KS.sfz", "Tuba-KS.sfz"]);
        let library = &manifest.claims;
        assert_eq!(library.name, set("VSCO 2 Community Edition".to_owned()));
        assert_eq!(library.license, set("CC0 1.0".to_owned()));
        assert_eq!(library.attribution, Field::Cleared);
        assert_eq!(
            library.source,
            set(LibrarySource {
                url: "https://github.com/sgossner/VSCO-2-CE".to_owned(),
                revision: Some("branch SFZ".to_owned()),
            })
        );
        assert_eq!(
            library.checked,
            set(ProfileCheck {
                checked: jiff::civil::date(2026, 10, 30),
                method: CheckMethod::CodeReading,
            })
        );

        let [violin, horn, kit] = library.entries.as_slice() else {
            panic!("{:?}", library.entries);
        };
        assert_eq!(
            violin.target,
            EntryTarget::Path("ViolinEns-KS.sfz".to_owned())
        );
        assert_eq!(violin.family, set(InstrumentFamily::Strings));
        assert_eq!(violin.instrument, set(InstrumentType::Violin));
        assert_eq!(
            violin.ensemble,
            set(EnsembleClaim {
                kind: EnsembleKind::Section,
                players: None
            })
        );
        let plain = |name, via| ArticulationEntry {
            name,
            via,
            member: None,
            label: None,
            is_default: false,
            mute: Mute::Open,
            vibrato: Vibrato::Ordinary,
            attack_ms: None,
        };
        assert_eq!(
            violin.articulations,
            set(ArticulationList::Mapped(vec![
                ArticulationEntry {
                    label: Some("Sustain Vibrato".to_owned()),
                    is_default: true,
                    ..plain(
                        named(Articulation::Sustain),
                        ArticulationVia::Keyswitch { key: 36 }
                    )
                },
                ArticulationEntry {
                    attack_ms: Some(12),
                    ..plain(
                        named(Articulation::Staccato),
                        ArticulationVia::Velocity { lo: 1, hi: 64 }
                    )
                },
                plain(
                    named(Articulation::Pizzicato),
                    ArticulationVia::Cc { cc: 32, value: 3 }
                ),
                ArticulationEntry {
                    vibrato: Vibrato::Off,
                    ..plain(ArticulationName::Ordinario, ArticulationVia::Patch)
                },
            ]))
        );
        assert_eq!(violin.dynamics, set(DynamicsMode::Velocity));
        assert_eq!(
            violin.legato,
            set(LegatoMode::Overlap {
                crossfade_ms: 90,
                start_offset_ms: 60,
                gap_ms: 0
            })
        );
        assert_eq!(violin.in_situ, Field::Cleared);
        assert_eq!(violin.key_offset, set(-12));
        assert_eq!(
            violin.verdict,
            set(Verdict {
                state: VerdictState::Partly,
                lost: vec!["sw_default".to_owned(), "ampeg_dynamic".to_owned()],
            })
        );
        assert_eq!(violin.license, set("CC0 1.0 (this recording)".to_owned()));
        assert_eq!(violin.keymap, Field::Unset);

        assert_eq!(horn.target, EntryTarget::Set("horn-solo".to_owned()));
        assert_eq!(
            horn.ensemble,
            set(EnsembleClaim {
                kind: EnsembleKind::Solo,
                players: Some(1)
            })
        );
        let Field::Set(ArticulationList::Mapped(members), _) = &horn.articulations else {
            panic!("{:?}", horn.articulations);
        };
        let members: Vec<(&str, Option<&str>, Mute, ArticulationVia)> = members
            .iter()
            .map(|a| (a.name.id(), a.member.as_deref(), a.mute, a.via))
            .collect();
        assert_eq!(
            members,
            [
                (
                    "sustain",
                    Some("FHornSus.sfz"),
                    Mute::Open,
                    ArticulationVia::Patch
                ),
                (
                    "sustain",
                    Some("FHornMute.sfz"),
                    Mute::Generic,
                    ArticulationVia::Patch
                ),
                (
                    "staccato",
                    Some("FHornStac.sfz"),
                    Mute::Open,
                    ArticulationVia::Keyswitch { key: 24 }
                ),
            ]
        );
        assert_eq!(horn.dynamics, set(DynamicsMode::Cc(1)));
        assert_eq!(horn.legato, set(LegatoMode::Trigger));
        assert_eq!(horn.name, Field::Unset);

        assert_eq!(kit.instrument, Field::Cleared);
        assert_eq!(
            kit.keymap,
            set(vec![
                KeymapEntry {
                    keys: vec![36],
                    instrument: InstrumentType::BassDrum,
                    articulation: None,
                    name: None,
                },
                KeymapEntry {
                    keys: vec![39],
                    instrument: InstrumentType::SnareDrum,
                    articulation: Some(named(Articulation::Roll)),
                    name: None,
                },
                KeymapEntry {
                    keys: vec![78, 79],
                    instrument: InstrumentType::Triangle,
                    articulation: None,
                    name: Some("Triangle".to_owned()),
                },
            ])
        );
        assert_eq!(kit.dynamics, set(DynamicsMode::Fixed(100)));
        assert_eq!(kit.legato, set(LegatoMode::None));
        assert_eq!(kit.in_situ, set(false));
        assert_eq!(
            kit.verdict,
            set(Verdict {
                state: VerdictState::AsIs,
                lost: Vec::new()
            })
        );
    }

    /// `null` clears a field, leaving it out leaves it unset, and a blank text is as if it were
    /// left out.
    #[test]
    fn null_clears_and_absence_leaves_unset() {
        let manifest = read(&json!({
            "version": 2, "name": null, "license": " ", "source": null, "profile": null,
            "instruments": [{"path": "a.sfz", "name": null, "family": null,
                "articulations": null, "keymap": null, "key_offset": null, "dynamics": null,
                "legato": null, "verdict": null, "license": null, "ensemble": null},
                {"path": "b.sfz", "articulations": []}]
        }));
        assert_eq!(manifest.claims.name, Field::Cleared);
        assert_eq!(manifest.claims.license, Field::Unset);
        assert_eq!(manifest.claims.source, Field::Cleared);
        assert_eq!(manifest.claims.checked, Field::Cleared);
        assert_eq!(manifest.claims.attribution, Field::Unset);
        let [cleared, empty] = manifest.claims.entries.as_slice() else {
            panic!("{:?}", manifest.claims.entries);
        };
        assert_eq!(cleared.name, Field::Cleared);
        assert_eq!(cleared.family, Field::Cleared);
        assert_eq!(cleared.articulations, Field::Cleared);
        assert_eq!(cleared.keymap, Field::Cleared);
        assert_eq!(cleared.key_offset, Field::Cleared);
        assert_eq!(cleared.dynamics, Field::Cleared);
        assert_eq!(cleared.legato, Field::Cleared);
        assert_eq!(cleared.verdict, Field::Cleared);
        assert_eq!(cleared.license, Field::Cleared);
        assert_eq!(cleared.ensemble, Field::Cleared);
        assert_eq!(cleared.instrument, Field::Unset);
        assert_eq!(cleared.in_situ, Field::Unset);
        assert_eq!(
            empty.articulations,
            set(ArticulationList::Mapped(Vec::new()))
        );
    }

    /// Each invalid entry is rejected with its own reason (TST-03).
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "a table of every reason an entry is rejected for"
    )]
    fn invalid_entries_are_rejected_with_their_reason() {
        let cases: Vec<(Value, &str)> = vec![
            (json!(5), "an entry must be a JSON object"),
            (json!({"name": "x"}), "an entry names a path or a set"),
            (
                json!({"path": "a.sfz", "set": "a"}),
                "an entry names a path or a set, not both",
            ),
            (
                json!({"path": "@a.sfz"}),
                "a patch path must not start with '@'",
            ),
            (
                json!({"path": "../a.sfz"}),
                "the path must not hold empty, '.' or '..' parts",
            ),
            (
                json!({"set": "Horn"}),
                "set \"Horn\": use lower-case letters, digits and '-' (1 to 64), starting with a \
                 letter or digit",
            ),
            (
                json!({"path": "a.sfz", "instrument": "kazoo"}),
                "unknown instrument \"kazoo\"",
            ),
            (
                json!({"path": "a.sfz", "family": "Brass"}),
                "unknown family \"Brass\"",
            ),
            (
                json!({"path": "a.sfz", "articulations": [{"id": "pizz", "via": "patch"}]}),
                "unknown articulation \"pizz\"",
            ),
            (
                json!({"path": "a.sfz", "articulations": [{"id": "sustain", "via": "keyswitch",
                    "key": 128}]}),
                "articulation \"sustain\": key 128 is not between 0 and 127",
            ),
            (
                json!({"path": "a.sfz", "articulations": [{"id": "sustain", "via": "keyswitch"}]}),
                "articulation \"sustain\": via keyswitch needs a key",
            ),
            (
                json!({"path": "a.sfz", "articulations": [{"id": "sustain", "via": "patch",
                    "key": 36}]}),
                "articulation \"sustain\": via patch takes no key",
            ),
            (
                json!({"path": "a.sfz", "articulations": [{"id": "sustain", "via": "velocity",
                    "lo": 0, "hi": 64}]}),
                "articulation \"sustain\": lo 0 is not between 1 and 127",
            ),
            (
                json!({"path": "a.sfz", "articulations": [{"id": "sustain", "via": "velocity",
                    "lo": 90, "hi": 64}]}),
                "articulation \"sustain\": lo 90 is above hi 64",
            ),
            (
                json!({"path": "a.sfz", "articulations": [{"id": "sustain", "via": "cc",
                    "cc": 1, "value": 200}]}),
                "articulation \"sustain\": value 200 is not between 0 and 127",
            ),
            (
                json!({"path": "a.sfz", "articulations": [
                    {"id": "sustain", "via": "keyswitch", "key": 36, "default": true},
                    {"id": "staccato", "via": "keyswitch", "key": 37, "default": true}]}),
                "more than one articulation is the default",
            ),
            (
                json!({"path": "a.sfz", "articulations": [
                    {"id": "sustain", "via": "keyswitch", "key": 36},
                    {"id": "sustain", "via": "keyswitch", "key": 37}]}),
                "articulation \"sustain\" is listed twice",
            ),
            (
                json!({"path": "a.sfz", "articulations": [{"id": "sustain", "via": "patch",
                    "mute": "generic"}]}),
                "articulation \"sustain\": unknown mute \"generic\"",
            ),
            (
                json!({"path": "a.sfz", "articulations": [{"id": "sustain", "via": "patch",
                    "vibrato": "con_sordino"}]}),
                "articulation \"sustain\": unknown vibrato \"con_sordino\"",
            ),
            (
                json!({"path": "a.sfz", "articulations": [{"id": "sustain", "via": "patch",
                    "attack_ms": 2001}]}),
                "articulation \"sustain\": attack_ms 2001 is not between 0 and 2000",
            ),
            (
                json!({"path": "a.sfz", "articulations": [{"id": "sustain", "via": "patch",
                    "path": "b.sfz"}]}),
                "articulation \"sustain\": only an articulation of a set names a path; the \
                 entry's own patch plays it",
            ),
            (
                json!({"set": "horn", "articulations": [{"id": "sustain", "via": "patch"}]}),
                "articulation \"sustain\": an articulation of a set names its patch with a path",
            ),
            (
                json!({"set": "horn", "articulations": [{"id": "sustain", "via": "patch",
                    "path": "../Horn.sfz"}]}),
                "articulation \"sustain\": \"../Horn.sfz\": the path must not hold empty, '.' \
                 or '..' parts",
            ),
            (
                json!({"path": "a.sfz", "ensemble": {"kind": "section", "players": 65}}),
                "players 65 is not between 1 and 64",
            ),
            (
                json!({"path": "a.sfz", "ensemble": {"kind": "solo", "players": 2}}),
                "a solo has one player",
            ),
            (
                json!({"path": "a.sfz", "key_offset": 49}),
                "key_offset 49 is not between -48 and 48",
            ),
            (
                json!({"path": "a.sfz", "dynamics": {"via": "fixed", "velocity": 0}}),
                "velocity 0 is not between 1 and 127",
            ),
            (
                json!({"path": "a.sfz", "dynamics": {"via": "cc"}}),
                "dynamics via cc needs a cc",
            ),
            (
                json!({"path": "a.sfz", "dynamics": {"via": "velocity", "cc": 1}}),
                "dynamics via velocity takes no cc",
            ),
            (
                json!({"path": "a.sfz", "legato": {"kind": "overlap", "crossfade_ms": 5,
                    "start_offset_ms": 0, "gap_ms": 0}}),
                "crossfade_ms 5 is not between 10 and 500",
            ),
            (
                json!({"path": "a.sfz", "legato": {"kind": "overlap", "crossfade_ms": 50,
                    "start_offset_ms": 0, "gap_ms": 101}}),
                "gap_ms 101 is not between 0 and 100",
            ),
            (
                json!({"path": "a.sfz", "legato": {"kind": "overlap", "crossfade_ms": 50}}),
                "legato overlap needs start_offset_ms",
            ),
            (
                json!({"path": "a.sfz", "legato": {"kind": "none", "gap_ms": 0}}),
                "legato none takes no gap_ms",
            ),
            (
                json!({"path": "a.sfz", "keymap": [{"keys": [], "instrument": "triangle"}]}),
                "a key map row needs at least one key",
            ),
            (
                json!({"path": "a.sfz", "keymap": [{"keys": [36], "instrument": "triangle"},
                    {"keys": [37, 36], "instrument": "snare_drum"}]}),
                "key 36 is mapped twice",
            ),
            (
                json!({"path": "a.sfz", "keymap": [{"keys": [36], "instrument": "kazoo"}]}),
                "unknown instrument \"kazoo\"",
            ),
            (
                json!({"path": "a.sfz", "keymap": [{"keys": [128], "instrument": "triangle"}]}),
                "key 128 is not between 0 and 127",
            ),
        ];
        for (entry, reason) in cases {
            assert_eq!(rejected(&entry), reason, "{entry}");
        }
        // A value of the wrong type is serde's message about it, and rejects only that entry.
        for entry in [
            json!({"path": "a.sfz", "articulations": ["sustain"]}),
            json!({"path": "a.sfz", "ensemble": {"kind": "duet"}}),
            json!({"path": "a.sfz", "verdict": {"state": "fine"}}),
            json!({"path": "a.sfz", "in_situ": "yes"}),
            json!({"path": "a.sfz", "key_offset": 1.5}),
            json!({"path": 5}),
        ] {
            assert!(!rejected(&entry).is_empty(), "{entry}");
        }
        assert!(
            rejected(&json!({"path": "a.sfz", "articulations": ["sustain"]}))
                .contains("expected an articulation object with an id and a via")
        );
    }

    #[test]
    fn a_rejected_entry_is_named_by_its_path_set_or_position() {
        let manifest = with_entries(&[
            json!({"path": "Good.sfz"}),
            json!({"path": "Bad.sfz", "instrument": "kazoo"}),
            json!({"set": "bad-set", "articulations": 3}),
            json!({"path": 7}),
            json!(null),
            json!({"path": "good.SFZ"}),
            json!({"set": "horn", "name": "Horn"}),
            json!({"set": "horn"}),
        ]);
        let labels: Vec<&str> = manifest
            .rejected
            .iter()
            .map(|(entry, _)| entry.as_str())
            .collect();
        assert_eq!(
            labels,
            ["Bad.sfz", "@bad-set", "#4", "#5", "good.SFZ", "@horn"]
        );
        let reasons: Vec<&str> = manifest
            .rejected
            .iter()
            .map(|(_, reason)| reason.as_str())
            .collect();
        assert_eq!(reasons.get(4), Some(&"listed twice"));
        assert_eq!(reasons.get(5), Some(&"listed twice"));
        let kept: Vec<&EntryTarget> = manifest.claims.entries.iter().map(|e| &e.target).collect();
        assert_eq!(
            kept,
            [
                &EntryTarget::Path("Good.sfz".to_owned()),
                &EntryTarget::Set("horn".to_owned())
            ]
        );
    }

    /// The manifests `docs/instrument-catalog.md` shows are read as written, with no entry
    /// rejected, so the documented format is the one this reader takes (TST-05).
    #[test]
    fn the_documented_examples_are_valid() {
        let doc = include_str!("../../../../../docs/instrument-catalog.md");
        let examples: Vec<&str> = doc
            .split("```json\n")
            .skip(1)
            .filter_map(|block| block.split("\n```").next())
            .collect();
        assert_eq!(examples.len(), 2, "a version 1 and a version 2 example");
        // The version of each example and the entries it lists.
        for (example, (version, entries)) in examples.into_iter().zip([(1, 1), (2, 3)]) {
            let manifest = parse(example.as_bytes(), MANIFEST)
                .unwrap_or_else(|err| panic!("{err}: {example}"));
            assert_eq!(manifest.version, version);
            assert_eq!(manifest.rejected, [], "{example}");
            assert_eq!(manifest.claims.entries.len(), entries);
        }
    }

    /// A library field that does not validate rejects the whole manifest.
    #[test]
    fn invalid_library_fields_reject_the_manifest() {
        let invalid =
            |json: Value| parse(json.to_string().as_bytes(), MANIFEST).expect_err("invalid");
        assert!(matches!(
            invalid(json!({"version": 2, "detect": ["../a.sfz"]})),
            ManifestError::Library {
                field: "detect",
                ..
            }
        ));
        assert!(matches!(
            invalid(json!({"version": 2, "source": {"url": " "}})),
            ManifestError::Library {
                field: "source",
                ..
            }
        ));
        assert!(matches!(
            invalid(json!({"version": 2, "profile": {"checked": "30.10.2026",
                "method": "render"}})),
            ManifestError::Library {
                field: "profile",
                ..
            }
        ));
        assert!(matches!(
            invalid(json!({"version": 2, "profile": {"checked": "2026-10-30",
                "method": "guess"}})),
            ManifestError::Parse(_)
        ));
        assert!(matches!(
            invalid(json!({"version": 2, "instruments": {"path": "a.sfz"}})),
            ManifestError::Parse(_)
        ));
        let manifest = read(
            &json!({"version": 2, "source": {"url": "u", "revision": " "},
            "profile": {"checked": "2026-01-02", "method": "render"}}),
        );
        assert_eq!(
            manifest.claims.source,
            set(LibrarySource {
                url: "u".to_owned(),
                revision: None
            })
        );
        assert_eq!(
            manifest.claims.checked.value().map(|check| check.method),
            Some(CheckMethod::Render)
        );
    }

    /// Any JSON value: the shapes an entry can take, with the field names and ids an entry uses
    /// so that the generated values reach the checks behind them.
    fn any_json() -> impl Strategy<Value = Value> {
        let word = prop::sample::select(vec![
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
            "id",
            "via",
            "key",
            "lo",
            "hi",
            "cc",
            "value",
            "default",
            "mute",
            "vibrato",
            "attack_ms",
            "keys",
            "kind",
            "players",
            "state",
            "lost",
            "crossfade_ms",
            "start_offset_ms",
            "gap_ms",
            "velocity",
            "articulation",
            "a.sfz",
            "../a.sfz",
            "@a.sfz",
            "horn",
            "Horn",
            "violin",
            "strings",
            "sustain",
            "ordinario",
            "pizz",
            "keyswitch",
            "patch",
            "solo",
            "section",
            "overlap",
            "none",
            "fixed",
            "con_sordino",
            "non_vibrato",
            "as_is",
            "",
        ])
        .prop_map(str::to_owned);
        let leaf = prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::Bool),
            (-200_i64..300).prop_map(Value::from),
            any::<f64>().prop_map(Value::from),
            word.clone().prop_map(Value::String),
            ".{0,8}".prop_map(Value::String),
        ];
        leaf.prop_recursive(4, 48, 6, move |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
                prop::collection::vec((word.clone(), inner), 0..8)
                    .prop_map(|fields| Value::Object(fields.into_iter().collect())),
            ]
        })
    }

    proptest! {
        /// The reader never panics on an entry of any shape, and decides each entry on its own:
        /// every entry is kept or rejected, and a bad entry added among them changes nothing
        /// else (TST-09).
        #[test]
        fn every_entry_is_kept_or_rejected_on_its_own(
            entries in prop::collection::vec(any_json(), 0..8),
            position in any::<prop::sample::Index>(),
        ) {
            let manifest = with_entries(&entries);
            prop_assert_eq!(
                manifest.claims.entries.len() + manifest.rejected.len(),
                entries.len()
            );
            let mut with_junk = entries.clone();
            with_junk.insert(position.index(entries.len() + 1), json!(["junk"]));
            let junk = with_entries(&with_junk);
            prop_assert_eq!(&junk.claims, &manifest.claims);
            prop_assert_eq!(junk.rejected.len(), manifest.rejected.len() + 1);
        }

        /// A whole manifest of any shape, of a version this build reads, is read with every
        /// entry kept or rejected, or rejected as a whole for what it is, never for its version
        /// and never with a panic.
        #[test]
        fn any_manifest_is_read_or_rejected(fields in any_json(), version in 1_u32..3) {
            let mut json = match fields {
                Value::Object(map) => Value::Object(map),
                other => json!({"instruments": [other]}),
            };
            json["version"] = version.into();
            match parse(json.to_string().as_bytes(), MANIFEST) {
                Ok(manifest) => {
                    prop_assert_eq!(manifest.version, version);
                    let listed = json.get("instruments").and_then(Value::as_array).map_or(0, Vec::len);
                    prop_assert_eq!(manifest.claims.entries.len() + manifest.rejected.len(), listed);
                }
                Err(err) => prop_assert!(!matches!(err, ManifestError::Version { .. }), "{err}"),
            }
        }
    }
}
