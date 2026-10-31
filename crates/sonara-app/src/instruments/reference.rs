//! The orchestration reference of the AI Chat's system prompt, as plain text: a guide to writing
//! orchestral parts with the tools (`reference_guide.md`), every instrument id with its ranges
//! and the articulation ids a note may carry ([`sonara_midi::orchestration::reference`]), and the
//! presets of the library profiles Sonara ships, by the instrument id they play, with the
//! articulations each plays and the keys that select them.
//!
//! The presets are rendered from the profiles themselves ([`super::profiles::shipped`]), so they
//! say what the catalog will claim for a library a profile detects. They do not say what is
//! installed: an agent reads that from the sampler's `instrument` choices.
//!
//! The composer cannot reach the metadata or the catalog (the chat's privacy guard,
//! `tests/chat_save.rs`), so it includes the text as a file: the example `gen_chat_reference`
//! writes [`agent_reference`] to `src/composer/system_prompt_orchestra.md`, and its test fails
//! while the committed file differs (DOC-01).

use super::manifest::{
    ArticulationEntry, ArticulationList, ArticulationName, ArticulationVia, EntryTarget,
    KeymapEntry, ManifestEntry, VerdictState,
};
use super::model::InstrumentId;
use super::profiles::{Profile, shipped};
use sonara_midi::orchestration::{InstrumentType, Mute, Vibrato, reference};

/// How orchestral parts are written with Sonara's tools: the part, the sampler preset, the
/// articulations, the dynamics and legato. Hand-written; it comes first in the reference.
const GUIDE: &str = include_str!("reference_guide.md");

/// What a line names instead of an instrument id for a patch whose keys play several instruments
/// (a percussion kit).
const KIT: &str = "kit";

/// The word that starts a line naming the folder of the presets on the lines below it.
const FOLDER: &str = "folder";

/// A preset on a line: its path below the library without the extension, and what it plays
/// ([`played`]).
struct Preset {
    stem: String,
    played: String,
}

/// The orchestration reference for an agent's system prompt: the guide, the instruments of the
/// orchestration metadata and the presets of the shipped library profiles, each separated by a
/// blank line, ending with a newline. Deterministic: the same build renders the same text.
#[must_use]
pub fn agent_reference() -> String {
    [GUIDE, &reference::render(), &shipped_presets()].join("\n")
}

/// The block of the shipped profiles: a level-2 heading, how to read a line, then per profile
/// its name and its lines ([`profile_lines`]). It ends with a newline.
fn shipped_presets() -> String {
    let mut text = render(shipped()).join("\n");
    text.push('\n');
    text
}

/// The lines of the block for `profiles`, in the order given.
fn render(profiles: &[Profile]) -> Vec<String> {
    let mut lines = vec![
        "## Presets of the library profiles Sonara ships".to_owned(),
        String::new(),
        "Presets of the libraries below by the instrument id they play; only those that \
         `list_plugins` offers as `instrument` choices are installed. Preset id: library id, \
         `/`, the `folder` line above (if any), name. After the name: the articulations it \
         plays (none: the ordinary sound), `+` the mute or vibrato recorded. `keys 36 sustain, \
         37 staccato`: a note on key 36 selects sustain for the following notes and does not \
         sound. On percussion lines a key alone plays the ordinary stroke."
            .to_owned(),
    ];
    for profile in profiles {
        lines.push(String::new());
        let name = profile.claims.name.value().map_or("", String::as_str);
        lines.push(format!("{} ({name}):", profile.id));
        lines.extend(profile_lines(profile));
    }
    lines
}

