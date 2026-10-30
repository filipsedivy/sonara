# AI Composer

The AI Composer is a harness that runs a headless Claude Code (`claude -p`) as an agentic composer
inside the open project. Choose **AI → AI Composer…**, describe the music, and press Compose: the
agent adds tracks, MIDI and effects to the timeline while you watch, and the panel ends with
everything Claude Code returned. The code is `sonara_app::composer` (the logic) and
`apps/sonara-desktop/src-tauri/src/composer.rs` (the commands); the panel is
`apps/sonara-desktop/src/lib/composer/`.

## How a run works

1. **Check.** The panel runs `claude --version`. `claude` is looked for on the `PATH`, then in
   `~/.local/bin`, `~/.claude/local`, `~/.npm-global/bin`, `/opt/homebrew/bin` and
   `/usr/local/bin`, because an application started from the Finder has the minimal `PATH` of
   `launchd`. Claude Code 2.1.259 or newer is required (`--permission-prompts`). Whether it is
   signed in is not probed: a run that is not reports it in its result.
2. **Pin.** The run gets a private folder (mode `0700`, removed afterwards) with an empty working
   directory and a `SONARA_HOME` whose `run/` holds a copy of **this window's** discovery file
   only. `sonara mcp` attaches to the newest running Sonara, so without this a second instance
   could be edited instead of the window you see. `--connect always` makes the attach fail instead
   of falling back to a headless session nobody can see.
3. **Run.** `claude -p --output-format stream-json --verbose --include-partial-messages` with the
   prompt on stdin, the system prompt appended, the MCP server `sonara` (the bundled `sonara
   mcp`), and the tool boundaries below. Its edits go through the window's agent session, so the
   timeline, mixer and piano roll update as for any agent, and the whole run is one undo step
   (see Undo below).
4. **Stream.** The output is parsed into events (`composer::stream`): the session start with the
   MCP status, text as it is written, tool calls and their answers (a Sonara `"ok": false`
   envelope counts as failed), API retries, and the final `result`.
5. **End.** The panel shows the outcome and everything the result carries: the final message,
   time, turns, estimated cost, tokens, session id, denied tools, the exit code, and the whole
   `result` line and standard error under "Full output".

## Tool boundaries

| | Tools |
|---|---|
| Allowed | the whole `sonara` MCP server, `WebSearch`, `WebFetch` |
| Denied | `Bash`, `Edit`, `Write`, `NotebookEdit`, `AskUserQuestion`; the Sonara tools `save_project`, `open_project`, `create_project`, `export_audio`, `export_midi`, `export_stems`, `render_preview`, `relink_media`, `start_recording`, `set_midi_input`, `verify_project`, `analyze_audio`, `get_audio_analysis` |

`--permission-mode dontAsk` and `--permission-prompts none` deny everything else instead of
waiting for an answer nobody can give. A web page can carry a prompt injection, which is why the
agent has no shell and no way to write files. Operations that need a confirmation
(`CONFIRMATION_REQUIRED`, for example deleting a track with content) cannot be confirmed by a
headless run, and the system prompt tells the agent to leave them out.

`--bare` is not used: it reads only `ANTHROPIC_API_KEY` and never the subscription login.
Isolation comes from `--strict-mcp-config` (no MCP server of the user's own) and the empty working
directory (no repository `CLAUDE.md`, `.mcp.json` or project hooks). Hooks and `CLAUDE.md` of
`~/.claude` still load.

## The system prompt

`crates/sonara-app/src/composer/system_prompt.md` is appended to Claude Code's own. It says what
Sonara is, that the user watches the timeline live, how to work in small visible steps (frame,
tracks, material section by section, sound, check), the conventions of the tools (beats, dB,
opaque identifiers, error envelopes), what the agent must not do, and how to end (a short
summary in the language of the brief).

## Stopping

**Stop** sends `SIGINT` to Claude Code, which ends the turn and still writes its `result`
(`SIGTERM` would exit with 143 and no result). A run that does not end within 10 s gets `SIGTERM`
and, 5 s later, `SIGKILL`, both to its process group, which also ends the `sonara mcp` child.
Quitting Sonara stops a run the same way.

## Undo

A run is one undo step, **Undo AI Composer**, and a stopped or failed run is one too.

- When a run starts, the window's undo history opens a group (`ProjectSession::begin_undo_group`)
  and seals the last step. While the run goes, its steps (source `agent`) stay separate, so the
  agent's own `undo` tool still takes back exactly one step instead of the whole run.
- When the process has exited, and before the panel hears of it, the group is closed
  (`end_undo_group`): the run's steps become one, with the project from before the first as its
  `before` and the one after the last as its `after`. The step is sealed, so your next edit never
  merges into it. A run that changed nothing leaves the history alone.
- Nothing about the tools changes (CTR-01): this is a rule of the window's history, not of the
  agent API.

Limits of this:

- An edit **you** make while the run goes is a step of its own and splits the run into two steps
  (before and after it). Each undo snapshot is the whole project, so merging it would take your
  edit along.
- Another agent attached to the same window also has the source `agent`, and its edits during a run
  join the run's step. The source does not tell agents apart without an IPC change.
- The history keeps 500 steps. A run of more steps than that loses its oldest ones before the
  group is closed, as any history does.

## Limits

- One run at a time; at most 200 turns and a 16 000 character prompt.
- The cost is Claude Code's own client-side estimate.
- macOS (Unix) only: the run module uses process groups and signals.

## Tests

- Undo groups: `crates/sonara-core/tests/history.rs` (the session) and
  `crates/sonara-agent/tests/undo_group.rs` (tool calls through the agent service).
- Unit tests of the parser, the command line, the workspace, the search for `claude` and the
  process handling (with a fake `claude`), in `crates/sonara-app/src/composer/`.
- `apps/sonara-desktop/tests/unit/composer.test.ts` and `tests/page/composer.spec.ts` for the
  panel.
- `apps/sonara/tests/composer_live.rs` runs a real Claude Code against a headless session. It is
  local only and ignored by default, because it spends quota:
  `cargo test -p sonara --test composer_live -- --ignored --nocapture`.
