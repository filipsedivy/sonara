import { expect, test } from "@playwright/test";
import { MIN_TEXT_CONTRAST, contrastRatio, parseColor } from "../../src/lib/chat/contrast";
import { BETA_EXPLANATION } from "../../src/lib/chat/model";
import { PALETTES, cssTokens, type Palette, type ThemeName } from "../../src/lib/theme";

// The AI Chat's colours: WCAG 2.x contrast of the pairs its CSS uses, computed from the palettes
// of lib/theme.ts (the page tests read the rendered colours of the real window).

const THEMES: ThemeName[] = ["dark", "light"];

test("the contrast ratio follows WCAG 2.x", () => {
  expect(contrastRatio("#000000", "#ffffff")).toBeCloseTo(21, 5);
  expect(contrastRatio("#ffffff", "#000000")).toBeCloseTo(21, 5);
  expect(contrastRatio("#336699", "#336699")).toBeCloseTo(1, 5);
  // The well-known grey: #767676 on white is the lightest grey that passes AA, 4.54:1.
  expect(contrastRatio("#767676", "#ffffff")).toBeCloseTo(4.54, 2);
  expect(contrastRatio("#777777", "#ffffff")).toBeLessThan(MIN_TEXT_CONTRAST);
});

test("a colour is read from a hex code or from the rgb() a browser reports", () => {
  expect(parseColor("#1f6fd1")).toEqual([0x1f, 0x6f, 0xd1]);
  expect(parseColor("rgb(31, 111, 209)")).toEqual([31, 111, 209]);
  expect(parseColor("rgba(31, 111, 209, 0.5)")).toEqual([31, 111, 209]);
  expect(parseColor("rgb(31 111 209 / 50%)")).toEqual([31, 111, 209]);
});

// The pairs of text and surface the window's CSS uses, by palette field: the Beta badge and its
// popover (text on panelAlt), the thread and the queue (text and dim text on panel), the header
// and the picker (on window).
const PAIRS: [keyof Palette, keyof Palette][] = [
  ["text", "window"],
  ["text", "panel"],
  ["text", "panelAlt"],
  ["textDim", "window"],
  ["textDim", "panel"],
];

for (const name of THEMES) {
  test(`every text and surface pair of the window passes 4.5:1 in the ${name} theme`, () => {
    const palette = PALETTES[name];
    for (const [foreground, background] of PAIRS) {
      const ratio = contrastRatio(palette[foreground], palette[background]);
      expect(ratio, `${name}: ${foreground} on ${background}`).toBeGreaterThanOrEqual(MIN_TEXT_CONTRAST);
    }
  });

  test(`the Beta badge's text and its popover's pass 4.5:1 in the ${name} theme`, () => {
    // The badge and the popover draw `--text` on `--panel-alt`.
    const tokens = cssTokens(name);
    expect(tokens["--text"]).toBe(PALETTES[name].text);
    expect(tokens["--panel-alt"]).toBe(PALETTES[name].panelAlt);
    expect(contrastRatio(tokens["--text"], tokens["--panel-alt"])).toBeGreaterThanOrEqual(MIN_TEXT_CONTRAST);
  });
}

test("the pairs the window avoids are the ones that fail", () => {
  // Why dim text never sits on panelAlt in the dark theme, and why an error is written in the
  // text colour with a border in the light theme: these pairs are under 4.5:1.
  expect(contrastRatio(PALETTES.dark.textDim, PALETTES.dark.panelAlt)).toBeLessThan(MIN_TEXT_CONTRAST);
  for (const surface of ["window", "panel", "panelAlt"] as const) {
    expect(contrastRatio(cssTokens("light")["--error"], PALETTES.light[surface])).toBeLessThan(MIN_TEXT_CONTRAST);
  }
});

test("the Beta explanation is the contract's wording", () => {
  expect(BETA_EXPLANATION).toBe(
    "Experimental feature: behaviour may change. The agent edits your project. Undo works step by step.",
  );
});
