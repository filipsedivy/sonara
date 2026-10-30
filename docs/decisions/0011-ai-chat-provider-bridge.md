# ADR 0011: AI Chat provider bridge (native CLI adapters, no ACP, no Node)

- **Status:** Accepted (2026-10-30)
- **Deciders:** Sonara core, for the `feature/ai-chat` branch.
- **Research:** [0011-ai-chat-research.md](0011-ai-chat-research.md) holds every fact below with
  its source, version and status (verified or assumption).

## Context

The AI Composer (`sonara_app::composer`, [docs/ai-composer.md](../ai-composer.md)) runs one
headless `claude -p` per press of Compose. It is a single, one-shot run of one provider. AI Chat
(Beta) is meant to be a conversation: a queue of tasks that run one after another, a thread that
keeps the earlier tasks, and a choice of provider. Claude Code stays the reference backend.
OpenAI's Codex CLI is the second. Google's Gemini CLI was researched.

The constraints that decide the design:

- **The security boundary is part of the product.** The agent may call Sonara's MCP tools and
  nothing that writes files, saves, exports or runs a program. Today only the Claude boundary is
  built and has a live (ignored) test, `apps/sonara/tests/composer_live.rs`. A provider is
  enabled only when its boundary is enforced by its own configuration.
- **CTR-01.** No change to the 57 tool schemas, the `.sonara` format, the IPC protocol or the
  CLI. Everything here lives in `sonara-app` and the desktop crate.
- **RT-01.** Nothing here touches the real-time crates.
- **Provider CLIs are subprocesses of the user's own installation** with the user's own login.
  Sonara never reads token files.
- **QG-10, QG-13, LIC-01.** A new dependency needs a reason, a licence on the `deny.toml`
  allow-list and a row in `docs/dependencies.md`; a library is preferred over our own code.

Three ways to reach several providers were compared.

## Variants compared

- **A. Native adapters.** One small adapter per provider over that provider's own headless
  streaming mode (Claude `stream-json`, Codex `exec --json`). Each adapter builds its argv and
  configuration, parses its stream into the existing normalised `Event`, and carries its own stop
  policy. The `Run` supervisor is shared.
- **B. Unified ACP.** One Agent Client Protocol client in Rust (`agent-client-protocol`) talks to
  per-provider ACP agents: `claude-agent-acp`, `codex-acp`, `gemini --acp`.
- **C. Hybrid.** A for Claude and Codex, ACP only where a provider offers no usable headless
  stream (in practice Gemini).

