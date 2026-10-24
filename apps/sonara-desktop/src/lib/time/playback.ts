// The notes of a MIDI clip as heard on the timeline (sonara-core's `playback_notes`): the
// timeline's mini notes ("MIDI clips") are drawn where they play, through the clip's content
// offset and loop. Checked against tests/fixtures/time-v1.json (tests/unit/time.test.ts).

import type { MidiClipView, NoteView } from "../types";
import { EPSILON } from "./signatures";

const MAX_LOOP_ITERATIONS = 100_000;

/** A note at its timeline start. */
export interface PlayedNote {
  start: number;
  note: NoteView;
}

/** Python's `%` for floats: the result takes the divisor's sign. */
function pyMod(value: number, divisor: number): number {
  const rem = value % divisor;
  return rem !== 0 && rem < 0 !== divisor < 0 ? rem + divisor : rem;
}

/**
 * `playback_notes`: each note with its timeline start. Notes starting outside the clip span
 * are dropped (the caller shortens a note that crosses the clip end); a loop repeats the
 * content `[0, loop_length)` from the content offset on.
 */
export function playbackNotes(clip: MidiClipView): PlayedNote[] {
  const clipEnd = clip.start + clip.length;
  const loop = clip.loop_length;
  if (loop === null) {
    const out: PlayedNote[] = [];
    for (const note of clip.notes) {
      const t = clip.start + (note.start - clip.content_offset);
      if (note.start + EPSILON >= clip.content_offset && t < clipEnd - EPSILON) {
        out.push({ start: t, note });
      }
    }
    return out;
  }
  const looped = clip.notes.filter((note) => note.start < loop - EPSILON);
  const iterationStart = clip.start - pyMod(clip.content_offset, loop);
  const out: PlayedNote[] = [];
  for (let k = 0; iterationStart + k * loop < clipEnd - EPSILON; ) {
    const base = iterationStart + k * loop;
    for (const note of looped) {
      const t = base + note.start;
      if (t + EPSILON >= clip.start && t < clipEnd - EPSILON) out.push({ start: t, note });
    }
    k += 1;
    if (k > MAX_LOOP_ITERATIONS) break;
  }
  return out;
}
