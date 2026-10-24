//! The `sonara` command (`sonara.cli.main`; and
//! 8): the tools of the agent interface from the command line, batches of tool calls, project and
//! audio inspection, and the MCP server.
//!
//! The commands, arguments and JSON output are a contract (CTR-01): [`cli::command`] reproduces
//! the prototype's argparse tree (golden `contracts/cli.json`), and every command prints what the
//! prototype printed, formatted as Python's `json.dumps(obj, indent=2)`.
//!
//! The binary is a thin wrapper around [`main`]; the library exposes the command tree and
//! [`batch::run_batch`] for the golden tests.
#![forbid(unsafe_code)]

pub mod batch;
pub mod cli;
mod commands;
mod logging;

use std::ffi::OsString;
use std::process::ExitCode;

use clap::FromArgMatches as _;

/// Runs the `sonara` command with `args` (the program name first) and returns its exit code:
/// 0 on success, 1 for a failed tool or command, 2 for a usage error.
pub fn main<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let parsed = cli::command()
        .try_get_matches_from(args)
        .and_then(|matches| cli::Cli::from_arg_matches(&matches));
    let cli = match parsed {
        Ok(cli) => cli,
        Err(error) => {
            // `--help` and `--version` exit with 0, usage errors with 2 (as `argparse` does).
            let code = error.exit_code();
            // Nothing is left to report to when the terminal fails.
            let _ = error.print();
            return ExitCode::from(u8::try_from(code).unwrap_or(2));
        }
    };
    let cli::Cli {
        version: _,
        log_level,
        command,
    } = cli;
    if let Err(message) = logging::init(&log_level) {
        return commands::Failure::Fatal(message).report();
    }
    commands::run(command).unwrap_or_else(commands::Failure::report)
}