| Criterion | A. Native adapters | B. Unified ACP | C. Hybrid |
|---|---|---|---|
| New Rust dependencies and licences | None. `serde_json`, `crossbeam-channel` and `rustix` are already used by `composer`. | `agent-client-protocol` 3.6.0 and `agent-client-protocol-schema` 1.13.0, both Apache-2.0, MSRV 1.88.0 (toolchain pin is 1.99.1). With `process` and `stdio`: 82 crate names in the tree, 24 not in `Cargo.lock`, among them a second async family (`async-io`, `polling`, `blocking`). `cargo deny check` passed in a spike. | As B, for one provider that is not implemented. |
| Runtime Node for the end user | No. Claude and Codex (Homebrew casks) are native arm64 executables. | Yes for Claude (`claude-agent-acp` 0.99.1, `engines.node >=22`) and for Codex (`codex-acp` 2.5.0 is a Node package; no `engines` field; no published binary). Gemini CLI is itself a Node package in every variant. | Only for the provider that needs ACP. |
| Redistribution | Nothing is bundled. | The Claude adapter wraps `@anthropic-ai/claude-agent-sdk`, licensed "SEE LICENSE IN README.md" (Anthropic terms) with a native binary package marked all rights reserved; it must not ship in `Sonara.app`. | As B. |
| Security boundary per backend | The provider's own flags and config, one contract test each. Claude: proven argv. | ACP is a transport, not a boundary. Not advertising `fs` and `terminal` does not remove the agent's own tools. `claude-agent-acp` manages `permissionMode` itself, so `dontAsk` cannot be forced at `session/new`, and it reads the user's `~/.claude` settings by default. The boundary would still be per-adapter config, now one layer further away. | A for Claude and Codex. |
| Stop | A per-provider `StopPolicy` (signals). Behaviour differs (see below). | `session/cancel` is protocol-level and the agent must answer `cancelled`. Not tested against a real adapter. | A for Claude and Codex. |
| MCP hand-off of the `sonara` server | Claude: `--mcp-config` JSON (`env` is a map). Codex: `-c mcp_servers.sonara.*`. Gemini: a settings file. Three wire forms from one `McpSpec`. | One form: `session/new` `mcpServers`, stdio entry (`env` is an array of `{name, value}`). | Two forms. |
| Parsers | Two (Claude, Codex); a third for Gemini if it is ever built. | One event model plus a JSON-RPC client and its handlers (a `block_task` inside a request handler deadlocked the spike). | Two plus one. |
| Maturity | The Claude path exists and is tested. Codex `exec --json` is a documented, versioned event schema. | Crate 3.0.0 to 3.6.0 between 2026-10-06 and 2026-10-28; protocol v2 is a draft that changes the MCP server shape. | As B. |
| Lines of code | Claude today: `composer/` is 1,677 lines including tests (`stream.rs` 469, `run.rs` 413). A Codex adapter is not written yet; its size is not known. | Spike: 108 lines for `initialize`, `session/new`, one prompt and one permission request against the researcher's own agent. Not representative of real adapters. | Not estimated. |

## Decision

**Variant A.** Native adapters over each provider's headless streaming mode, one shared `Run`
supervisor, no ACP, no Node, no new Cargo or npm dependency.

Reasons:

1. **ACP does not provide the boundary.** The boundary has to be enforced by each provider's
   configuration either way. Through ACP it is enforced behind an adapter that overrides some of
   it (`permissionMode`) and reads the user's settings, so the proven Claude boundary
   (`--permission-mode dontAsk`, `--permission-prompts none`, allow and deny lists,
   `--strict-mcp-config`) would be weaker, not stronger.
2. **ACP needs Node >= 22 for Claude and Node for Codex, and the Claude SDK may not be
   redistributed.** Node is today only a build tool on the development host (ADR 0009). Making
   it a run-time requirement for people who installed `claude` or `codex` natively is a
   product decision that has not been taken (see QG-13 below).
3. **Cost.** About two dozen new crates, a second async runtime family beside tokio, weekly API
   churn and a draft v2, for a gain (one event model, `session/cancel`) that two native
   adapters over a stable `Event` already deliver (ARC-05, DES-03).
4. **Evidence.** Everything the Claude adapter needs already exists and is tested. The Codex
   stream and boundary were run against the installed `codex` 0.166.0 as far as no login allows
   (see "What was verified").

**QG-13 considered.** `agent-client-protocol` 3.6.0 is an established crate with an allowed
licence (Apache-2.0) and `cargo deny` passes with it. It is not adopted because the agents it
talks to for Claude and Codex are Node packages (and the Claude SDK is not redistributable), and
because it does not remove the per-provider boundary work. This is the stated reason QG-13 asks
for. The seam below keeps C open: an ACP transport can be added later for a provider that has no
usable headless stream, behind the same `Provider` trait.

**Variant C is not built now.** Its only candidate user, Gemini, is not implemented (below), so
adding a second transport would be a hypothetical need (DES-03).

## The provider bridge

Lives in `sonara_app::chat` (state) and `sonara_app::chat::provider` (adapters), next to
`composer`. The `composer` module keeps its public API and behaviour (`Run::start` keeps its
signature and Claude defaults).

