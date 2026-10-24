//! The command line (`build_parser` of `sonara.cli.main` and `main` of
//! `sonara.agent.mcp_server`): every command, argument, default, choice and help text of the
//! prototype's argparse trees (golden `contracts/cli.json`; CTR-01).
//!
//! The `clap` types stay private to the crate: their doc comments would become help texts, which
//! the contract fixes. [`command`] and [`mcp_command`] expose the trees for inspection.
//!
//! How the argparse trees map onto `clap`:
//!
//! - the parser's `description` is the command's `about`; a subcommand's `help` (its line in
//!   the parent's listing) is the subcommand's `about`, and its `description` (`None` for every
//!   subcommand) would be its `long_about`;
//! - `store` options are `Option<T>` or carry a default, `store_true` flags are `bool`, `append`
//!   options are `Vec<T>` (`default=[]`) or `Option<Vec<T>>` (`default=None`);
//! - `choices` are possible values; typed choices (`--bit-depth`) parse to their type;
//! - `nargs="?"` positionals are optional, `argparse.REMAINDER` takes every remaining argument;
//! - `--version` is the prototype's: long form only, printing `sonara <version>`; there is no
//!   `help` subcommand.

use std::ffi::OsString;
use std::path::PathBuf;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{ArgAction, Args, CommandFactory, Parser, Subcommand};

/// The audio backends of `--audio`.
const AUDIO: [&str; 3] = ["auto", "native", "null"];
/// The attach modes of `--connect`.
const CONNECT: [&str; 3] = ["auto", "always", "never"];
/// The export formats of `render --format`.
const FORMATS: [&str; 7] = ["wav", "bwf", "aiff", "flac", "mp3", "ogg", "opus"];
/// The normalization modes of `render --normalize`.
const NORMALIZE: [&str; 4] = ["none", "peak", "true_peak", "loudness"];

/// `--bit-depth`: `type=int, choices=[16, 24, 32]`.
fn bit_depth() -> impl TypedValueParser<Value = i64> {
    PossibleValuesParser::new(["16", "24", "32"]).try_map(|text| text.parse::<i64>())
}

/// `sonara` (`build_parser`).
#[derive(Debug, Parser)]
#[command(
    name = "sonara",
    about = "Sonara digital audio workstation",
    version,
    disable_version_flag = true,
    disable_help_subcommand = true,
    subcommand_required = true
)]
pub(crate) struct Cli {
    #[arg(
        long = "version",
        action = ArgAction::Version,
        help = "show program's version number and exit"
    )]
    pub(crate) version: Option<bool>,
    #[arg(long = "log-level", default_value = "WARNING")]
    pub(crate) log_level: String,
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// The commands, in the prototype's order.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    #[command(about = "launch the desktop application")]
    Gui { project: Option<PathBuf> },
    #[command(about = "headless session with IPC server")]
    Serve(ServeArgs),
    #[command(about = "MCP stdio server")]
    Mcp {
        // `nargs=argparse.REMAINDER`: one `store` of every remaining argument.
        #[arg(
            action = ArgAction::Set,
            trailing_var_arg = true,
            allow_hyphen_values = true,
            num_args = 0..
        )]
        rest: Vec<OsString>,
    },
    #[command(about = "list tools")]
    Tools { name: Option<String> },
    #[command(about = "call a tool")]
    Tool(ToolArgs),
    #[command(about = "run tool steps from a JSON file")]
    Batch(BatchArgs),
    #[command(about = "export a project")]
    Render(RenderArgs),
    #[command(about = "inspect an audio file or project")]
    Info { file: PathBuf },
    #[command(about = "analyze an audio file")]
    Analyze {
        file: PathBuf,
        #[arg(long)]
        series: bool,
    },
    #[command(about = "list audio devices")]
    Devices,
    #[command(about = "check project integrity")]
    Verify {
        project: PathBuf,
        #[arg(long, help = "skip hash verification")]
        quick: bool,
    },
}

/// `sonara serve`.
#[derive(Debug, Args)]
pub(crate) struct ServeArgs {
    #[arg(long)]
    pub(crate) project: Option<PathBuf>,
    #[arg(long, value_parser = AUDIO)]
    pub(crate) audio: Option<String>,
    #[arg(long, action = ArgAction::Append)]
    pub(crate) allow: Vec<PathBuf>,
}

/// `sonara tool`.
#[derive(Debug, Args)]
pub(crate) struct ToolArgs {
    pub(crate) name: String,
    #[arg(help = "JSON object with arguments")]
    pub(crate) json: Option<String>,
    #[arg(short = 'a', long = "arg", action = ArgAction::Append, help = "key=value (JSON values allowed)")]
    pub(crate) arg: Vec<String>,
    #[arg(long, help = "open this project first (in-process mode)")]
    pub(crate) project: Option<String>,
    #[arg(long, help = "save the project afterwards")]
    pub(crate) save: bool,
    #[command(flatten)]
    pub(crate) session: SessionArgs,
}

