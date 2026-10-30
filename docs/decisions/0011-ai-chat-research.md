# Research record for ADR 0011 (AI Chat provider bridge)

Backs [0011-ai-chat-provider-bridge.md](0011-ai-chat-provider-bridge.md). Research date:
2026-10-29 and 2026-10-30.

**Status labels.**

- **run**: observed by running the tool on the macOS development machine.
- **source**: read in the named source at the named tag or commit; not run.
- **docs**: read in vendor documentation, which was fetched through a summarising tool, so wording
  is not exact. Used only to corroborate.
- **assumption**: inferred, not checked.

**Tools and sources.**

| Tool | Version | Where it came from |
|---|---|---|
| Claude Code | 2.1.308 | installed (Homebrew cask, native arm64) |
| Codex CLI | 0.166.0 | installed (Homebrew cask, native arm64); source at tag `rust-v0.166.0`; npm and GitHub latest on 2026-10-30 |
| Gemini CLI | 0.66.0 (npm `latest`); `main` at b633c07 (2026-10-29, 0.67.0-nightly) | **not installed**; source only |
| `agent-client-protocol` | 3.6.0 (schema 1.13.0) | crates.io, built in a spike |
| `@agentclientprotocol/claude-agent-acp` | 0.99.1 | npm tarball read, not run |
| `@agentclientprotocol/codex-acp` | 2.5.0 | npm tarball read, not run |

## 1. Claude Code 2.1.308