/// One line per instrument id the profile's playable patches play, under a `folder` line when
/// its presets share a folder. The folders come in the score order of their first instrument,
/// the lines in score order within a folder, and the kits last. A patch the profile judges
/// unplayable, a multi-patch set (the sampler plays one patch) and a patch that claims no
/// instrument and maps no key are left out.
fn profile_lines(profile: &Profile) -> Vec<String> {
    let mut rows: Vec<(Option<InstrumentType>, Vec<Preset>)> = Vec::new();
    for entry in &profile.claims.entries {
        let EntryTarget::Path(path) = &entry.target else {
            continue;
        };
        if entry
            .verdict
            .value()
            .is_some_and(|verdict| verdict.state == VerdictState::Not)
        {
            continue;
        }
        let instrument = entry.instrument.value().copied();
        let keymap = entry.keymap.value().map_or(&[][..], Vec::as_slice);
        if instrument.is_none() && keymap.is_empty() {
            continue;
        }
        let id = InstrumentId::new(&profile.id, path);
        let stem = id
            .as_str()
            .strip_prefix(&format!("{}/", profile.id))
            .unwrap_or(id.as_str())
            .to_owned();
        let preset = Preset {
            stem,
            played: played(entry, instrument),
        };
        match rows.iter_mut().find(|(row, _)| *row == instrument) {
            Some((_, presets)) => presets.push(preset),
            None => rows.push((instrument, vec![preset])),
        }
    }
    // Score order, then the kits (`None` sorts first, so it is moved last).
    rows.sort_by_key(|(instrument, _)| (instrument.is_none(), *instrument));
    let mut folders: Vec<(String, Vec<String>)> = Vec::new();
    for (instrument, presets) in rows {
        let folder = common_folder(presets.iter().map(|preset| preset.stem.as_str()));
        let items: Vec<String> = presets
            .iter()
            .map(|preset| {
                let name = preset.stem.get(folder.len()..).unwrap_or(&preset.stem);
                format!("{name}{}", preset.played)
            })
            .collect();
        let label = instrument.map_or(KIT, InstrumentType::id);
        let line = format!("{label}: {}", items.join("; "));
        match folders.iter_mut().find(|(known, _)| *known == folder) {
            Some((_, lines)) => lines.push(line),
            None => folders.push((folder, vec![line])),
        }
    }
    folders
        .into_iter()
        .flat_map(|(folder, lines)| {
            let header = (!folder.is_empty()).then(|| format!("{FOLDER} {folder}"));
            header.into_iter().chain(lines)
        })
        .collect()
}

/// The longest folder, ending in `/`, that holds every one of `stems`; empty when there is none.
fn common_folder<'a>(mut stems: impl Iterator<Item = &'a str>) -> String {
    let folder = |stem: &'a str| stem.rfind('/').map_or("", |slash| &stem[..=slash]);
    let Some(first) = stems.next() else {
        return String::new();
    };
    let mut common = folder(first);
    for stem in stems {
        while !stem.starts_with(common) {
            common = common
                .get(..common.len().saturating_sub(1))
                .and_then(|trimmed| trimmed.rfind('/').map(|slash| &trimmed[..=slash]))
                .unwrap_or("");
        }
    }
    common.to_owned()
}

/// What follows a preset's name: the articulations it plays and how, then its key map.
fn played(entry: &ManifestEntry, instrument: Option<InstrumentType>) -> String {
    let articulations = match entry.articulations.value() {
        Some(ArticulationList::Mapped(articulations)) => articulations_text(articulations),
        _ => String::new(),
    };
    let keys = match entry.keymap.value() {
        Some(keymap) if !keymap.is_empty() => {
            let rows: Vec<String> = keymap.iter().map(|row| key_row(row, instrument)).collect();
            format!(" keys {}", rows.join(", "))
        }
        _ => String::new(),
    };
    articulations + &keys
}

/// ` keys 36 sustain, 37 staccato` when two or more articulations are reached by key switches,
/// else the ids of the articulations other than the ordinary sound, ` sustain+con_sordino`.
/// Empty for a patch whose only articulation is its ordinary sound.
fn articulations_text(articulations: &[ArticulationEntry]) -> String {
    let switched: Vec<String> = articulations
        .iter()
        .filter_map(|entry| match entry.via {
            ArticulationVia::Keyswitch { key } => Some(format!("{key} {}", articulation(entry))),
            _ => None,
        })
        .collect();
    if switched.len() > 1 {
        return format!(" keys {}", switched.join(", "));
    }
    let named: Vec<String> = articulations
        .iter()
        .filter(|entry| {
            entry.name != ArticulationName::Ordinario
                || entry.mute != Mute::Open
                || entry.vibrato != Vibrato::Ordinary
        })
        .map(articulation)
        .collect();
    if named.is_empty() {
        String::new()
    } else {
        format!(" {}", named.join(" "))
    }
}

/// `sustain`, or with the recorded mute and vibrato, `sustain+con_sordino`.
fn articulation(entry: &ArticulationEntry) -> String {
    let mut text = entry.name.id().to_owned();
    for state in [entry.mute.articulation(), entry.vibrato.articulation()]
        .into_iter()
        .flatten()
    {
        text.push('+');
        text.push_str(state.id());
    }
    text
}

