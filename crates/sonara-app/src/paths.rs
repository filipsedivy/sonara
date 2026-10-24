//! Platform application directories (`sonara.config.paths`). `SONARA_HOME` overrides all of
//! them.
//!
//! | Platform | Home of the application |
//! |---|---|
//! | macOS | `~/Library/Application Support/Sonara` |
//! | Windows | `%APPDATA%\Sonara` (`~\AppData\Roaming\Sonara` without `APPDATA`) |
//! | other | `$XDG_DATA_HOME/sonara` (`~/.local/share/sonara` without `XDG_DATA_HOME`) |
//!
//! QG-13: the rules are written here instead of taken from `dirs` or `directories`, whose
//! `dirs-sys` pulls in the MPL-2.0 crate `option-ext` (an owner decision under LIC-01), and whose
//! answers differ from the prototype's: the Windows known-folder API instead of `%APPDATA%`, and a
//! relative `XDG_DATA_HOME` ignored instead of used. `etcetera` differs in the same two places.
//! The prototype's rules are three environment lookups.
//!
//! This module also holds the `pathlib` operations the services apply to user paths, so that
//! paths in tool results are spelled as the prototype spells them: [`expand_user`] and
//! [`resolve`], which interfaces that resolve user paths themselves (the agent's path sandbox)
//! use too, and the suffix rules of [`split_suffix`] and [`with_suffix`].

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// The home directory is unknown, so the application directories are too.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("could not determine the home directory; set SONARA_HOME")]
pub struct NoHomeDirectory;

/// Results of the directory lookups.
pub type DirResult = Result<PathBuf, NoHomeDirectory>;

/// The platforms whose directory rules differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Platform {
    MacOs,
    Windows,
    Other,
}

/// What the directory rules read: environment variables, the home directory and the platform.
struct Env<'a> {
    var: &'a dyn Fn(&str) -> Option<OsString>,
    home: Option<PathBuf>,
    platform: Platform,
}

impl Env<'_> {
    fn system() -> Env<'static> {
        let platform = if cfg!(target_os = "macos") {
            Platform::MacOs
        } else if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Other
        };
        Env {
            var: &|name| std::env::var_os(name),
            home: std::env::home_dir(),
            platform,
        }
    }

    /// A non-empty variable (Python's `if override:`).
    fn non_empty(&self, name: &str) -> Option<OsString> {
        (self.var)(name).filter(|value| !value.is_empty())
    }

    fn home(&self) -> DirResult {
        self.home.clone().ok_or(NoHomeDirectory)
    }

    fn app_home(&self) -> DirResult {
        if let Some(home) = self.non_empty("SONARA_HOME") {
            return Ok(py_path(&self.expand_user(Path::new(&home))));
        }
        let base = match self.platform {
            Platform::MacOs => {
                return Ok(self
                    .home()?
                    .join("Library")
                    .join("Application Support")
                    .join("Sonara"));
            }
            Platform::Windows => match (self.var)("APPDATA") {
                Some(appdata) => PathBuf::from(appdata),
                None => self.home()?.join("AppData").join("Roaming"),
            },
            Platform::Other => match (self.var)("XDG_DATA_HOME") {
                Some(data) => PathBuf::from(data),
                None => self.home()?.join(".local").join("share"),
            },
        };
        let name = if self.platform == Platform::Windows {
            "Sonara"
        } else {
            "sonara"
        };
        Ok(py_path(&base.join(name)))
    }

    fn default_projects_dir(&self) -> DirResult {
        if let Some(dir) = self.non_empty("SONARA_PROJECTS_DIR") {
            return Ok(py_path(&self.expand_user(Path::new(&dir))));
        }
        Ok(self.home()?.join("Music").join("Sonara"))
    }

    fn expand_user(&self, path: &Path) -> PathBuf {
        let mut components = path.components();
        match (components.next(), &self.home) {
            (Some(Component::Normal(first)), Some(home)) if first == "~" => {
                home.join(components.as_path())
            }
            _ => path.to_path_buf(),
        }
    }
}

/// The application's home directory.
///
/// # Errors
///
/// [`NoHomeDirectory`] when neither `SONARA_HOME` nor the home directory is known.
pub fn app_home() -> DirResult {
    Env::system().app_home()
}

