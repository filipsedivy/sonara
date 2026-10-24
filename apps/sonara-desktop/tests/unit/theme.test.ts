import { expect, test } from "@playwright/test";
import fixture from "../fixtures/theme-v1.json" with { type: "json" };
import { cssTokens, PALETTES, themeName, type ThemeName } from "../../src/lib/theme";
import { THEME, TRACK_COLORS } from "../../src/lib/timeline/colors";

// The two themes against the prototype's `sonara/ui/theme.py`, as recorded in
// `tests/fixtures/theme-v1.json` (owner, 2026-10-24).

const camel = (field: string) => field.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase());
const NAMES: ThemeName[] = ["dark", "light"];

test("both palettes are the prototype's, field by field", () => {
  for (const name of NAMES) {
    const recorded = fixture.palettes[name] as Record<string, string>;
    const ours = PALETTES[name] as Record<string, string>;
    expect(Object.keys(ours).sort()).toEqual(Object.keys(recorded).map(camel).sort());
    for (const [field, value] of Object.entries(recorded)) expect(ours[camel(field)], `${name}.${field}`).toBe(value);
  }
  // The canvases draw with the dark palette in both themes.
  expect(PALETTES.dark).toBe(THEME);
  expect([...TRACK_COLORS]).toEqual(fixture.track_colors);
});

test("a stored setting picks the palette as theme.apply does", () => {
  for (const [setting, picked] of Object.entries(fixture.apply)) expect(themeName(setting), JSON.stringify(setting)).toBe(picked);
  expect(themeName(null)).toBe("dark");
  expect(themeName(undefined)).toBe("dark");
});

test("the panels' tokens carry the palette and the Qt roles the prototype gave them", () => {
  for (const name of NAMES) {
    const tokens = cssTokens(name);
    const p = fixture.palettes[name] as Record<string, string>;
    const roles = fixture.roles[name] as Record<string, string>;
    const role = (r: string) => p[roles[r]] ?? roles[r];
    // One token per palette field.
    for (const [field, value] of Object.entries(p)) expect(tokens[`--${field.replace(/_/g, "-")}`], `${name} ${field}`).toBe(value);
    // The window, the text, the buttons and the highlight are the Qt roles' colours.
    expect(tokens["--window"]).toBe(role("Window"));
    expect(tokens["--bg"]).toBe(role("Window"));
    expect(tokens["--text"]).toBe(role("WindowText"));
    expect(tokens["--panel-alt"]).toBe(role("Button"));
    expect(tokens["--accent"]).toBe(role("Highlight"));
    expect(tokens["--text-dim"]).toBe(role("PlaceholderText"));
  }
  // Fields and lists: Qt's Base in the light theme; colours stay in the dark one.
  expect(cssTokens("light")["--bar"]).toBe(fixture.palettes.light[fixture.roles.light.Base as "panel"]);
  expect(cssTokens("dark")["--bar"]).toBe("#202226");
  expect(cssTokens("dark")["--error"]).toBe("#ff8a80");
  expect(cssTokens("light")["--error"]).toBe(fixture.palettes.light.meter_red);
});
