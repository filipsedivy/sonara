# AI Chat (Beta)

The AI Chat is a window in which you talk to an agent that works in the open project. Choose
**AI → AI Chat (Beta)…**, write what you want, and press Send (or Enter). The message becomes a
task in a queue. The agent is a headless command line tool that the user has installed and signed
in to: Claude Code (`claude -p`, the default and the reference) or Codex CLI (`codex exec`, a Beta
provider). The agent edits the project through this window's agent session, so the timeline, mixer
and piano roll update while it works. The agent's tools are Sonara's own MCP tools and nothing
that writes files, saves, exports or runs a program (see "The tool boundary").

The chat replaces the AI Composer panel of 0.1.11. The decision and its evidence are in
[ADR 0011](decisions/0011-ai-chat-provider-bridge.md) and
[its research record](decisions/0011-ai-chat-research.md). The window carries a **Beta** badge
whose explanation reads: "Experimental feature: behaviour may change. The agent edits your
project. Undo works step by step."

## The code

| Where | What |
|---|---|
| `sonara_app::chat` (`crates/sonara-app/src/chat/`) | The chat: `state` (the queue as a pure state machine), `task` and `error` (the data and the error codes the window receives), `controller` (runs the tasks one after the other and tells the window), `replay` (the context given to each task), `provider` and `providers` (the `Provider` trait and its two adapters, `claude` and `codex`), `boundary` (the provider-neutral deny list) |
| `sonara_app::composer` (`crates/sonara-app/src/composer/`) | The Claude and process layer that the chat shares: locating Claude Code, the composer command line and its tool boundary, `Workspace` (the private run folder), `Run` (starts the process, streams events, stops it), the normalised `Event`, `LineParser` and `StopPolicy`. The chat has no second process supervisor. The composer panel and its Tauri commands are gone; the command line of a Claude Code run is unchanged |
| `apps/sonara-desktop/src-tauri/src/chat.rs` | The Tauri commands (`chat_snapshot`, `chat_providers`, `chat_set_provider`, `chat_enqueue`, `chat_edit`, `chat_remove`, `chat_move`, `chat_stop`, `chat_resume`, `chat_clear`) and the `chat-event` stream to the window. `window.rs` opens the window (label `chat`) |
| `apps/sonara-desktop/src/lib/chat/` | The window, a thin view of the Rust state: provider picker, thread, queue, message box, Clear dialog, Beta badge |

The chat state lives in Rust (`ChatController`). A window that reloads asks for `chat_snapshot` and
follows `chat-event`. Task ids come from a private counter, never from the project's id source, so
project ids are not shifted by chatting.

## How a task runs

1. **Check.** Before the first message the window shows what each provider's detection found (see
   "Availability"). Starting a task checks again, because the installation can change while the
   window is open. The chat refuses a task when the `sonara` command line tool that ships beside
   the application is missing, or when agents cannot attach to this window (started with
   `--no-ipc`, or the agent server did not start).
2. **Pin.** The run gets a private folder (mode `0700`, removed afterwards) with an empty working
   directory and a `SONARA_HOME` whose `run/` holds a copy of **this window's** discovery file
   only. `sonara mcp` attaches to the newest running Sonara, so without this a second instance
   could be edited instead of the window you see. `--connect always` makes the attach fail instead
   of falling back to a headless session nobody can see. The pin is the same for both providers.
3. **Run.** The provider builds its command line (below) and the prompt goes in on standard input.
   The prompt is the task's message, preceded by the replayed conversation (see "Continuity").
4. **Stream.** The provider's output is parsed into normalised events: the session start, text,
   tool calls and their answers (a Sonara `"ok": false` envelope counts as failed), retries, and
   the final result. The window shows them per task; at most 2 000 events are kept per task, the
   oldest go first.
