# Orchestra research and design

Research records and the proposal for orchestral and keyboard instruments in Sonara. Start with the summary
([../orchestra-research.md](../orchestra-research.md)) and the proposal ([../orchestra-design.md](../orchestra-design.md),
[ADR 0011](../decisions/0011-orchestral-instruments-and-performance.md)). Nothing here is implemented.

| Path | Content |
|---|---|
| [model.md](model.md) | The 85 instrument ids: ranges, transpositions, clefs, GM programs, articulations, keyboard mechanics, proposed corrections to the orchestration metadata. Generated from the cards' json blocks |
| [cards/](cards/) | One research card per instrument group (ten fields per instrument, every fact Cited or Estimate, with a fact-check log and a consistency review), plus [cards/free_libraries.md](cards/free_libraries.md) and [cards/apple.md](cards/apple.md) |
| [ensembles.md](ensembles.md) | Score order, part naming, seating, ensemble templates |
| [doubling-and-balance.md](doubling-and-balance.md) | Functions, doubling, balance, voicing and 67 checkable rules |
| [articulation-vocabulary.md](articulation-vocabulary.md) | The canonical articulation ids and modelling options |
| [sound-mapping.md](sound-mapping.md) | Library presets per instrument and articulation, the degradation chain, the GM table |
| [sfz-conformance.md](sfz-conformance.md) | What Sonara's SFZ parser and sampler do with these libraries, checked in the code |
| [coverage.md](coverage.md) | Coverage matrices, ranked gaps, a starter kit of user-installed libraries |
| [open-questions.md](open-questions.md) | 170 triaged open questions: owner decisions, human checks, data disagreements, follow-ups |
| [design/](design/) | The four layers of the proposal: instrument model, sound source, performance layer, user interface |
| [implementation-plan.md](implementation-plan.md) | The 111 work items of the four layers as one backlog per milestone, with owner gates, the first audible result and done criteria |

Conventions: MIDI note numbers with C4 = 60; ranges are sounding; transposition = sounding − written. Sonara bundles
no sample library (ADR 0010).
