//! The plugin pickers of an interface: the mixer's "+" and "Add Effect"
//! menus and, later, the device panel's, as the prototype's `plugin_menu`
//! (`sonara/ui/devices/menus.py`) builds them from the registry.
//!
//! The project view carries only the descriptors of the plugins in use; a picker offers every
//! registered plugin of one kind. [`plugin_menu`] groups them by category: the groups are sorted
//! by their titles (the prototype's `CATEGORY_TITLES`, else the category in title case), and
//! each group's plugins by name, then ID, then description. A menu of one group shows its
//! plugins directly instead of a submenu, as the prototype's did.
//!
//! The piano roll's keyboard names the notes of a drum track by the General MIDI
//! percussion map: [`drum_map`] gives the drum kit's plugin and each mapped note's name, as the
//! prototype's `drum_label` (`sonara/ui/piano_roll/widgets.py`) writes it from `GM_MAP`.

use serde::{Deserialize, Serialize};
use sonara_core::plugins::{PluginKind, PluginRegistry};
use sonara_midi::gm::GM_MAP;
use sonara_midi::smf::DRUM_PLUGIN;

/// The kind of plugins a picker offers, as an interface names it (`"effect"`, `"instrument"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PickerKind {
    /// The audio effects (the mixer's inserts).
    Effect,
    /// The instruments (the device panel's instrument choice).
    Instrument,
}

impl From<PickerKind> for PluginKind {
    fn from(kind: PickerKind) -> Self {
        match kind {
            PickerKind::Effect => Self::Effect,
            PickerKind::Instrument => Self::Instrument,
        }
    }
}

/// The prototype's titles of the categories (`CATEGORY_TITLES`).
const CATEGORY_TITLES: [(&str, &str); 9] = [
    ("dynamics", "Dynamics"),
    ("eq", "EQ"),
    ("filter", "Filter"),
    ("delay", "Delay"),
    ("reverb", "Reverb"),
    ("distortion", "Distortion"),
    ("utility", "Utility"),
    ("synth", "Synthesizers"),
    ("drums", "Drums"),
];

/// One entry of a picker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginItem {
    /// The plugin ID that the edit names.
    pub id: String,
    /// The plugin's name, the entry's text.
    pub name: String,
    /// The plugin's description, the entry's tooltip.
    pub description: String,
}

/// One category of a picker: its submenu's title and its plugins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginGroup {
    /// The category (`eq`, `dynamics`, …).
    pub category: String,
    /// The submenu's title (`EQ`, `Dynamics`, …).
    pub title: String,
    /// The category's plugins, sorted.
    pub plugins: Vec<PluginItem>,
}

/// The title of `category`: the prototype's, else Python's `str.title()` of it.
#[must_use]
pub fn category_title(category: &str) -> String {
    if let Some((_, title)) = CATEGORY_TITLES.iter().find(|(name, _)| *name == category) {
        return (*title).to_owned();
    }
    title_case(category)
}

/// Python's `str.title()`: a letter after a non-letter is upper case, every other letter lower
/// case.
pub(crate) fn title_case(text: &str) -> String {
    let mut title = String::with_capacity(text.len());
    let mut after_letter = false;
    for c in text.chars() {
        if c.is_alphabetic() {
            if after_letter {
                title.extend(c.to_lowercase());
            } else {
                title.extend(c.to_uppercase());
            }
            after_letter = true;
        } else {
            title.push(c);
            after_letter = false;
        }
    }
    title
}

/// The picker of every registered plugin of `kind`, grouped by category (see the module docs).
#[must_use]
pub fn plugin_menu(plugins: &PluginRegistry, kind: PluginKind) -> Vec<PluginGroup> {
    let mut groups: Vec<PluginGroup> = Vec::new();
    for descriptor in plugins.all(Some(kind)) {
        let item = PluginItem {
            id: descriptor.id.clone(),
            name: descriptor.name.clone(),
            description: descriptor.description.clone(),
        };
        match groups
            .iter_mut()
            .find(|group| group.category == descriptor.category)
        {
            Some(group) => group.plugins.push(item),
            None => groups.push(PluginGroup {
                category: descriptor.category.clone(),
                title: category_title(&descriptor.category),
                plugins: vec![item],
            }),
        }
    }
    // Python sorts the categories by their titles, and each category's `(name, id,
    // description)` tuples.
    groups.sort_by(|a, b| a.title.cmp(&b.title));
    for group in &mut groups {
        group.plugins.sort_by(|a, b| {
            (&a.name, &a.id, &a.description).cmp(&(&b.name, &b.id, &b.description))
        });
    }
    groups
}

/// The GM name of a drum note ("Kick", "Hat Closed"), `None` for a note the map lacks: the
/// prototype's `drum_label`, `GM_MAP`'s sound with spaces for underscores, in title case.
#[must_use]
pub fn drum_label(pitch: u8) -> Option<String> {
    GM_MAP
        .iter()
        .find(|(note, _)| *note == pitch)
        .map(|(_, sound)| title_case(&sound.replace('_', " ")))
}

/// A note of the drum map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DrumName {
    /// The MIDI note.
    pub pitch: u8,
    /// Its GM name.
    pub label: String,
}

/// The names of a drum track's notes for the piano roll's keyboard and notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DrumMap {
    /// The instrument plugin that makes an instrument track a drum track (`sonara.drums`).
    pub plugin_id: &'static str,
    /// Every note of the GM map with its name, in note order.
    pub names: Vec<DrumName>,
}

/// The drum map of the piano roll (see the module docs).
#[must_use]
pub fn drum_map() -> DrumMap {
    DrumMap {
        plugin_id: DRUM_PLUGIN,
        names: GM_MAP
            .iter()
            .filter_map(|(pitch, _)| {
                drum_label(*pitch).map(|label| DrumName {
                    pitch: *pitch,
                    label,
                })
            })
            .collect(),
    }
}
