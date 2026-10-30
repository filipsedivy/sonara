//! The instruments the sampler can play on this machine, by preset ID: the choices of the
//! sampler's `instrument` parameter and the resolver that turns a stored value into a patch.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde_json::Value;
use sonara_core::model::JsonMap;
use sonara_core::plugins::SAMPLER_ID;
use sonara_media::InstrumentSource;

use super::discovery::{InstrumentCatalog, relative_to};
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
/// One value is built when the application starts, and the registry, the media provider and
/// the edit path all read it, so the choices and the resolver always agree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstalledInstruments {
    /// The patch file of every playable preset, by ID.
    patches: BTreeMap<String, PathBuf>,
    /// Per library directory, the preset IDs by lower-cased patch path below it.
    libraries: Vec<LibraryPaths>,
}

/// The patch paths of one library's playable presets.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LibraryPaths {
    root: PathBuf,
    by_path: HashMap<String, String>,
}

impl InstalledInstruments {
    /// The playable presets of `catalog`: those whose [`super::Availability`] is available. A
    /// preset that is missing or invalid is not offered, and an old path to it is not mapped.
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
        for preset in catalog.available() {
            let id = preset.id.to_string();
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
            patches.insert(id, preset.patch_path.clone());
        }
        Self { patches, libraries }
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

    /// What a stored `instrument` value plays: its preset ([`Self::preset_id`]) and that
    /// preset's patch file. This is the media provider's instrument resolver.
    #[must_use]
    pub fn resolve(&self, value: &str) -> Option<InstrumentSource> {
        let (id, patch) = self.patches.get_key_value(self.preset_id(value)?)?;
        Some(InstrumentSource {
            id: id.clone(),
            patch: patch.clone(),
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
