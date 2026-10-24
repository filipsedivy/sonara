//! The file sandbox (`PathPolicy` of `sonara.agent.permissions`): agents read and write only
//! inside allowed roots, and symbolic links are resolved before the check.
//!
//! Paths are resolved like the prototype's `Path.expanduser().resolve()` on Python 3.13: the
//! non-strict `posixpath.realpath`, which resolves every symbolic link that exists, keeps the
//! components that do not exist as they are, applies `..` to the path resolved so far (after
//! the links before it), and leaves a link that loops unresolved. `std::fs::canonicalize` cannot
//! be used because it fails for paths that do not exist yet, and no crate reproduces these
//! rules (QG-13), so [`realpath`] ports the algorithm.

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, MAIN_SEPARATOR_STR, Path, PathBuf};

use serde_json::Value;
use sonara_core::error::{DomainError, ErrorCode};

use crate::pystr::py_strip;

/// The environment variable with more allowed roots, separated like `PATH`.
pub const ENV_ALLOWED: &str = "SONARA_AGENT_ALLOWED_PATHS";

/// The `hint` of a `PERMISSION_DENIED` error.
const HINT: &str = "ask the user to allow it (Sonara settings 'agent_allowed_paths' or \
                    $SONARA_AGENT_ALLOWED_PATHS)";

/// Why a home directory cannot be expanded: Python's `RuntimeError` text.
const NO_HOME: &str = "Could not determine home directory.";

/// Why a path with a NUL character cannot be resolved.
const NUL: &str = "embedded null character in path";

/// What an agent wants to do with a path (the prototype's `purpose`), as named in the
/// `PERMISSION_DENIED` message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Reading an existing file or project.
    Read,
    /// Creating or overwriting a file or directory.
    Write,
}

impl Access {
    /// `"read"` or `"write"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

impl fmt::Display for Access {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The allowed roots of one agent client, and the base directory that relative paths resolve
/// against. Roots are kept resolved, in the order they were added, without duplicates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathPolicy {
    roots: Vec<PathBuf>,
    base: PathBuf,
}

impl PathPolicy {
    /// A policy with `roots` (each added with [`PathPolicy::allow`]) and the resolved `base`.
    ///
    /// Unlike the roots, `base` is not `~`-expanded (as in the prototype). A relative `base`
    /// resolves against the working directory; if that cannot be read, it stays relative.
    #[must_use]
    pub fn new<P: AsRef<Path>>(roots: impl IntoIterator<Item = P>, base: &Path) -> Self {
        let base = absolute(base).map_or_else(|| base.to_owned(), |path| realpath(&path));
        let mut policy = Self {
            roots: Vec::new(),
            base,
        };
        for root in roots {
            policy.allow(root);
        }
        policy
    }

    /// The prototype's `PathPolicy.from_environment`: the roots from the settings
    /// (`configured`), then those in `env_allowed` (the value of [`ENV_ALLOWED`], split like
    /// `PATH`, blank entries skipped), then `extra`, then `base` itself when it is given. The
    /// policy's base is `base`, or the working directory.
    ///
    /// # Errors
    ///
    /// The error of [`std::env::current_dir`] when `base` is `None` and the working directory
    /// cannot be read (the prototype fails there too).
    pub fn from_environment(
        configured: &[String],
        env_allowed: Option<&OsStr>,
        base: Option<&Path>,
        extra: &[PathBuf],
    ) -> io::Result<Self> {
        let mut roots: Vec<PathBuf> = configured.iter().map(PathBuf::from).collect();
        if let Some(env) = env_allowed {
            roots.extend(
                std::env::split_paths(env).filter(|p| !py_strip(&p.to_string_lossy()).is_empty()),
            );
        }
        roots.extend(extra.iter().cloned());
        let base = match base {
            Some(base) => {
                roots.push(base.to_owned());
                base.to_owned()
            }
            None => std::env::current_dir()?,
        };
        Ok(Self::new(roots, &base))
    }

    /// Adds `root` (`~`-expanded, made absolute against the working directory and resolved)
    /// unless it is already allowed.
    ///
    /// A root that cannot be resolved is skipped, as the prototype skips one whose resolution
    /// fails: a relative root while the working directory cannot be read, a NUL character, or
    /// a `~user` root (where the prototype raises instead; see [`PathPolicy::resolve`]).
    pub fn allow(&mut self, root: impl AsRef<Path>) {
        let Ok(expanded) = prepare(root.as_ref()) else {
            return;
        };
        let Some(absolute) = absolute(&expanded) else {
            return;
        };
        let resolved = realpath(&absolute);
        if !self.roots.contains(&resolved) {
            self.roots.push(resolved);
        }
    }

