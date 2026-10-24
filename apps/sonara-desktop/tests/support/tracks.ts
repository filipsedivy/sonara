import type { Page } from "@playwright/test";

// The track headers as assistive technology sees them: the list box
// "Tracks", whose options read "name, volume" ("Drums 1, 0.0 dB"), and each track's M, S and R in
// a group "name controls" beside it.

/** The track list. */
export const trackList = (page: Page) => page.getByRole("listbox", { name: "Tracks" });

const escaped = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** The option of the track named `name` (its name is the label's first part). */
export const trackRow = (page: Page, name: string) =>
  trackList(page).getByRole("option", { name: new RegExp(`^${escaped(name)}(,|$)`) });

/** The group of the track's M, S and R buttons (and its rename field). */
export const trackControls = (page: Page, name: string) => page.getByRole("group", { name: `${name} controls`, exact: true });
