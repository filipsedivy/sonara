// The built-in plugins' descriptors for the tests: golden/plugins/catalog.json (recorded from the
// prototype, N1), copied to tests/fixtures/plugin-catalog.json because the container's frontend
// task runs on a copy of this app alone. sonara-app's crates/sonara-app/tests/gui_devices.rs checks
// that the copy equals the golden file byte for byte, and that the project view's `PluginView` of
// every built-in plugin is the descriptor mapped as `pluginView` maps it here.

import catalog from "../fixtures/plugin-catalog.json" with { type: "json" };
import type { ParamValue, ParamView, PluginView } from "../../src/lib/types";

/** A parameter of the golden catalog (`plain(ParamSpec)`). */
interface GoldenParam {
  id: string;
  name: string;
  kind: string;
  default: ParamValue;
  minimum: number | null;
  maximum: number | null;
  unit: string;
  choices: string[];
  scale: string;
  automatable: boolean;
  description: string;
}

/** A descriptor of the golden catalog (`plain(PluginDescriptor)`). */
export interface GoldenDescriptor {
  id: string;
  name: string;
  kind: string;
  category: string;
  description: string;
  sidechain: boolean;
  presets: { name: string; description: string; params: Record<string, ParamValue> }[];
  params: GoldenParam[];
}

/** The descriptor as the project view carries it (sonara-app `view::PluginView::of`). */
export function pluginView(d: GoldenDescriptor): PluginView {
  return {
    id: d.id,
    name: d.name,
    kind: d.kind as PluginView["kind"],
    category: d.category,
    description: d.description,
    sidechain: d.sidechain,
    presets: d.presets.map((p) => p.name),
    params: d.params.map(
      (p): ParamView => ({
        id: p.id,
        name: p.name,
        kind: p.kind as ParamView["kind"],
        default: p.default,
        min: p.minimum,
        max: p.maximum,
        unit: p.unit,
        choices: p.choices,
        log_scale: p.scale === "log",
        automatable: p.automatable,
        description: p.description,
      }),
    ),
  };
}

/** Every built-in plugin's descriptor, as the golden catalog has them. */
export const GOLDEN: readonly GoldenDescriptor[] = catalog as GoldenDescriptor[];

/** Every built-in plugin as the project view carries it, in the catalog's order. */
export const PLUGINS: readonly PluginView[] = GOLDEN.map(pluginView);

/** The built-in plugin `id`. */
export function plugin(id: string): PluginView {
  const found = PLUGINS.find((p) => p.id === id);
  if (!found) throw new Error(`no built-in plugin ${id}`);
  return found;
}

/** The view's `plugins` map of the plugins `ids` (every built-in one by default). */
export function pluginMap(ids: readonly string[] = PLUGINS.map((p) => p.id)): Record<string, PluginView> {
  return Object.fromEntries(ids.map((id) => [id, plugin(id)]));
}

/** The parameters of `id` at their defaults, as a device view resolves them. */
export function defaults(id: string): Record<string, ParamValue> {
  return Object.fromEntries(plugin(id).params.map((p) => [p.id, p.default]));
}
