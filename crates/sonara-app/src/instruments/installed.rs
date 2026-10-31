//! The instruments the sampler can play on this machine, by preset ID: the choices of the
//! sampler's `instrument` parameter, the same choices grouped by library with the names the
//! device editor shows, and the resolver that turns a stored value into a patch, or the
//! patches of a multi-patch set, and the compiled mapping of its preset.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use sonara_core::model::JsonMap;
use sonara_core::plugins::SAMPLER_ID;
use sonara_dsp::bank::SamplerMapping;
use sonara_media::InstrumentSource;

use super::compile::sampler_mapping;
use super::discovery::{InstrumentCatalog, relative_to};
use super::model::{InstrumentPreset, Library, LibraryId, PresetKind};
use crate::settings::Settings;

/// The playable presets of the instrument catalog, by ID, and the patch paths that projects made
/// before preset IDs stored for them.
///
/// The sampler's `instrument` parameter offers the IDs ([`InstalledInstruments::ids`]) and a
/// project stores the chosen ID, which names the same instrument wherever the library is
/// installed. An older project stores the patch's path, the library directory joined with the
/// patch; such a path maps to the preset of the same patch when it lies below a configured
/// library ([`InstalledInstruments::preset_id`]). The patch part is compared ignoring case, as
/// discovery matches manifest entries; the library directory must be spelled as configured.
///
/// A multi-patch set (`<library>/@<set>`) is offered beside its patches: one sampler device
/// loads all of its patches and switches among them by the articulation.
///
/// One value is built when the application starts, and the registry, the media provider and
/// the edit path all read it, so the choices and the resolver always agree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstalledInstruments {
    /// The patch files and the compiled sampler mapping of every playable preset, by ID.
    patches: BTreeMap<String, Playable>,
    /// Per library directory, the preset IDs by lower-cased patch path below it.
    libraries: Vec<LibraryPaths>,
    /// The same presets grouped by library, with their names.
    choices: Vec<LibraryChoices>,
}

/// One library's playable presets among the sampler's `instrument` choices. The device editor
/// shows them as a group titled with the library's name, so the user picks an instrument by its
/// name and never sees a file path or a preset ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LibraryChoices {
    /// The library's ID, the first part of its presets' IDs.
    pub id: LibraryId,
    /// The library's name to show ([`Library::name`]).
    pub name: String,
    /// The library's playable presets, sorted by label ignoring case, then by ID.
    pub presets: Vec<PresetChoice>,
}

/// One playable preset as a choice of the sampler's `instrument` parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PresetChoice {
    /// The preset ID: the value the parameter stores when the preset is chosen.
    pub id: String,
    /// The text to show: the preset's name ([`InstrumentPreset::display_name`]). When another
    /// preset of the same library has the same name, ignoring case, the name is followed by the
    /// patch path below the library in parentheses, `Violin (Solo/Violin)`, so that the two do
    /// not read alike.
    pub label: String,
}

/// What a playable preset plays: its patch file, or the patch files of a multi-patch set in the
/// order its mapping numbers them, and its sampler mapping compiled from what is claimed about
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Playable {
    patches: Vec<PathBuf>,
    mapping: Arc<SamplerMapping>,
}

/// The patch paths of one library's playable presets.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LibraryPaths {
    root: PathBuf,
    by_path: HashMap<String, String>,
}

impl InstalledInstruments {
    /// The playable presets of `catalog`: the patches and the multi-patch sets
    /// ([`PresetKind::Set`]) whose [`super::Availability`] is available, a set when every one of
    /// its patches is. A preset that is missing or invalid is not offered, and an old path to it
    /// is not mapped. A set has no path of its own, so only its ID names it; each of its patches
    /// is offered as its own preset too.
    #[must_use]
    pub fn from_catalog(catalog: &InstrumentCatalog) -> Self {
        let mut libraries: Vec<LibraryPaths> = catalog
            .libraries()
            .iter()
            .map(|library| LibraryPaths {
                root: library.root.clone(),
                by_path: HashMap::new(),
            })
            .collect();
        let mut patches = BTreeMap::new();
        let mut playable = BTreeMap::new();
        for preset in catalog.available() {
            let id = preset.id.to_string();
            let files = match &preset.kind {
                PresetKind::Patch => {
                    let paths = catalog
                        .libraries()
                        .iter()
                        .position(|library| library.id == preset.library)
                        .and_then(|index| libraries.get_mut(index));
                    if let Some(paths) = paths
                        && let Some(relative) = relative_to(&paths.root, &preset.patch_path)
                    {
                        paths.by_path.insert(relative.to_lowercase(), id.clone());
                    }
                    vec![preset.patch_path.clone()]
                }
                // In the order the compiled targets number the set's patches.
                PresetKind::Set { members } => members
                    .iter()
                    .map(|member| member.patch_path.clone())
                    .collect(),
            };
            playable.insert(id.clone(), preset);
            patches.insert(
                id,
                Playable {
                    patches: files,
                    mapping: Arc::new(sampler_mapping(preset)),
                },
            );
        }
        let choices = library_choices(catalog.libraries(), playable.into_values());
        Self {
            patches,
            libraries,
            choices,
        }
    }

