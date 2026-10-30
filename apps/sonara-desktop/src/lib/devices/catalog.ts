// What the device cards pick from, from sonara-app: the instrument picker of "Replace
// Instrument" (the prototype's `plugin_menu(…, PluginKind.INSTRUMENT, …)`, sonara-app's
// `catalog::plugin_menu`) and the sampler's instrument choices grouped by library
// (`instruments::InstalledInstruments::by_library`). The registry and the sample libraries do not
// change while the application runs, so the page asks for each once; a failed request is asked
// again next time.

import * as backend from "../backend";
import type { LibraryChoices, PluginGroup } from "../types";

let instruments: Promise<PluginGroup[]> | null = null;
let samplerChoices: Promise<LibraryChoices[]> | null = null;
const presets = new Map<string, Promise<string | null>>();

/** The instrument picker, grouped by category. */
export function instrumentGroups(): Promise<PluginGroup[]> {
  instruments ??= backend.pluginMenu("instrument").catch((error: unknown) => {
    instruments = null;
    throw error;
  });
  return instruments;
}

/** The sampler's instrument choices, grouped by library, with the presets' names. */
export function samplerInstruments(): Promise<LibraryChoices[]> {
  samplerChoices ??= backend.samplerInstruments().catch((error: unknown) => {
    samplerChoices = null;
    throw error;
  });
  return samplerChoices;
}

/**
 * The preset ID a stored sampler `instrument` value plays, null when it names nothing installed
 * here; asked once per value.
 */
export function samplerPresetOf(value: string): Promise<string | null> {
  let preset = presets.get(value);
  if (!preset) {
    preset = backend.samplerPresetOf(value).catch((error: unknown) => {
      presets.delete(value);
      throw error;
    });
    presets.set(value, preset);
  }
  return preset;
}
