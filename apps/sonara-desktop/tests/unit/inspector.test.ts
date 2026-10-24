import { expect, test } from "@playwright/test";
import fixture from "../fixtures/dialogs-v1.json" with { type: "json" };
import { parseSpin, qtRound, spinText, spinValue, stepSpin } from "../../src/lib/dialogs/fields";
import {
  CLIP_GAIN,
  CLIP_LENGTH,
  FADE,
  LOOP_LENGTH,
  SONG_END,
  TRACK_PAN,
  contentEnd,
  effectsText,
  fadeEdit,
  inputIndex,
  inspected,
  instrumentText,
  lengthEdit,
  lengthText,
  loopEdit,
  loopLengthValue,
  mediaText,
  metadataEdit,
  outputChoices,
  projectLengthText,
  songEndEdit,
  startText,
} from "../../src/lib/inspector/inspector";
import { EMPTY_SELECTION } from "../../src/lib/state/selection";
import type {
  AssetView,
  AudioClipView,
  DeviceView,
  MidiClipView,
  OpenProject,
  TempoView,
  TimeSignatureView,
} from "../../src/lib/types";
import { audioClip, midiClip, project as makeProject, track } from "../support/project";

// The Inspector's logic (–IN-15) against the prototype's own labels and fields:
// tests/fixtures/dialogs-v1.json holds what `InspectorPanel` showed for an audio clip, a MIDI clip
// with a loop, one without, an audio track and an instrument track
// (scripts/golden/gui_dialog_oracles.py), with the tempo map and the signatures behind them.

const oracle = fixture.inspector;

type Row = [string | null, string, ...unknown[]];

function rows(group: string, title: string): Row[] {
  const groups = oracle[group as keyof typeof oracle] as { title: string; rows: Row[] }[];
  const found = groups.find((g) => g.title === title);
  if (!found) throw new Error(`no ${title} group in ${group}`);
  return found.rows;
}

function row(group: string, title: string, name: string | null, kind?: string): Row {
  const found = rows(group, title).find((r) => r[0] === name && (kind === undefined || r[1] === kind));
  if (!found) throw new Error(`no row ${name} in ${group}/${title}`);
  return found;
}

/** The oracle's project as the view shows it. */
function oracleProject(): OpenProject {
  const clips = oracle.clips;
  const audio: AudioClipView = {
    ...audioClip("audio", "asset", clips.audio_clip.start, clips.audio_clip.length),
    name: "inspected tone",
    gain_db: -3.5,
    fade_in: { seconds: 0.125, curve: "equal_power" },
    fade_out: { seconds: 1.5, curve: "s_curve" },
  };
  const notes = (list: number[][]) =>
    list.map(([start, length], i) => ({ id: `n${i}`, pitch: 60 + i, start, length, velocity: 90, channel: 0 }));
  const riff: MidiClipView = {
    ...midiClip("midi", clips.midi_clip.start, clips.midi_clip.length),
    name: "Riff",
    notes: notes(clips.midi_clip.notes),
    loop_length: clips.midi_clip.loop_length,
  };
  const plain: MidiClipView = {
    ...midiClip("plain", clips.plain_clip.start, clips.plain_clip.length),
    name: "Plain",
    notes: notes(clips.plain_clip.notes),
    loop_length: null,
  };
  const device = (name: string, preset: string | null = null): DeviceView => ({
    id: `dev_${name}`,
    plugin_id: "x",
    name,
    enabled: true,
    preset,
    sidechain_id: null,
    available: true,
    params: {},
  });
  const keys = {
    ...track("keys", "Keys", "instrument", [riff, plain]),
    instrument: device("Sonara Poly Synth", "Warm Pad"),
    inserts: [device("Parametric EQ"), device("Reverb")],
  };
  const tone = {
    ...track("tone", "inspected tone", "audio", [audio]),
    input: { channels: [0], armed: false, monitor: "auto" as const },
    output_targets: [{ id: "master", name: "Master" }],
  };
  const asset: AssetView = {
    id: "asset",
    kind: "audio",
    name: oracle.assets[0].name,
    location: oracle.assets[0].location as AssetView["location"],
    sample_rate: oracle.assets[0].sample_rate,
    channels: oracle.assets[0].channels,
    frames: 0,
    duration_seconds: oracle.assets[0].duration_seconds,
  };
  return makeProject([tone, keys], {
    name: "Inspected",
    sample_rate: oracle.sample_rate,
    end_beat: oracle.end_beat,
    length_beats: oracle.length_beats,
    tempo: oracle.time.tempo as TempoView[],
    time_signatures: oracle.time.time_signatures as TimeSignatureView[],
    metadata: { title: "Inspected", artist: "", album: "", genre: "", composer: "", comment: "" },
    assets: [asset],
  });
}

