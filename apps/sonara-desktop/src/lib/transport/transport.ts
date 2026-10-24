// The transport bar's behaviour (`TransportBar` in sonara/ui/widgets/transport_bar.py): play and
// stop (a second stop returns to the last start), return to start, record,
// loop and metronome, the count-in, the base tempo and
// the time signature at bar 1, and the engine label. The transport's calls and the edits
// are sonara-app's; this keeps the bar's own state (the last start, the Record button) and words
// the label.

import type { EngineState, LiveFrame, LiveState } from "../live/frame";
import { pyFixed, pyG, pyRepr } from "../time/format";
import type { Edit, EngineStatus, OpenProject, Stopped } from "../types";

/** The time signatures the combo offers. */
export const SIGNATURES = ["4/4", "3/4", "6/8", "2/4", "5/4", "7/8", "12/8"] as const;
/** The count-in choices, by bars. */
export const COUNT_IN = ["No count-in", "1 bar", "2 bars"] as const;
/** The tempo range of the BPM field. */
export const MIN_BPM = 10;
export const MAX_BPM = 999;

/** What the bar talks to. */
export interface TransportDeps {
  play(startBeat: number, looped: boolean): Promise<Record<string, unknown>>;
  stop(): Promise<Stopped>;
  record(startBeat: number): Promise<Record<string, unknown>>;
  seek(beat: number): Promise<Record<string, unknown>>;
  edit(edit: Edit): Promise<unknown>;
  /** The transport's state and engine, from the latest live frame or command. */
  transport(): { state: LiveState; engine: EngineState };
  project(): OpenProject | null;
  playhead(): number;
  setPlayhead(beat: number, playing?: boolean): void;
  /** A refused call or edit, for the status bar. */
  fail(error: unknown): void;
  /** A command answered with the transport's new state. */
  answered(state: LiveState): void;
  /** Checks or unchecks the Record button. */
  setRecording(checked: boolean): void;
}

function rolling(state: LiveState): boolean {
  return state === "playing" || state === "recording";
}

/** Python's `repr` of a string, for "invalid time signature '…'". */
export function pyStrRepr(text: string): string {
  const quote = text.includes("'") && !text.includes('"') ? '"' : "'";
  const body = text.replaceAll("\\", "\\\\").replaceAll(quote, `\\${quote}`);
  return `${quote}${body}${quote}`;
}

/** `int(part)` for each side of "n/d"; null when Python's parse would fail. */
export function parseSignature(text: string): [number, number] | null {
  const parts = text.trim().split("/");
  if (parts.length !== 2) return null;
  const numbers = parts.map((part) => (/^\s*[+-]?\d+\s*$/.test(part) ? Number.parseInt(part, 10) : NaN));
  return numbers.some(Number.isNaN) ? null : [numbers[0], numbers[1]];
}

export class TransportControl {
  /** The beat the last play or record started from (a second stop returns there). */
  lastStart = 0;

  constructor(private readonly deps: TransportDeps) {}

  /** TB-01: stops while rolling; otherwise plays from the playhead, looping when Loop is on. */
  async togglePlay(): Promise<void> {
    if (rolling(this.deps.transport().state)) {
      await this.stop();
      return;
    }
    const project = this.deps.project();
    if (!project) return;
    this.lastStart = this.deps.playhead();
    try {
      const status = await this.deps.play(this.lastStart, project.transport.loop_enabled);
      if (typeof status.state === "string") this.deps.answered(status.state as LiveState);
    } catch (error) {
      this.deps.fail(error);
    }
  }

  /**
   * Stops while rolling ("Recorded N take(s)" comes from the backend) and unchecks
   * Record; a second stop puts the playhead back at the last start and seeks there.
   */
  async stop(): Promise<void> {
    if (rolling(this.deps.transport().state)) {
      try {
        await this.deps.stop();
        this.deps.answered("stopped");
      } catch (error) {
        this.deps.fail(error);
      }
      this.deps.setRecording(false);
      return;
    }
    this.deps.setPlayhead(this.lastStart, false);
    try {
      await this.deps.seek(this.lastStart);
    } catch (error) {
      this.deps.fail(error);
    }
  }

