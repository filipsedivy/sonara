//! The launch options of `sonara-desktop` (the prototype's `sonara-gui` parser).
//!
//! The owner kept `--audio`, `--no-ipc` and `--log-level` and dropped `--template`,
//! `--screenshot` and `--smoke-play`. None of them is in the CLI contract:
//! `sonara gui [PROJECT]` passes the project only (golden `contracts/cli.json`).

use std::path::PathBuf;

use clap::Parser;

/// The backends of `--audio`: the prototype's `sonara-gui` choices.
const AUDIO: [&str; 4] = ["auto", "native", "native-null", "null"];

/// `sonara-desktop [PROJECT] [--audio BACKEND] [--no-ipc] [--log-level LEVEL]`.
#[derive(Debug, Parser)]
#[command(
    name = "sonara-desktop",
    about = "The Sonara desktop application",
    version
)]
pub struct Options {
    /// Project to open.
    pub project: Option<PathBuf>,
    /// Audio backend, instead of the one in the settings.
    #[arg(long, value_parser = AUDIO)]
    pub audio: Option<String>,
    /// Do not accept agent connections.
    #[arg(long)]
    pub no_ipc: bool,
    /// Python's logging level names: DEBUG, INFO, WARNING, ERROR, CRITICAL.
    #[arg(long, default_value = "INFO")]
    pub log_level: String,
}
