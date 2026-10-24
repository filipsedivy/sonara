// The texts of the ARIA layer: what assistive technology reads for a track in the track list, a clip on
// the arrangement, a note on the piano roll's grid and a level meter. The prototype had none of
// them (its canvases, headers and meters had no accessible representation), so the wording follows
// the ADR 0009 check ("Drums 1, 0.0 dB, selected, 1 of 100": the name and the volume; the state and
// the position come from the option's role) and the inventory's examples ("C4, bar 2 beat 1, 1/8,
// velocity 100"). Plain functions, tested without a page.

import { dbText } from "../mixer/controls";
import { pyG } from "../time/format";
import { position, pyRound, TICKS_PER_BEAT } from "../time/signatures";
import type { ClipView, NoteView, TimeSignatureView, TrackView } from "../types";

/** "bar 2 beat 1", with " tick 480" when the beat does not start on a beat (960 ticks a beat). */
export function positionText(signatures: readonly TimeSignatureView[] | null, beat: number): string {
  const pos = position(signatures, Math.max(0, beat));
  const ticks = pyRound(pos.fraction * TICKS_PER_BEAT);
  return `bar ${pos.bar} beat ${pos.beat}${ticks > 0 ? ` tick ${ticks}` : ""}`;
}

/** "1 beat", "4 beats", "0.5 beats". */
export function beatsText(beats: number): string {
  return `${pyG(beats)} ${beats === 1 ? "beat" : "beats"}`;
}

/** The largest note value a length is written with (a 1/128 note), as a fraction of a whole note. */
const SMALLEST_NOTE = 128;

/**
 * A note's length as a note value: a whole note is 4 beats, so half a beat is "1/8", three
 * quarters of a beat "3/16" and a bar of 4/4 "1/1". A length that is no multiple of a 1/128 note
 * (a triplet, a free length) is read in beats.
 */
export function noteLengthText(beats: number): string {
  const units = (beats / 4) * SMALLEST_NOTE;
  const whole = Math.round(units);
  if (whole <= 0 || Math.abs(units - whole) > 1e-6) return beatsText(beats);
  let numerator = whole;
  let denominator = SMALLEST_NOTE;
  while (numerator % 2 === 0 && denominator > 1) {
    numerator /= 2;
    denominator /= 2;
  }
  return `${numerator}/${denominator}`;
}

/** A track in the track list: "Drums 1, 0.0 dB", and its lit buttons ("muted", "solo", "armed"). */
export function trackOptionLabel(track: TrackView): string {
  const parts = [track.name];
  if (track.kind !== "folder") parts.push(dbText(track.mixer.volume_db));
  if (track.mixer.mute) parts.push("muted");
  if (track.mixer.solo) parts.push("solo");
  if (track.input.armed) parts.push("armed");
  return parts.join(", ");
}

/** A clip on the arrangement: "Riff, bar 2 beat 1, 4 beats", and "muted" when it is. */
export function clipOptionLabel(clip: ClipView, signatures: readonly TimeSignatureView[] | null): string {
  const parts = [clip.name || "Clip", positionText(signatures, clip.start), beatsText(clip.length)];
  if (clip.muted) parts.push("muted");
  return parts.join(", ");
}

/**
 * A note on the piano roll's grid: "C4, bar 2 beat 1, 1/8, velocity 100". `name` is the pitch's
 * name, or the GM drum name on a drum track; `timelineBeat` is where the note starts on the
 * timeline (the clip's content mapped through its start and offset), so the bar is the ruler's.
 */
export function noteOptionLabel(
  note: NoteView,
  name: string,
  timelineBeat: number,
  signatures: readonly TimeSignatureView[] | null,
): string {
  return `${name}, ${positionText(signatures, timelineBeat)}, ${noteLengthText(note.length)}, velocity ${note.velocity}`;
}

/** A level in whole dB, never "-0". */
export function wholeDb(db: number): number {
  const rounded = Math.round(db);
  return rounded === 0 ? 0 : rounded;
}

/**
 * A meter's value: the louder channel's displayed peak in whole dB ("-12 dB"), "silent" at the
 * floor, and ", clipped" while the clip indicator is lit.
 */
export function meterText(peakDb: number, floorDb: number, clipped: boolean): string {
  const level = peakDb <= floorDb ? "silent" : `${wholeDb(peakDb)} dB`;
  return clipped ? `${level}, clipped` : level;
}
