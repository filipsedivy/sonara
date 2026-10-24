//! The Browser panel of an interface (–BR-05): the Devices
//! tab's tree of instruments, effects and their presets ([`device_tree`]), and the Files tab's
//! listing of a folder ([`list_directory`]).
//!
//! **The tree** is the prototype's `BrowserPanel._populate`: "Instruments" with each instrument
//! and its presets, then "Effects" with a branch per category (`eq` is "EQ", every other one
//! Python's `str.title()` of it, which is not the mixer picker's wording), each effect and its
//! presets. Plugins are taken in the order of `(kind, category, name)`; an item's tooltip is the
//! description, a preset's its description or "`<plugin> preset`".
//!
//! **The listing** is the prototype's `QFileSystemModel` rooted at `~/Music` (or `~` when that is
//! missing) and filtered to the audio files: folders and the files whose suffix the import takes
//! ([`is_audio_import`], the one list of the timeline, the window's drops and the file panels;
//! G12), without hidden entries. The web view cannot list folders, so the page asks
//! here, for the root and the folders under it only (TYP-06). Entries are sorted as the model
//! sorts them on macOS: by name, case-insensitively, with numbers compared by value, folders and
//! files mixed.

use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::plugins::{PluginKind, PluginRegistry};

use crate::catalog::title_case;
use crate::error::{AppResult, IoContext as _};
use crate::media::is_audio_import;
use crate::paths::{expand_user, resolve};

/// One item of the Devices tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BrowserItem {
    /// The text.
    pub text: String,
    /// The plugin of a plugin or preset item.
    pub plugin_id: Option<String>,
    /// The preset of a preset item.
    pub preset: Option<String>,
    /// `instrument` or `effect` for a plugin or preset item.
    pub kind: Option<&'static str>,
    /// The tooltip; empty for a branch.
    pub tooltip: String,
    /// The items under it.
    pub children: Vec<BrowserItem>,
}

impl BrowserItem {
    fn branch(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            plugin_id: None,
            preset: None,
            kind: None,
            tooltip: String::new(),
            children: Vec::new(),
        }
    }
}

/// The browser's label of an effect category.
fn category_label(category: &str) -> String {
    if category == "eq" {
        "EQ".to_owned()
    } else {
        title_case(category)
    }
}

/// The Devices tab's tree: "Instruments", then "Effects" (see the module docs).
#[must_use]
pub fn device_tree(plugins: &PluginRegistry) -> Vec<BrowserItem> {
    let mut descriptors = plugins.all(None);
    descriptors.sort_by(|a, b| {
        (a.kind.as_str(), a.category.as_str(), a.name.as_str()).cmp(&(
            b.kind.as_str(),
            b.category.as_str(),
            b.name.as_str(),
        ))
    });
    let mut instruments = BrowserItem::branch("Instruments");
    let mut effects = BrowserItem::branch("Effects");
    // The categories in the order they first appear.
    let mut categories: Vec<(String, BrowserItem)> = Vec::new();
    for descriptor in descriptors {
        let kind = descriptor.kind.as_str();
        let mut item = BrowserItem {
            text: descriptor.name.clone(),
            plugin_id: Some(descriptor.id.clone()),
            preset: None,
            kind: Some(kind),
            tooltip: descriptor.description.clone(),
            children: Vec::new(),
        };
        for preset in &descriptor.presets {
            item.children.push(BrowserItem {
                text: preset.name.clone(),
                plugin_id: Some(descriptor.id.clone()),
                preset: Some(preset.name.clone()),
                kind: Some(kind),
                tooltip: if preset.description.is_empty() {
                    format!("{} preset", descriptor.name)
                } else {
                    preset.description.clone()
                },
                children: Vec::new(),
            });
        }
        if descriptor.kind == PluginKind::Effect {
            if let Some((_, branch)) = categories
                .iter_mut()
                .find(|(category, _)| *category == descriptor.category)
            {
                branch.children.push(item);
            } else {
                let mut branch = BrowserItem::branch(category_label(&descriptor.category));
                branch.children.push(item);
                categories.push((descriptor.category.clone(), branch));
            }
        } else {
            instruments.children.push(item);
        }
    }
    effects.children = categories.into_iter().map(|(_, branch)| branch).collect();
    vec![instruments, effects]
}

/// One entry of a folder listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileEntry {
    /// The file or folder name.
    pub name: String,
    /// Its full path.
    pub path: String,
    /// Whether it is a folder (it lists children) rather than an audio file.
    pub folder: bool,
}

/// The Files tab's root: `~/Music`, or `~` when that is missing.
#[must_use]
pub fn browser_root() -> PathBuf {
    let home = expand_user(Path::new("~"));
    let music = home.join("Music");
    if music.is_dir() { music } else { home }
}

/// The folders and audio files in `folder`, which must be `root` or a folder under it.
///
/// # Errors
///
/// - `PERMISSION_DENIED` for a folder outside `root`;
/// - `NOT_FOUND` for a path that is not a folder;
/// - [`crate::error::AppError::Io`] when the folder cannot be read.
pub fn list_directory(root: &Path, folder: &Path) -> AppResult<Vec<FileEntry>> {
    let root = resolve(root);
    let folder = resolve(folder);
    if !folder.starts_with(&root) {
        return Err(DomainError::new(
            ErrorCode::PermissionDenied,
            format!("{} is outside {}", folder.display(), root.display()),
        )
        .into());
    }
    if !folder.is_dir() {
        return Err(DomainError::new(
            ErrorCode::NotFound,
            format!("{} is not a folder", folder.display()),
        )
        .into());
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(&folder).io("list", &folder)? {
        // ERR-06: an entry that vanished or cannot be read is not listed, as the model skips it.
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        // Follows symbolic links, as the model does.
        let folder = path.is_dir();
        if folder || (path.is_file() && is_audio_import(&path)) {
            entries.push(FileEntry {
                name,
                path: path.display().to_string(),
                folder,
            });
        }
    }
    entries.sort_by(|a, b| natural_order(&a.name, &b.name));
    Ok(entries)
}

/// Names compared case-insensitively, with runs of digits compared by their value (`QCollator`
/// with numeric mode, as the model sorts); equal names by their text.
fn natural_order(a: &str, b: &str) -> Ordering {
    let (lower_a, lower_b) = (a.to_lowercase(), b.to_lowercase());
    let mut left = lower_a.chars().peekable();
    let mut right = lower_b.chars().peekable();
    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |chars: &mut std::iter::Peekable<std::str::Chars<'_>>| {
                    let mut digits = String::new();
                    while let Some(&c) = chars.peek().filter(|c| c.is_ascii_digit()) {
                        digits.push(c);
                        chars.next();
                    }
                    digits
                };
                let (x, y) = (take(&mut left), take(&mut right));
                let (tx, ty) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                let by_value = tx.len().cmp(&ty.len()).then_with(|| tx.cmp(ty));
                if by_value != Ordering::Equal {
                    return by_value;
                }
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(&y);
                }
                left.next();
                right.next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::natural_order;

    #[test]
    fn names_sort_naturally_and_case_insensitively() {
        let mut names = vec![
            "take 10.wav",
            "Take 2.wav",
            "drums",
            "Bass.wav",
            "take 1.wav",
        ];
        names.sort_by(|a, b| natural_order(a, b));
        assert_eq!(
            names,
            vec![
                "Bass.wav",
                "drums",
                "take 1.wav",
                "Take 2.wav",
                "take 10.wav"
            ]
        );
        assert_eq!(natural_order("a01", "a1"), "a01".cmp("a1"));
        assert_eq!(natural_order("A", "a"), Ordering::Less);
    }
}
