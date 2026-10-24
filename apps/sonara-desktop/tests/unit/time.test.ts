import { expect, test } from "@playwright/test";
import fixture from "../fixtures/time-v1.json" with { type: "json" };
import { formatTime, pyFixed, pyG, pyRepr } from "../../src/lib/time/format";
import { playbackNotes } from "../../src/lib/time/playback";
import {
  barLength,
  barToBeat,
  beatToBar,
  formatPosition,
  pyRound,
} from "../../src/lib/time/signatures";
import { TempoMap } from "../../src/lib/time/tempo";
import type { MidiClipView, TempoView, TimeSignatureView } from "../../src/lib/types";

// The page's display arithmetic against sonara-core: tests/fixtures/time-v1.json is written by
// crates/sonara-app/tests/gui_window.rs from TempoMap, TimeSignatureMap and playback_notes.

const tempo = new TempoMap(fixture.tempo as TempoView[]);
const signatures = fixture.time_signatures as TimeSignatureView[];

function close(actual: number, expected: number) {
  expect(Math.abs(actual - expected)).toBeLessThanOrEqual(1e-12 * Math.max(1, Math.abs(expected)));
}

test("beats map to seconds and back as sonara-core maps them, ramps included", () => {
  for (const [beat, seconds] of fixture.beat_to_seconds) close(tempo.beatToSeconds(beat), seconds);
  for (const [seconds, beat] of fixture.seconds_to_beat) close(tempo.secondsToBeat(seconds), beat);
  expect(tempo.baseBpm).toBe(100);
  expect(tempo.hasChanges).toBe(true);
  expect(new TempoMap([{ beat: 0, bpm: 120, ramp: false }]).hasChanges).toBe(false);
});

test("linear pieces follow the map within a ramp step", () => {
  // Over the ramp (beats 0–16) the pieces are at most 0.25 beats; after it, one per segment.
  const pieces = tempo.linearPieces(14, 30);
  expect(pieces[0].beat0).toBe(14);
  expect(pieces.at(-1)?.beat1).toBe(30);
  for (const piece of pieces) {
    close(tempo.beatToSeconds(piece.beat0), piece.seconds0);
    const end = piece.seconds0 + piece.secondsPerBeat * (piece.beat1 - piece.beat0);
    close(tempo.beatToSeconds(piece.beat1), end);
    if (piece.beat1 <= 16) expect(piece.beat1 - piece.beat0).toBeLessThanOrEqual(0.25 + 1e-12);
  }
  expect(pieces.filter((p) => p.beat0 >= 16).map((p) => [p.beat0, p.beat1])).toEqual([
    [16, 24],
    [24, 30],
  ]);
  expect(new TempoMap([{ beat: 0, bpm: 120, ramp: false }]).linearPieces(2, 10)).toEqual([
    { beat0: 2, beat1: 10, seconds0: 1, secondsPerBeat: 0.5 },
  ]);
});

test("positions read Bar.Beat.Tick across signature changes", () => {
  for (const [beat, text, bar] of fixture.positions as [number, string, number][]) {
    expect(formatPosition(signatures, beat), `beat ${beat}`).toBe(text);
    close(beatToBar(signatures, beat), bar);
  }
  for (const [bar, beat, length] of fixture.bars) {
    close(barToBeat(signatures, bar), beat);
    expect(barLength(signatures, Math.floor(bar))).toBe(length);
  }
  expect(formatPosition([], 0)).toBe("1.1.000");
});

test("MIDI clips play their notes through the content offset and the loop", () => {
  for (const { clip, playback } of fixture.clips) {
    const played = playbackNotes(clip as MidiClipView).map((p) => [p.start, p.note.id]);
    expect(played).toEqual(playback);
  }
});

test("numbers are worded as Python words them", () => {
  expect(pyRound(0.5)).toBe(0);
  expect(pyRound(1.5)).toBe(2);
  expect(pyRound(2.5)).toBe(2);
  expect(pyFixed(2.0625, 3)).toBe("2.062");
  expect(pyFixed(2.0635, 3)).toBe((2.0635).toFixed(3));
  expect(pyFixed(12.4, 0)).toBe("12");
  expect(pyFixed(12.5, 0)).toBe("12");
  expect(pyFixed(13.5, 0)).toBe("14");
  expect(pyG(120)).toBe("120");
  expect(pyG(120.5)).toBe("120.5");
  expect(pyG(44.1)).toBe("44.1");
  expect(pyRepr(16)).toBe("16.0");
  expect(pyRepr(10.67)).toBe("10.67");
  expect(formatTime(0)).toBe("00:00.000");
  expect(formatTime(-3)).toBe("00:00.000");
  expect(formatTime(65.25)).toBe("01:05.250");
  expect(formatTime(3600 + 2.0625)).toBe("60:02.062");
});
