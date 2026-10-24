// The theme the page shows now (theme.ts has the palettes): `applyTheme` sets the panels' CSS
// custom properties on the page's root, and components that compute a colour in script read
// `palette()`, which follows the theme.

import { cssTokens, PALETTES, type Palette, type ThemeName } from "./theme";

/** The theme shown now; components that compute colours read {@link palette}. */
export const theme = $state<{ name: ThemeName }>({ name: "dark" });

/** The panels' palette of the theme shown now. */
export function palette(): Palette {
  return PALETTES[theme.name];
}

/** Shows `name` on `root` (the page's `<html>`): its tokens, `data-theme` and the native controls' scheme. */
export function applyTheme(name: ThemeName, root: HTMLElement = document.documentElement): void {
  for (const [token, value] of Object.entries(cssTokens(name))) root.style.setProperty(token, value);
  root.style.colorScheme = name;
  root.dataset.theme = name;
  theme.name = name;
}
