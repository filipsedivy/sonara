//! The sample libraries installed on this machine (ADR 0010): the directories the settings key
//! `sample_libraries` lists, and the SFZ instruments found in them.
//!
//! Sonara ships no sample library. Libraries are installed by the user, so the sampler is
//! offered only when a configured directory holds at least one `.sfz` patch
//! ([`sampler_descriptor`]). The scan reads directory entries only; patches are loaded when a
//! track plays them, off the real-time thread.

use std::fs;
use std::path::{Path, PathBuf};

use sonara_core::plugins::{
    PluginDescriptor, PluginRegistry, builtin_registry, sampler_descriptor,
};

use crate::settings::Settings;

/// The settings key listing the sample library directories.
pub const LIBRARIES_KEY: &str = "sample_libraries";
/// The deepest directory level a scan enters below a library root.
const MAX_DEPTH: usize = 8;
/// The most patches a scan reports; a directory tree with more is cut short.
pub const MAX_PATCHES: usize = 5_000;

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
/// links are skipped, so a link cannot loop the scan; an unreadable directory is skipped.
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
        let hidden = entry.file_name().to_string_lossy().starts_with('.');
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if hidden || kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            walk(&path, depth + 1, found);
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
/// when the configured libraries hold instruments.
#[must_use]
pub fn registry_for(settings: &Settings) -> PluginRegistry {
    let mut registry = builtin_registry();
    let patches = scan_patches(&library_roots(settings));
    if let Some(descriptor) = sampler_descriptor(&patches) {
        register(&mut registry, descriptor);
    }
    registry
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

    use serde_json::json;
    use sonara_core::plugins::PluginKind;

    use super::{LIBRARIES_KEY, registry_for, scan_patches};
    use crate::settings::Settings;

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

    #[test]
    fn the_sampler_is_registered_only_when_a_library_holds_instruments() {
        let none = registry_for(&Settings::default());
        assert!(
            none.find("sonara.sampler").is_none(),
            "no library: the golden catalog"
        );
        assert_eq!(none.all(None).len(), 12);

        let dir = tempfile::tempdir().expect("a tempdir");
        touch(&dir.path().join("Violin.sfz"));
        let mut settings = Settings::default();
        settings.extra.insert(
            LIBRARIES_KEY.to_owned(),
            json!([dir.path().to_str().expect("UTF-8"), 7]),
        );
        let registry = registry_for(&settings);
        let sampler = registry.find("sonara.sampler").expect("registered");
        assert_eq!(sampler.kind, PluginKind::Instrument);
        let instrument = sampler
            .param("instrument")
            .expect("the instrument parameter");
        assert_eq!(instrument.choices.len(), 1);
        assert!(instrument.choices[0].ends_with("Violin.sfz"));
        assert_eq!(registry.all(None).len(), 13);
    }
}