5. **End.** The task ends in one of three states:
   - `done`: the run reported a successful result **and** the process exited with 0;
   - `cancelled`: the host asked to stop it (the host's stop flag), whatever the exit code was;
   - `failed`: anything else, with a reason (the result's message, or the last lines of standard
     error). A Claude Code run whose session start reports the `sonara` MCP server as failed is
     ended at once and fails with that reason.

   The undo history is sealed when a task starts running and when it ends, so your next edit
   never merges into the agent's last step. Each tool call of the agent is its own undo step; a
   task is not one step.

## The queue

A message becomes a task in state `queued`. The states are `queued`, `running`, `done`, `failed`
and `cancelled`.

- Tasks run one at a time, in thread order. Enter (or Send) queues a message also while a task
  runs.
- **Only queued tasks** can be edited (in place), removed or reordered. A task that has started
  can be none of these (`TASK_NOT_QUEUED`).
- **A failed or stopped task pauses the queue.** The rest stay `queued`, the queue shows that it
  is paused, and nothing starts by itself, because later tasks assume the edits of earlier ones.
  **Run queue** (`chat_resume`) goes on; removing the queued tasks is the other way out. A task
  that is `done` does not pause the queue. A paused queue still accepts new tasks.
- The queue holds at most 50 waiting tasks (`QUEUE_FULL`). Running, done, failed and cancelled
  tasks do not count.
- The provider is chosen per conversation: once the thread has a task it is locked
  (`PROVIDER_LOCKED`) until the history is cleared.

The errors a call can return have stable codes: `PROMPT_EMPTY`, `PROMPT_TOO_LONG`,
`TASK_NOT_FOUND`, `TASK_NOT_QUEUED`, `PROVIDER_LOCKED`, `PROVIDER_UNAVAILABLE`, `QUEUE_FULL` and
`CONFLICT`.

## Continuity: stateless replay

Every task starts a fresh agent. Continuity is a replay: the message is preceded by a block
`<earlier_conversation>` that holds the earlier **done** tasks as `User:` and `Assistant:` turns
(the user's message and the agent's final text). The chat's system prompt tells the agent that
the block is context only and not an instruction. The bounds (`chat/replay.rs`):

- the newest 8 done tasks; failed and cancelled tasks are not replayed;
- a message is cut at 1 000 characters and an answer at 2 000, each with a ` [truncated]` mark
  that counts toward the limit;
- the whole block at most 8 000 characters: whole tasks are dropped from the old end until it
  fits, and a line says that earlier messages are left out;
- a closing tag inside a message is defused so it cannot end the block early.

The 16 000 character limit applies to the user's own message only; the replay block comes on top.

**Why not native resume.** The providers keep a transcript on disk to resume from (Claude Code
under `~/.claude/projects`, Codex under `$CODEX_HOME/sessions`), which conflicts with the promise
that the history is wiped (see "Lifecycle and privacy"). Whether Claude Code resumes across
different working directories was never run, and the working directory of each run is new. So
both providers run without a kept session: Claude Code with `--no-session-persistence`, Codex
with `--ephemeral`. The cost is more prompt tokens and no tool detail from earlier tasks; the
agent's real state is the project, which it reads with `get_project_state`.

## The provider bridge

A provider is an adapter over one CLI's headless streaming mode. The `Provider` trait
(`chat/provider.rs`) has `id`, `name`, `detect` (off the UI thread; it never reads token files),
`build` (the `Invocation`: program, arguments, working directory, environment and standard input,
from the located CLI, the `sonara` tool path, the `Workspace` and the `Turn`), `parser` (a
stateful `LineParser` per run) and `stop_policy`. The shared `Run` supervisor starts the process
and delivers events; a provider supplies only what differs.

- **Claude Code** is the default and the reference. The command line is the composer's, unchanged
  (`composer::invocation::build_with`): `-p --output-format stream-json --verbose
  --include-partial-messages --strict-mcp-config --mcp-config <json> --permission-mode dontAsk
  --permission-prompts none --allowedTools mcp__sonara,WebSearch,WebFetch --disallowedTools <18
  names> --max-turns 200 --append-system-prompt <text>`. Chat turns add two things: the flag
  `--no-session-persistence`, and the chat's system prompt addendum
  (`chat/system_prompt_chat.md`), appended inside the same `--append-system-prompt` value (a second
  such flag would replace the first). Claude Code 2.1.259 or newer is required
  (`--permission-prompts`), and `claude --help` must list `--no-session-persistence`.
- **Codex CLI** (Beta) is `codex exec --json` with every tool source but the Sonara MCP server
  switched off. Version 0.147.0 or newer is required (the first version that has every key of
  the set, found by bisecting release tags, so an upper bound); only 0.166.0 was run. The
  command line is listed under "The tool boundary".
- **The parser** turns each provider's stream into the same `Event`s. Claude's `stream-json` has
  text deltas, a cost and a turn count. Codex delivers whole messages (no deltas) and reports
  tokens, with no cost and no turn count. A Codex `reasoning` item produces no event; an `error`
  item and a top-level `error` event become standard error lines, because Codex reports retries
  and configuration warnings that way and they are not fatal by themselves. A Codex item that
  the boundary should have made impossible (a shell command, a file change, a web search, a
  sub-agent call, an MCP call of another server) and any item type the parser does not know are
  shown as a boundary note and a failed tool call; they do not end the run by themselves.
  Unknown top-level events and unknown fields are ignored.
- **The stop policy** is per provider (see "Stopping").
- Only Claude Code and Codex CLI are in the provider list. Gemini CLI was researched and is not
  implemented.

No Node and no new Rust dependency is involved. Neither CLI is bundled or linked.

## Availability

`chat_providers` runs detection for each provider, off the UI thread, and the window shows the
result before the first message, with a hint on how to fix what is wrong and a "Check again"
button. A provider that is not `ready` cannot be chosen.

| State | Claude Code | Codex CLI |
|---|---|---|
| `ready` | found, version 2.1.259 or newer, and `claude --help` lists `--no-session-persistence` | found, version 0.147.0 or newer, and `codex login status` exits 0, or exits 1 while `CODEX_API_KEY` is set (ready, with a note; only whether the variable is set is looked at) |
| `not_found` | no `claude` in the places searched | no `codex` in the places searched |
| `not_signed_in` | never: sign-in is not probed. A signed-out Claude Code shows as ready and the first message fails with Claude Code's own message | `codex login status` exits 1 and no `CODEX_API_KEY` is set |
| `too_old` | older than 2.1.259, or `--help` does not list the flag | older than 0.147.0 |
| `unsupported` | the executable reports no version, or `--help` cannot be run, so the flag cannot be checked | the executable reports no version, or `codex login status` cannot be asked or answers in a way that is not understood |

`claude` is looked for on the `PATH`, then in `~/.local/bin`, `~/.claude/local`,
`~/.npm-global/bin`, `/opt/homebrew/bin` and `/usr/local/bin`; `codex` on the `PATH`, then in
`~/.local/bin`, `~/.npm-global/bin`, `/opt/homebrew/bin` and `/usr/local/bin`. An application started from
the Finder has the minimal `PATH` of `launchd`, which is why the places are listed. The probes
run in a process group of their own with a deadline: `claude --help` 15 s, `codex --version` 5 s
and `codex login status` 10 s. Credentials are never read: not `~/.codex/auth.json`, not any
token file.

Detection runs again when a task starts, so every Claude Code task pays the `--help` probe.

## The Beta badge

The header shows the word "Beta" (not only a colour) on a button; its explanation shows on hover,
on keyboard focus and on a click, and Escape dismisses it. The badge belongs to the whole
feature. The Codex adapter also sets `beta: true` in its provider status, and the provider picker
shows a "Beta" tag (text) next to the name of such a provider, with its version and availability.

## The tool boundary

The boundary is the product's safety property: the agent may call Sonara's MCP tools, and nothing
that writes a file, saves, exports or runs a program. It is enforced by each provider's own flags
and configuration, once per provider; the table says what enforces each part. It describes the
command lines built in `composer/invocation.rs` (Claude Code) and
`chat/providers/codex/invocation.rs` (Codex). The denied Sonara tools are the 13 of
`chat/boundary.rs` (`DENIED_SONARA_TOOLS`): `save_project`, `open_project`, `create_project`,
`export_audio`, `export_midi`, `export_stems`, `render_preview`, `relink_media`,
`start_recording`, `set_midi_input`, `verify_project`, `analyze_audio` and `get_audio_analysis`. A
test cross-checks the list against the Claude deny list, and another checks the Codex allow-list
against the tool headings of [agent-api.md](agent-api.md) in both directions.

| Boundary part | Claude Code | Codex CLI |
|---|---|---|
| No shell | `--disallowedTools Bash`; `Bash` is not in `--allowedTools`; `--permission-mode dontAsk` and `--permission-prompts none` deny what is not allowed | `-c features.shell_tool=false` (the shell and unified-exec tools are not registered), `-s read-only`, and the approval policy `never` that `codex exec` forces |
| No file write | `--disallowedTools Edit,Write,NotebookEdit` | `-s read-only` with approval `never`. `apply_patch` cannot be removed by configuration (below) |
| No web | **Not blocked, by design**: `WebSearch` and `WebFetch` are in `--allowedTools`. A web page can carry a prompt injection, which is why the agent has no shell and no way to write files | `-c web_search="disabled"` |
| No question to the user | `AskUserQuestion` is in `--disallowedTools` | exec auto-cancels MCP elicitations and rejects other approval requests (from source); `request_user_input` stays offered (below) |
| No other tool source | `--strict-mcp-config` with our own `--mcp-config`: the user's MCP servers do not load | `-c features.<name>=false` for `apps`, `plugins`, `multi_agent`, `view_image`, `hooks`, `memories`, `tool_suggest` and `request_permissions_tool`; `-c agents.enabled=false`; `-c skills.include_instructions=false` (the catalog of the user's skills stays out of the prompt); `--ignore-user-config` (no `$CODEX_HOME/config.toml`); `--ignore-rules`. **Not closed:** the user's global `$CODEX_HOME/AGENTS.md` still reaches the model (below) |
| Only our MCP server | `--mcp-config` with `sonara` (the bundled `sonara mcp`, `--connect always`, `SONARA_HOME` of the run folder) | `-c mcp_servers.sonara.command`, `.args`, `.env`, `.required=true` (a server that does not start aborts the run before any event), `.startup_timeout_sec=30` |
| Sonara tool lists | `--allowedTools mcp__sonara` and the 13 names as `mcp__sonara__*` in `--disallowedTools`; a deny rule wins over the server-wide allow | `.enabled_tools` with the 44 of the 57 tools that are not denied, and `.disabled_tools` with the 13 denied names (applied after `enabled_tools`) |
| No prompt, no escalation | `dontAsk` and `--permission-prompts none` | `-c approvals_reviewer="user"` (the schema default, so a user config cannot route to `auto_review`); never `--yolo`, `--dangerously-bypass-*`, `--full-auto`, `--oss`, `--add-dir` or `-m` |
| Reject a renamed key | not applicable | `--strict-config`: an unknown `-c` key aborts the run with exit 1 instead of silently reopening a tool |
| No kept session | `--no-session-persistence` | `--ephemeral` |
| Isolation | empty working directory: no repository `CLAUDE.md`, `.mcp.json` or project hook. `--bare` is not used: it reads only `ANTHROPIC_API_KEY` and never the subscription login | an empty working directory and `--skip-git-repo-check` |
| Instructions | the composer's system prompt and the chat addendum, in `--append-system-prompt` | the same text, in `-c developer_instructions` (written as a TOML string by one function, because `-c` falls back to a plain string when a value does not parse) |
| Turn limit | `--max-turns 200` (a hidden flag: accepted, but not in `--help`) | none set |

