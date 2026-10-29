import type { Page } from "@playwright/test";

// A stand-in for the Tauri runtime in the page (what `mockIPC` of `@tauri-apps/api/mocks`
// does, with events and channels): commands answer from `responses` (an entry `{reject: …}`
// rejects), every call is recorded in `window.__sonaraMock.calls`, `emit` delivers an event to
// the page's listeners, and `sendChannel` delivers bytes to every channel the page handed to a
// command (the live frames of `subscribe_live`). It is installed before the page's scripts run.

export type Responses = Record<string, unknown>;

/** The panes of the shell's answer: all showing, at the prototype's sizes. */
export const PANES = {
  browser: { size: 240, open: true },
  inspector: { size: 280, open: true },
  editors: { size: 300, open: true },
};

/** The audio suffixes of the shell's answer (sonara-app's `AUDIO_IMPORT_SUFFIXES`). */
export const AUDIO_SUFFIXES = [".wav", ".wave", ".aif", ".aiff", ".flac", ".mp3", ".ogg", ".oga", ".opus", ".w64", ".rf64", ".caf", ".m4a", ".aac"];

/**
 * The answers of the data layer's startup calls: no project, the live channel, the focus; the
 * drum map the piano roll asks for when it shows; the Browser's tree; and the menu's
 * context, which the page reports as the focus moves.
 */
export const DATA_LAYER: Responses = {
  project_view: { version: 0, project: null },
  subscribe_live: null,
  set_midi_focus: null,
  set_menu_context: null,
  set_pane_layout: null,
  plugin_menu: [],
  transport_status: null,
  drum_map: { plugin_id: "sonara.drums", names: [] },
  browser_devices: [],
};

interface MockState {
  calls: { cmd: string; args: unknown }[];
  answers: Responses;
  emit: (event: string, payload: unknown) => void;
  sendChannel: (bytes: number[]) => void;
  sendChannelMessage: (message: unknown) => void;
}

declare global {
  interface Window {
    __sonaraMock: MockState;
  }
}

/** Installs the runtime; `label` is the web view's (`"mixer"` for the mixer's own window). */
export async function mockTauri(page: Page, responses: Responses, label = "main"): Promise<void> {
  await page.addInitScript(([answers, webview]: [Responses, string]) => {
    const callbacks = new Map<number, (data: unknown) => void>();
    const listeners = new Map<string, number[]>();
    const channels: { id: number; next: number }[] = [];
    let next = 1;
    const state: MockState = {
      calls: [],
      answers,
      emit(event, payload) {
        for (const id of listeners.get(event) ?? []) callbacks.get(id)?.({ event, id, payload });
      },
      sendChannelMessage(message) {
        for (const channel of channels) {
          callbacks.get(channel.id)?.({ index: channel.next, message });
          channel.next += 1;
        }
      },
      sendChannel(bytes) {
        for (const channel of channels) {
          const message = new Uint8Array(bytes).buffer;
          callbacks.get(channel.id)?.({ index: channel.next, message });
          channel.next += 1;
        }
      },
    };
    window.__sonaraMock = state;
    // The runtime's globals, declared by @tauri-apps/api with their full shapes.
    const runtime = window as unknown as Record<string, unknown>;
    runtime.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener(_event: string, id: number) {
        callbacks.delete(id);
      },
    };
    runtime.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: webview }, currentWebview: { windowLabel: webview, label: webview } },
      transformCallback(callback: (data: unknown) => void) {
        const id = next++;
        callbacks.set(id, callback);
        return id;
      },
      unregisterCallback(id: number) {
        callbacks.delete(id);
      },
      async invoke(cmd: string, args: { event?: string; handler?: number; channel?: { id: number } }) {
        state.calls.push({ cmd, args });
        if (cmd === "plugin:event|listen" && args.event && args.handler !== undefined) {
          listeners.set(args.event, [...(listeners.get(args.event) ?? []), args.handler]);
          return args.handler;
        }
        if (cmd.startsWith("plugin:event|")) return null;
        if (args?.channel && typeof args.channel.id === "number") {
          channels.push({ id: args.channel.id, next: 0 });
        }
        if (!(cmd in state.answers)) throw new Error(`unmocked command ${cmd}`);
        const answer = state.answers[cmd];
        if (answer && typeof answer === "object" && "reject" in answer) {
          throw (answer as { reject: unknown }).reject;
        }
        return answer;
      },
    };
  }, [responses, label] as [Responses, string]);
}

/** Sends `event` with `payload` to the page's listeners. */
export async function emit(page: Page, event: string, payload: unknown): Promise<void> {
  await page.evaluate(([name, data]) => window.__sonaraMock.emit(name as string, data), [
    event,
    payload,
  ] as const);
}

/** Changes what `cmd` answers from now on. */
export async function answer(page: Page, cmd: string, value: unknown): Promise<void> {
  await page.evaluate(([name, data]) => (window.__sonaraMock.answers[name as string] = data), [
    cmd,
    value,
  ] as const);
}

/** Sends the bytes of `hex` to every channel the page subscribed (a live frame). */
export async function sendChannel(page: Page, hex: string): Promise<void> {
  const bytes = hex.match(/../g)?.map((pair) => parseInt(pair, 16)) ?? [];
  await page.evaluate((data) => window.__sonaraMock.sendChannel(data), bytes);
}

/** Sends the JSON `message` to every channel the page handed to a command (a composer event). */
export async function sendChannelMessage(page: Page, message: unknown): Promise<void> {
  await page.evaluate((data) => window.__sonaraMock.sendChannelMessage(data), message);
}

/** The events the page sent to the other windows (`emit`): `[event, payload]`. */
export async function emitted(page: Page): Promise<[string, unknown][]> {
  const all = await page.evaluate(() => window.__sonaraMock.calls);
  return all
    .filter((call) => call.cmd === "plugin:event|emit")
    .map((call) => {
      const args = call.args as { event: string; payload: unknown };
      return [args.event, args.payload];
    });
}

/** The commands the page called, without the event plugin's. */
export async function calls(page: Page): Promise<{ cmd: string; args: unknown }[]> {
  const all = await page.evaluate(() => window.__sonaraMock.calls);
  return all.filter((call) => !call.cmd.startsWith("plugin:event|"));
}