    /// The allowed roots as strings, in order (the `allowed_roots` of a denial, and of
    /// `get_capabilities`).
    #[must_use]
    pub fn roots(&self) -> Vec<String> {
        self.roots.iter().map(|root| display(root)).collect()
    }

    /// The allowed roots, in order.
    #[must_use]
    pub fn roots_paths(&self) -> &[PathBuf] {
        &self.roots
    }

    /// The resolved base directory of relative paths.
    #[must_use]
    pub fn base(&self) -> &Path {
        &self.base
    }

    /// Resolves `path` (`~`-expanded, joined to the base when relative, symbolic links
    /// resolved) and returns it if it is an allowed root or inside one.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::PermissionDenied`]:
    ///
    /// - `"<access> access to <resolved> is outside the allowed locations"`, with the details
    ///   `allowed_roots` and `hint`;
    /// - `"cannot resolve <path>: <reason>"` for a path with a NUL character, and for a path
    ///   that starts with `~user`, whose home directory this port does not look up. The
    ///   prototype crashes on both (its registry reports `INTERNAL`) or, for a `~user` that
    ///   exists, expands it.
    pub fn resolve(&self, path: impl AsRef<Path>, access: Access) -> Result<PathBuf, DomainError> {
        let raw = path.as_ref();
        let expanded = prepare(raw).map_err(|reason| {
            DomainError::new(
                ErrorCode::PermissionDenied,
                format!("cannot resolve {}: {reason}", raw.display()),
            )
        })?;
        let resolved = realpath(&self.base.join(expanded));
        if self.roots.iter().any(|root| resolved.starts_with(root)) {
            return Ok(resolved);
        }
        Err(DomainError::new(
            ErrorCode::PermissionDenied,
            format!(
                "{access} access to {} is outside the allowed locations",
                resolved.display()
            ),
        )
        .with_detail("allowed_roots", Value::from(self.roots()))
        .with_detail("hint", HINT))
    }
}

/// A path as Python's `str(path)` shows it (lossy for paths that are not UTF-8).
fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// `path` made absolute against the working directory, or `None` when that cannot be read.
fn absolute(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() {
        Some(path.to_owned())
    } else {
        std::env::current_dir().ok().map(|cwd| cwd.join(path))
    }
}

/// Rejects a NUL character, then expands a leading `~` (`Path.expanduser`).
fn prepare(path: &Path) -> Result<PathBuf, &'static str> {
    if path.as_os_str().as_encoded_bytes().contains(&0) {
        return Err(NUL);
    }
    expand_user(path)
}

/// `Path.expanduser()`: a relative path whose first component is `~` starts at the home
/// directory instead ([`std::env::home_dir`]: `$HOME`, then the user database, as in
/// `posixpath.expanduser`). Every other path is returned unchanged.
///
/// A first component `~user` is an error: the prototype looks up that user's home directory
/// (and raises if there is none), which needs the user database API that this crate does not
/// bind.
fn expand_user(path: &Path) -> Result<PathBuf, &'static str> {
    let mut rest = path.components();
    // `Path` drops a leading `.`, so `./~` expands like `~`.
    let first = loop {
        match rest.next() {
            Some(Component::CurDir) => {}
            other => break other,
        }
    };
    let Some(Component::Normal(first)) = first else {
        return Ok(path.to_owned());
    };
    match first.as_encoded_bytes() {
        [b'~'] => {}
        [b'~', ..] => return Err(NO_HOME),
        _ => return Ok(path.to_owned()),
    }
    let home = std::env::home_dir().ok_or(NO_HOME)?;
    // `userhome.rstrip('/') or '/'`: an empty home is the root; one that still starts with `~`
    // was not expanded.
    let home = if home.as_os_str().is_empty() {
        PathBuf::from(MAIN_SEPARATOR_STR)
    } else {
        home
    };
    if home.as_os_str().as_encoded_bytes().first() == Some(&b'~') {
        return Err(NO_HOME);
    }
    Ok(home.join(rest.as_path()))
}

/// One step of [`realpath`]'s walk.
enum Part {
    /// A path component to append and resolve.
    Name(OsString),
    /// `..`: drop the last component of the path resolved so far.
    Parent,
    /// The target of this symbolic link has been walked: the resolved path is its value.
    Resolved(PathBuf),
}

