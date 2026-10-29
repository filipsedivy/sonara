# Conversational orchestration (SON-P13)

`sonara_agent::orchestration` turns a request in plain language into a plan of calls to the
existing agent tools, runs the plan and reports the result. It is a library API: it adds no tool,
schema field or error code (CTR-01), and it never writes the project itself. Each effect is one
tool call through a `ToolRunner` (`AgentService` implements it), so the tools' validation,
permissions and audit log apply to every step.

## What it understands

The planner is deterministic and needs no language model.

| Request | Tool | Notes |
|---|---|---|
| "set the tempo to 96 bpm" | `set_tempo` | one number, 10 to 999 |
| "use Poly Synth on Keys" | `set_instrument` | one instrument plugin and one instrument track, matched by name |
| "delete track Drums" | `delete_track` | destructive |

Requests can be joined with `and`, `then`, `,` or `;`. Track names match on word boundaries, and a
longer matching name wins ("Keys 2" is not "Keys").

## Flow and guarantees

1. `plan(request)` calls only read tools (`list_tracks`, `list_plugins`).
2. If any part is unknown, a number is missing or out of range, or a track or instrument is not
   found or not unique, the result is a list of questions (`NeedsClarification`). Nothing is
   changed, not even the clear parts of the request.
3. A plan with a destructive step is only proposed (`NeedsConfirmation`) until the caller runs it
   with `Approval::Confirmed`.
4. Every step is validated with `dry_run` before the first commit. A refusal changes nothing
   (`Rejected`).
5. Steps then commit in order. A failure stops the plan (`Failed`); the report lists the steps
   already applied, which are undoable through the normal history.
6. If the tool layer demands its own `confirmation_token` for a confirmed destructive step, the
   step is repeated with that token.

The `Report` holds the status, the plan, the executed steps with their error codes and a summary.

## Limits

No relative requests ("faster"), no composition requests, no language model. These produce
questions. Tests use a stub runner (`crates/sonara-agent/tests/orchestration.rs`).