const project = oracleProject();
const clipOf = (id: string) => project.tracks.flatMap((t) => t.clips).find((c) => c.id === id)!;

test("the read-only rows word the clip as the prototype's labels", () => {
  const audio = clipOf("audio");
  expect(startText(project, audio)).toBe(row("audio_clip", "Clip", "Start")[2]);
  expect(lengthText(project, audio)).toBe(row("audio_clip", "Clip", "Length")[2]);
  expect(mediaText(project.assets[0])).toBe(row("audio_clip", "Clip", "Media")[2]);
  for (const [group, id] of [
    ["midi_clip", "midi"],
    ["plain_clip", "plain"],
  ] as const) {
    const clip = clipOf(id);
    expect(startText(project, clip)).toBe(row(group, "Clip", "Start")[2]);
    expect(lengthText(project, clip)).toBe(row(group, "Clip", "Length")[2]);
    expect(String((clip as MidiClipView).notes.length)).toBe(row(group, "Clip", "Notes")[2]);
  }
  expect(projectLengthText(project)).toBe(row("audio_clip", "Project", "Length")[2]);
  expect(`${project.sample_rate} Hz`).toBe(row("audio_clip", "Project", "Sample rate")[2]);
});

test("the fields open with the prototype's values, held as its spin boxes hold them", () => {
  const audio = clipOf("audio") as AudioClipView;
  expect(spinValue(audio.length, CLIP_LENGTH)).toBe(row("audio_clip", "Clip", "Set length")[2]);
  expect(spinValue(audio.gain_db, CLIP_GAIN)).toBe(row("audio_clip", "Clip", "Gain")[2]);
  expect(spinValue(audio.fade_in.seconds, FADE)).toBe(row("audio_clip", "Clip", "Fade in")[2]);
  expect(spinValue(audio.fade_out.seconds, FADE)).toBe(row("audio_clip", "Clip", "Fade out")[2]);
  expect(spinValue(project.length_beats ?? 0, SONG_END)).toBe(row("audio_clip", "Project", "Song end")[2]);
  // The loop: the clip's, else its content's end (at least a beat), and disabled without a loop.
  const riff = clipOf("midi") as MidiClipView;
  const plain = clipOf("plain") as MidiClipView;
  expect(row("midi_clip", "Clip", null, "check")).toEqual([null, "check", "Loop content", true]);
  expect(spinValue(loopLengthValue(riff), LOOP_LENGTH)).toBe(row("midi_clip", "Clip", "Loop length")[2]);
  expect(row("plain_clip", "Clip", "Loop length")[3]).toBe(false);
  expect(spinValue(loopLengthValue(plain), LOOP_LENGTH)).toBe(row("plain_clip", "Clip", "Loop length")[2]);
  expect(contentEnd(plain)).toBe(7.75);
  // The curves as the prototype's combo lists them.
  expect(row("audio_clip", "Clip", "Fade curve")[3]).toEqual(["linear", "equal_power", "exponential", "s_curve"]);
});

test("the track groups name the instrument, the effects and the inputs as the prototype", () => {
  const keys = project.tracks[1];
  const tone = project.tracks[0];
  expect(instrumentText(keys)).toBe(row("instrument_track", "Track · instrument", "Instrument")[2]);
  expect(effectsText(keys)).toBe(row("instrument_track", "Track · instrument", "Effects")[2]);
  expect(effectsText(tone)).toBe(row("audio_track", "Track · audio", "Effects")[2]);
  expect(instrumentText(tone)).toBeNull();
  expect(row("audio_track", "Track · audio", "Input")[3]).toEqual(["In 1", "In 2", "In 1+2 (stereo)"]);
  expect(inputIndex(tone.input.channels)).toBe(0);
  expect(inputIndex([1])).toBe(1);
  expect(inputIndex([0, 1])).toBe(2);
  expect(inputIndex([3])).toBe(0);
  expect(row("audio_track", "Track · audio", "Monitoring")[3]).toEqual(["off", "auto", "on"]);
  const outputs = outputChoices({ ...tone, output_id: "bus", output_targets: [{ id: "master", name: "Master" }, { id: "bus", name: "Verb" }] });
  expect(outputs.index).toBe(1);
  expect(outputChoices({ ...tone, output_id: "gone" }).index).toBe(0);
});

