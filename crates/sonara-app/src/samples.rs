//! The sample libraries installed on this machine (ADR 0010): the directories the settings key
//! `sample_libraries` lists, and the SFZ instruments found in them.
//!
//! Sonara ships no sample library. Libraries are installed by the user, so the sampler is
//! offered only when a configured directory holds at least one playable `.sfz` patch
//! ([`sampler_descriptor`]). Its `instrument` choices are the IDs of the instrument catalog's
//! playable presets ([`crate::instruments::InstalledInstruments`]), not file paths. The scan
//! reads directory entries only; patches are loaded when a track plays them, off the real-time
//! thread.
//!
//! **Include fragments are not patches.** Libraries such as Sonatina keep the pieces their
//! patches `#include` in folders named `includes` (190 `.sfz` files in Sonatina). A fragment
//! holds no region of its own, or names its samples relative to the patch that includes it, so
//! it cannot be played alone. The scan does not enter a folder named `includes` (in any case),
//! so the fragments are neither presets nor choices of the sampler's `instrument` parameter.
//!
//! **The sampler's other enum parameters** take their choices from the orchestration metadata:
//! `instrument_type` offers `auto` and every [`InstrumentType`] in score order, `articulation`
//! offers `auto`, `ordinario` and the articulation vocabulary in its frozen order
//! ([`Articulation::ALL`]), `mute` offers `auto` and every [`Mute`] in its order
//! ([`Mute::ALL`]), and `vibrato` offers `auto` and every [`Vibrato`] in its order
//! ([`Vibrato::ALL`]). A mute or vibrato is named by the articulation ID that names it
//! (`con_sordino`, `non_vibrato`), and the states without one by [`MUTE_OPEN`] and
//! [`VIBRATO_ORDINARY`], as library manifests spell them. Every sampler device shares these
//! lists, whatever preset it plays.

use std::fs;
use std::path::{Path, PathBuf};

use sonara_core::plugins::{
    PluginDescriptor, PluginRegistry, SamplerChoices, builtin_registry, sampler_descriptor,
};
use sonara_midi::orchestration::{Articulation, InstrumentType, Mute, Vibrato};

use crate::settings::Settings;

/// The settings key listing the sample library directories.
pub const LIBRARIES_KEY: &str = "sample_libraries";
/// The deepest directory level a scan enters below a library root.
const MAX_DEPTH: usize = 8;
/// The most patches a scan reports; a directory tree with more is cut short.
pub const MAX_PATCHES: usize = 5_000;
/// The name of the folders that hold include fragments, compared ignoring ASCII case; the scan
/// does not enter them.
const INCLUDES_DIR: &str = "includes";
/// The choice of the sampler's `mute` for [`Mute::Open`], which no articulation ID names.
pub const MUTE_OPEN: &str = "open";
/// The choice of the sampler's `vibrato` for [`Vibrato::Ordinary`], which no articulation ID
/// names.
pub const VIBRATO_ORDINARY: &str = "ordinary";