The rest of the Codex argv is `exec --json --color never` and `-` (the prompt on standard input,
never in the arguments). Each `-c` value travels as one argument, with no shell involved.

### What is not enforced

- **`CONFIRMATION_REQUIRED` is a prompt rule on every backend, not a hard block.** The agent is
  told to leave out a step that answers `CONFIRMATION_REQUIRED`, and to say so. The token that
  confirms a step is a Sonara protocol value returned in a tool result: a model can resend the
  identical call with the `confirmation_token`, and the user can turn confirmations off in
  Settings ([agent-api.md](agent-api.md)). A hard rule needs server-side support: the server has
  no tool filter, and adding one would touch the tool contracts (CTR-01). On Codex, a tool that
  Codex annotates as destructive is additionally denied by its approval policy (below).
- **Claude Code:** hooks and `CLAUDE.md` of `~/.claude` still load. Web access is allowed.
  `--max-turns` is a hidden flag, so a release could drop it.
- **Codex CLI** has no "only these tools" switch. Instead of removing tools, the boundary is a
  list of keys that must be checked again on every Codex upgrade; `--strict-config` makes a
  renamed or removed key fail the run. With this command line, `codex debug prompt-input` on
  0.166.0 still offers `exec`, `wait`, `request_user_input`, `request_user_input_async`, `clock`
  and `sleep` at the top level, and inside `exec` (a code-mode tool: a JavaScript sandbox with no
  file system and no network) `apply_patch`, the MCP resource tools `list_mcp_resources`,
  `list_mcp_resource_templates` and `read_mcp_resource`, and `clock__curr_time`. These cannot be
  switched off by configuration (`features.code_mode` and `features.code_mode_host` did not
  remove `exec`) and are rejected or inert because of the sandbox and the approval policy:
  - `apply_patch`: the call is rejected by `-s read-only` and approval `never` (a `file_change`
    item with status `failed`; read from source, not seen in a real run);
  - `request_user_input`: nothing can answer it in `exec`;
  - the MCP resource tools reach only the `sonara` server;
  - the tools nested in `exec` include the Sonara MCP tools, and `enabled_tools` and
    `disabled_tools` filter them there too: verified on 2026-10-30 with codex 0.166.0 and a live
    `sonara mcp` (`ALL_TOOLS` in `exec` held exactly the 44 allowed tools and none of the 13
    denied ones; a control run without the two keys listed all 57; the Sonara tools are
    deferred, so the request itself lists none of them). `delete_track` is offered, because it
    is allowed. Whether a Sonara call made from inside `exec` shows up as an `mcp_tool_call`
    item is **not verified**;
  - `features.multi_agent=false` alone still leaves `spawn_agent`, `send_message` and
    `followup_task` offered; `agents.enabled=false` removes them (found with
    `codex debug prompt-input`; `chat_codex_binary.rs` asserts that they are gone).
