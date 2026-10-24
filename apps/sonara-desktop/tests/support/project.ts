// Project views for the tests, shaped as sonara-app serializes them (src/lib/types.ts).

import type {
  AudioClipView,
  ClipView,
  MidiClipView,
  NoteView,
  OpenProject,
  ProjectView,
  TrackKind,
  TrackView,
} from "../../src/lib/types";

export function track(id: string, name: string, kind: TrackKind = "instrument", clips: ClipView[] = []): TrackView {
  return {
    id,
    name,
    kind,
    color: null,
    parent_id: null,
    depth: 0,
    mixer: { volume_db: 0, pan: 0, mute: false, solo: false, solo_safe: false },
    output_id: "master",
    output_targets: [],
    sends: [],
    send_targets: [],
    input: { channels: [], armed: false, monitor: "auto" },
    instrument: null,
    inserts: [],
    clips,
  };
}

/** `midi_clip` of the prototype's tests: four notes from C4, half a beat apart. */
export function midiClip(id: string, start: number, length: number, extra: Partial<MidiClipView> = {}): MidiClipView {
  const notes: NoteView[] = [0, 1, 2, 3].map((i) => ({
    id: `${id}_n${i}`,
    pitch: 60 + i,
    start: i * 0.5,
    length: 0.5,
    velocity: 100,
    channel: 0,
  }));
  return {
    type: "midi",
    id,
    name: "Clip",
    start,
    length,
    muted: false,
    color: null,
    content_offset: 0,
    loop_length: null,
    notes,
    ...extra,
  };
}

export function audioClip(id: string, assetId: string, start: number, length: number): AudioClipView {
  return {
    type: "audio",
    id,
    name: "Take",
    start,
    length,
    muted: false,
    color: null,
    asset_id: assetId,
    source_offset: 0,
    gain_db: 0,
    fade_in: { seconds: 0, curve: "linear" },
    fade_out: { seconds: 0, curve: "linear" },
  };
}

export function project(tracks: TrackView[], extra: Partial<OpenProject> = {}): OpenProject {
  return {
    id: "prj_1",
    name: "Timeline",
    path: null,
    dirty: false,
    sample_rate: 48000,
    length_beats: null,
    end_beat: Math.max(0, ...tracks.flatMap((t) => t.clips.map((c) => c.start + c.length))),
    tempo: [{ beat: 0, bpm: 120, ramp: false }],
    time_signatures: [{ bar: 0, beat: 0, numerator: 4, denominator: 4, beats_per_bar: 4 }],
    transport: {
      loop_enabled: false,
      loop_start: 0,
      loop_end: 16,
      punch_enabled: false,
      punch_in: 0,
      punch_out: 16,
      metronome: false,
      count_in_bars: 0,
    },
    metadata: { title: "", artist: "", album: "", genre: "", composer: "", comment: "" },
    tracks,
    master: { ...track("master", "Master"), kind: "master" },
    markers: [],
    sections: [],
    assets: [],
    plugins: {},
    history: { undo: null, redo: null },
    meter_layout: 1,
    ...extra,
  };
}

export function view(version: number, open: OpenProject | null): ProjectView {
  return { version, project: open };
}