```text
trait Provider: Send + Sync {
    fn id(&self) -> ProviderId;                     // "claude" | "codex"
    fn name(&self) -> &str;
    fn detect(&self) -> ProviderStatus;             // off the UI thread, never reads token files
    fn build(&self, &Located, &Workspace, &Turn) -> Invocation;
    fn parser(&self) -> Box<dyn LineParser>;        // stateful: Codex items need correlation
    fn stop_policy(&self) -> StopPolicy;
}
Run::start_with(invocation, workspace, parser, stop_policy, sink)
```

- **Normalised events.** The existing `composer::stream::Event` (`Init`, `Text`, `ToolUse`,
  `ToolResult`, `Retry`, `Result`, `Stderr`, `Output`, `Exited`), unchanged, so the Composer
  panel and its tests keep working. Fields a provider cannot fill are `None`: Codex reports tokens
  but no cost, no turn count and no denied-tools list; it delivers whole messages, not deltas.
  `Exited` is always last.
- **Intended Codex mapping** (details live in the adapter): `thread.started` to `Init`;
  `item.completed` `agent_message` to `Text`; `mcp_tool_call` start and end to `ToolUse` and
  `ToolResult` (`ok` is `status == completed`); `turn.completed` and `turn.failed` to `Result`;
  top-level `error` and `item` type `error` to `Retry` or `Output`, never to a failure by
  themselves (they are not always fatal). Any `command_execution`, `file_change`, `web_search`
  or `collab_tool_call` item is a boundary violation that the adapter surfaces, not hides.
  Unknown item types and new fields are tolerated.
- **Per-provider `StopPolicy`** (first signal, graces, escalation), because the providers stop
  differently:

  | Provider | On SIGINT | Policy |
  |---|---|---|
  | Claude 2.1.308 | emits a `result`, exits 130 (existing behaviour, `run.rs`) | SIGINT to the pid, 10 s, SIGTERM to the group, 5 s, SIGKILL (unchanged) |
  | Codex 0.166.0 | first SIGINT interrupts the turn; no `turn.*` event; exit 1; a second SIGINT is swallowed; no SIGTERM handler found in `exec` | SIGINT, a short grace, then SIGKILL to the group (no SIGTERM step). The grace is a code constant; not measured. |
  | Gemini 0.66.0 (assessed) | stable: no `result` event; nightly: `result` with `FatalCancellationError`, exit 130 | would be SIGINT, grace, SIGKILL; not implemented |