/// User settings (`settings.json`).
///
/// # Errors
///
/// [`NoHomeDirectory`], as [`app_home`].
pub fn config_dir() -> DirResult {
    Ok(app_home()?.join("config"))
}

/// Scratch workspaces for unsaved and unpacked projects (also the crash-recovery area).
///
/// # Errors
///
/// [`NoHomeDirectory`], as [`app_home`].
pub fn workspaces_dir() -> DirResult {
    Ok(app_home()?.join("workspaces"))
}

/// IPC discovery files and sockets (mode `0700`).
///
/// # Errors
///
/// [`NoHomeDirectory`], as [`app_home`].
pub fn runtime_dir() -> DirResult {
    Ok(app_home()?.join("run"))
}

/// Log files.
///
/// # Errors
///
/// [`NoHomeDirectory`], as [`app_home`].
pub fn logs_dir() -> DirResult {
    Ok(app_home()?.join("logs"))
}

/// Where new projects are saved by default: `SONARA_PROJECTS_DIR`, else `~/Music/Sonara`.
///
/// # Errors
///
/// [`NoHomeDirectory`] without `SONARA_PROJECTS_DIR` and a home directory.
pub fn default_projects_dir() -> DirResult {
    Env::system().default_projects_dir()
}

/// Creates `path` with its parents and, outside Windows, restricts it to its owner (`0700`).
///
/// # Errors
///
/// The I/O error of creating the directory or changing its mode.
pub fn ensure_private_dir(path: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(path.to_path_buf())
}

/// `Path.expanduser()` for a leading `~`: the current user's home directory. `~user` and a path
/// without a known home stay unchanged (the prototype raises for those).
#[must_use]
pub fn expand_user(path: &Path) -> PathBuf {
    Env::system().expand_user(path)
}

/// A file name split as Python's `PurePath.stem` and `PurePath.suffix` split it (3.12 and
/// 3.13): the suffix starts at the last `.` that is neither the first nor the last character,
/// and is empty without one.
#[must_use]
pub fn split_suffix(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(dot) if dot > 0 && dot + 1 < name.len() => name.split_at(dot),
        _ => (name, ""),
    }
}

/// `PurePath.with_suffix(suffix)`: the file name with its suffix ([`split_suffix`]) replaced by
/// `suffix`, which starts with a dot. A name that is not UTF-8 gets Rust's extension rule.
#[must_use]
pub fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    match path.file_name().and_then(|name| name.to_str()) {
        Some(name) => path.with_file_name(format!("{}{suffix}", split_suffix(name).0)),
        None => path.with_extension(suffix.trim_start_matches('.')),
    }
}

/// A path as Python's `Path()` spells it: no `.` segments, no repeated or trailing separators.
fn py_path(path: &Path) -> PathBuf {
    path.components()
        .filter(|c| !matches!(c, Component::CurDir))
        .collect()
}

