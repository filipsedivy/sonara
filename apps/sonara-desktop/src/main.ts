import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";
import { guiTheme, onTheme, webviewLabel } from "./lib/backend";
import { themeName } from "./lib/theme";
import { applyTheme } from "./lib/theme.svelte";
import MixerWindow from "./MixerWindow.svelte";

const target = document.getElementById("app");
if (!target) throw new Error("missing #app");
// The dark theme until the stored one is known, then the stored one, and every change (A1).
applyTheme("dark");
guiTheme()
  .then((stored) => applyTheme(themeName(stored)))
  .catch(() => {
    // Without the command (a page test without it) the window stays dark.
  });
onTheme((stored) => applyTheme(themeName(stored))).catch(() => {});
// The main window, or the mixer's own window, which loads the same page.
export default mount(webviewLabel() === "mixer" ? MixerWindow : App, { target });
