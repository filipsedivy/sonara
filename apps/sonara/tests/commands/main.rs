//! The `sonara` binary end to end: each test runs the built binary in a sandbox.
//!
//! - `cli`: the port of the prototype's `tests/agent/test_cli.py`;
//! - `inspect`: `info`, `analyze`, `verify` and `devices` against golden media and analysis data;
//! - `workflows`: `batch` on every `examples/` workflow against the golden transcripts;
//! - `mcp_stdio`: the port of `tests/agent/test_mcp_stdio.py`, a real MCP round trip on stdio;
//! - `attach`: the port of `tests/agent/test_mcp_attach.py`: `sonara serve`, and the MCP server
//!   and the CLI attached to it over IPC;
//! - `gui`: `sonara gui` starts the desktop application next to it.

#[cfg(unix)]
mod attach;
mod cli;
#[cfg(unix)]
mod gui;
mod inspect;
mod mcp_stdio;
mod support;
mod workflows;
