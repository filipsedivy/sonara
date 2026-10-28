# Song form generator (SON-P10)

`sonara_midi::form` plans the sections of a song: a named template, a role for every section, a
length in bars, and energy and density levels. It is a library API. It adds no agent tool, CLI
command or schema field (CTR-01); the agent API belongs to SON-P12.

## Model

- **Role** (`SectionRole`): intro, verse, pre-chorus, chorus, bridge, instrumental, breakdown,
  outro. Roles, energy and density live only in the plan (`SongForm`). The persisted `Section`
  has an ID, a name, a start, a length and a colour, so the plan is **not saved** with the
  project. The name is a display label and never the source of truth for the role.
- **Templates** (`FormTemplate`, `steps()` gives roles and default bars):

  | Name | Sections | Bars |
  |---|---|---|
  | `verse_chorus` | intro, verse, chorus, verse, chorus, outro | 40 |
  | `pop_pre_chorus` | intro, (verse, pre-chorus, chorus) ×2, bridge, chorus, outro | 64 |
  | `aaba` | verse, verse, bridge, verse | 32 |
  | `dance` | intro, (verse, breakdown, chorus) ×2, outro | 96 |

- **Options** (`FormConfig`): `start_bar` (0-based, default 0), `omit_intro`, `omit_outro`, and
  `bars` (a length override per role, 1 to 64). An option that cannot apply (omitting an intro
  the template lacks, resizing an absent role) is an `InvalidArgument` error, never ignored.
- **Levels** (`Level`, whole percent 0–100). Defaults per role: energy intro/outro 20, verse 40,
  pre-chorus 60, chorus 90, bridge 50, instrumental 70, breakdown 30; density 20, 40, 60, 85,
  60, 70, 40. Contrast is derived, not stored: `contrast_with_previous` is the energy change
  from the section before.
- **Labels:** the role's name, numbered per role when the role occurs more than once
  ("Verse 1", "Verse 2", "Bridge").

## Timing

Lengths are bars. Start and length in beats come from the project's `TimeSignatureMap`
(`bar_to_beat`), so 3/4, 7/8 and meter changes inside a song are exact, and each section starts
where the previous one ends. Nothing is random: the same configuration, meter map and ID source
give the same plan. IDs come from an `IdGenerator` (`SequentialIds` in tests).

## Using the project

- `plan_form(config, signatures, ids)` is pure and touches no project.
- `apply_song_form(session, form, source)` adds all sections in one `Transaction`: one undo
  step, all or nothing. A project that already has sections is a `Conflict` (remove them first).
- `regenerate_section(session, form, id, change, source)` changes one section through a single
  `UpdateSection`: same ID, start and length, new name. It returns the new plan. Other sections
  and their labels are never touched. If the project's section was edited since planning
  (start or length differ), it is a `Conflict`. A length change is deliberately not a
  regeneration, because it would move every later section.
- `regenerate_in_plan` is the same change on the plan alone.

## Research notes (assumptions, not verified against live sources)

- Approaches compared: rule/template forms (chosen), probabilistic or learned structure models.
  Templates are deterministic, inspectable and need no model or licence; learned models add a
  dependency and non-reproducible output. The listed forms are common popular-music
  conventions (AABA, verse–chorus, with a pre-chorus or a dance breakdown/drop); lengths of 4
  and 8 bars follow the usual phrase grouping. No external source was consulted for this
  change, so treat the default numbers as product defaults that can be tuned.
- Motif continuity: sections with the same role are expected to share material, and
  `SongForm::sections()` lets a later task find them. Note-level motifs use the SON-P05
  `theory::motif` types and belong to the regeneration task (SON-P11).

## Acceptance criteria

| Criterion | Evidence |
|---|---|
| Form and sections representable in the project model | `apply_song_form`; `applying_creates_the_sections_in_one_undo_step` |
| Regenerating one section keeps the others | `regenerate_section`; `regenerating_one_section_leaves_every_other_section_untouched` |
| Tests check stability outside the target | the same test, plus `regenerating_an_unknown_section_fails_and_changes_nothing` and `regenerating_a_section_edited_since_planning_is_a_conflict` |

Tests: `crates/sonara-midi/tests/form.rs`.
