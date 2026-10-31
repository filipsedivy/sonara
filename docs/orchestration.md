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
| "set the tempo to 96 bpm" | `set_tempo` | one number, 10 to 999, once per request |
| "use Poly Synth on Keys" | `set_instrument` | one instrument plugin and one instrument track, matched by name |
| "delete track Drums" | `delete_track` | destructive |

Requests can be joined with `and`, `then`, `,` or `;`. Track names match on word boundaries, and a
longer matching name wins ("Keys 2" is not "Keys").

`execute` also runs a plan built by hand, with the same checks.

## Reading orchestration requests

`grammar::parse(request)` reads a request that gives instruments something to play, in English or
Czech, into clauses and the questions it leaves open. It reads no project and calls no tool, and
the same request always reads the same. `Orchestrator::plan` still understands only the three
request kinds above; the grammar is its own entry point.

"strings play a legato melody, horns and timpani underneath" and "smyčce hrají legato melodii,
lesní rohy a tympány pod tím" both read as two clauses: the strings with the role melody and the
articulation legato, then the horns and the timpani with the role "underneath".

| A clause holds | Examples |
|---|---|
| instruments, categories and families | the aliases of `sonara_midi::orchestration::resolve` in English and Czech, every listed case: "horns", "lesních rohů", "tympány", "strings", "clarinets" |
| a count, the parts of a section, solo or section | "two horns", "čtyři lesní rohy", "4 horns"; "first violins", "1. housle", "horn 3", "horns 1 and 2"; "solo violin", "sekce" |
| instruments that double the others | "violins with flute", "s pěti lesními rohy" |
| a role | melody, counter-melody, harmony ("chords", "akordy"), bass line ("basovou linku"), pedal, rhythm, colour, or "underneath" ("pod tím"), which leaves the role to the instrument |
| an articulation, a mute and a vibrato | the vocabulary's ids ("legato", "col legno"), "pizz", "držené", "trylky", "con sordino", "s dusítkem", "senza vibrato", "ordinario" |
| a dynamic and a change of it | "pp" to "ff", "softly", "potichu", "velmi nahlas"; "crescendo", "zesiluje", "diminuendo", "swell" |
| bars, a key, a density | "in bars 5-12", "v taktech 5–12"; "in D minor", "in F# minor", "v d moll", "v b dur" (B♭); "full", "plně", "thin" |

Czech forms are listed one by one, as the instrument aliases are; nothing is stemmed. When an
alias and a word of the grammar start at the same word, the longer one wins and the alias wins a
tie: "bass line" is a role, "bass" alone an instrument, and "piano" always the instrument.

**Clauses.** A semicolon and "then" ("potom", "pak") end a clause. A comma, and "and" ("a", "i")
anywhere but in a list of instruments, part the request too, but a part that only lists
instruments joins the part after it ("flutes, oboes and clarinets play the melody"), and a part
that names no instrument but adds a role or a manner of playing continues the part before it
("strings play the melody, softly"). So "horns and timpani underneath" is one clause, and "a"
works both as the Czech "and" and as the English article. A clause that names no instrument and
gives no role or manner of playing ("set the tempo to 96") has no assignment and is left to the
planner's request kinds.

**Questions.** Nothing is to be changed while one is open:

- words where an instrument belongs that name none (after a count, beside an instrument in a list,
  after "with", or before the verb or the "underneath" of a clause without instruments), with up
  to three "did you mean" suggestions: aliases one edit away for words of four or five letters,
  two edits for longer words, none for shorter ones. The distance (`strsim`) only suggests, it
  never matches: "viola" and "violin" are one edit apart;
- words that name several targets ("bass": the double bass or the bass voice) and a category
  without a usual member ("bells");
- "horns" in a request that names nothing an orchestra template plays: French horns, or a brass
  and saxophone section;
