// The instrument picker of the device cards' "Replace Instrument" (the prototype's
// `plugin_menu(…, PluginKind.INSTRUMENT, …)`), from sonara-app's `catalog::plugin_menu`. The
// registry does not change while the application runs, so the page asks for it once; a failed
// request is asked again next time.

import * as backend from "../backend";
import type { PluginGroup } from "../types";

let instruments: Promise<PluginGroup[]> | null = null;

/** The instrument picker, grouped by category. */
export function instrumentGroups(): Promise<PluginGroup[]> {
  instruments ??= backend.pluginMenu("instrument").catch((error: unknown) => {
    instruments = null;
    throw error;
  });
  return instruments;
}
