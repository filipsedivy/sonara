// WCAG 2.x contrast, to check the window's text against its backgrounds in both themes (the
// tests compute it from the palettes of lib/theme.ts and from the colours the page renders).

import { parseHex, type Rgb } from "../timeline/colors";

/** The least contrast of normal text (WCAG 1.4.3, level AA). */
export const MIN_TEXT_CONTRAST = 4.5;

/** `#rrggbb`, `#rgb` or the `rgb(r, g, b)` / `rgb(r g b / a)` a browser reports, as channels. */
export function parseColor(color: string): Rgb {
  const match = /^rgba?\(\s*(\d+(?:\.\d+)?)[\s,]+(\d+(?:\.\d+)?)[\s,]+(\d+(?:\.\d+)?)/.exec(color.trim());
  if (match) return [Number(match[1]), Number(match[2]), Number(match[3])];
  return parseHex(color.trim());
}

/** The relative luminance of a colour (WCAG 2.x). */
export function luminance(color: string): number {
  const [r, g, b] = parseColor(color).map((channel) => {
    const value = channel / 255;
    return value <= 0.039_28 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** The contrast ratio of two colours, 1 to 21. */
export function contrastRatio(a: string, b: string): number {
  const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (light + 0.05) / (dark + 0.05);
}