  /** TB-02: the playhead to 0, and a seek once the transport exists or is not stopped. */
  async returnToStart(): Promise<void> {
    this.deps.setPlayhead(0);
    const transport = this.deps.transport();
    if (transport.engine !== "absent" || transport.state !== "stopped") {
      try {
        await this.deps.seek(0);
      } catch (error) {
        this.deps.fail(error);
      }
    }
  }

  /** TB-04: checking Record records from the playhead (unchecked again on failure). */
  async toggleRecord(checked: boolean): Promise<void> {
    this.deps.setRecording(checked);
    if (!checked) {
      await this.stop();
      return;
    }
    if (!this.deps.project()) {
      this.deps.setRecording(false);
      return;
    }
    this.lastStart = this.deps.playhead();
    try {
      await this.deps.record(this.lastStart);
      this.deps.answered("recording");
    } catch (error) {
      this.deps.setRecording(false);
      this.deps.fail(error);
    }
  }

  /** TB-05–TB-07: a transport setting, only when it changes. */
  async setTransport(key: "loop_enabled" | "metronome", value: boolean): Promise<void>;
  async setTransport(key: "count_in_bars", value: number): Promise<void>;
  async setTransport(key: "loop_enabled" | "metronome" | "count_in_bars", value: boolean | number): Promise<void> {
    const project = this.deps.project();
    if (!project || project.transport[key] === value) return;
    await this.edit({ op: "set_transport", [key]: value } as Edit);
  }

  /** TB-08: the base tempo, clamped to 10–999 with two decimals; unchanged values do nothing. */
  async setTempo(bpm: number): Promise<void> {
    const project = this.deps.project();
    if (!project || !Number.isFinite(bpm)) return;
    const value = Math.round(Math.min(Math.max(bpm, MIN_BPM), MAX_BPM) * 100) / 100;
    if (Math.abs(project.tempo[0].bpm - value) < 1e-6) return;
    await this.edit({ op: "set_tempo", bpm: value });
  }

  /**
   * "n/d" at bar 1. Text that does not parse is reported ("invalid time signature
   * '…'") and returns false, so the field shows the project's again.
   */
  async setSignature(text: string): Promise<boolean> {
    const trimmed = text.trim();
    const parsed = parseSignature(trimmed);
    if (!parsed) {
      this.deps.fail(`invalid time signature ${pyStrRepr(trimmed)}`);
      return false;
    }
    const project = this.deps.project();
    if (!project) return true;
    const sig = project.time_signatures[0];
    if (sig && sig.numerator === parsed[0] && sig.denominator === parsed[1]) return true;
    return this.edit({ op: "set_time_signature", numerator: parsed[0], denominator: parsed[1] });
  }

  private async edit(edit: Edit): Promise<boolean> {
    try {
      await this.deps.edit(edit);
      return true;
    } catch (error) {
      this.deps.fail(error);
      return false;
    }
  }
}

/** The engine label: its text and its tooltip. */
export interface EngineLabel {
  text: string;
  title: string;
}

/**
 * `_on_tick`'s label: "Audio engine unavailable" with the reason, or "48 kHz · 256 · render
 * 12%" (with " · N dropouts" after underruns) with the latency estimate and the lookahead;
 * nothing before the transport has a device.
 */
export function engineLabel(frame: LiveFrame | null, status: EngineStatus | null): EngineLabel | null {
  if (!frame) return null;
  if (frame.engine === "unavailable") {
    return { text: "Audio engine unavailable", title: String(status?.reason ?? "") };
  }
  if (frame.engine === "absent" || frame.sample_rate <= 0) return null;
  const load = pyFixed(frame.render_load * 100, 0);
  const dropouts = frame.underruns ? ` · ${frame.underruns} dropouts` : "";
  return {
    text: `${pyG(frame.sample_rate / 1000)} kHz · ${frame.buffer_frames} · render ${load}%${dropouts}`,
    title: `Output latency estimate ${pyRepr(frame.output_latency_ms)} ms, lookahead ${pyRepr(frame.lookahead_ms)} ms`,
  };
}