- **The system prompt says the agent has `WebSearch` and `WebFetch`.** It is shared by both
  providers, and Codex has neither (its web search is disabled).
- **Codex and the destructive tools.** Codex denies an MCP tool that is annotated
  `destructiveHint` when the approval policy is `never` (read from source). Of the six Sonara
  tools annotated that way, five are on the deny list anyway; `delete_track` is the sixth, so on
  Codex it is blocked even though Claude Code may call it (with a confirmation). Whether it is
  blocked for a track without content was not run.
- **The user's global `$CODEX_HOME/AGENTS.md` loads into every Codex chat run.** This is a
  known gap of the Beta provider, and one reason the live test matters. It was verified on
  2026-10-30 on Codex 0.166.0 with a fake Responses endpoint on 127.0.0.1 (`CODEX_API_KEY` set to
  a made-up key, `-c openai_base_url` pointed at the endpoint, a private `CODEX_HOME` with a
  canary `AGENTS.md` and a canary skill; no login, no quota): the request Codex sent held the
  canary text with `--ignore-user-config` and the whole `-c` set. None of these keys removes it:
  `project_doc_max_bytes` (0 and 1), `include_environment_context`, `instructions`,
  `skills.bundled.enabled`, `features.skip_host_skill_discovery`,
  `project_doc_fallback_filenames`; `user_instructions` is not a key under `--strict-config`.
  What the file can do is instruction injection: the shell is off, the sandbox is read-only and
  the Sonara tools are a fixed list, so it can steer the model but cannot add a tool. The hint of
  a ready Codex tells the user that their global Codex instructions apply. A private
  `CODEX_HOME` would close it but would also lose the sign-in, which Sonara never reads or
  copies. `chat_codex_binary.rs` pins the gap (`#[ignore]`d, local): it fails when a Codex
  release lets it be closed.
