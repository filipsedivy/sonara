//! The product's identity and the Help dialogs of an interface: About
//! Sonara (DL-06: [`about`]) and Agent Connection (DL-03: [`agent_connection`]), as the
//! prototype's `sonara.about`, `AboutDialog` and `AgentInfoDialog` word them.
//!
//! **About.** The version is the workspace's (VER-01); the system rows name the components of
//! this application instead of Python, Qt and `PySide6`, whose lines are not ported.
//! The credits list the main components shipped with Sonara. The full licence texts are
//! generated when the bundle is built (`apps/sonara-desktop/scripts/stage-licenses.mjs`, with
//! `cargo-about`) into the bundle's licences folder, whose joined text is
//! [`LICENSES_FILE`]. Without a bundle (a development build), [`About::licenses`] is `None` and
//! the dialog says where they will come from.
//!
//! **Agent Connection.** The prototype told the user to register `uv run --project <repo>
//! sonara-mcp`. The desktop bundles the `sonara` CLI next to its own executable
//! (`Sonara.app/Contents/MacOS/sonara`), so the registration names that
//! file: `claude mcp add sonara -- <bundle>/Contents/MacOS/sonara mcp`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// The product's name.
pub const PRODUCT: &str = "Sonara";
/// The version (the workspace's).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The release stage.
pub const STAGE: &str = "alpha";
/// The tagline.
pub const TAGLINE: &str = "A digital audio workstation with a first-class agent interface.";
/// The author.
pub const AUTHOR: &str = "Filip Sedivy";
/// The author's address, a `mailto:` link in the dialog.
pub const AUTHOR_EMAIL: &str = "mail@filipsedivy.cz";
/// The copyright line.
pub const COPYRIGHT: &str = "© 2026 Filip Sedivy. Free software under the 0BSD licence.";
/// The generated licence texts, relative to the bundle's resources
/// (`Sonara.app/Contents/Resources`): the plain-text file that joins every file of the
/// `licenses` folder.
pub const LICENSES_FILE: &str = "licenses/THIRD-PARTY-LICENSES.txt";

/// Main components shipped with Sonara: name, licence, home page (`docs/dependencies.md` has
/// every one).
const CREDITS: [(&str, &str, &str); 12] = [
    ("Tauri", "Apache-2.0 or MIT", "https://tauri.app"),
    ("Svelte", "MIT", "https://svelte.dev"),
    ("miniaudio", "Public domain or MIT-0", "https://miniaud.io"),
    ("midir", "MIT", "https://github.com/Boddlnagg/midir"),
    (
        "Symphonia",
        "MPL-2.0",
        "https://github.com/pdeljanov/Symphonia",
    ),
    ("LAME", "LGPL-2.0-or-later", "https://lame.sourceforge.io"),
    (
        "libsoxr",
        "LGPL-2.1-or-later",
        "https://sourceforge.net/projects/soxr/",
    ),
    ("libFLAC", "BSD-3-Clause", "https://xiph.org/flac/"),
    ("Ogg, Vorbis, Opus", "BSD-3-Clause", "https://xiph.org"),
    (
        "RustFFT, RealFFT",
        "MIT or Apache-2.0",
        "https://github.com/ejmahler/RustFFT",
    ),
    ("Serde", "MIT or Apache-2.0", "https://serde.rs"),
    (
        "Model Context Protocol Rust SDK",
        "Apache-2.0",
        "https://github.com/modelcontextprotocol/rust-sdk",
    ),
];

/// The note under the credits: the LGPL libraries are separate and replaceable.
pub const LGPL_NOTE: &str = "LGPL components (LAME, libsoxr) are separate dynamic libraries \
                             that Sonara loads at run time and that can be replaced; their \
                             source code is available from the projects linked above.";

/// One credited component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Credit {
    /// Its name.
    pub name: &'static str,
    /// Its licence.
    pub license: &'static str,
    /// Its home page.
    pub url: &'static str,
}

/// What About Sonara shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct About {
    /// "Sonara".
    pub product: &'static str,
    /// "Version 0.1.0 (alpha)".
    pub version: String,
    /// The tagline.
    pub tagline: &'static str,
    /// The author.
    pub author: &'static str,
    /// The author's address.
    pub email: &'static str,
    /// The copyright line.
    pub copyright: &'static str,
    /// The System tab's rows, in order; "Copy System Info" copies them as `key: value` lines.
    pub system: Vec<(String, String)>,
    /// The Credits tab's components.
    pub credits: Vec<Credit>,
    /// The note under them.
    pub lgpl_note: &'static str,
    /// The generated licence texts, once the bundle carries them.
    pub licenses: Option<String>,
}

/// About Sonara: the identity, the system rows (`environment()`, then `extra`, for example the
/// audio engine's state and the agent IPC address), the credits, and the licence texts of
/// `licenses` when that file can be read.
#[must_use]
pub fn about(extra: Vec<(String, String)>, licenses: Option<&Path>) -> About {
    let mut system = environment();
    system.extend(extra);
    About {
        product: PRODUCT,
        version: format!("Version {VERSION} ({STAGE})"),
        tagline: TAGLINE,
        author: AUTHOR,
        email: AUTHOR_EMAIL,
        copyright: COPYRIGHT,
        system,
        credits: CREDITS
            .iter()
            .map(|&(name, license, url)| Credit { name, license, url })
            .collect(),
        lgpl_note: LGPL_NOTE,
        licenses: licenses.and_then(|path| fs::read_to_string(path).ok()),
    }
}

