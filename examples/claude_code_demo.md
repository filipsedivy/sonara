# Claude Code demonstration (recorded 2026-10-20)

A real, non-interactive Claude Code session drove Sonara **only through its MCP tools**. The
session ran Claude Sonnet via `claude -p` and the headless MCP server of the original Python
prototype, which `sonara mcp` replaces with the same tools and flags.

## How it was run

```bash
claude -p "$(cat examples/claude_code_demo_prompt.txt)" \
  --mcp-config mcp.json --strict-mcp-config --allowedTools mcp__sonara --model sonnet
```

`mcp.json` started the server; with the Rust CLI that is
`sonara mcp --connect never --audio null --base-dir <dir> --allow <dir>`.
Pass `--connect auto` instead to attach the session to an open Sonara window.

The request is in [claude_code_demo_prompt.txt](claude_code_demo_prompt.txt): a 32-bar house
track at 124 BPM in A minor with an intro, a drop and an outro. It asks for drums, a
sidechained bass, a pad on a reverb bus and a lead in the drop. The session had to preview,
analyse, fix problems, export WAV and MP3 at −14 LUFS plus MIDI, and save.

## What happened

The session took 52 turns and made **49 tool calls with 0 errors**. The audit log counts:

| Tool | Calls |
|---|---|
| `generate_midi` | 10 |
| `render_preview` | 7 |
| `create_track` | 5 |
| `list_plugins` | 4 |
| `add_section`, `add_effect`, `set_effect_parameter`, `set_track_volume` | 3 each |
| `verify_project`, `export_audio` | 2 each |
| `get_capabilities`, `create_project`, `configure_routing`, `get_project_state`, `export_midi`, `save_project`, `list_tracks` | 1 each |

The agent inspected the rendered analysis and fixed the problems it found:

1. **True peak.** The first preview read −0.75 dBTP, above −1. The agent added a master
   limiter and rebalanced the mix.
2. **Balance.** Per-track loudness showed the pad (−15.9 LUFS) and the lead (−15.5 LUFS)
   5–8 LU above the drums (−20.5) and bass (−23.0). It lowered their faders.
3. **Sidechain.** The bass compressor was keyed by the whole drum kit, so hats and claps
   kept it ducked. It retuned the threshold, ratio, attack and release. This finding led to a
   new `sidechain_lpf_hz` parameter on the compressor; the `Sidechain Pump` preset now keys
   on the kick at 150 Hz.
4. **Correlation.** A correlation dip to −0.45 was traced to the decorrelated reverb tail,
   by comparing pad-only, bus-only and no-tail renders. The agent left it unchanged.

Final export: **−14.0 LUFS integrated and −1.07 dBTP**. The MP3 decodes at −1.05 dBTP. The
file runs 66.15 s: 61.9 s of music plus the reverb tail. The MIDI file holds 748 notes on 5
tracks, the project was saved, and `verify_project` reported it intact.