- **The user's skills** are kept out with `skills.include_instructions=false`: with it, the
  catalog entry of a canary skill is not in the request; without it, it is (the same test). An
  explicit `$skill-name` mention in the prompt was not tried. Whether `hooks.json` loads under
  `--ignore-user-config` was not verified (`features.hooks=false` is set). Codex keeps its
  sign-in in the real `CODEX_HOME`.
- **No live deny test has been recorded for Codex.** See "Tests".

## Stopping

**Stop** returns at once and the task becomes `cancelled` when the process has ended. The first
signal is always `SIGINT` to the process; the escalation goes to its process group, which also
ends the `sonara mcp` child.

| | First signal | Then |
|---|---|---|
| Claude Code | `SIGINT`: it ends the turn and still writes its `result` (exit 130). `SIGTERM` would exit with 143 and no result | after 10 s `SIGTERM` to the group, after 5 s more `SIGKILL` |
| Codex CLI | `SIGINT`: the first one interrupts the turn, with no `turn.*` event, and exits 1 (from source; a second `SIGINT` is not acted on) | after 4 s `SIGTERM` to the group, after 2 s more `SIGKILL` |

The Codex graces are constants chosen for a quick answer to the button, and were not measured on
a real turn. Whether a task is `cancelled` is decided by the host's stop flag, never by the exit
code (Codex exits 1 on both a failure and an interrupt) or by a missing result.