/// Whether About may open `url` in the system's applications: the author's `mailto:` address
/// or a credited component's page (TYP-06: the page names no other link).
#[must_use]
pub fn is_known_link(url: &str) -> bool {
    url.strip_prefix("mailto:") == Some(AUTHOR_EMAIL)
        || CREDITS.iter().any(|credit| credit.2 == url)
}

/// The runtime versions for bug reports (`about.environment()`): the product, the platform, the
/// audio engine and the MP3 encoder.
#[must_use]
pub fn environment() -> Vec<(String, String)> {
    let mp3 = sonara_media::export::mp3_encoder_version()
        .map_or_else(|_| "not available".to_owned(), |v| format!("LAME {v}"));
    vec![
        (PRODUCT.to_owned(), format!("{VERSION} ({STAGE})")),
        (
            "Platform".to_owned(),
            format!("{} ({})", std::env::consts::OS, std::env::consts::ARCH),
        ),
        (
            "Audio engine".to_owned(),
            format!("miniaudio {}", sonara_io::miniaudio_version()),
        ),
        ("MP3 encoder".to_owned(), mp3),
    ]
}

/// The System tab as "Copy System Info" copies it: one `key: value` line per row.
#[must_use]
pub fn system_text(rows: &[(String, String)]) -> String {
    rows.iter()
        .map(|(key, value)| format!("{key}: {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// What Agent Connection shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentConnection {
    /// "This window accepts agent connections at …", or that the server is disabled.
    pub status: String,
    /// The IPC address, when agents can attach.
    pub address: Option<String>,
    /// The bundled CLI: `<bundle>/Contents/MacOS/sonara`.
    pub cli: String,
    /// The registration command.
    pub command: String,
    /// The text of the dialog's box.
    pub text: String,
    /// The discovery file, when agents can attach.
    pub discovery: Option<String>,
}

/// The `sonara` CLI next to `executable` (the desktop's own executable): in the bundle,
/// `Sonara.app/Contents/MacOS/sonara`.
#[must_use]
pub fn bundled_cli(executable: &Path) -> PathBuf {
    executable
        .parent()
        .map_or_else(|| PathBuf::from("sonara"), |dir| dir.join("sonara"))
}

/// Agent Connection for a window whose agents attach at `address` (with the discovery file
/// `discovery`), or that accepts none, registering the CLI `cli`.
#[must_use]
pub fn agent_connection(
    address: Option<&str>,
    discovery: Option<&str>,
    cli: &Path,
) -> AgentConnection {
    let status = address.map_or_else(
        || "The agent IPC server is disabled for this window.".to_owned(),
        |address| format!("This window accepts agent connections at {address}."),
    );
    let cli = cli.display().to_string();
    let command = format!("claude mcp add sonara -- {} mcp", shell_word(&cli));
    let text = format!(
        "Register Sonara with Claude Code (once):\n\n    {command}\n\n\
         The MCP server connects to this running window automatically, so the agent edits\n\
         the project you see here (shared undo history). Without a running window it\n\
         hosts a headless session instead.\n\n\
         Discovery file: {}\n",
        discovery.unwrap_or("—")
    );
    AgentConnection {
        status,
        address: address.map(str::to_owned),
        cli,
        command,
        text,
        discovery: discovery.map(str::to_owned),
    }
}

/// `word` as one shell word: quoted when it holds anything but a plain path's characters.
fn shell_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-+:@".contains(c));
    if plain {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{agent_connection, bundled_cli};

    #[test]
    fn the_registration_names_the_bundled_cli() {
        let cli = bundled_cli(Path::new(
            "/Applications/Sonara.app/Contents/MacOS/sonara-desktop",
        ));
        assert_eq!(
            cli,
            Path::new("/Applications/Sonara.app/Contents/MacOS/sonara")
        );
        let shown = agent_connection(Some("/tmp/s.sock"), Some("/tmp/run/gui.json"), &cli);
        assert_eq!(
            shown.command,
            "claude mcp add sonara -- /Applications/Sonara.app/Contents/MacOS/sonara mcp"
        );
        assert!(shown.text.contains("Discovery file: /tmp/run/gui.json"));
        assert_eq!(
            shown.status,
            "This window accepts agent connections at /tmp/s.sock."
        );
        let spaced = agent_connection(None, None, Path::new("/Volumes/My Apps/sonara"));
        assert_eq!(
            spaced.command,
            "claude mcp add sonara -- '/Volumes/My Apps/sonara' mcp"
        );
        assert_eq!(
            spaced.status,
            "The agent IPC server is disabled for this window."
        );
        assert!(spaced.text.contains("Discovery file: —"));
    }

    #[test]
    fn only_the_authors_address_and_the_credits_open() {
        assert!(super::is_known_link("mailto:mail@filipsedivy.cz"));
        assert!(super::is_known_link("https://tauri.app"));
        assert!(!super::is_known_link("https://example.com"));
        assert!(!super::is_known_link("file:///etc/passwd"));
        assert!(!super::is_known_link("mailto:someone@else.cz"));
    }
}
