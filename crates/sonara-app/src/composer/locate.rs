//! Finding the Claude Code CLI and checking its version.
//!
//! A Finder-launched application has the minimal `PATH` of `launchd`, which holds neither
//! `/opt/homebrew/bin` nor `~/.local/bin`, so the search looks at the `PATH` and then at the
//! places where Claude Code installs itself.

use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;

/// The oldest Claude Code that has every flag the composer passes (`--permission-prompts`, from
/// 2.1.259).
pub const MIN_VERSION: Version = Version {
    major: 2,
    minor: 1,
    patch: 259,
};

/// A `major.minor.patch` version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Version {
    /// The major number.
    pub major: u32,
    /// The minor number.
    pub minor: u32,
    /// The patch number.
    pub patch: u32,
}

impl Version {
    /// The version at the start of `output`, as `claude --version` prints it
    /// (`2.1.312 (Claude Code)`); `None` when it does not start with three numbers.
    #[must_use]
    pub fn parse(output: &str) -> Option<Self> {
        let token = output.split_whitespace().next()?;
        let mut numbers = token.split('.').map(str::parse::<u32>);
        let major = numbers.next()?.ok()?;
        let minor = numbers.next()?.ok()?;
        let patch = numbers.next()?.ok()?;
        numbers.next().is_none().then_some(Self {
            major,
            minor,
            patch,
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A usable installation of an agent CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Located {
    /// The executable.
    pub path: PathBuf,
    /// Its version.
    pub version: Version,
}

/// A usable Claude Code installation.
pub type ClaudeCli = Located;

/// Why no Claude Code can be used.
#[derive(Debug, thiserror::Error)]
pub enum LocateError {
    /// There is no `claude` executable in any searched place.
    #[error("Claude Code is not installed (looked for `claude` in {} places)", .searched.len())]
    NotFound {
        /// The places that were searched.
        searched: Vec<PathBuf>,
    },
    /// The executable does not answer `--version` with a version.
    #[error("{} does not report a Claude Code version: {reason}", .path.display())]
    Unusable {
        /// The executable.
        path: PathBuf,
        /// What went wrong.
        reason: String,
    },
    /// The installed Claude Code lacks a flag the composer needs.
    #[error("Claude Code {found} at {} is older than the required {required}", .path.display())]
    TooOld {
        /// The executable.
        path: PathBuf,
        /// Its version.
        found: Version,
        /// The oldest supported version.
        required: Version,
    },
}

/// The places to look for `claude`, in order and without repeats: the directories of `path_var`,
/// then the installer's usual places under `home` and the package managers' prefixes.
#[must_use]
pub fn candidates(home: Option<&Path>, path_var: Option<&OsStr>) -> Vec<PathBuf> {
    candidates_named(
        "claude",
        &[".local/bin", ".claude/local", ".npm-global/bin"],
        home,
        path_var,
    )
}

/// The places to look for the executable `binary`, in order and without repeats: the
/// directories of `path_var`, then each of `home_dirs` (relative to `home`), then Homebrew's
/// and `/usr/local`'s `bin`.
#[must_use]
pub fn candidates_named(
    binary: &str,
    home_dirs: &[&str],
    home: Option<&Path>,
    path_var: Option<&OsStr>,
) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut add = |path: PathBuf| {
        if !found.contains(&path) {
            found.push(path);
        }
    };
    if let Some(path_var) = path_var {
        for directory in std::env::split_paths(path_var) {
            add(directory.join(binary));
        }
    }
    if let Some(home) = home {
        for directory in home_dirs {
            add(home.join(directory).join(binary));
        }
    }
    add(PathBuf::from("/opt/homebrew/bin").join(binary));
    add(PathBuf::from("/usr/local/bin").join(binary));
    found
}

/// The first of `places` that exists and answers `probe` (the output of `--version`) with a
/// version of at least [`MIN_VERSION`].
///
/// # Errors
///
/// [`LocateError::NotFound`] when none exists; [`LocateError::Unusable`] or
/// [`LocateError::TooOld`] for the first one that exists but cannot be used.
pub fn locate_in(
    places: &[PathBuf],
    exists: impl Fn(&Path) -> bool,
    probe: impl Fn(&Path) -> io::Result<String>,
) -> Result<ClaudeCli, LocateError> {
    locate_named_in(places, MIN_VERSION, exists, probe)
}

/// [`locate_in`] for any agent CLI: the first of `places` that exists and answers `probe` with
/// a version of at least `min_version`.
///
/// The messages of the [`LocateError`]s name Claude Code; a caller for another provider
/// matches the variant and words its own message.
///
/// # Errors
///
/// As [`locate_in`].
pub fn locate_named_in(
    places: &[PathBuf],
    min_version: Version,
    exists: impl Fn(&Path) -> bool,
    probe: impl Fn(&Path) -> io::Result<String>,
) -> Result<Located, LocateError> {
    let Some(path) = places.iter().find(|place| exists(place)) else {
        return Err(LocateError::NotFound {
            searched: places.to_vec(),
        });
    };
    let output = probe(path).map_err(|error| LocateError::Unusable {
        path: path.clone(),
        reason: error.to_string(),
    })?;
    let version = Version::parse(&output).ok_or_else(|| LocateError::Unusable {
        path: path.clone(),
        reason: format!("`--version` printed {:?}", output.trim()),
    })?;
    if version < min_version {
        return Err(LocateError::TooOld {
            path: path.clone(),
            found: version,
            required: min_version,
        });
    }
    Ok(Located {
        path: path.clone(),
        version,
    })
}

/// Finds Claude Code on this machine: the `PATH` and the usual installation places, then
/// `claude --version`. Runs a process, so call it off the UI thread.
///
/// # Errors
///
/// As [`locate_in`].
pub fn locate() -> Result<ClaudeCli, LocateError> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let path_var = std::env::var_os("PATH");
    let places = candidates(home.as_deref(), path_var.as_deref());
    locate_in(&places, Path::is_file, |path| {
        probe_version(path, path_var.as_deref())
    })
}

/// The output of `<program> --version`, run with the `PATH` of a composer run (the directory of
/// `program` first, then `path_var`, then the usual prefixes).
///
/// # Errors
///
/// The I/O error of starting the program, or one that says the exit status was not success.
pub fn probe_version(program: &Path, path_var: Option<&OsStr>) -> io::Result<String> {
    let output = Command::new(program)
        .arg("--version")
        .env("PATH", super::invocation::search_path(program, path_var))
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(io::Error::other(format!(
            "`--version` exited with {}",
            output.status
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(version: &'static str) -> impl Fn(&Path) -> io::Result<String> {
        move |_| Ok(version.to_owned())
    }

    #[test]
    fn parses_the_version_line_of_claude() {
        assert_eq!(
            Version::parse("2.1.312 (Claude Code)\n"),
            Some(Version {
                major: 2,
                minor: 1,
                patch: 312
            })
        );
        assert_eq!(Version::parse("2.1"), None);
        assert_eq!(Version::parse("2.1.3.4"), None);
        assert_eq!(Version::parse("claude 2.1.3"), None);
        assert_eq!(Version::parse(""), None);
    }

    #[test]
    fn versions_compare_by_number_not_by_text() {
        assert!(Version::parse("2.1.1000") > Version::parse("2.1.259"));
        assert!(Version::parse("2.0.999") < Version::parse("2.1.0"));
        assert_eq!(MIN_VERSION.to_string(), "2.1.259");
    }

    #[test]
    fn the_search_order_is_path_then_the_installers_places_without_repeats() {
        let places = candidates(
            Some(Path::new("/Users/a")),
            Some(OsStr::new("/opt/homebrew/bin:/usr/bin")),
        );
        assert_eq!(places[0], Path::new("/opt/homebrew/bin/claude"));
        assert_eq!(places[1], Path::new("/usr/bin/claude"));
        assert!(places.contains(&PathBuf::from("/Users/a/.local/bin/claude")));
        assert!(places.contains(&PathBuf::from("/Users/a/.claude/local/claude")));
        assert_eq!(
            places
                .iter()
                .filter(|p| **p == Path::new("/opt/homebrew/bin/claude"))
                .count(),
            1
        );
    }

    #[test]
    fn a_finder_launch_still_finds_claude_in_the_home_directory() {
        // The `PATH` of launchd has neither Homebrew nor `~/.local/bin`.
        let places = candidates(
            Some(Path::new("/Users/a")),
            Some(OsStr::new("/usr/bin:/bin")),
        );
        let found = locate_in(
            &places,
            |p| p == Path::new("/Users/a/.local/bin/claude"),
            ok("2.1.312 (Claude Code)"),
        )
        .unwrap();
        assert_eq!(found.path, Path::new("/Users/a/.local/bin/claude"));
        assert_eq!(found.version.to_string(), "2.1.312");
    }

    #[test]
    fn a_missing_cli_lists_where_it_looked() {
        let places = candidates(None, None);
        let error = locate_in(&places, |_| false, ok("")).unwrap_err();
        let LocateError::NotFound { searched } = error else {
            panic!("expected NotFound");
        };
        assert_eq!(searched, places);
    }

    #[test]
    fn an_old_or_garbled_version_is_refused_with_its_reason() {
        let places = vec![PathBuf::from("/x/claude")];
        assert!(matches!(
            locate_in(&places, |_| true, ok("2.1.100 (Claude Code)")),
            Err(LocateError::TooOld { .. })
        ));
        assert!(matches!(
            locate_in(&places, |_| true, ok("not a version")),
            Err(LocateError::Unusable { .. })
        ));
        assert!(matches!(
            locate_in(&places, |_| true, |_| Err(io::Error::other("boom"))),
            Err(LocateError::Unusable { .. })
        ));
    }
}

#[cfg(test)]
mod named_tests {
    use super::*;

    #[test]
    fn candidates_are_candidates_named_claude() {
        let home = Some(Path::new("/Users/a"));
        let path_var = Some(OsStr::new("/usr/bin:/opt/homebrew/bin"));
        assert_eq!(
            candidates(home, path_var),
            candidates_named(
                "claude",
                &[".local/bin", ".claude/local", ".npm-global/bin"],
                home,
                path_var
            )
        );
        // The order the Claude search always had.
        assert_eq!(
            candidates(home, None),
            [
                "/Users/a/.local/bin/claude",
                "/Users/a/.claude/local/claude",
                "/Users/a/.npm-global/bin/claude",
                "/opt/homebrew/bin/claude",
                "/usr/local/bin/claude",
            ]
            .map(PathBuf::from)
        );
    }

    #[test]
    fn another_binary_is_searched_by_its_own_name_without_repeats() {
        let places = candidates_named(
            "codex",
            &[".local/bin"],
            Some(Path::new("/Users/a")),
            Some(OsStr::new("/opt/homebrew/bin:/x/bin")),
        );
        assert_eq!(
            places,
            [
                "/opt/homebrew/bin/codex",
                "/x/bin/codex",
                "/Users/a/.local/bin/codex",
                "/usr/local/bin/codex",
            ]
            .map(PathBuf::from)
        );
    }

    #[test]
    fn a_named_search_uses_its_own_minimum_version() {
        let places = vec![PathBuf::from("/x/codex")];
        let min = Version::parse("0.100.0").unwrap();
        let ok = |_: &Path| Ok("0.166.0\n".to_owned());
        let found = locate_named_in(&places, min, |_| true, ok).unwrap();
        assert_eq!(found.version.to_string(), "0.166.0");
        let old = |_: &Path| Ok("0.99.9".to_owned());
        let Err(LocateError::TooOld {
            found, required, ..
        }) = locate_named_in(&places, min, |_| true, old)
        else {
            panic!("expected TooOld");
        };
        assert_eq!((found.to_string(), required), ("0.99.9".to_owned(), min));
        assert!(matches!(
            locate_named_in(&places, min, |_| false, ok),
            Err(LocateError::NotFound { .. })
        ));
    }

    #[test]
    fn the_real_probe_reports_a_failing_program() {
        let error = probe_version(Path::new("/usr/bin/false"), None).unwrap_err();
        assert!(error.to_string().contains("--version"), "{error}");
        assert!(probe_version(Path::new("/nonexistent/claude"), None).is_err());
    }
}
