// The window's two themes (owner, 2026-10-24): the prototype's dark and
// light palettes of `sonara/ui/theme.py`, which theme.svelte.ts applies to the page's panels as CSS
// custom properties at startup and whenever the stored `theme` setting changes (the Settings
// dialog). The canvases (the
// timeline, the ruler, the piano roll, the meters and the EQ curve) stay dark in both, as the
// feature matrix's "partial" light theme has it: they draw with `timeline/colors.ts`'s `THEME`,
// which is the dark palette below. tests/fixtures/theme-v1.json, recorded from the prototype by
// scripts/golden/gui_theme_oracles.py, pins both palettes (tests/unit/theme.test.ts).

import { THEME } from "./timeline/colors";

/** The stored setting's two values. */
export type ThemeName = "dark" | "light";

/** A palette: the fields of the prototype's `Theme`. */
export type Palette = { readonly [K in keyof typeof THEME]: string };

/** `theme.LIGHT`. */
const LIGHT: Palette = {
  window: "#e9eaed",
  panel: "#f4f5f7",
  panelAlt: "#e2e4e8",
  lane: "#f7f8fa",
  laneAlt: "#eef0f3",
  gridBar: "#a3a8b2",
  gridBeat: "#cdd1d8",
  gridSub: "#e1e4e9",
  text: "#202226",
  textDim: "#5d636d",
  accent: "#1f6fd1",
  selection: "#d18e00",
  playhead: "#e23b30",
  loop: "#1f6fd1",
  meterGreen: "#2e9f52",
  meterYellow: "#c8a21e",
  meterRed: "#d33a3a",
  border: "#b9bdc4",
};

/** Both palettes; the dark one is the canvases' `THEME`. */
export const PALETTES: Readonly<Record<ThemeName, Palette>> = { dark: THEME, light: LIGHT };

/** The palette `theme.apply` picks for a stored setting: light for "light", dark for anything else. */
export function themeName(setting: string | null | undefined): ThemeName {
  return setting === "light" ? "light" : "dark";
}

/**
 * The CSS custom properties of `name`: one per palette field, and three without a field of their
 * own. `--bg` (text fields) is the window colour; `--bar` (fields, lists and the status bar) is
 * `#202226` in the dark theme and the panel colour in the light one, Qt's `Base` role of
 * the prototype's fields and lists; `--error` is `#ff8a80` in the dark theme and the
 * meter's red in the light one.
 */
export function cssTokens(name: ThemeName): Record<string, string> {
  const p = PALETTES[name];
  return {
    "--window": p.window,
    "--bg": p.window,
    "--panel": p.panel,
    "--panel-alt": p.panelAlt,
    "--bar": name === "dark" ? "#202226" : p.panel,
    "--lane": p.lane,
    "--lane-alt": p.laneAlt,
    "--grid-bar": p.gridBar,
    "--grid-beat": p.gridBeat,
    "--grid-sub": p.gridSub,
    "--border": p.border,
    "--text": p.text,
    "--text-dim": p.textDim,
    "--accent": p.accent,
    "--selection": p.selection,
    "--playhead": p.playhead,
    "--loop": p.loop,
    "--meter-green": p.meterGreen,
    "--meter-yellow": p.meterYellow,
    "--meter-red": p.meterRed,
    "--error": name === "dark" ? "#ff8a80" : p.meterRed,
  };
}