- a clause without instruments, two different values of one kind in a clause ("legato
  staccato"), bars that are not a range, and a count of 0.

Other unknown words are read past and listed in the clause's `ignored` words.

## Steps that use an earlier step's result

A step can take a value from the reply of an earlier step of the same plan, such as the id of the
track that `create_track` makes. The value does not exist when the plan is made, so the step lists
a `StepRef`, and its argument holds the reference's placeholder until the step runs:

```rust
// Step 1 writes a clip on the track that step 0 creates.
let track = StepRef::new("/track_id", 0, "/result/track_id");
let args = json!({"track_id": track.placeholder(), "start": 0, "length": 4});
// args["track_id"] is "{step 0: /result/track_id}" until step 0 has committed.
```

- `arg` is a JSON pointer into the step's arguments (`/track_id`, `/add_sends/0/target`).
- `step` is the index of an earlier step in the plan.
- `pointer` is a JSON pointer into that step's reply. Edit tools put the ids they create under
  `result` (`create_track` replies with `result.track_id` and `result.instrument_device_id`). The
  value there must be a string.
- The placeholder may stand alone or inside a longer string: `"{step 0: /result/x}:dynamics"`
  becomes `"dev_7:dynamics"`. It must appear exactly once in its argument.

The value comes from the committed call's reply, never from a dry run: a dry run makes ids that
the commit does not use.

## Flow and guarantees

1. `plan(request)` calls only read tools (`list_tracks`, `list_plugins`).
2. If any part is unknown, a number is missing or out of range, the tempo is set more than once,
   or a track or instrument is not found or not unique, the result is a list of questions
   (`NeedsClarification`). Nothing is changed, not even the clear parts of the request.
3. A plan with a destructive step is only proposed (`NeedsConfirmation`) until the caller runs it
   with `Approval::Confirmed`.
4. **Validation before the first commit.** Every reference must name an earlier step and sit in
   its argument. Every step without references must pass its tool's `dry_run` against the project
   as it is. A step with references cannot be dry-run yet, so it must pass its tool's input
   validation with the placeholders in place (unknown fields, types, ranges). A refusal here
   changes nothing (`Rejected`).
5. **Stages.** The plan commits in stages. A stage ends before the first step that needs a value
   from a step of the same stage, so a plan without references is one stage. Before any step of a
   later stage commits, its references are filled in from the earlier replies, and every step of
   the stage is dry-run against the project as the earlier stages left it.
6. **Rollback.** If anything fails after the first commit (a dry run of a later stage, a
   reference whose value is missing from the reply, or a commit), the plan stops (`Failed`), and
   the steps it already applied are undone with the `undo` tool, newest first. A step whose reply
   says it changed nothing (`committed: false`) made no undo step and is skipped.
7. **The rollback never undoes someone else's edit.** Before each `undo` it reads
   `get_project_state`: the newest undo step must carry the label of that step's own commit
   (`operation` in its reply), and the project version must have moved on since that commit only
   by the plan's own later commits and the rollback's own undos. If either check fails, for
   example because the user edited the project while the plan ran, the rollback stops there, and
   the report says why and which steps are still applied. They stay undoable through the normal
   history.
8. If the tool layer demands its own `confirmation_token` for a confirmed destructive step, the
   step is repeated with that token.

In short: a plan that is refused before its first commit changes nothing; a plan that fails later
is undone as far as the checks of item 7 allow, and the report says exactly how far.

The rollback relies on every applied step having made exactly one undo step of its own. Every
edit tool commits one undo step, but the history merges two edits of the same value made within
1.5 seconds one after the other (the tempo, or the pan or volume of one track) into one. The
planner therefore never sets the tempo twice in one plan, and a plan built by hand must not change
the same value in two steps in a row. An edit of the same value that the user made less than 1.5
seconds before the plan's first step can merge into that step and is then undone with it; inside
an undo group (`sonara_app::edit::begin_undo_group`) the steps of the agent service never merge
with the user's. The undone steps move to the redo stack, so Redo can apply them again.

## The report

The `Report` holds:

- `status`: `done`, `needs_clarification`, `needs_confirmation`, `rejected` or `failed`;
- `plan`: the plan as made, with references still as placeholders;
- `executed`: the steps that committed, then the step that failed or was refused. Each
  `StepResult` holds the step as it ran (references filled in), whether it succeeded, the error
  code and message, the tool's reply (the commit's, or the dry run's for a refused step; none for
  a step refused without a call) and whether the rollback undid it. A reference that cannot be
  used is reported as `INVALID_ARGUMENT`, and a step with references for a tool that does not
  exist as `NOT_FOUND`;
- `rollback`, only when the run `failed`: whether every applied step was undone and, if not, why
  the rollback stopped;
- `summary`: the plan and its outcome in a sentence or two for the user.

## Limits

No relative requests ("faster"), no composition requests, no language model. These produce
questions. Tests use stub runners and the real agent service
(`crates/sonara-agent/tests/orchestration.rs`, `crates/sonara-agent/tests/orchestration_stages.rs`).
