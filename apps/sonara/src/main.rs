//! The `sonara` command: the CLI, the MCP server and the application entry point.
//! The commands live in the library (`sonara::main`).
#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    sonara::main(std::env::args_os())
}