/// One row of a key map: `60/61`, then the instrument when it is not the line's, then the
/// articulation when it is not the ordinary stroke.
fn key_row(row: &KeymapEntry, line: Option<InstrumentType>) -> String {
    let keys: Vec<String> = row.keys.iter().map(u8::to_string).collect();
    let mut text = keys.join("/");
    if line != Some(row.instrument) {
        text.push(' ');
        text.push_str(row.instrument.id());
    }
    if let Some(articulation) = row.articulation {
        text.push(' ');
        text.push_str(articulation.id());
    }
    text
}

#[cfg(test)]
mod tests {
    use super::super::profiles::parse;
    use super::*;

    /// The line of `label` in the block of the profile `id`.
    fn line<'a>(text: &'a str, id: &str, label: &str) -> &'a str {
        let header = format!("{id} (");
        let (_, after) = text
            .split_once(&header)
            .unwrap_or_else(|| panic!("no profile {id}"));
        after
            .lines()
            .skip(1)
            .take_while(|line| !line.is_empty())
            .find(|line| line.starts_with(&format!("{label}: ")))
            .unwrap_or_else(|| panic!("{id} has no line {label}"))
    }

    #[test]
    fn a_shipped_line_lists_each_patch_with_its_switch_keys() {
        let text = shipped_presets();
        // Oracle: `profiles/vsco2ce.json` (the violin patches, solo and section, in file order;
        // the key switch patches select sustain, tremolo, spiccato and pizzicato on 36 to 39).
        assert_eq!(
            line(&text, "vsco2ce", "violin"),
            "violin: SViolin-KS keys 36 sustain, 37 tremolo, 38 spiccato, 39 pizzicato; \
             SViolinVib sustain; SViolinVib-Quiet sustain; SViolinTrem tremolo; \
             SViolinSpic spiccato; SViolinPizz pizzicato; ViolinEns-KS keys 36 sustain, \
             37 tremolo, 38 spiccato, 39 pizzicato; ViolinEnsSusVib sustain; \
             ViolinEnsSusVib-Quiet sustain; ViolinEnsTrem tremolo; ViolinEnsSpic spiccato; \
             ViolinEnsPizz pizzicato"
        );
        // A mute or vibrato the recording carries is added to its articulation.
        assert_eq!(
            line(&text, "vsco2ce", "horn_f"),
            "horn_f: FHornSus sustain; FHornStac staccato; FHornMute sustain+con_sordino"
        );
        // A key map leaves out the line's own instrument, and the folder the presets share is
        // written once above the lines of that folder.
        assert_eq!(
            line(&text, "vcsl", "triangle"),
            "triangle: Triangles keys 60/65/69, 63/68/72 roll"
        );
        let (_, vcsl) = text.split_once("vcsl (").expect("the vcsl block");
        let folder = vcsl
            .lines()
            .take_while(|line| !line.starts_with("triangle: "))
            .filter(|line| line.starts_with("folder "))
            .last();
        assert_eq!(folder, Some("folder Idiophones/Struck Idiophones/"));
        // A kit names the instrument of each key.
        assert!(
            line(&text, "vsco2ce", KIT).starts_with(
                "kit: GM-StylePerc keys 36 bass_drum, 38 snare_drum, 39 snare_drum roll"
            )
        );
    }

    #[test]
    fn every_listed_preset_is_a_playable_patch_of_the_profile() {
        let text = shipped_presets();
        for profile in shipped() {
            let header = format!("{} (", profile.id);
            let (_, after) = text.split_once(&header).expect("every profile has a block");
            let block: Vec<&str> = after
                .lines()
                .skip(1)
                .take_while(|line| !line.is_empty())
                .collect();
            for entry in &profile.claims.entries {
                let EntryTarget::Path(path) = &entry.target else {
                    continue;
                };
                let stem = path
                    .rsplit_once('.')
                    .map_or(path.as_str(), |(stem, _)| stem);
                let name = stem.rsplit_once('/').map_or(stem, |(_, name)| name);
                let listed = entry.instrument.value().is_some()
                    || entry.keymap.value().is_some_and(|keys| !keys.is_empty());
                let label = entry.instrument.value().map_or(KIT, |t| t.id());
                if listed {
                    let line = block
                        .iter()
                        .find(|line| line.starts_with(&format!("{label}: ")))
                        .unwrap_or_else(|| panic!("{}: no line {label}", profile.id));
                    assert!(
                        line.contains(name),
                        "{}: {path} is not on the line {label}",
                        profile.id
                    );
                }
            }
            // Every line's label is an instrument id or a kit, each once.
            let mut labels: Vec<&str> = block
                .iter()
                .filter(|line| !line.starts_with("folder "))
                .map(|line| line.split_once(": ").expect("a label").0)
                .collect();
            assert!(
                labels
                    .iter()
                    .all(|label| *label == KIT || InstrumentType::from_id(label).is_some())
            );
            let count = labels.len();
            labels.sort_unstable();
            labels.dedup();
            assert_eq!(labels.len(), count, "{}: a label twice", profile.id);
        }
    }

    #[test]
    fn unplayable_sets_and_unknown_patches_are_left_out() {
        let profile = parse(
            r#"{"version": 2, "id": "lib", "name": "A Library", "detect": ["a.sfz"],
                "instruments": [
                  {"path": "Strings/Cello Sus.sfz", "instrument": "cello",
                   "articulations": [{"id": "sustain", "via": "patch", "default": true}]},
                  {"path": "Strings/Cello Pizz.sfz", "instrument": "cello",
                   "articulations": [{"id": "pizzicato", "via": "patch", "default": true}]},
                  {"path": "Strings/Cello Broken.sfz", "instrument": "cello",
                   "verdict": {"state": "not", "lost": ["no region plays"]}},
                  {"set": "cello", "name": "Cello", "instrument": "cello",
                   "articulations": [
                     {"id": "sustain", "via": "patch", "path": "Strings/Cello Sus.sfz",
                      "default": true}]},
                  {"path": "Other/Whistle.sfz", "family": "other"},
                  {"path": "Flute.sfz", "instrument": "flute",
                   "articulations": [{"id": "ordinario", "via": "patch", "default": true}]}
                ]}"#,
        )
        .expect("a valid profile");
        let lines = render(std::slice::from_ref(&profile));
        let block: Vec<&str> = lines
            .iter()
            .skip_while(|line| !line.starts_with("lib ("))
            .map(String::as_str)
            .collect();
        // Oracle: the flute comes before the cello in score order; the broken patch, the set
        // and the patch of no instrument are not offered; the ordinary sound names nothing; the
        // cello's presets share a folder, the flute's has none.
        assert_eq!(
            block,
            [
                "lib (A Library):",
                "flute: Flute",
                "folder Strings/",
                "cello: Cello Sus sustain; Cello Pizz pizzicato",
            ]
        );
    }

    #[test]
    fn the_reference_is_the_guide_the_instruments_and_the_presets() {
        let text = agent_reference();
        let rest = text.strip_prefix(GUIDE).expect("the guide comes first");
        let rest = rest
            .strip_prefix(&format!("\n{}", reference::render()))
            .expect("the instruments come second");
        assert_eq!(rest, format!("\n{}", shipped_presets()));
        assert!(text.ends_with('\n') && !text.ends_with("\n\n"));
        // Every instrument id has its line, and each shipped library its block.
        for instrument in InstrumentType::ALL {
            assert!(
                text.contains(&format!("\n{} | ", instrument.id())),
                "{}",
                instrument.id()
            );
        }
        for profile in shipped() {
            let name = profile.claims.name.value().expect("a profile has a name");
            assert!(text.contains(&format!("\n{} ({name}):\n", profile.id)));
        }
    }

    #[test]
    fn the_guide_names_tools_that_exist() {
        let api = include_str!("../../../../docs/agent-api.md");
        for tool in [
            "create_track",
            "set_instrument",
            "list_plugins",
            "write_midi_controllers",
        ] {
            assert!(GUIDE.contains(&format!("`{tool}")), "{tool}");
            assert!(
                api.contains(&format!("### `{tool}`")),
                "{tool} is not a tool"
            );
        }
        assert!(GUIDE.contains(sonara_core::plugins::SAMPLER_ID));
    }

    #[test]
    fn the_common_folder_is_the_longest_shared_one() {
        let folder = |stems: &[&str]| common_folder(stems.iter().copied());
        assert_eq!(folder(&["A/B/c", "A/B/d"]), "A/B/");
        assert_eq!(folder(&["A/B/c", "A/Bx/d"]), "A/");
        assert_eq!(folder(&["A/B/c", "C/d"]), "");
        assert_eq!(folder(&["c", "d"]), "");
        assert_eq!(folder(&[]), "");
    }
}
