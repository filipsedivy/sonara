# Localized regeneration (SON-P11)

`sonara_midi::regenerate` replaces the notes of one bounded region of a MIDI clip. It is a
library API. It adds no agent tool, CLI command or schema field (CTR-01); the agent API belongs
to SON-P12. It ships **no generator**: the musical algorithm is supplied through the
`RegionGenerator` trait, and the tests use deterministic doubles.

## Flow

1. `propose(session, request, generator, ids)` reads the session, resolves the region, calls the
   generator, validates the candidate and returns a `RegenerationPlan`. The project is not touched.
2. `apply(session, plan, source)` commits the plan as one `Transaction` (`DeleteNotes` then
   `AddNotes`, label "Regenerate Notes") through `ProjectSession::execute`. One undo restores the
   clip. The user can inspect the plan first and simply not apply it to reject it.
3. `regenerate(...)` does both.

## Request

`RegenerationRequest { clip_id, target, constraints, seed }`.

- `target`: `Beats { start, end }` (timeline beats) or `Section(id)` (its start and length).
- `constraints`: `pitch_range` (inclusive) and `max_notes`; a candidate that breaks one is
  rejected. A reversed range or `max_notes` of 0 is an invalid request.
- Only MIDI clips without a loop are supported; anything else is rejected, not approximated.
- The region is half-open, `[start, end)`, must be finite, non-empty and inside the clip. It is
  never clamped silently.

## Write scope and boundaries

- A note belongs to the region when its **start** is in `[start, end)`. Only those are removed.
- A note starting before the region is kept, untruncated, even if it sounds into it. These are
  listed in `plan.crossing`. A note starting in the region is replaced whole, even if it sounds
  past the end.
- A candidate note must start in the region and end by `end`.
- The generator's read-only context is the preserved notes of the clip and the notes of other MIDI
  tracks sounding in the region. Context never widens the write scope.

## Validation

Hard errors (`RegenError::Rejected`, all issues listed): empty candidate, pitch, velocity,
channel or length out of the model's range, a note outside the region, a pitch outside the
constraint, too many notes. Advisory (`plan.warnings`): a new note that overlaps a preserved note
of the same pitch and channel. Nothing is repaired silently.

## Identity, staleness, repeats

The generator returns notes without IDs; the service assigns fresh, unused ones. Retained notes
keep theirs. A plan records `ProjectSession::version()`; `apply` returns `RegenError::Stale`
when it differs, so unrelated edits made during generation are never overwritten and a plan
cannot be applied twice. A failed command leaves the version unchanged, so a retry is safe.

## Errors

`RegenError::code()` maps to `invalid_argument` (request, rejected candidate, generator failure),
`not_found` and `conflict` (stale). A failed project command keeps its own code.

## Cancellation

The interface is synchronous. Not calling `apply` discards a plan; it does not stop a running
generator, and there is no timeout in this layer.

## Not done

A production generator; an agent tool (SON-P12); reconciliation of a stale plan instead of
rejecting it; controller events (CC, pitch bend) and audio clips; instrument ranges from the
orchestral metadata are not read (the caller passes `pitch_range`).