- **`cancelled` is decided by the host.** `Event::Exited.stopped` (the host's stop flag) is the
  only source of the `cancelled` state. Never the exit code (Codex exits 1 on both failure and
  interrupt, Claude 130), and never the presence or absence of a terminal event.
- **One boundary list as data.** The denied Sonara tools are provider-neutral data (the 13 names
  of `DENIED_TOOLS` without the `mcp__sonara__` prefix). Each provider renders it in its own
  syntax (`mcp__sonara__x` for Claude; bare names in `disabled_tools` for Codex). The existing
  test that cross-checks the list against the headings of `docs/agent-api.md` stays the oracle
  (TST-05).
- **Rules applied.**
  - TRT-04 and ARC-05: the `Provider` trait is added now because there are two implementations
    (Claude, Codex) and a test seam (a fake provider script, as `run.rs` does for `claude`).
    TRT-03: `dyn LineParser` is a runtime choice.
  - TYP-02: `TaskState`, `QueueMode`, `Availability` and `ProviderId` are enums, not booleans;
    `cancelled` is a state, not a flag next to `failed`.
  - ERR-03 to ERR-05: detection, spawn and parse failures are typed errors with `source()`;
    they reach the window as `ChatError` codes (`PROVIDER_UNAVAILABLE` and the others of the
    contract), and a failed task keeps its reason in `TaskView.error`.
  - TST-02, TST-03, TST-05, TST-07: a fake provider for queue and stop tests, a test per
    `ChatError` code, argv and config contract tests per provider, and the invariant that after
    a wipe no event of an older generation reaches the thread.
  - CTR-01 and RT-01: see Context. LIC-01 and QG-10: no dependency is added.
- **The `Workspace` stays per run**: a private `0700` folder with an empty cwd, a `SONARA_HOME`
  holding only this window's discovery file, and `--connect always`. This pin is provider
  independent.

### How an API backend plugs in later

Not built, not scheduled; recorded so the seam is not a dead end.

- **UI contract unchanged.** The normalised `Event`, `ChatEvent`, `ChatState` and the Tauri
  commands stay the same.
- **The seam does change.** `build() -> Invocation`, `parser()` and `StopPolicy` describe a
  subprocess. An API backend needs a non-process run path (a second method on `Provider` or a
  small enum of run kinds). That change belongs to its own ADR.
- **Own tool loop.** The host sends the model the tool list and executes the calls itself, so
  it owns the boundary. It can make the deny list and `CONFIRMATION_REQUIRED` hard host-side
  rules instead of flags handed to someone else's binary. No CLI backend can.
- **MCP client in-process** (or direct calls into the tool registry). A Rust MCP client crate
  would be a QG-10 decision of its own.
- **Key in the macOS Keychain**, never in the project, in settings files or in logs. A Keychain
  crate and an HTTP/TLS stack are two more QG-10 decisions.
- **Own boundary list**, generated from the same data list as above.

### CLI agent and AI API compared

| | CLI agent (Claude Code, Codex) | AI API (later) |
|---|---|---|
| Tool loop | the CLI | Sonara |
| MCP client | the CLI, started with our `sonara mcp` entry | Sonara, in-process |
| Authentication | the user's own login of that CLI (subscription or API key); Sonara never reads token files. Sign-in state: Codex has `codex login status` (exit 0 or 1, text on stderr); Claude has no cheap probe, a run that is not signed in reports it in its result | an API key in the Keychain |
| Security boundary | the CLI's flags and config; one contract per provider; some parts are only prompt rules | host code; can be a hard rule everywhere |
| Cost and limits | the user's plan or key. Claude reports `total_cost_usd` and tokens; Codex reports tokens only. Limits are the provider's. | per-token billing to the key; limits are the API's; the host sees exact usage |
| Data privacy | prompt and tool results go to the provider. The CLI keeps its own transcripts on disk unless told not to (`--no-session-persistence`, `--ephemeral`). User-level config of the CLI (`~/.claude` hooks, `CLAUDE.md`) can still load. | only what the host sends; no provider-side transcripts on disk; the host decides retention |
| Runtime requirement | the user's installed CLI at a minimum version | network and a key |
| Streaming | Claude: text deltas. Codex: whole messages only. | up to the API |

## Per-backend enforcement of the boundary

What enforces each part, and what it cannot enforce. "Verified by running" and "from source" are
defined in the last section.

| Boundary part | Claude Code 2.1.308 (flags) | Codex CLI 0.166.0 (`-c` and flags) | Gemini CLI (assessed, not implemented) |
|---|---|---|---|
| No shell | `--disallowedTools Bash`, `--permission-mode dontAsk`, `--permission-prompts none`; `Bash` is not in `--allowedTools` | `-c features.shell_tool=false` (shell and unified-exec tools are not registered, from source); `-s read-only`; exec forces approval policy `never` | deny-all policy rule (`toolName="*"`, priority 900) plus the headless denial of `run_shell_command`; `tools.core=[]` in the injected system settings |
| No file write | `--disallowedTools Edit,Write,NotebookEdit` | `-s read-only` with approval `never`. `apply_patch` cannot be removed by config: the model still sees it and the call is rejected by sandbox and approval policy (`file_change` item, status `failed`). | the same deny-all rule; `write_file` and `replace` are denied in headless mode by default |
| No web | **Not blocked, by design**: `WebSearch` and `WebFetch` are in `--allowedTools` (prompt-injection risk accepted and documented in `ai-composer.md`) | `-c web_search="disabled"` | optional allow rules for `google_web_search` and `web_fetch`; read-only tools such as `read_file` are allowed by default, so deny-all is needed |
| No other tool sources | `--strict-mcp-config` with our `--mcp-config`: the user's MCP servers do not load | `-c features.apps=false`, `features.plugins=false`, `features.multi_agent=false`, `features.view_image=false`, `features.hooks=false`, `features.memories=false`, `features.tool_suggest=false`, `features.request_permissions_tool=false`; `--ignore-user-config` (no `$CODEX_HOME/config.toml`), `--ignore-rules` | `--allowed-mcp-server-names sonara`; `hooksConfig.enabled=false` in the injected settings |
| Only our MCP server | `--mcp-config` with `sonara` (`env` map) | `-c mcp_servers.sonara.command`, `.args`, `.env`, `.required=true`, `.enabled_tools`, `.disabled_tools`. `required=true` makes a server that fails to start abort the run before any event, exit 1 (run). | system settings `mcpServers.sonara` through `GEMINI_CLI_SYSTEM_SETTINGS_PATH` (merged last) |
| Sonara tool deny list | `--disallowedTools` with 13 `mcp__sonara__*` names; deny wins over the server-wide allow `mcp__sonara` | `disabled_tools` (the same 13 bare names) after `enabled_tools`. In addition, tools annotated `destructiveHint=true` (`create_project`, `open_project`, `delete_track`, `export_audio`, `export_stems`, `export_midi`) are denied by Codex under approval `never` (from source), so `delete_track` is blocked even though Claude allows it with a confirmation | `--policy` file, user tier: allow `mcpName="sonara"` at 950, deny list at 990 |
| No prompts, no escalation | `dontAsk` and `--permission-prompts none` | exec's harness policy is `never`; `-c approvals_reviewer="user"` (the schema default) so a user config cannot route to `auto_review`; exec auto-cancels MCP elicitations and rejects other approval requests. Never `--yolo` or any `--dangerously-bypass-*` flag. | `--approval-mode default`; `ask_user` is denied in headless mode; folder trust needs `--skip-trust` in the app-owned empty cwd |
| Reject a typo or a renamed key | not applicable | `--strict-config`: an unknown `-c` key aborts with exit 1 (run) | not applicable |
| No provider transcript | `--no-session-persistence` (to be added in the chat argv) | `--ephemeral` | none found; transcripts go to `~/.gemini/tmp` |
| `CONFIRMATION_REQUIRED` | prompt rule only (`system_prompt.md`) | prompt rule only (`developer_instructions`), plus the destructive-tool denial above | prompt rule only |

**What each backend cannot enforce.**

- **On every backend, replaying a confirmation token is a prompt-level rule.** The token is a
  Sonara protocol value returned in a tool result. A model can resend the identical call with
  `confirmation_token`, and the user can turn confirmations off in Settings
  ([agent-api.md](../agent-api.md)). `ai-composer.md` says such a step "cannot be confirmed by a
  headless run"; that overstates a prompt rule. A hard rule needs server-side support, and the
  server has no tool filter: `ClientContext` carries only `base_dir` and `allowed_paths`, a path
  allow-list, and `sonara mcp` has no option that restricts tools. Adding one touches CTR-01, so
  it is not done here; an API backend (above) is the other way.
- **Claude:** `~/.claude` hooks and `CLAUDE.md` still load (documented); `--max-turns` is a hidden
  flag, accepted by 2.1.308 but absent from `--help`, so a release could drop it. Web access is
  allowed.
- **Codex:** `apply_patch` stays visible to the model and relies on the sandbox and approval
  rejection; `update_plan` and the MCP resource tools (`list_mcp_resources`,
  `read_mcp_resource`) cannot be switched off, but reach only the `sonara` server. There is no
  "only these tools" switch, so the boundary is a list of keys that must be re-checked on each
  Codex upgrade. Whether `$CODEX_HOME` level `AGENTS.md`, skills and `hooks.json` still load
  under `--ignore-user-config` was not verified. Auth stays in the real `CODEX_HOME`.
- **Gemini:** user-level `GEMINI.md` and extensions still load; the policy file and `tools.core=[]`
  are untested at run time.

## Decisions

1. **Stateless replay instead of native resume.** Each task's prompt is preceded by a bounded
   transcript of the earlier done tasks (user prompt and final assistant text). The agent's real
   state is the project (`get_project_state`), so little is lost. Claude gets
   `--no-session-persistence`, Codex `--ephemeral`, so no provider transcript is kept, which
   matches the wipe promise. Native resume would leave transcripts in `~/.claude/projects`,
   `$CODEX_HOME/sessions` or `~/.gemini/tmp`, and whether Claude resumes across different
   working directories is unverified (the workspace cwd is new per run). Cost: more prompt tokens
   and no tool detail from earlier tasks.
2. **The queue pauses on failure or Stop.** One run at a time. A failed or cancelled task leaves
   the rest `queued` and sets the mode to `paused`; the user presses Run queue or removes tasks.
   Later tasks assume the edits of earlier ones, so they never auto-continue.
3. **History in memory only.** Never on disk, never in the `.sonara` project (CTR-01 untouched).
4. **A separate chat window with Rust-owned state.** The thread, queue and provider live in
   `sonara_app::chat`; the window is a thin view that resyncs from `chat_snapshot` after a
   reload. Task ids come from a private counter, not from the project's `IdSource`, so project
   ids and byte-identity save tests are unaffected.
5. **Wipe on `Loaded`, `Closed`, window close and `chat_clear`.** The wipe is Rust-side: stop the
   running task, drop the queue, bump a `generation` counter. Run events carry the generation so
   a late event of the old thread is ignored. A UI-only wipe would leave an agent editing the new
   project.
6. **No Node.** Neither bundled nor required.
7. **Codex is enabled as Beta**, with a version floor of 0.147.0 and `--strict-config`. The floor
   is the first version that has every key of the set, found by bisecting release tags (an upper
   bound); only 0.166.0 was run. Claude keeps its floor 2.1.259.
8. **Gemini is not implemented**, and not shown in the provider list. The research stays in this
   record. It would need a private policy file, a system settings file, trust handling, stdin
   discipline and a live deny test first.
9. **Web access follows the existing Claude composer** (allowed) and is off for Codex
   (`web_search="disabled"`).

## Consequences

- **The Composer's Claude path does not change.** Its argv, deny list and `Run::start` stay byte
  for byte. Only chat turns add `--no-session-persistence` (the chat contract says both that the
  Claude argv does not change and that chat runs are not persisted; this ADR reads it as: the
  Composer argv is frozen, chat turns add the one flag).
- **Each provider is a boundary to maintain.** Codex's boundary is about a dozen `-c` keys whose
  names have changed across releases. `--strict-config` makes a renamed or removed key abort the
  run loudly (it fails closed) instead of silently reopening a tool, so a Codex upgrade can
  break the chat until the key list is updated. A contract test per provider (argv, config,
  deny list against `agent-api.md`) and an ignored live test (TST-02, TST-03, TST-05) carry this.
- **Codex feels less live.** No text deltas; the UI shows whole messages.
- **Cold start per task** (a new provider process and a new `sonara mcp` each time) is not
  measured. A queue of short tasks pays it each time.
- **Provider-side data remains in two places.** Codex keeps state files in `$CODEX_HOME`
  (SQLite files appeared there on an `--ephemeral` run; contents not inspected). User-level
  config of each CLI can still influence a run.
- **ACP stays possible.** The seam (`Provider`, `LineParser`, `StopPolicy`, `Invocation`) is the
  place for it.

**Risks.**

- Codex's `apply_patch` is a write path held shut only by sandbox and approval semantics. A
  Codex release that changes read-only or the exec approval override would open it. Keep `-s
  read-only` and a test of the argv.
- `CONFIRMATION_REQUIRED` replay is not blocked on any backend.
- An orphaned `sonara mcp` after a SIGKILL of the provider: unmeasured whether it exits on stdin
  EOF.
- The destructive-tool denial in Codex depends on MCP annotations. `disabled_tools` repeats the
  deny list so the boundary does not depend on them, and `default_tools_approval_mode="approve"`
  must never be set.
- Provider CLIs change their flags. Version floors, `--strict-config` and the per-provider
  contract tests are the defence.

## What was verified and what was not

**Verified by running** (2026-10-30, on this machine):

- **claude 2.1.308.** `--help` lists `--permission-mode`, `--permission-prompts`,
  `--allowedTools`, `--disallowedTools`, `--strict-mcp-config`, `--mcp-config`,
  `--append-system-prompt`, `--include-partial-messages`, `--no-session-persistence`.
  Free error probes: `--permission-mode bogus` lists `dontAsk` as a choice;
  `--permission-prompts bogus` lists `host, none`; `--max-turns abc` is rejected as "must be a
  number" while an unknown flag gives "unknown option" (so the hidden flag exists);
  `--session-id notauuid` is rejected; `--resume <unknown uuid>` answers "No conversation
  found". No paid prompt was run.
- **codex 0.166.0** (native arm64 executable from the Homebrew cask):
  - `--strict-config` rejects an unknown `-c` key:
    `Error loading config.toml: unknown configuration field `features.shell_toool` in -c/--config
    override`, exit 1. Without it the same key only produces two `item.completed` items of type
    `error` ("is ignored") and the run continues.
  - The planned set (`--strict-config --ignore-user-config --ignore-rules -s read-only`, the nine
    `features.*` keys, `web_search="disabled"`, `approvals_reviewer="user"`,
    `mcp_servers.sonara.{command,args,env,required,enabled_tools,disabled_tools}`) is accepted:
    config loading passes and the run reaches the MCP start.
  - `required=true` with a server that cannot start aborts before any JSONL event and exits 1.
  - `codex login status` prints "Not logged in" on stderr, nothing on stdout, exit 1.
  - An unauthenticated `exec` still emits `thread.started` and `turn.started`, then top-level
    `error` events ("Reconnecting... N/5", HTTP 401) over WebSocket and then HTTPS. So `error`
    events are not terminal, and `detect()` must use `codex login status` rather than a run.
  - An `--ephemeral` run created no `sessions/` directory, but did create SQLite state files in
    `CODEX_HOME`.

**Verified from source** (openai/codex `rust-v0.166.0`; Gemini `v0.66.0` and `main` at b633c07;
agent-client-protocol 3.6.0 and the 0.99.1 and 2.5.0 adapter packages): the Codex event schema
and its processor (no token deltas; `item.updated` only for `todo_list`); the exec approval
override and its `auto_review` exception; `shell_tool` and `apply_patch` registration; the
`destructiveHint` handling under approval `never`; SIGINT semantics (first signal only, no
`turn.*` event for an interrupted turn, exit 1); Gemini's stream-json schema, policy tiers and
cancellation difference between 0.66.0 and main; the ACP session shapes and the adapter
dependencies. All eleven feature keys checked (the nine used plus `codex_hooks` and
`standalone_web_search`) exist in the 0.166.0 config schema, and `approvals_reviewer` defaults to
`user`.

**Not verified:**

- **No Codex login on this machine**, so no real turn was recorded. The Codex fixture is
  reconstructed from source, and the text of a denied-tool error is approximate.
- **No `gemini` binary.** Nothing about Gemini was run; its boundary is read from source.
- **The ACP spike used the researcher's own agent and client.** It proves the SDK wire shapes
  and the stdio MCP entry, not the behaviour of `claude-agent-acp`, `codex-acp` or `gemini --acp`.
- **Claude resume across working directories** was never run; only error paths were probed.
- **Cold-start latency** of any provider plus `sonara mcp` is unmeasured.
- **Whether `sonara mcp` exits on stdin EOF** is unmeasured.
- Whether `$CODEX_HOME` `AGENTS.md`, skills and `hooks.json` load under `--ignore-user-config`.
- The Codex minimum version 0.147.0 is a bisect result, and the full `-c` set was only run on
  0.166.0.
- A live deny test (shell, write, save, export, user MCP servers) for Codex and Gemini.
