# ADR 0005: One tool registry for Python, CLI, IPC and MCP

- **Status:** Accepted (2026-10-20)
- **Update (2026-10-24):** the Rust application has no Python API. The CLI, IPC and MCP share
  the one registry in `sonara-agent`, with the same tools, schemas and protocol.

## Context

Claude Code must operate Sonara through MCP. The same operations must be available from
Python and from a structured CLI, and an agent working while the GUI is open must edit the
GUI's project, not a copy of it.

## Decision

- `sonara.agent.tools` defines a **tool registry**. Each tool has a name, a Pydantic input
  model (from which the JSON Schema is generated), a result model, a safety class (`read`,
  `write`, `destructive`, `filesystem-write`) and a handler. The handler calls application
  services and commands only. It never touches widgets.
- **Python API** (`sonara.agent.api.SonaraClient`) calls the registry in-process or through
  IPC.
- **CLI** (`sonara tool <name> --json '{...}'`, `sonara tools`, plus convenience
  subcommands) prints the structured result as JSON.
- **IPC.** A running Sonara process (GUI or `sonara serve`) exposes the registry over a local
  stream socket using newline-delimited JSON-RPC 2.0:
  - Unix domain socket on macOS and Linux, created inside a `0700` runtime directory.
  - TCP bound to `127.0.0.1` on Windows.
  - Every connection must first send `hello` with a random 256-bit token. The token is read
    from the runtime discovery file, which has mode `0600`.
  - The protocol version is checked in `hello`.
  - Per-request timeouts apply, and long operations become jobs that are polled with
    `get_operation_status`.
- **MCP server** (`sonara-mcp`, stdio). On startup it looks for a running Sonara session
  through the discovery file. If it finds one, it forwards tool calls over IPC. Otherwise it
  hosts a headless in-process session. Either way, the same registry handles the call, so
  GUI and agent edits share one undo stack.
- **Safety.**
  - File arguments are resolved and checked against allowed roots. Defaults are the Sonara
    projects directory, the MCP server's working directory and `SONARA_AGENT_ALLOWED_PATHS`.
  - Destructive tools (deleting tracks with content, overwriting files, discarding unsaved
    changes) use two-phase confirmation: the first call returns `CONFIRMATION_REQUIRED` with
    a description and a single-use token, and the call is repeated with the token.
  - Every mutating tool supports `dry_run`.
  - Render length, note counts and concurrent jobs are capped.
  - All calls are appended to a JSONL audit log without secrets.

## Consequences

The registry is the contract that is tested once and then exercised through all four
transports. Adding a tool makes it available everywhere at once.
