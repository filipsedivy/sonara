# Sonara brand

Created by **Filip Sedivy** (mail@filipsedivy.cz).

## Concept

The name joins *sonar* and *sonora* (sound). The mark draws the "S" as two half-waves of a
sound wave, built from two circular arcs. Concentric sonar arcs radiate from it: sound going
out, and the agent listening back to the rendered result.

The wordmark is monoline and geometric. Its "S" uses the same construction as the mark, and
its round letters ("o", "a") use the same radius family.

## Files

| File | Use |
|---|---|
| `resources/brand/svg/icon.svg` | App icon (1024 canvas, macOS-style rounded square) |
| `resources/brand/svg/icon-small.svg` | App icon at 16–32 px: the "S" only, bolder |
| `resources/brand/svg/mark.svg` | The mark on a transparent background |
| `resources/brand/svg/logo-dark.svg` | Horizontal logo for dark backgrounds (light wordmark) |
| `resources/brand/svg/logo-light.svg` | Horizontal logo for light backgrounds (dark wordmark) |
| `resources/brand/png/` | PNG renders: icon at 16–1024 px, both logos, the mark |
| `resources/brand/Sonara.icns` | macOS icon; the desktop bundle's `src-tauri/icons/` holds copies |

These files are the frozen brand masters; the generator that first drew them is gone. Change
them only deliberately,
and keep the copies in step: the About dialog's SVGs in `apps/sonara-desktop/src/lib/dialogs/brand/`
and the bundle icons in `apps/sonara-desktop/src-tauri/icons/`. Tests check that the copies equal
these masters (`crates/sonara-app/tests/gui_dialogs.rs`, `apps/sonara-desktop/src-tauri/tests/bundle_icon.rs`).

## Colours

| Name | Hex | Use |
|---|---|---|
| Sonar Blue | `#4C9BF0` | Gradient start, UI accent |
| Violet | `#7D6BD9` | Gradient end |
| Night | `#16171A` | Icon background (bottom) |
| Night Highlight | `#23252B` | Icon background (top) |
| Ink | `#1E1F22` | Wordmark on light backgrounds; app window background |
| Paper | `#F4F5F7` | Wordmark on dark backgrounds |

The mark is always a diagonal gradient from Sonar Blue (top left) to Violet (bottom right).

## Usage

- **Clear space:** keep at least the height of the wordmark's "o" free around the logo.
- **Minimum size:** use the logo at 120 px wide or more. Below that, use the icon alone. At
  16–32 px, use `icon-small.svg`.
- **Variants:** use `logo-dark.svg` on dark surfaces and `logo-light.svg` on light ones. Do
  not place the gradient mark on busy images or on blue or violet backgrounds.
- **Don't** stretch, rotate, recolour, outline or add effects to the mark or the wordmark,
  and don't retype the wordmark in a font.