/// The library directories of `settings`: the strings of the `sample_libraries` list. Anything
/// else in the key is ignored.
#[must_use]
pub fn library_roots(settings: &Settings) -> Vec<PathBuf> {
    settings
        .extra
        .get(LIBRARIES_KEY)
        .and_then(serde_json::Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(serde_json::Value::as_str)
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

/// The `.sfz` files below `roots`, as sorted absolute UTF-8 paths. Hidden entries and symbolic
/// links are skipped, so a link cannot loop the scan; an unreadable directory is skipped. A
/// folder named `includes` below a root, in any case, is skipped with everything in it: it holds
/// fragments that other patches include, not patches (see the module documentation). A root is
/// scanned whatever its name.
#[must_use]
pub fn scan_patches(roots: &[PathBuf]) -> Vec<String> {
    let mut found = Vec::new();
    for root in roots {
        walk(root, 0, &mut found);
    }
    found.sort();
    found.dedup();
    found.truncate(MAX_PATCHES);
    found
}

fn walk(dir: &Path, depth: usize, found: &mut Vec<String>) {
    if depth > MAX_DEPTH || found.len() >= MAX_PATCHES {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let hidden = name.to_string_lossy().starts_with('.');
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if hidden || kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if !name.eq_ignore_ascii_case(INCLUDES_DIR) {
                walk(&path, depth + 1, found);
            }
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("sfz"))
            && let Some(text) = path.to_str()
        {
            found.push(text.to_owned());
        }
    }
}

/// The plugin registry of the application: the prototype's built-ins, plus `sonara.sampler`
/// when there are installed instruments, whose `instrument` choices are `presets`, the IDs of
/// [`crate::instruments::InstalledInstruments::ids`].
#[must_use]
pub fn registry_for(presets: &[String]) -> PluginRegistry {
    let mut registry = builtin_registry();
    if let Some(descriptor) = sampler_descriptor(&sampler_choices(presets)) {
        register(&mut registry, descriptor);
    }
    registry
}

/// The sampler's choices: `presets` for `instrument`, every instrument type in score order for
/// `instrument_type`, the articulation vocabulary in its frozen order for `articulation`, and
/// every mute and vibrato state in its order for `mute` and `vibrato`.
fn sampler_choices(presets: &[String]) -> SamplerChoices {
    SamplerChoices {
        presets: presets.to_vec(),
        instrument_types: InstrumentType::ALL
            .iter()
            .map(|instrument_type| instrument_type.id().to_owned())
            .collect(),
        articulations: Articulation::ALL
            .iter()
            .map(|articulation| articulation.id().to_owned())
            .collect(),
        mutes: Mute::ALL
            .iter()
            .map(|mute| {
                mute.articulation()
                    .map_or(MUTE_OPEN, Articulation::id)
                    .to_owned()
            })
            .collect(),
        vibratos: Vibrato::ALL
            .iter()
            .map(|vibrato| {
                vibrato
                    .articulation()
                    .map_or(VIBRATO_ORDINARY, Articulation::id)
                    .to_owned()
            })
            .collect(),
    }
}

fn register(registry: &mut PluginRegistry, descriptor: PluginDescriptor) {
    // The ID `sonara.sampler` is not in the built-in catalog, so this cannot collide; a clash
    // would only mean the sampler stays unavailable.
    if let Err(error) = registry.register(descriptor) {
        log::warn!("the sampler is unavailable: {error}");
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use std::collections::HashSet;

    use serde_json::json;
    use sonara_core::plugins::{ParamKind, PluginDescriptor, PluginKind};
    use sonara_midi::orchestration::{Articulation, InstrumentType, Mute, Vibrato};

    use super::{LIBRARIES_KEY, registry_for, scan_patches};
    use crate::instruments::InstalledInstruments;
    use crate::settings::Settings;

    /// The registry of the libraries `settings` configure.
    fn registry_of(settings: &Settings) -> sonara_core::plugins::PluginRegistry {
        registry_for(&InstalledInstruments::from_settings(settings).ids())
    }

    fn touch(path: &std::path::Path) {
        fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
        fs::write(path, "<region> sample=a.wav\n").expect("write");
    }

    #[test]
    fn the_scan_finds_sfz_files_and_skips_hidden_ones() {
        let dir = tempfile::tempdir().expect("a tempdir");
        touch(&dir.path().join("Strings/Violin.sfz"));
        touch(&dir.path().join("Brass/Horn.SFZ"));
        touch(&dir.path().join("Brass/readme.txt"));
        touch(&dir.path().join(".hidden/Secret.sfz"));
        let found = scan_patches(&[dir.path().to_owned(), dir.path().join("missing")]);
        let names: Vec<_> = found
            .iter()
            .map(|p| p.rsplit('/').next().unwrap_or(p).to_owned())
            .collect();
        assert_eq!(names, ["Horn.SFZ", "Violin.sfz"]);
    }

    /// The paths of `found` below `root`, `/`-separated.
    fn relative(root: &std::path::Path, found: &[String]) -> Vec<String> {
        found
            .iter()
            .map(|path| {
                let parts: Vec<&str> = std::path::Path::new(path)
                    .strip_prefix(root)
                    .expect("below the root")
                    .components()
                    .map(|part| part.as_os_str().to_str().expect("UTF-8"))
                    .collect();
                parts.join("/")
            })
            .collect()
    }

    #[test]
    fn include_fragments_are_not_patches() {
        // TST-06: Sonatina's layout, where 190 fragments in `includes` folders were listed as
        // patches next to the patches that include them.
        let dir = tempfile::tempdir().expect("a tempdir");
        let root = dir.path();
        touch(&root.join("Strings - Notation/1st Violins.sfz"));
        touch(&root.join("Strings - Notation/includes/1st-violins-sustain.sfz"));
        touch(&root.join("Strings - Notation/INCLUDES/nested/vibrato.sfz"));
        touch(&root.join("Includes/Common.sfz"));
        // Only a folder with exactly that name is a fragment folder; a patch file named like it
        // and a folder whose name merely contains it are scanned.
        touch(&root.join("Brass/includes.sfz"));
        touch(&root.join("Brass/my includes/Horn.sfz"));

        let found = scan_patches(&[root.to_owned()]);

        assert_eq!(
            relative(root, &found),
            [
                "Brass/includes.sfz",
                "Brass/my includes/Horn.sfz",
                "Strings - Notation/1st Violins.sfz"
            ]
        );
    }

    #[test]
    fn a_root_named_includes_is_still_scanned() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let root = dir.path().join("includes");
        touch(&root.join("Piano.sfz"));
        touch(&root.join("includes/fragment.sfz"));
        let found = scan_patches(std::slice::from_ref(&root));
        assert_eq!(relative(&root, &found), ["Piano.sfz"]);
    }

    #[test]
    fn a_library_of_fragments_only_offers_no_sampler() {
        let dir = tempfile::tempdir().expect("a tempdir");
        touch(&dir.path().join("includes/violin-sustain.sfz"));
        let mut settings = Settings::default();
        settings.extra.insert(
            LIBRARIES_KEY.to_owned(),
            json!([dir.path().to_str().expect("UTF-8")]),
        );
        let registry = registry_of(&settings);
        assert!(registry.find("sonara.sampler").is_none());
        assert_eq!(registry.all(None).len(), 12, "the golden catalog only");
    }

    #[test]
    fn the_sampler_is_registered_only_when_a_library_holds_instruments() {
        let none = registry_of(&Settings::default());
        assert!(
            none.find("sonara.sampler").is_none(),
            "no library: the golden catalog"
        );
        assert_eq!(none.all(None).len(), 12);

        let dir = tempfile::tempdir().expect("a tempdir");
        let root = dir.path().join("My Strings");
        touch(&root.join("Violin.sfz"));
        touch(&root.join("Solo/Cello.sfz"));
        // A fragment is not offered as an instrument, and neither is an empty patch file.
        touch(&root.join("includes/violin-sustain.sfz"));
        fs::write(root.join("Empty.sfz"), "").expect("write");
        let mut settings = Settings::default();
        settings.extra.insert(
            LIBRARIES_KEY.to_owned(),
            json!([root.to_str().expect("UTF-8"), 7]),
        );
        let registry = registry_of(&settings);
        let sampler = registry.find("sonara.sampler").expect("registered");
        assert_eq!(sampler.kind, PluginKind::Instrument);
        let instrument = sampler
            .param("instrument")
            .expect("the instrument parameter");
        assert_eq!(
            instrument.choices,
            ["my-strings/Solo/Cello", "my-strings/Violin"],
            "the preset IDs, sorted, never a path"
        );
        assert_eq!(registry.all(None).len(), 13);
    }

    /// The sampler of a registry with one installed preset.
    fn sampler() -> PluginDescriptor {
        registry_for(&["lib/Brass/Horn".to_owned()])
            .find("sonara.sampler")
            .expect("registered")
            .clone()
    }

    /// The first choices of `articulation`, as released (TST-05). An automation lane stores
    /// the index of a choice, so none of these ever moves; a new articulation is appended at
    /// the end of the list.
    const RELEASED_ARTICULATIONS: [&str; 17] = [
        "auto",
        "ordinario",
        "sustain",
        "legato",
        "staccato",
        "spiccato",
        "tenuto",
        "marcato",
        "pizzicato",
        "tremolo",
        "trill",
        "col_legno",
        "harmonics",
        "sul_ponticello",
        "con_sordino",
        "flutter_tongue",
        "roll",
    ];

    #[test]
    fn the_articulation_choices_are_auto_ordinario_and_the_frozen_vocabulary() {
        let sampler = sampler();
        let articulation = sampler.param("articulation").expect("the parameter");
        assert_eq!(articulation.kind, ParamKind::Enum);
        assert!(articulation.automatable);
        assert_eq!(
            articulation.choices.get(..RELEASED_ARTICULATIONS.len()),
            Some(&RELEASED_ARTICULATIONS.map(str::to_owned)[..])
        );
        // After `auto` and `ordinario`, the vocabulary in its own frozen order, so choice
        // `i + 2` is articulation `i` of every later version too.
        let expected: Vec<&str> = ["auto", "ordinario"]
            .into_iter()
            .chain(Articulation::ALL.iter().map(|a| a.id()))
            .collect();
        assert_eq!(articulation.choices, expected);
        assert_eq!(articulation.choices.len(), 79);
        let distinct: HashSet<&String> = articulation.choices.iter().collect();
        assert_eq!(
            distinct.len(),
            articulation.choices.len(),
            "no choice twice"
        );
    }

    /// The choices of `mute`, as released (TST-05): `auto`, then every mute in its order. An
    /// automation lane stores the index of a choice, so none of these ever moves.
    const RELEASED_MUTES: [&str; 10] = [
        "auto",
        "open",
        "con_sordino",
        "mute_cup",
        "mute_harmon",
        "mute_plunger",
        "mute_bucket",
        "stopped",
        "palm_mute",
        "xylophonic",
    ];

    /// The choices of `vibrato`, as released (TST-05).
    const RELEASED_VIBRATOS: [&str; 4] = ["auto", "ordinary", "vibrato_on", "non_vibrato"];

    #[test]
    fn the_mute_and_vibrato_choices_are_auto_and_every_state_in_its_order() {
        let sampler = sampler();
        let mute = sampler.param("mute").expect("the parameter");
        assert_eq!(mute.kind, ParamKind::Enum);
        assert!(mute.automatable, "a lane switches it");
        assert_eq!(
            mute.choices.get(..RELEASED_MUTES.len()),
            Some(&RELEASED_MUTES.map(str::to_owned)[..])
        );
        // Choice `i + 1` is mute `i`: the open sound, then each mute by the articulation ID
        // that names it.
        assert_eq!(mute.choices.len(), 1 + Mute::ALL.len());
        for (index, state) in Mute::ALL.iter().enumerate() {
            let named = state
                .articulation()
                .map_or(super::MUTE_OPEN, Articulation::id);
            assert_eq!(mute.choices[index + 1], named, "{state:?}");
        }

        let vibrato = sampler.param("vibrato").expect("the parameter");
        assert_eq!(vibrato.kind, ParamKind::Enum);
        assert!(vibrato.automatable, "a lane switches it");
        assert_eq!(vibrato.choices, RELEASED_VIBRATOS.map(str::to_owned));
        for (index, state) in Vibrato::ALL.iter().enumerate() {
            let named = state
                .articulation()
                .map_or(super::VIBRATO_ORDINARY, Articulation::id);
            assert_eq!(vibrato.choices[index + 1], named, "{state:?}");
        }
        for choices in [&mute.choices, &vibrato.choices] {
            let distinct: HashSet<&String> = choices.iter().collect();
            assert_eq!(distinct.len(), choices.len(), "no choice twice");
        }
    }

    #[test]
    fn the_instrument_type_choices_are_auto_and_every_type_in_score_order() {
        let sampler = sampler();
        let instrument_type = sampler.param("instrument_type").expect("the parameter");
        assert_eq!(instrument_type.kind, ParamKind::Enum);
        assert!(!instrument_type.automatable, "projects store it as text");
        let (auto, types) = instrument_type.choices.split_first().expect("choices");
        assert_eq!(auto, "auto");
        assert_eq!(types.len(), 85);
        // Every choice after `auto` names an instrument type, and they follow score order.
        let parsed: Vec<InstrumentType> = types
            .iter()
            .map(|id| InstrumentType::from_id(id).unwrap_or_else(|| panic!("{id}")))
            .collect();
        assert_eq!(parsed, InstrumentType::ALL);
        assert!(
            parsed.windows(2).all(|pair| pair[0] < pair[1]),
            "score order"
        );
        for released in [
            "piccolo",
            "horn_f",
            "timpani",
            "harp",
            "choir_bass",
            "double_bass",
        ] {
            assert!(types.iter().any(|id| id == released), "{released}");
        }
    }

    #[test]
    fn every_sampler_device_shares_the_lists_whatever_it_offers() {
        let one = sampler();
        let other = registry_for(&["lib/Strings/Violin".to_owned(), "lib/Keys/Piano".to_owned()])
            .find("sonara.sampler")
            .expect("registered")
            .clone();
        for id in ["instrument_type", "articulation", "mute", "vibrato"] {
            assert_eq!(
                one.param(id).expect("the parameter").choices,
                other.param(id).expect("the parameter").choices,
                "{id}"
            );
        }
        let dynamics = one.param("dynamics").expect("the parameter");
        assert_eq!(dynamics.kind, ParamKind::Float);
        assert!(dynamics.automatable);
    }
}
