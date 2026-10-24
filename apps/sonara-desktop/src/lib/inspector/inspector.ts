// The Inspector's logic (the prototype's `InspectorPanel`), as plain
// functions over the project view: which groups show, the texts of the read-only rows worded as
// the prototype's labels (tests/fixtures/dialogs-v1.json holds them), the spin boxes' ranges,
// and the edit each field sends. The edits run through the edit boundary with the prototype's
// commands and merge keys; the inspector never seals the undo step (consecutive edits with one
// merge key coalesce, as in the prototype).
//
// Each fade has its own curve, and an edit of one fade sends only that
// fade, so the other keeps its curve.

import type { SpinSpec } from "../dialogs/fields";
import type { Selection } from "../state/selection";
import { pyFixed, pyG } from "../time/format";
import { beatToBar, formatPosition } from "../time/signatures";
import { TempoMap } from "../time/tempo";
import type { AssetView, ClipView, Edit, FadeView, MidiClipView, OpenProject, TrackView } from "../types";

/** What the Inspector shows: one clip, else the current track, and always the project. */
export interface Inspected {
  readonly clip: { readonly track: TrackView; readonly clip: ClipView } | null;
  readonly track: TrackView | null;
}

/** The track (or the master) with `id`. */
function findTrack(project: OpenProject, id: string): TrackView | null {
  if (id === project.master.id) return project.master;
  return project.tracks.find((track) => track.id === id) ?? null;
}

/**
 * `refresh`: the Clip group when exactly one clip is selected and exists, else the Track group of
 * the current track when it exists.
 */
export function inspected(project: OpenProject, selection: Selection): Inspected {
  if (selection.clips.length === 1) {
    const id = selection.clips[0];
    for (const track of project.tracks) {
      const clip = track.clips.find((c) => c.id === id);
      if (clip) return { clip: { track, clip }, track: null };
    }
  }
  const track = selection.currentTrack ? findTrack(project, selection.currentTrack) : null;
  return { clip: null, track };
}

// -- the read-only rows -----------------------------------------------------------------------------

/** IN-02 "Start": "bar.beat.tick  (s.sss s)". */
export function startText(project: OpenProject, clip: ClipView): string {
  const seconds = new TempoMap(project.tempo).beatToSeconds(clip.start);
  return `${formatPosition(project.time_signatures, clip.start)}  (${pyFixed(seconds, 3)} s)`;
}

/** IN-02 "Length": "n beats  (s.sss s)", the beats as Python's `:g`. */
export function lengthText(project: OpenProject, clip: ClipView): string {
  const tempo = new TempoMap(project.tempo);
  const seconds = tempo.beatToSeconds(clip.start + clip.length) - tempo.beatToSeconds(clip.start);
  return `${pyG(clip.length)} beats  (${pyFixed(seconds, 3)} s)`;
}

/** IN-07 "Media": the name, then "rate Hz · n ch · s.ss s · linked or embedded". */
export function mediaText(asset: AssetView): string {
  return `${asset.name}\n${asset.sample_rate} Hz · ${asset.channels} ch · ${pyFixed(asset.duration_seconds, 2)} s · ${asset.location}`;
}

/** IN-13 "Length" of the project: "b.bb bars · s.s s". */
export function projectLengthText(project: OpenProject): string {
  const end = project.end_beat;
  const seconds = new TempoMap(project.tempo).beatToSeconds(end);
  return `${pyFixed(beatToBar(project.time_signatures, end), 2)} bars · ${pyFixed(seconds, 1)} s`;
}

/** IN-11 "Instrument": "name · preset". */
export function instrumentText(track: TrackView): string | null {
  const device = track.instrument;
  if (!device) return null;
  return device.preset ? `${device.name} · ${device.preset}` : device.name;
}

/** IN-11 "Effects": the inserts' names, or "—". */
export function effectsText(track: TrackView): string {
  return track.inserts.map((device) => device.name).join(", ") || "—";
}

/** The last note end of a MIDI clip's content (`content_end`); 0 without notes. */
export function contentEnd(clip: MidiClipView): number {
  return clip.notes.reduce((end, note) => Math.max(end, note.start + note.length), 0);
}