Quitting Sonara ends the run before it exits, because the escalation above runs on a timer
thread that does not outlive the application, and an agent that ignores `SIGINT` would be left
running with its `sonara mcp`. It sends `SIGINT`, waits up to 2 s for the process to end, then
sends `SIGTERM` to the process group, and `SIGKILL` 0.3 s later, all on the quitting thread, and
refuses further calls. A helper outside the process group that keeps the output pipes open delays
the end of a run by at most 2 s: after that the exit is reported without the rest of the
output.

## Lifecycle and privacy

The history (the thread, the queue and the logs) is **in memory only** and owned by Rust. It is
never written to disk and never part of a `.sonara` project or of any export. It is wiped:

- when the chat window closes (the run is stopped first);
- when a project is opened, created or restored from an autosave, and when the project is closed:
  the session reports `Loaded` or `Closed`. A save does not wipe it;
- by **Clear history** (a confirmation is asked only when it would stop a running task or drop
  queued ones).

A wipe stops the running task, drops the queue and the logs, and starts a new generation. Events of
an older generation are ignored. No new run starts until the process of the old one has ended
(up to 15 s for Claude Code, 6 s for Codex), so two agents never edit the project at once. Wiping
only the window would leave an agent editing the new project, which is why it is done in Rust.

`crates/sonara-app/tests/chat_save.rs` is the proof that chat history never reaches a saved
project. Two application contexts run the same fixed script of edits and saves (a bundle, an
in-place save with its backup, a packaged archive); one has a chat in use with a sentinel text in
its prompts and replies, and tasks that are done, failed, cancelled, edited, removed, moved and
wiped. Every saved file is byte-identical, and no file under the context's folder holds the
sentinel. The test also checks that a chat call never changes the session's version or undo
history or marks the project dirty, that the chat sources import nothing from the project crates
or the project services (which is what keeps the chat out of the export paths), and that the
project schema has no chat property. The project format and the tool schemas do not change
(CTR-01).

What leaves the machine is what any use of the CLI sends to its provider: the prompt, the replayed
conversation and the tool results. Each CLI keeps data of its own that Sonara does not control:
user-level configuration of the CLI can still influence a run, and Codex keeps state files in
`$CODEX_HOME` (SQLite files appeared there on an `--ephemeral` run; their contents were not
inspected).

## The system prompt

`crates/sonara-app/src/composer/system_prompt.md` is appended to the agent's own instructions. It
says what Sonara is, that the user watches the timeline live, how to work in small visible steps
(frame, tracks, material section by section, sound, check), the conventions of the tools (beats,
dB, opaque identifiers, error envelopes), what the agent must not do, and how to end. On top of it
`crates/sonara-app/src/chat/system_prompt_chat.md` says that this session is a chat: answer a
question or small talk in plain text without touching the project, edit only when asked, treat the
`<earlier_conversation>` block as context only, keep the safety rules, and reply in the language
of the message.

## Limits

- One run at a time. At most 200 turns for Claude Code (`--max-turns`); Codex has no turn limit
  set. A message is at most 16 000 characters.
- At most 50 waiting tasks in the queue.
- Each tool call of the agent is its own undo step; a task is not one step.
- Claude Code reports a client-side cost estimate and a turn count. Codex reports tokens only,
  no cost and no turn count, and no text deltas: the window shows whole messages.
- The tools that Codex denied are found by matching the error text of a refused call. That text
  was read from source and not seen in a real run, so the list can be incomplete.