/// Python's non-strict `Path.resolve()` (`posixpath.realpath` of `CPython` 3.13): absolute, with
/// `.` dropped, symbolic links resolved where the path exists, and `..` applied to the path
/// resolved so far. Missing parts are kept as they are. Relative paths resolve against the
/// current directory; without one, the path stays unchanged.
#[cfg(unix)]
#[must_use]
pub fn resolve(path: &Path) -> PathBuf {
    use std::collections::HashMap;
    use std::ffi::OsStr;
    use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};

    /// An unresolved part, or the end of a symbolic link's target (which caches its result).
    enum Part {
        Name(Vec<u8>),
        LinkEnd(Vec<u8>),
    }
    fn parts(text: &[u8]) -> impl Iterator<Item = Part> + '_ {
        text.split(|b| *b == b'/')
            .rev()
            .map(|p| Part::Name(p.to_vec()))
    }

    let filename = path.as_os_str().as_bytes();
    let mut resolved: Vec<u8> = if filename.starts_with(b"/") {
        b"/".to_vec()
    } else {
        match std::env::current_dir() {
            Ok(cwd) => cwd.into_os_string().into_vec(),
            Err(_) => return path.to_path_buf(),
        }
    };
    let mut rest: Vec<Part> = parts(filename).collect();
    // Links being resolved (None) and resolved links, as CPython's `seen`.
    let mut seen: HashMap<Vec<u8>, Option<Vec<u8>>> = HashMap::new();
    while let Some(part) = rest.pop() {
        let name = match part {
            Part::LinkEnd(link) => {
                seen.insert(link, Some(resolved.clone()));
                continue;
            }
            Part::Name(name) => name,
        };
        if name.is_empty() || name == b"." {
            continue;
        }
        if name == b".." {
            let cut = resolved.iter().rposition(|b| *b == b'/').unwrap_or(0);
            resolved.truncate(cut);
            if resolved.is_empty() {
                resolved.push(b'/');
            }
            continue;
        }
        let mut candidate = resolved.clone();
        if candidate != b"/" {
            candidate.push(b'/');
        }
        candidate.extend_from_slice(&name);
        let candidate_path = Path::new(OsStr::from_bytes(&candidate));
        let is_link =
            fs::symlink_metadata(candidate_path).is_ok_and(|m| m.file_type().is_symlink());
        if !is_link {
            resolved = candidate;
            continue;
        }
        if let Some(cached) = seen.get(&candidate) {
            // A resolved link, or a loop (the link is still being resolved).
            resolved = cached.clone().unwrap_or(candidate);
            continue;
        }
        match fs::read_link(candidate_path) {
            Ok(target) => {
                let target = target.into_os_string().into_vec();
                if target.starts_with(b"/") {
                    resolved = b"/".to_vec();
                }
                seen.insert(candidate.clone(), None);
                rest.push(Part::LinkEnd(candidate));
                rest.extend(parts(&target));
            }
            Err(_) => resolved = candidate,
        }
    }
    PathBuf::from(OsString::from_vec(resolved))
}

