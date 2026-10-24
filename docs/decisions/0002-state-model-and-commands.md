# ADR 0002: Immutable project snapshots, command bus and snapshot undo

- **Status:** Accepted (2026-10-20)

## Context

The GUI, the CLI, the MCP server and the audio renderer must all see a single authoritative
project. Edits come from several threads (Qt main thread, IPC worker threads, background
jobs), while the render thread reads the project continuously. Every edit must be undoable,
and agent edits must report exactly which objects they touched.

## Options considered

1. **Mutable object graph with observers** (classic MVC). Cheap edits, but the render thread
   needs locks or deep copies, undo needs hand-written inverse operations per command, and
   partial failures can leave the model half-edited.
2. **Immutable snapshots.** The project is a tree of frozen dataclasses. A command is a pure
   function `Project -> Project`. Unchanged subtrees are shared between snapshots.
3. **Event sourcing.** The project is a fold over an event log. This gives a full history,
   but a schema change becomes an event migration problem, and loading is slow for large
   sessions.

## Decision

Option 2.

- `sonara.domain` contains frozen, slotted dataclasses with tuple collections and a small
  hashable `FrozenMap`. The domain has no I/O, no Qt and no audio-device code.
- Every mutation is a `Command` (a frozen dataclass) whose `apply(project)` returns a new
  snapshot plus a `ChangeSet` (created, updated and deleted object IDs by kind). Commands
  generate their new IDs at construction time, so `apply` is deterministic and replayable.
- `ProjectSession` is the single owner of the current snapshot. It serializes commands
  under a lock, assigns a monotonically increasing `version`, pushes undo entries and
  notifies subscribers after the commit, outside the lock.
- Undo/redo is **snapshot-based**: an entry stores the before and after snapshots. With
  structural sharing this costs only the changed path. Consecutive commands with the same
  `merge_key` (for example a fader drag) collapse into one undo step within a time window.
- Readers (renderer, GUI, queries) take a snapshot reference and never need a lock.
- Validation happens inside `apply`. A failing command raises a typed `CommandError` and
  leaves the session unchanged, so commands are transactional by construction. A batch
  (`Transaction`) applies several commands atomically as one undo step.
- `dry_run=True` runs `apply` and returns the `ChangeSet` without committing it.

## Consequences

- No inverse-operation code is needed and no class of undo bugs exists. Memory is bounded by
  a configurable undo depth.
- Large note lists are copied as tuples on edit (O(n) references). This is acceptable for
  clips with tens of thousands of notes. Revisit with persistent vectors if profiling shows
  otherwise.
- Media files are immutable once added. Undo never needs to restore audio data, and media
  cleanup must consider snapshots that are still reachable from the undo history.