| Claim | Status | Source and version |
|---|---|---|
| The composer's flags exist: `-p`, `--output-format stream-json`, `--verbose`, `--include-partial-messages`, `--strict-mcp-config`, `--mcp-config`, `--permission-mode`, `--permission-prompts`, `--allowedTools`, `--disallowedTools`, `--append-system-prompt` | run | `claude --help`, 2.1.308 |
| `--permission-mode` choices: `acceptEdits, auto, bypassPermissions, manual, dontAsk, plan`; `--permission-prompts` choices: `host, none` | run | free error probes with a bogus value, 2.1.308 |
| `--max-turns` is not in `--help` but is accepted (`argument 'abc' is invalid. must be a number`); an unknown flag gives `unknown option` | run | free error probes, 2.1.308 |
| `--no-session-persistence`: "sessions will not be saved to disk and cannot be resumed (only works with --print)" | run | `--help`, 2.1.308 |
| `--session-id notauuid` is rejected; `--resume <unknown uuid>` answers "No conversation found with session ID" | run | free probes, 2.1.308 |
| A successful `--resume` in `-p` mode, and resume across a different cwd | assumption (not run; no paid prompt was made) | |
| Sessions are keyed by cwd (`--continue`: "most recent conversation in the current directory") | assumption | `--help` wording only |
| `--system-prompt-snapshot` defaults to on: a resumed session reuses the first system prompt | run (`--help` text) | `--help`, 2.1.308 |
| `--restricted`, `--setting-sources`, `--bare` exist and could narrow isolation; not used | run (flags listed) | `--help`, 2.1.308 |
| The composer argv is `-p --output-format stream-json --verbose --include-partial-messages --strict-mcp-config --mcp-config <json> --permission-mode dontAsk --permission-prompts none --allowedTools mcp__sonara,WebSearch,WebFetch --disallowedTools <18 names> --max-turns 200 --append-system-prompt <text>`, prompt on stdin | source | `crates/sonara-app/src/composer/invocation.rs` |
| `DENIED_TOOLS` has 18 entries: `Bash`, `Edit`, `Write`, `NotebookEdit`, `AskUserQuestion` and 13 `mcp__sonara__*` tools (`save_project`, `open_project`, `create_project`, `export_audio`, `export_midi`, `export_stems`, `render_preview`, `relink_media`, `start_recording`, `set_midi_input`, `verify_project`, `analyze_audio`, `get_audio_analysis`) | source | `invocation.rs` |
| `WebSearch` and `WebFetch` are allowed on purpose | source | `ALLOWED_TOOLS` in `invocation.rs`; `docs/ai-chat.md` (the former `docs/ai-composer.md`) |
| `~/.claude` hooks and `CLAUDE.md` still load | source (documented by the module) | `invocation.rs` module docs; `docs/ai-chat.md` (the former `docs/ai-composer.md`) |
| Minimum version 2.1.259 | source (a repo constant; the reason given is `--permission-prompts`) | `locate.rs` `MIN_VERSION`; not re-bisected |
| `Run::stop` sends SIGINT to the pid, SIGTERM to the process group after 10 s, SIGKILL after 5 s more; the child leads its own process group | source | `crates/sonara-app/src/composer/run.rs` |
| SIGINT makes Claude emit a `result` and exit 130 | source (the module's documented rationale; tested with a fake `claude`, not a real one) | `run.rs` docs and tests |
| A Claude run that is not signed in is reported in its result, not probed beforehand | source | `docs/ai-chat.md` (the former `docs/ai-composer.md`) |
| `total_cost_usd` and token usage are in the `result` event | source | `composer/stream.rs` |

## 2. Codex CLI 0.166.0

### 2.1 JSONL event schema (`codex exec --json`, alias `--experimental-json`)

Source: `codex-rs/exec/src/exec_events.rs` and `event_processor_with_jsonl_output.rs` at
`rust-v0.166.0`. Stdout carries only JSONL; tracing goes to stderr (`exec/src/lib.rs` header).

| Event (`type`) | Fields | Status |
|---|---|---|
| `thread.started` | `thread_id` | source; seen in a run |
| `turn.started` | none | source; seen in a run |
| `turn.completed` | `usage`: `input_tokens`, `cached_input_tokens`, `cache_write_input_tokens` (default 0; absent before 0.145.0), `output_tokens`, `reasoning_output_tokens` (all integers) | source |
| `turn.failed` | `error.message` (falls back to the last error or "turn failed") | source |
| `item.started`, `item.updated`, `item.completed` | `item`: `{id: "item_N", type, ...}` | source; `item.completed` seen in a run |
| `error` (top level) | `message` | source; seen in a run (`Reconnecting... N/5`) |

| Item `type` | Fields | Notes |
|---|---|---|
| `agent_message` | `text` | arrives only as `item.completed` (no deltas) |
| `reasoning` | `text` | |
| `command_execution` | `command`, `aggregated_output`, `exit_code` (int or null), `status`: `in_progress`, `completed`, `failed`, `declined` | must not occur when the boundary holds |
| `file_change` | `changes`: `[{path, kind: add, delete, update}]`, `status`: `in_progress`, `completed`, `failed` | only emitted as completed or failed |
| `mcp_tool_call` | `server`, `tool`, `arguments`, `result` (`{content, structured_content, _meta?}` or null), `error` (`{message}` or null), `status`: `in_progress`, `completed`, `failed` | |
| `collab_tool_call` | `tool` (`spawn_agent`, `send_input`, `wait`, `close_agent`), `sender_thread_id`, `receiver_thread_ids`, `prompt`, `agents_states`, `status` | must not occur |
| `web_search` | `id`, `query`, `action`, `results?` | must not occur with `web_search="disabled"` |
| `todo_list` | `items`: `[{text, completed}]` | the only item with `item.updated` |
| `error` | `message` | non-fatal; warnings, config warnings and model reroutes arrive this way (seen in a run for an ignored config key and for a transport fallback) |

| Behaviour | Status | Source |
|---|---|---|
| No token deltas; text granularity is the whole message | source | `event_processor_with_jsonl_output.rs` |
| Top-level `error` is emitted for every server error notification, including retryable ones; the end signals are `turn.completed`, `turn.failed` and process exit | source; run (`Reconnecting... 2/5` through `5/5` and a fall back from WebSocket to HTTPS, in an unauthenticated run) | same file; `codex exec`, 0.166.0 |
| An interrupted turn emits `item.completed` for open items, then no `turn.completed` and no `turn.failed`; the final message is cleared (so `-o` is not written); exit code 1 | source | `event_processor_with_jsonl_output.rs`, `exec/src/lib.rs` |
| The unknown-field tolerance must be on our side: the schema grew across versions | assumption (design guidance) | `cache_write_input_tokens` added in 0.145.0 |

### 2.2 Flags and configuration

| Claim | Status | Source and version |
|---|---|---|
| `codex exec [OPTIONS] [PROMPT]`; a missing prompt or `-` reads stdin; piped stdin together with a prompt argument appends a `<stdin>` block (the run printed "Reading additional input from stdin...") | source; run | `exec/src/cli.rs`; `codex exec`, 0.166.0 |
| `exec` flags: `--json`, `--ephemeral`, `--skip-git-repo-check`, `--ignore-user-config`, `--ignore-rules`, `--strict-config`, `-s/--sandbox read-only\|workspace-write\|danger-full-access`, `-C/--cd`, `-c key=value`, `--dangerously-bypass-approvals-and-sandbox`, `--dangerously-bypass-hook-trust` | run | `codex exec --help`, 0.166.0 |
| `exec` has no `-a/--ask-for-approval` | source | `exec/src/cli.rs`, `utils/cli/src/shared_options.rs` |
| `-c` takes a dotted key and a TOML value (arrays and inline tables work), otherwise a literal string | source | `utils/cli/src/config_override.rs` |
| `--strict-config` rejects an unknown `-c` feature key: `Error loading config.toml: unknown configuration field `features.shell_toool` in -c/--config override`, exit 1 | run | 0.166.0 |
| Without `--strict-config` the same key gives two `item.completed` items of type `error` ("... is ignored") and the run continues | run | 0.166.0 |
| The planned set is accepted under `--strict-config`: `--ignore-user-config --ignore-rules -s read-only`, `features.{shell_tool,apps,plugins,multi_agent,view_image,hooks,memories,tool_suggest,request_permissions_tool}=false`, `web_search="disabled"`, `approvals_reviewer="user"`, `mcp_servers.sonara.{command,args,env,required,enabled_tools,disabled_tools}` | run | 0.166.0; the run passed config loading and reached the MCP start |
| A typo in that set (`features.view_imagee`) is rejected | run | 0.166.0 |
| All eleven feature keys (the nine above plus `codex_hooks` and `standalone_web_search`) exist in the schema; `web_search` is `disabled\|cached\|indexed\|live`; `approvals_reviewer` defaults to `user` | source | `core/config.schema.json` |
| `mcp_servers.<n>` keys: `command`, `args`, `env`, `env_vars`, `cwd`, `required`, `startup_timeout_sec`, `tool_timeout_sec`, `enabled`, `enabled_tools`, `disabled_tools` (applied after `enabled_tools`), `default_tools_approval_mode` (`auto\|prompt\|writes\|approve`), `tools.<t>.approval_mode` | source | `config/src/mcp_types.rs`, schema |
| `required=true` and a server that fails to start: no JSONL event, `Failed to create session: required MCP servers failed to initialize`, exit 1 | run | 0.166.0 (a server that does not exist, and a server that closes on `initialize`) |
| System prompt: `-c developer_instructions="..."`; there is no `--append-system-prompt` | source | schema key |

### 2.3 What enforces the boundary

| Part | Mechanism | Status | Source |
|---|---|---|---|
| Approval policy | `exec` sets harness override `approval_policy = never`, which wins over `-c approval_policy` and `config.toml`; exceptions: `--yolo`, and a resolved `approvals_reviewer = auto_review` rebuilds the config without the override (hence `approvals_reviewer="user"`) | source | `exec/src/lib.rs:597`, `build_exec_config` (about 787-815); `core/src/config/mod.rs` |
| Approvals cannot be answered in `exec` | it auto-cancels MCP elicitations and rejects command, file-change, `apply_patch`, permissions and user-input requests | source | `exec/src/lib.rs` `handle_server_request` |
| No shell | `features.shell_tool=false`: `add_shell_tools` returns early, so shell and unified-exec tools are not registered | source | `core/src/tools/spec_plan.rs:1243-1250` |
| No file write | `-s read-only` plus approval `never`; `apply_patch` is registered whenever the model catalog sets `apply_patch_tool_type` (no feature flag), so the model sees it and `assess_patch_safety` rejects the write | source | `spec_plan.rs:1426`, `core/src/safety.rs` |
| Hosted connector server | `features.apps=false` removes `codex_apps`; `features.plugins=false` gates plugin MCP servers and skills | source | `ext/mcp/src/lib.rs`, `core/src/config/mod.rs` |
| Destructive MCP tools | `requires_mcp_tool_approval` is true for `destructiveHint == true`, false for `readOnlyHint == true`, and false when `destructiveHint=false` and `openWorldHint=false`; with approval `never` the call is denied ("MCP tool call requires approval, but approval policy is never"). `default_tools_approval_mode="approve"` would disable this | source | `core/src/mcp_tool_call.rs` (`requires_mcp_tool_approval`, `custom_mcp_tool_approval_mode`) |
| Sonara annotations | `readOnlyHint = (safety == Read)`, `destructiveHint = (safety == Destructive)`, `openWorldHint = false`; Destructive tools: `create_project`, `open_project`, `delete_track`, `export_audio`, `export_stems`, `export_midi` | source | `crates/sonara-agent/src/mcp/mod.rs`, `contract/tools.rs` |
| Tools that stay | `update_plan`; MCP resource tools (`list_mcp_resources`, `list_mcp_resource_templates`, `read_mcp_resource`) whenever an MCP server exists; `apply_patch` | source | `spec_plan.rs` |
| Tools that stay with the argv of the chat | at the top level `exec` (code mode: a JavaScript sandbox with no file system and no network), `wait`, `request_user_input`, `request_user_input_async`, `clock`, `sleep`; inside `exec` `apply_patch`, the three MCP resource tools and `clock__curr_time`. `update_plan` did not appear. Run without the `sonara` server, so its tools are not in this listing | run | `codex debug prompt-input`, 0.166.0, `crates/sonara-app/tests/chat_codex_binary.rs` |
| `features.multi_agent=false` is not enough to remove the sub-agent tools; `agents.enabled=false` is | run (the model was still offered `spawn_agent`, `send_message` and `followup_task` with the first alone) | `codex debug prompt-input`, 0.166.0; the test asserts they are gone with both |
| `features.code_mode` and `features.code_mode_host` do not remove `exec` | run (reported by the implementer; no committed test) | `codex debug prompt-input`, 0.166.0 |
| The Sonara MCP tools are among the tools nested in `exec`; whether a nested call is an `mcp_tool_call` item | the first part: run (reported by the implementer); the second: not verified | `codex debug prompt-input`, 0.166.0 |
| Hooks under `--ignore-user-config`, `$CODEX_HOME` `AGENTS.md` and skills | whether they still load | assumption (not checked) | |
| Whether `model_catalog_json` can remove `apply_patch` | not checked | assumption | |

### 2.4 Signals, auth, sessions

| Claim | Status | Source and version |
|---|---|---|
| `exec` awaits `tokio::signal::ctrl_c()` once: the first SIGINT sends `turn/interrupt` and waits for the turn to end; a second SIGINT is not acted on | source | `exec/src/lib.rs:1197-1203` |
| No SIGTERM handler in `exec` (default termination) | assumption (grep for signal handling found only `ctrl_c`) | `exec/src/lib.rs` |
| Exit code 1 for turn failure, interruption, bad config, git-repo check failure; no finer scheme documented | source | many `process::exit(1)` sites |
| `codex login status`: exit 0 when signed in, 1 when not; output on stderr (`Not logged in`), nothing on stdout; it ignores `CODEX_API_KEY`, while `exec` honours it | run (not signed in: stderr text, empty stdout, exit 1); source (the rest) | 0.166.0; `cli/src/login.rs`; `exec/src/lib.rs:515` |
| An unauthenticated `exec` starts a thread and retries with 401 over WebSocket and HTTPS, so a run is a bad sign-in probe | run | 0.166.0 |
| Auth lives in `CODEX_HOME` even with `--ignore-user-config` | source (flag description) | `exec/src/cli.rs` |
| `--ephemeral` ("run without persisting session files to disk"); non-ephemeral runs persist rollout files in `$CODEX_HOME/sessions` | source and docs for the persistence; a rollout file was found in an earlier probe home whose flags are not recorded | `exec/src/cli.rs`; probe home |
| An `--ephemeral` run created no `sessions/` directory but created SQLite state files (`state_5.sqlite`, `logs_2.sqlite`, others) in `CODEX_HOME`; contents not inspected | run | 0.166.0 |
| `exec resume --last` is filtered by cwd; explicit `thread_id` resumes | source | `exec/src/cli.rs` |

## 3. Gemini CLI (assessed, not implemented; nothing was run)

Source: a clone of `google-gemini/gemini-cli` at `main` b633c07 and tag `v0.66.0`; the npm package
0.66.0 was installed with `--ignore-scripts` and never executed. The files relied on were identical
between the two, except `nonInteractiveCli.ts` and `errors.ts` (cancellation). npm `dist-tags`: `latest` 0.66.0, `preview` 0.67.0-preview.0,
`nightly` 0.67.0-nightly; Apache-2.0; `engines.node >=20`.

### 3.1 Stream-json events (`gemini -p "<prompt>" -o stream-json`)

Source: `packages/core/src/output/types.ts`, `packages/cli/src/nonInteractiveCli.ts`.

| Event | Fields |
|---|---|
| `init` | `session_id`, `model` |
| `message` | `role` (`user` or `assistant`), `content`, `delta?`; the user prompt is echoed; assistant text arrives as `delta: true` chunks only, no consolidated message |
| `tool_use` | `tool_name`, `tool_id`, `parameters` |
| `tool_result` | `tool_id`, `status` (`success` or `error`), `output?` (set only when the display result is a string), `error?` `{type, message}` |
| `error` | `severity` (`warning` or `error`), `message`; non-fatal |
| `result` | `status`, `error?`, `stats` (`total_tokens`, `input_tokens`, `output_tokens`, `cached`, `input`, `duration_ms`, `tool_calls`, `models`) |

Every event has `type` and an ISO `timestamp`. There is no thinking event and no cost field.

### 3.2 Mechanisms

| Claim | Status | Source |
|---|---|---|
| Exit codes `0`, `41` (auth), `42` (input), `52` (config), `130` (cancel). Docs also list `1` and `53` (turn limit); those two are not in `exitCodes.ts` | source; docs for 1 and 53 | `packages/core/src/utils/exitCodes.ts` |
| `--approval-mode default\|auto_edit\|yolo\|plan`; approval mode is not the boundary | source | `packages/cli/src/config/config.ts` |
| Headless: `ask_user` becomes deny; `write.toml` denies `replace`, `run_shell_command`, `write_file`, `activate_skill`, `web_fetch`; `read-only.toml` allows `read_file`, `glob`, `grep_search`, `list_directory`, `google_web_search` and others | source | `packages/core/src/policy/policies/*.toml` |
| `--policy <file\|dir>` exists in 0.66.0 and main; files load at the user tier (`4 + priority/1000`) and replace `~/.gemini/policies`; tiers: default 1, extension 2, workspace 3, user 4, admin 5; workspace policies are disabled in code | source (tier constants and `disableWorkspacePolicies = true` re-read) | `policy/config.ts`, `cli/src/config/policy.ts` |
| MCP tool names are `mcp_<server>_<tool>` (single underscore); the prefix is the same at 0.66.0 and main | source (re-read at both) | `packages/core/src/tools/mcp-tool.ts:37` |
| `GEMINI_CLI_SYSTEM_SETTINGS_PATH` points to a system settings file merged last; it can carry `mcpServers`, `hooksConfig.enabled=false`, `tools.core=[]`, `security.disableYoloMode` | source (the env var re-read) | `cli/src/config/settings.ts:107` |
| `--allowed-mcp-server-names` stops other MCP servers from connecting | source | `mcp-client-manager.ts`, `config.ts` |
| An empty `tools.core` registers no built-in tool | assumption (read from logic, not run) | `core/src/config/config.ts` |
| Folder trust defaults to enabled; a headless run in an untrusted folder fails unless `--skip-trust` or `GEMINI_CLI_TRUST_WORKSPACE=true` | source | `trustedFolders.ts`, `userStartupWarnings.ts` |
| With a non-TTY stdin Gemini reads stdin and prepends it to the `-p` prompt | source | `packages/cli/src/gemini.tsx` |
| There is no `auth status` command; headless auth comes from `security.auth.selectedType` or env (`GEMINI_API_KEY`, `GOOGLE_GENAI_USE_GCA`, `GOOGLE_GENAI_USE_VERTEXAI`, ...), otherwise exit 41 | source | `validateNonInterActiveAuth.ts`, `contentGenerator.ts` |
| Sessions live in `~/.gemini/tmp/<hash>/chats`, keyed by cwd; no flag to disable was found | source | docs `session-management.md`, `config.ts` |
| Cancellation: `main` handles SIGINT in `nonInteractiveCli.ts` (a final `result` with `FatalCancellationError`, exit 130); `v0.66.0` has no SIGINT handler there (0 matches) and no `result` event on SIGINT | source (re-checked by grep) | `nonInteractiveCli.ts` at both |
| `gemini --acp` (alias `--experimental-acp`) speaks ACP over stdio with `@agentclientprotocol/sdk` 0.16.1; per-session `mcpServers` are merged into the user's servers | source | `packages/cli/src/acp/` |
| The `-e none` flag disables extensions | assumption | |

## 4. ACP and its adapters

### 4.1 Protocol

| Claim | Status | Source |
|---|---|---|
| `session/new` params: `cwd` (absolute), `mcpServers`; stdio entry `{name, command, args, env: [{name, value}]}` without a `type` field, supported by every agent; HTTP entry `{type:"http", name, url, headers}` only if `mcpCapabilities.http`; SSE likewise with `sse` | source; run in a spike | `agent-client-protocol-schema` 1.13.0; spike output |
| Draft v2 (`unstable_protocol_v2`) changes the MCP server shape | docs/source | agent-client-protocol 3.6.0 |
| Transport: JSON-RPC 2.0 over stdio, newline-delimited, UTF-8; stdout carries only ACP messages | docs | agentclientprotocol.com/protocol/transports |
| `session/prompt` returns `end_turn`, `max_tokens`, `max_turn_requests`, `refusal` or `cancelled`; `session/cancel` is a notification; the client answers pending permission requests with `cancelled` | docs | protocol/prompt-turn |
| `session/request_permission`: options `allow_once`, `allow_always`, `reject_once`, `reject_always`; agents MAY ask | docs; run in a spike (the researcher's own agent and client) | protocol/tool-calls; spike |
| Client `fs` and `terminal` capabilities announce client-provided services; they do not disable the agent's own tools | docs; source | protocol/initialization; `claude-agent-acp` 0.99.1 |
| The spike used its own agent and client, so it proves the SDK wire shapes only, not the behaviour of any real adapter | fact about the evidence | |

### 4.2 Rust SDK (spike on the pinned toolchain 1.99.1)

| Claim | Status | Source |
|---|---|---|
| `agent-client-protocol` 3.6.0 (2026-10-28), Apache-2.0, edition 2024, `rust-version` 1.88.0; `agent-client-protocol-schema` 1.13.0, Apache-2.0, MSRV 1.88 | run (manifests read) | `~/.cargo/registry` |
| Releases 3.0.0 (2026-10-06) to 3.6.0 (2026-10-28) | docs (crates.io API, first-round report) | |
| The core depends on `futures`, not tokio; `process` adds `async-process`, `async-io`, `polling`, `blocking`, `rustix`, `shell-words`; `stdio` adds `blocking` | source | crate `Cargo.toml` |
| With `process` and `stdio`: 82 crate names in `cargo tree -e normal` (81 without the spike crate), 178 tree lines; 24 names are not in the repo's 544-name `Cargo.lock`: 3 ACP crates (`agent-client-protocol`, `-derive`, `-schema`) and 21 others (`async-channel`, `async-io`, `async-lock`, `async-process`, `async-signal`, `async-task`, `blocking`, `concurrent-queue`, `convert_case`, `event-listener`, `event-listener-strategy`, `fixedbitset`, `futures-concurrency`, `futures-lite`, `parking`, `pin-project`, `pin-project-internal`, `piper`, `polling`, `shell-words`, `unicode-xid`) | run (recounted on 2026-10-30) | `cargo tree --offline` in the spike |
| `cargo deny check` with the repo's `deny.toml` passed (advisories, bans, licenses, sources); the only warnings were duplicate `syn` and unmatched licence exceptions | run (first-round report; not re-run) | cargo-deny 0.20.2 |
| Awaiting `send_request(..).block_task()` inside an `on_receive_request` handler deadlocks the dispatch loop; use `cx.spawn` | run in the spike; docs | `src/concepts/ordering.rs` |

### 4.3 Adapters

| Claim | Status | Source |
|---|---|---|
| `claude-agent-acp` 0.99.1: Apache-2.0, `engines.node >=22`, depends on `@anthropic-ai/claude-agent-sdk` 0.3.312; the SDK runs a native `claude` from an optional platform package (about 276 MB unpacked) or `CLAUDE_CODE_EXECUTABLE` | run (package.json read); docs | npm tarball |
| `@anthropic-ai/claude-agent-sdk` licence is "SEE LICENSE IN README.md" (Anthropic terms) and its binary package "SEE LICENSE IN LICENSE.md" (all rights reserved); the ACP registry lists `claude-acp` as proprietary; do not redistribute | run (npm metadata read) | npm; registry |
| `claude-agent-acp` defaults to the `claude_code` preset, so `Bash`, `Write` and `Edit` run inside the `claude` binary; `_meta.claudeCode.options` can carry `tools`, `allowedTools`, `disallowedTools`, `strictMcpConfig`, `settingSources`; `permissionMode` is adapter-managed; default `settingSources` is `user, project, local` | source | `dist/acp-agent.js` 0.99.1 |
| `canUseTool` becomes `session/request_permission` also for `mcp__*` tools, but only for calls that the mode and allow rules have not already settled | source | same |
| Whether `dontAsk` can be forced at `session/new` | assumption (no way found; `session/set_mode` accepts it, but whether it is advertised was not checked) | |
| `codex-acp` 2.5.0 (2026-10-29): Apache-2.0, TypeScript on Node wrapping the Codex App Server; depends on `@openai/codex ^0.165.0`; no `engines` field; GitHub releases v2.3.0 to v2.5.0 have no assets; a standalone binary would be self-built with bun | run (package.json read); docs (releases API) | npm; GitHub |
| `codex-acp` routes every MCP call through `request_permission`, and can disable the Codex shell | assumption | |
| The older Rust `zed-industries/codex-acp` is archived (last release v0.16.0, 2026-06-08) | docs | GitHub |
| `gemini --acp`: the ACP registry launches `@google/gemini-cli@0.66.0 --acp` through `npx` | docs | ACP registry |

## 5. Minimum versions

The Codex versions come from bisecting 224 release tags of `openai/codex`: the first tag whose
source file contains the key. A file that moved gives a too-late answer, so each number is an
**upper bound** on the true introduction version. Nothing below 0.166.0 was run.

| Codex feature | First tag found |
|---|---|
| `exec resume` | 0.35.0 |
| `--output-schema` | 0.41.0 |
| `mcp_servers.*.enabled_tools` and `disabled_tools` | 0.48.0 |
| `developer_instructions` | 0.53.0 |
| `features.shell_tool` | 0.59.0 |
| `features.apps` | 0.93.0 |
| `--ephemeral`, MCP `required` | 0.99.0 |
| `features.multi_agent` | 0.102.0 |
| `default_tools_approval_mode`, `features.memories` | 0.105.0 |
| `features.plugins` | 0.108.0 |
| exec auto-cancel of elicitations | 0.113.0 |
| `--ignore-user-config`, `--ignore-rules` | 0.122.0 |
| `features.hooks` key | 0.129.0 |
| `--strict-config` | 0.131.0 |
| `cache_write_input_tokens` | 0.145.0 |
| `features.view_image` key | 0.147.0 |

| CLI | Floor used | Basis |
|---|---|---|
| Claude Code | 2.1.259 | the repo constant `MIN_VERSION`; unchanged |
| Codex CLI | 0.147.0 (the latest of the rows above) | bisect upper bound; the full `-c` set was run only on 0.166.0 |
| Gemini CLI | 0.66.0 (if ever built) | npm `latest`; stable 0.66.0 sends no `result` on SIGINT |

## 6. Where this record differs from the first-round reports

| First-round statement | Corrected to |
|---|---|
| `--strict-config` covering `-c` overrides: assumption | verified by running on 0.166.0 (see 2.2) |
| Whether a misspelled `-c features.x` is ignored: open | without `--strict-config` it is only reported as ignored; with it the run exits 1 |
| `error` events are not always fatal: source only | also seen in a run |
| ACP tree: 97 unique crates, 172 lines, "4 ACP crates", "about 25 new" | 82 crate names, 178 lines, 24 new names: 3 ACP crates and 21 others (the "4" counted the spike crate). The default-feature figures (71 and 116) were not reproduced |
| Both ACP adapters need `engines.node >=22` | only `claude-agent-acp` declares it; `codex-acp` has no `engines` field but is still a Node package |
| `ClientContext.allowed` | the field is `allowed_paths`, next to `base_dir` |
| `approvals_reviewer="user"`: key value assumed | `user` is the schema default and was accepted by the run |
| Gemini exit codes 1 and 53 | not in `exitCodes.ts`; docs only |
| "`codex login status` shows login state before a run" | confirmed; an unauthenticated run does not fail fast, it retries |