/// Python 3.13's `posixpath.realpath(path, strict=False)` for an absolute `path`, over
/// [`Component`]s so that it also compiles for Windows paths (where the prototype used the
/// different `ntpath.realpath`).
///
/// Every component is looked up with `lstat`. A symbolic link is replaced by its target's parts
/// (an absolute target restarts from the root); a link met again while its own target is still
/// being walked is a loop and stays as it is; errors leave the component unresolved.
fn realpath(path: &Path) -> PathBuf {
    let mut resolved = PathBuf::new();
    let mut pending = Vec::new();
    push_parts(path, &mut resolved, &mut pending);
    // Each link seen so far, with its resolved value once its target has been walked.
    let mut seen: HashMap<PathBuf, Option<PathBuf>> = HashMap::new();
    while let Some(part) = pending.pop() {
        let name = match part {
            Part::Resolved(link) => {
                seen.insert(link, Some(resolved.clone()));
                continue;
            }
            Part::Parent => {
                resolved.pop();
                continue;
            }
            Part::Name(name) => name,
        };
        let candidate = resolved.join(&name);
        let is_link =
            fs::symlink_metadata(&candidate).is_ok_and(|meta| meta.file_type().is_symlink());
        if !is_link {
            resolved = candidate;
            continue;
        }
        match seen.get(&candidate) {
            Some(Some(value)) => {
                resolved.clone_from(value);
                continue;
            }
            Some(None) => {
                // A symlink loop: keep the link itself.
                resolved = candidate;
                continue;
            }
            None => {}
        }
        match fs::read_link(&candidate) {
            Ok(target) => {
                seen.insert(candidate.clone(), None);
                pending.push(Part::Resolved(candidate));
                push_parts(&target, &mut resolved, &mut pending);
            }
            Err(_) => resolved = candidate,
        }
    }
    resolved
}

/// Pushes the parts of `path` onto `pending` (last part at the bottom); a rooted `path`
/// restarts `resolved` at its root.
fn push_parts(path: &Path, resolved: &mut PathBuf, pending: &mut Vec<Part>) {
    let mut root: Option<PathBuf> = None;
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => root = Some(PathBuf::from(prefix.as_os_str())),
            Component::RootDir => {
                let mut start = root.take().unwrap_or_else(|| drive_of(resolved));
                start.push(MAIN_SEPARATOR_STR);
                root = Some(start);
            }
            Component::CurDir => {}
            Component::ParentDir => parts.push(Part::Parent),
            Component::Normal(name) => parts.push(Part::Name(name.to_owned())),
        }
    }
    if let Some(root) = root {
        *resolved = root;
    }
    pending.extend(parts.into_iter().rev());
}

/// The Windows drive prefix of `path` (empty elsewhere), which a rooted link target keeps.
fn drive_of(path: &Path) -> PathBuf {
    match path.components().next() {
        Some(Component::Prefix(prefix)) => PathBuf::from(prefix.as_os_str()),
        _ => PathBuf::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{Access, ENV_ALLOWED, HINT, PathPolicy, expand_user, realpath};

    #[test]
    fn the_hint_names_the_environment_variable() {
        assert!(HINT.ends_with(&format!("${ENV_ALLOWED})")));
        assert_eq!(Access::Write.to_string(), "write");
    }

    #[cfg(unix)]
    #[test]
    fn missing_paths_resolve_lexically() {
        let path = Path::new("/sonara-no-such-dir/a/../b/./c");
        assert_eq!(realpath(path), PathBuf::from("/sonara-no-such-dir/b/c"));
        assert_eq!(realpath(Path::new("/../..")), PathBuf::from("/"));
    }

    #[test]
    fn only_a_leading_tilde_component_expands() {
        assert_eq!(
            expand_user(Path::new("sub/~/x")),
            Ok(PathBuf::from("sub/~/x"))
        );
        assert_eq!(expand_user(Path::new("~x.wav")), Err(super::NO_HOME));
        let home = std::env::home_dir().expect("the test environment has a home directory");
        assert_eq!(expand_user(Path::new("./~/a")), Ok(home.join("a")));
    }

    #[test]
    fn a_nul_character_cannot_be_resolved() {
        let policy = PathPolicy::new(["/"], Path::new("/"));
        let err = policy.resolve("a\0b", Access::Read).expect_err("NUL");
        assert_eq!(
            err.message,
            "cannot resolve a\0b: embedded null character in path"
        );
        assert!(err.details.is_empty());
    }
}