    /// The playable presets of the libraries in the settings key `sample_libraries`
    /// ([`InstrumentCatalog::from_settings`]). Reads the library directories and manifests.
    #[must_use]
    pub fn from_settings(settings: &Settings) -> Self {
        Self::from_catalog(&InstrumentCatalog::from_settings(settings))
    }

    /// The IDs of the playable presets, sorted: the choices of the sampler's `instrument`.
    #[must_use]
    pub fn ids(&self) -> Vec<String> {
        self.patches.keys().cloned().collect()
    }

    /// The playable presets grouped by library, as the device editor offers the sampler's
    /// `instrument` choices: the libraries sorted by name ignoring case, then by ID, each with
    /// its presets ([`LibraryChoices`]). Together they hold every ID of [`Self::ids`] once.
    #[must_use]
    pub fn by_library(&self) -> &[LibraryChoices] {
        &self.choices
    }

    /// Whether no preset can be played.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }

    /// The preset ID a stored `instrument` value names: the value itself when it is a playable
    /// preset's ID, else the preset whose patch the value is the path of (see the type's
    /// documentation); `None` when the value names nothing playable here.
    #[must_use]
    pub fn preset_id(&self, value: &str) -> Option<&str> {
        if let Some((id, _)) = self.patches.get_key_value(value) {
            return Some(id);
        }
        let path = Path::new(value);
        self.libraries.iter().find_map(|library| {
            let relative = relative_to(&library.root, path)?;
            library
                .by_path
                .get(&relative.to_lowercase())
                .map(String::as_str)
        })
    }

    /// What a stored `instrument` value plays: its preset ([`Self::preset_id`]), that preset's
    /// patch file, or the patch files of a multi-patch set, and the sampler mapping compiled
    /// from what the library's manifest or Sonara's profile claims about the preset (the
    /// articulation targets, each with the patch of a set that plays it, dynamics, legato and
    /// key offset; nothing for a preset about which nothing is claimed). This is the media
    /// provider's instrument resolver.
    #[must_use]
    pub fn resolve(&self, value: &str) -> Option<InstrumentSource> {
        let (id, playable) = self.patches.get_key_value(self.preset_id(value)?)?;
        Some(InstrumentSource {
            id: id.clone(),
            patches: playable.patches.clone(),
            mapping: Arc::clone(&playable.mapping),
        })
    }

    /// Rewrites the `instrument` value of a sampler's parameter `values` from a patch path to its
    /// preset ID, so that an agent or script that passes a path, as before preset IDs, stores the
    /// ID. Values of other plugins, other parameters, preset IDs and values that name nothing
    /// playable are left as they are: the plugin's descriptor then accepts or rejects them.
    pub fn normalize_params(&self, plugin_id: &str, values: &mut JsonMap) {
        if plugin_id != SAMPLER_ID {
            return;
        }
        let Some(Value::String(value)) = values.get("instrument") else {
            return;
        };
        if self.patches.contains_key(value) {
            return;
        }
        if let Some(id) = self.preset_id(value) {
            let id = id.to_owned();
            values.insert("instrument".to_owned(), Value::String(id));
        }
    }
}

/// The playable `presets` grouped by library, named after `libraries`, in the order
/// [`InstalledInstruments::by_library`] documents. A library that is not in `libraries` (none,
/// for a catalog's own presets) is named by its ID.
fn library_choices<'a>(
    libraries: &[Library],
    presets: impl IntoIterator<Item = &'a InstrumentPreset>,
) -> Vec<LibraryChoices> {
    let mut groups: BTreeMap<&LibraryId, Vec<&InstrumentPreset>> = BTreeMap::new();
    for preset in presets {
        groups.entry(&preset.library).or_default().push(preset);
    }
    let mut choices: Vec<LibraryChoices> = groups
        .into_iter()
        .map(|(id, presets)| LibraryChoices {
            id: id.clone(),
            name: libraries
                .iter()
                .find(|library| library.id == *id)
                .map_or_else(|| id.to_string(), |library| library.name.clone()),
            presets: preset_choices(id, &presets),
        })
        .collect();
    choices.sort_by_cached_key(|group| (group.name.to_lowercase(), group.id.clone()));
    choices
}

/// The choices of one library's `presets`, labelled and sorted as [`PresetChoice`] and
/// [`LibraryChoices::presets`] document.
fn preset_choices(library: &LibraryId, presets: &[&InstrumentPreset]) -> Vec<PresetChoice> {
    let mut named: HashMap<String, usize> = HashMap::new();
    for preset in presets {
        *named.entry(preset.display_name.to_lowercase()).or_default() += 1;
    }
    let prefix = format!("{library}/");
    let mut choices: Vec<PresetChoice> = presets
        .iter()
        .map(|preset| {
            let id = preset.id.to_string();
            let shared = named
                .get(&preset.display_name.to_lowercase())
                .is_some_and(|count| *count > 1);
            let label = if shared {
                let path = id.strip_prefix(&prefix).unwrap_or(&id);
                format!("{} ({path})", preset.display_name)
            } else {
                preset.display_name.clone()
            };
            PresetChoice { id, label }
        })
        .collect();
    choices.sort_by_cached_key(|choice| (choice.label.to_lowercase(), choice.id.clone()));
    choices
}