/// `sonara batch`.
#[derive(Debug, Args)]
pub(crate) struct BatchArgs {
    pub(crate) file: PathBuf,
    #[arg(long = "keep-going")]
    pub(crate) keep_going: bool,
    #[arg(
        long,
        help = "write every step's full result envelope to this JSON file"
    )]
    pub(crate) results: Option<PathBuf>,
    #[command(flatten)]
    pub(crate) session: SessionArgs,
}

/// The session options `tool` and `batch` share.
#[derive(Debug, Args)]
pub(crate) struct SessionArgs {
    #[arg(long, default_value = "auto", value_parser = CONNECT)]
    pub(crate) connect: String,
    #[arg(long, default_value = "null", value_parser = AUDIO)]
    pub(crate) audio: String,
    #[arg(long, action = ArgAction::Append)]
    pub(crate) allow: Vec<PathBuf>,
}

/// `sonara render`.
#[derive(Debug, Args)]
pub(crate) struct RenderArgs {
    pub(crate) project: PathBuf,
    #[arg(short = 'o', long)]
    pub(crate) output: PathBuf,
    // `default=None`; the prototype's `main` fills in `["wav"]` afterwards.
    #[arg(short = 'f', long, action = ArgAction::Append, value_parser = FORMATS)]
    pub(crate) format: Option<Vec<String>>,
    #[arg(long = "bit-depth", default_value = "24", value_parser = bit_depth())]
    pub(crate) bit_depth: i64,
    #[arg(long, default_value = "none", value_parser = NORMALIZE)]
    pub(crate) normalize: String,
    #[arg(long, default_value = "-14.0", allow_negative_numbers = true)]
    pub(crate) target: f64,
    #[arg(long)]
    pub(crate) overwrite: bool,
}

/// `sonara-mcp` (`sonara mcp REST...` parses `REST` with it).
#[derive(Debug, Parser)]
#[command(
    name = "sonara-mcp",
    about = "Sonara MCP server (stdio)",
    disable_help_subcommand = true
)]
pub(crate) struct McpCli {
    #[arg(
        long,
        default_value = "auto",
        value_parser = CONNECT,
        help = "use a running Sonara session (auto: if one is found)"
    )]
    pub(crate) connect: String,
    #[arg(long, action = ArgAction::Append, help = "additional allowed path (repeatable)")]
    pub(crate) allow: Vec<PathBuf>,
    // `default=Path.cwd()`, read when the server starts.
    #[arg(long = "base-dir", help = "directory relative paths resolve against")]
    pub(crate) base_dir: Option<PathBuf>,
    #[arg(long, value_parser = AUDIO, help = "audio backend for headless mode")]
    pub(crate) audio: Option<String>,
    #[arg(long = "log-level", default_value = "WARNING")]
    pub(crate) log_level: String,
}

/// The `sonara` command tree.
#[must_use]
pub fn command() -> clap::Command {
    Cli::command()
}

/// The `sonara-mcp` command tree, which parses the arguments after `sonara mcp`.
#[must_use]
pub fn mcp_command() -> clap::Command {
    McpCli::command()
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::{Cli, Command, McpCli};

    #[test]
    fn the_trees_are_consistent() {
        Cli::command().debug_assert();
        McpCli::command().debug_assert();
    }

    #[test]
    fn remainder_and_typed_values_parse_like_argparse() {
        let cli =
            Cli::try_parse_from(["sonara", "mcp", "--connect", "never", "--allow", "/x"]).unwrap();
        let Command::Mcp { rest } = cli.command else {
            panic!("mcp");
        };
        assert_eq!(rest, ["--connect", "never", "--allow", "/x"]);
        let cli = Cli::try_parse_from([
            "sonara",
            "render",
            "p.sonara",
            "-o",
            "out.wav",
            "--target",
            "-9",
            "--bit-depth",
            "16",
            "-f",
            "flac",
            "-f",
            "wav",
        ])
        .unwrap();
        let Command::Render(render) = cli.command else {
            panic!("render");
        };
        assert_eq!(render.target, -9.0);
        assert_eq!(render.bit_depth, 16);
        assert_eq!(
            render.format,
            Some(vec!["flac".to_owned(), "wav".to_owned()])
        );
        assert!(
            Cli::try_parse_from(["sonara", "render", "p", "-o", "o", "--bit-depth", "20"]).is_err()
        );
        assert!(Cli::try_parse_from(["sonara"]).is_err());
        assert!(Cli::try_parse_from(["sonara", "help"]).is_err());
    }
}
