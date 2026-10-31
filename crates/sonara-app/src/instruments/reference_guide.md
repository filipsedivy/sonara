# Orchestral parts

Sonara knows the orchestral instruments below by id. For an orchestral part:

- One instrument track per part: `create_track` with `"instrument": "sonara.sampler"` and a
  `part` naming the id (`{"instrument": "horn_f", "name": "Horn 1"}`), then `set_instrument`
  with `"plugin_id": "sonara.sampler"` and `"params": {"instrument": "<preset id>"}`.
  `list_plugins {"plugin_id": "sonara.sampler"}` lists the installed presets; without a sampler
  or a preset, use a built-in instrument and say so.
- Keep each part in its range, mostly in the comfortable one.
- A note's `articulation` plays the preset's recording of it, or the nearest one it has; do not
  write switch keys. A preset with one recording plays it for every note: for another
  articulation, use another track with a preset that plays it.
- Velocity sets a note's sample layer and level. Swells and fades on held notes: controller 11
  (expression) with `write_midi_controllers`, which changes the level, not the timbre.
  Controller 64 is the sustain pedal.
- No recorded legato: overlapping notes sound as separate attacks.