/// Python's non-strict `Path.resolve()` on Windows, approximated: absolute and lexically
/// normalized (the product targets macOS; symbolic links are not resolved here).
#[cfg(not(unix))]
#[must_use]
pub fn resolve(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut out = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use super::{
        Env, NoHomeDirectory, Platform, config_dir, ensure_private_dir, resolve, split_suffix,
        with_suffix,
    };

    #[test]
    fn suffixes_follow_pathlib() {
        // CPython 3.13: `PurePath(name).stem`, `.suffix` and `.with_suffix(".wav")`.
        for (name, stem, suffix, replaced) in [
            ("mix.wav", "mix", ".wav", "mix.wav"),
            ("a.tar.gz", "a.tar", ".gz", "a.tar.wav"),
            ("plain", "plain", "", "plain.wav"),
            (".hidden", ".hidden", "", ".hidden.wav"),
            ("trailing.", "trailing.", "", "trailing..wav"),
        ] {
            assert_eq!(split_suffix(name), (stem, suffix), "{name}");
            assert_eq!(
                with_suffix(&Path::new("/out").join(name), ".wav"),
                Path::new("/out").join(replaced),
                "{name}"
            );
        }
    }

    /// Runs `f` with exactly these variables, home directory and platform.
    fn with_env<T>(
        vars: &[(&str, &str)],
        platform: Platform,
        home: Option<&str>,
        f: impl FnOnce(&Env<'_>) -> T,
    ) -> T {
        let lookup = |name: &str| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        };
        let e = Env {
            var: &lookup,
            home: home.map(PathBuf::from),
            platform,
        };
        f(&e)
    }

    // The prototype's `test_paths_follow_sonara_home`.
    #[test]
    fn paths_follow_sonara_home() {
        let dir = tempfile::tempdir().expect("temp dir");
        let home = dir.path().join("sonara-home");
        let projects = home.join("projects");
        let vars = [
            ("SONARA_HOME", home.to_str().expect("UTF-8")),
            ("SONARA_PROJECTS_DIR", projects.to_str().expect("UTF-8")),
        ];
        for platform in [Platform::MacOs, Platform::Windows, Platform::Other] {
            with_env(&vars, platform, Some("/home/u"), |e| {
                assert_eq!(e.app_home(), Ok(home.clone()));
                assert_eq!(e.default_projects_dir(), Ok(projects.clone()));
            });
        }
        let private = ensure_private_dir(&home.join("run")).expect("created");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&private)
                .expect("exists")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700);
        }
        assert!(private.is_dir());
    }

    /// Variables, platform, expected home of the application.
    type Case<'a> = (&'a [(&'a str, &'a str)], Platform, &'a str);

    #[test]
    fn platform_defaults_match_the_prototype() {
        let cases: [Case<'_>; 7] = [
            (
                &[],
                Platform::MacOs,
                "/Users/u/Library/Application Support/Sonara",
            ),
            (
                &[("APPDATA", "/c/Roaming")],
                Platform::Windows,
                "/c/Roaming/Sonara",
            ),
            (&[], Platform::Windows, "/Users/u/AppData/Roaming/Sonara"),
            (
                &[("XDG_DATA_HOME", "/data")],
                Platform::Other,
                "/data/sonara",
            ),
            (&[], Platform::Other, "/Users/u/.local/share/sonara"),
            // An empty override is ignored; an empty XDG_DATA_HOME is used, as in Python.
            (
                &[("SONARA_HOME", "")],
                Platform::Other,
                "/Users/u/.local/share/sonara",
            ),
            (&[("XDG_DATA_HOME", "")], Platform::Other, "sonara"),
        ];
        for (vars, platform, expected) in cases {
            with_env(vars, platform, Some("/Users/u"), |e| {
                assert_eq!(e.app_home(), Ok(PathBuf::from(expected)), "{vars:?}");
            });
        }
        with_env(&[], Platform::Other, Some("/Users/u"), |e| {
            assert_eq!(
                e.default_projects_dir(),
                Ok(PathBuf::from("/Users/u/Music/Sonara"))
            );
        });
    }

    #[test]
    fn overrides_expand_the_home_directory_and_normalize() {
        let vars = [("SONARA_HOME", "~/x/./y/"), ("SONARA_PROJECTS_DIR", "~")];
        with_env(&vars, Platform::MacOs, Some("/Users/u"), |e| {
            assert_eq!(e.app_home(), Ok(PathBuf::from("/Users/u/x/y")));
            assert_eq!(e.default_projects_dir(), Ok(PathBuf::from("/Users/u")));
        });
    }

    #[test]
    fn no_home_directory_is_an_error() {
        with_env(&[], Platform::MacOs, None, |e| {
            assert_eq!(e.app_home(), Err(NoHomeDirectory));
            assert_eq!(e.default_projects_dir(), Err(NoHomeDirectory));
        });
        with_env(&[("SONARA_HOME", "/h")], Platform::MacOs, None, |e| {
            assert_eq!(e.app_home(), Ok(PathBuf::from("/h")));
        });
    }

    #[test]
    fn subdirectories_hang_off_the_home() {
        if let (Ok(home), Ok(config)) = (super::app_home(), config_dir()) {
            assert_eq!(config, home.join("config"));
            assert_eq!(super::workspaces_dir(), Ok(home.join("workspaces")));
            assert_eq!(super::runtime_dir(), Ok(home.join("run")));
            assert_eq!(super::logs_dir(), Ok(home.join("logs")));
        }
    }

    #[cfg(unix)]
    #[test]
    fn resolve_follows_python_realpath() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = resolve(dir.path());
        assert!(root.is_absolute());
        std::fs::create_dir_all(root.join("real/sub")).expect("dirs");
        std::os::unix::fs::symlink(root.join("real"), root.join("link")).expect("link");
        std::os::unix::fs::symlink("loop", root.join("loop")).expect("loop");
        let cases = [
            ("link/sub", "real/sub"),
            ("link/sub/../x", "real/x"),
            ("link/missing/../y", "real/y"),
            ("./real//sub/./", "real/sub"),
            ("missing/a/b", "missing/a/b"),
            ("loop/x", "loop/x"),
        ];
        for (input, expected) in cases {
            assert_eq!(resolve(&root.join(input)), root.join(expected), "{input}");
        }
        assert_eq!(resolve(Path::new("/..")), PathBuf::from("/"));
        assert_eq!(resolve(Path::new("/a/../../b")), PathBuf::from("/b"));
    }
}