/** IN-08 "Loop length" as it opens: the loop, else the content's end (at least 1 beat). */
export function loopLengthValue(clip: MidiClipView): number {
  return clip.loop_length ?? Math.max(1, contentEnd(clip));
}

// -- the fields ------------------------------------------------------------------------------------

export const CLIP_LENGTH: SpinSpec = { min: 0.001, max: 100000, step: 1, decimals: 3, suffix: " beats" };
export const CLIP_GAIN: SpinSpec = { min: -96, max: 24, step: 0.5, decimals: 1, suffix: " dB" };
export const FADE: SpinSpec = { min: 0, max: 60, step: 0.01, decimals: 3, suffix: " s" };
export const LOOP_LENGTH: SpinSpec = { min: 0.25, max: 4096, step: 1, decimals: 2, suffix: " beats" };
export const TRACK_VOLUME: SpinSpec = { min: -144, max: 12, step: 0.5, decimals: 1, suffix: " dB" };
export const TRACK_PAN: SpinSpec = { min: -1, max: 1, step: 0.05, decimals: 2 };
export const SONG_END: SpinSpec = {
  min: 0,
  max: 100000,
  step: 4,
  decimals: 2,
  suffix: " beats",
  special: "auto (end of content)",
};

/** The fade curves, as the prototype's combo lists them. */
export const FADE_CURVES: readonly FadeView["curve"][] = ["linear", "equal_power", "exponential", "s_curve"];

/** IN-12's input choices. */
export const INPUTS: readonly (readonly [string, readonly number[]])[] = [
  ["In 1", [0]],
  ["In 2", [1]],
  ["In 1+2 (stereo)", [0, 1]],
];

/** The input choice the track's channels select; the first when none matches. */
export function inputIndex(channels: readonly number[]): number {
  const index = INPUTS.findIndex(([, chosen]) => chosen.length === channels.length && chosen.every((c, i) => c === channels[i]));
  return Math.max(0, index);
}

/** IN-10: Master and the buses other than the track; the current one, else Master. */
export function outputChoices(track: TrackView): { choices: { id: string; name: string }[]; index: number } {
  const choices = track.output_targets.map((target) => ({ id: target.id, name: target.name }));
  const index = Math.max(0, choices.findIndex((choice) => choice.id === track.output_id));
  return { choices, index };
}

/** IN-06: the edit of one fade (G4: the other fade is not sent, so it keeps its curve). */
export function fadeEdit(clip: ClipView, which: "fade_in" | "fade_out", fade: FadeView): Edit {
  return which === "fade_in"
    ? { op: "set_clip_properties", clip_id: clip.id, fade_in: { seconds: fade.seconds, curve: fade.curve } }
    : { op: "set_clip_properties", clip_id: clip.id, fade_out: { seconds: fade.seconds, curve: fade.curve } };
}

/** IN-08: the loop on with `length` beats, or off. */
export function loopEdit(clip: ClipView, on: boolean, length: number): Edit {
  return { op: "set_clip_properties", clip_id: clip.id, loop_length: on ? length : null };
}

/** IN-03: "Set length" trims the end to `start + length`. */
export function lengthEdit(clip: ClipView, length: number): Edit {
  return { op: "trim_clip", clip_id: clip.id, end: clip.start + length };
}

/** IN-14: the song end; 0 is "auto (end of content)". */
export function songEndEdit(beats: number): Edit {
  return { op: "set_project_length", length_beats: beats > 0 ? beats : null };
}

/** The metadata fields of IN-15, in the prototype's order. */
export const METADATA: readonly (readonly ["artist" | "title" | "album" | "genre", string])[] = [
  ["artist", "Artist"],
  ["title", "Title"],
  ["album", "Album"],
  ["genre", "Genre"],
];

/** IN-15: a metadata field is sent only when it changed. */
export function metadataEdit(project: OpenProject, key: (typeof METADATA)[number][0], text: string): Edit | null {
  if (project.metadata[key] === text) return null;
  return { op: "set_metadata", values: { [key]: text } };
}

/** The asset of an audio clip. */
export function clipAsset(project: OpenProject, clip: ClipView): AssetView | null {
  if (clip.type !== "audio") return null;
  return project.assets.find((asset) => asset.id === clip.asset_id) ?? null;
}
