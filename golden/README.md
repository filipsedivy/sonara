# Golden data

This folder holds the behaviour recorded from Sonara's original Python prototype: tool, MCP, IPC
and CLI contracts, kernel and plugin vectors, renders, media imports and exports, MIDI files,
transport scenarios and complete agent workflows. The Rust tests compare against it, through
`sonara-testkit` and the crates' `tests/` (GLD-01).

- **Provenance.** `MANIFEST.json` lists every file with its SHA-256, the areas, and the
  recording environment (macOS 27 on arm64, Python 3.13, NumPy 2.5.4, SciPy 1.18.1,
  libsndfile 1.2.2). The prototype's recorder, `scripts/golden/record.py` and its `area_*.py`
  and `*_oracles.py` scripts, was removed with the prototype; test comments still name the script
  that recorded each area.
- **Frozen.** The data is never re-recorded and its tolerances are never loosened to make a
  test pass (GLD-01). A deliberate behaviour change, such as a fixed prototype defect, is
  written next to the test that pins the new behaviour.
- **Git LFS.** Audio, vectors and encoded exports are stored with Git LFS (`.gitattributes`); the
  JSON stays in normal git. Run `git lfs pull` after cloning; the tests report a missing or
  pointer-only file. `SONARA_GOLDEN` points the tests at another copy.