- Every task starts a new provider process and a new `sonara mcp`; this cold start was not
  measured. Whether `sonara mcp` exits on its own after its provider was killed is not
  measured either.
- An edit that an old run still lands after a wipe (the stop is a `SIGINT`, and a tool call that
  is already in flight completes) is not sealed into an undo step of its own: the history is
  sealed when the controller publishes a change, and the exit of a wiped generation publishes
  none. It can merge with the user's next edit, and after a project was replaced it lands in the
  new project.
- macOS (Unix) only: the run module uses process groups and signals.

## Tests

- **Unit tests** in `crates/sonara-app/src/chat/`: the rules of the queue (`state.rs`), the replay
  bounds, the controller against fake providers (shell scripts that behave like a Claude Code
  run), and per provider the command line, the escaping of `-c` values, detection and the stream
  parser. The composer's own unit tests (parser, command line, workspace, search for `claude`,
  process handling with a fake `claude`) stay in `composer/`.
- **Fake CLI:** `crates/sonara-app/tests/chat_codex.rs` runs the real `CodexProvider` against a
  fake `codex` script: detection, the command line, the parser and the stop policy, with only the
  model missing.
- **Fixtures reconstructed from source:** `crates/sonara-app/tests/chat_codex_stream.rs` feeds the
  `fixtures/codex-exec-reconstructed-0.166.0-*.jsonl` files to the parser and asserts the exact
  event sequences. They were reconstructed from the Codex source, not recorded from a real run:
  ids and numbers are invented, and the text of a denial error is the sentence in the source.
- **Privacy:** `crates/sonara-app/tests/chat_save.rs` (see above).
- **Commands match the window:** `apps/sonara-desktop/src-tauri/tests/chat_commands.rs` checks
  that every command the window calls is registered and takes the arguments the client sends,
  and that the event, the window label and its capability agree.
- **The window:** `apps/sonara-desktop/tests/unit/chat*.test.ts` and
  `tests/page/chat.spec.ts`; both run in `pnpm run test`, which the frontend CI job runs.
- **Local only, ignored by default (HW-01):**
  - `crates/sonara-app/tests/chat_codex_binary.rs` needs the `codex` binary, no login and no
    quota. Run it after every Codex upgrade and every change of the boundary keys (`CODEX_BIN`
    selects another executable):
    `cargo test -p sonara-app --test chat_codex_binary -- --ignored --nocapture`.
    It asserts, against the real CLI, that `--strict-config` accepts the keys of the boundary
    (config loading passes and only the required `sonara` server fails to start, before any
    turn), that the developer instructions come back from `codex debug prompt-input` exactly as
    written (the TOML escaping is right), that none of `exec_command`, `write_stdin`,
    `view_image`, `spawn_agent`, `send_message` and `followup_task` is offered (also not inside
    `exec`), and that an unknown key is rejected. It ran and passed against 0.166.0 on
    2026-10-30. A further test there, `codex_offers_no_denied_sonara_tool_with_a_live_sonara_server`
    (it also needs `cargo build -p sonara`, or `SONARA_BIN`, and skips when `codex` or `sonara`
    is missing), starts a headless session and the real command line against a fake Responses
    endpoint, runs `ALL_TOOLS` inside `exec` and asserts that exactly the allowed Sonara tools
    are offered and no denied one is.
  - `apps/sonara/tests/chat_live.rs` drives the real providers through `ChatController` against
    a headless `sonara serve --audio null`. For each of Claude Code and Codex there are four
    tests: no shell and no file written, the project cannot be saved, a plain question is
    answered, and an edit reaches the pinned session. They need a signed-in CLI and spend quota:
    `cargo test -p sonara --test chat_live -- --ignored --nocapture --test-threads=1`, or
    `cargo test -p sonara --test chat_live codex -- --ignored --nocapture` for one provider.
  - `apps/sonara/tests/composer_live.rs` still runs a real Claude Code against a headless session
    with the composer's command line:
    `cargo test -p sonara --test composer_live -- --ignored --nocapture`.

No live Codex turn has been recorded: the Codex stream is verified against fixtures and a fake
CLI, and its boundary against the real binary only as far as configuration loading and the
offered tools go. This document does not record a run of `chat_live.rs` for either provider.