test("the Clip group shows for exactly one selected clip, else the current track", () => {
  const one = inspected(project, { ...EMPTY_SELECTION, clips: ["midi"], currentTrack: "tone" });
  expect(one.clip?.clip.id).toBe("midi");
  expect(one.clip?.track.id).toBe("keys");
  const two = inspected(project, { ...EMPTY_SELECTION, clips: ["midi", "plain"], currentTrack: "tone" });
  expect(two.clip).toBeNull();
  expect(two.track?.id).toBe("tone");
  expect(inspected(project, { ...EMPTY_SELECTION, clips: ["gone"] })).toEqual({ clip: null, track: null });
  expect(inspected(project, { ...EMPTY_SELECTION, currentTrack: "master" }).track?.kind).toBe("master");
});

test("G4: a fade edit sends only its fade, so the other keeps its curve", () => {
  const audio = clipOf("audio") as AudioClipView;
  expect(fadeEdit(audio, "fade_in", { seconds: 0.25, curve: "equal_power" })).toEqual({
    op: "set_clip_properties",
    clip_id: "audio",
    fade_in: { seconds: 0.25, curve: "equal_power" },
  });
  expect(fadeEdit(audio, "fade_out", { seconds: 1.5, curve: "linear" })).toEqual({
    op: "set_clip_properties",
    clip_id: "audio",
    fade_out: { seconds: 1.5, curve: "linear" },
  });
});

test("the fields' edits are the prototype's commands", () => {
  const riff = clipOf("midi");
  expect(lengthEdit(riff, 2)).toEqual({ op: "trim_clip", clip_id: "midi", end: 23.5 });
  expect(loopEdit(riff, false, 2.5)).toEqual({ op: "set_clip_properties", clip_id: "midi", loop_length: null });
  expect(loopEdit(riff, true, 4)).toEqual({ op: "set_clip_properties", clip_id: "midi", loop_length: 4 });
  expect(songEndEdit(0)).toEqual({ op: "set_project_length", length_beats: null });
  expect(songEndEdit(64)).toEqual({ op: "set_project_length", length_beats: 64 });
  // Only a changed field is sent.
  expect(metadataEdit(project, "title", "Inspected")).toBeNull();
  expect(metadataEdit(project, "artist", "Filip")).toEqual({ op: "set_metadata", values: { artist: "Filip" } });
});

test("the spin boxes clamp, round and step as Qt's", () => {
  // Measured on PySide6 6.12: an exact tie goes away from zero, a near tie by its binary value.
  expect(qtRound(80.125, 2)).toBe(80.13);
  expect(qtRound(50.375, 2)).toBe(50.38);
  expect(qtRound(37.555, 2)).toBe(37.55);
  expect(qtRound(20.005, 2)).toBe(20);
  expect(qtRound(-3.125, 2)).toBe(-3.13);
  expect(spinValue(-200, CLIP_GAIN)).toBe(-96);
  expect(spinValue(6.81598131205117, CLIP_LENGTH)).toBe(6.816);
  expect(spinText(-3.5, CLIP_GAIN)).toBe("-3.5 dB");
  expect(spinText(-0.01, TRACK_PAN)).toBe("-0.01");
  expect(spinText(-0.001, TRACK_PAN)).toBe("0.00");
  expect(spinText(0, SONG_END)).toBe("auto (end of content)");
  expect(spinText(77.5, SONG_END)).toBe("77.50 beats");
  expect(parseSpin("12.5 dB", CLIP_GAIN)).toBe(12.5);
  expect(parseSpin("1,5", CLIP_GAIN)).toBe(1.5);
  expect(parseSpin("auto (end of content)", SONG_END)).toBe(0);
  expect(parseSpin("loud", CLIP_GAIN)).toBeNull();
  expect(stepSpin(0.98, 1, TRACK_PAN)).toBe(1);
  expect(stepSpin(0, 10, CLIP_GAIN)).toBe(5);
  expect(stepSpin(23.9, 10, CLIP_GAIN)).toBe(24);
});
