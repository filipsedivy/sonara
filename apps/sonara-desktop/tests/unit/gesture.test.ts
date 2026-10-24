import { expect, test } from "@playwright/test";
import { LiveGesture, commitOnRelease, type GestureDeps } from "../../src/lib/gesture";
import type { Edit } from "../../src/lib/types";

// The gesture protocol against a fake backend and a fake display: the order
// of edits and seals is what makes one gesture one undo step (`fader_drag_is_one_undo_step`,
// `drag_moves_note_in_time_and_pitch_as_one_undo_step`).

interface Deferred {
  edit: Edit;
  resolve: () => void;
  reject: (error: unknown) => void;
}

function harness() {
  const log: string[] = [];
  const open: Deferred[] = [];
  const frames: (() => void)[] = [];
  const errors: unknown[] = [];
  const deps: GestureDeps = {
    send: (edit) =>
      new Promise<void>((resolve, reject) => {
        log.push(`send ${JSON.stringify(edit)}`);
        open.push({
          edit,
          resolve: () => {
            log.push(`done ${JSON.stringify(edit)}`);
            resolve();
          },
          reject,
        });
      }),
    seal: async () => {
      log.push("seal");
    },
    nextFrame: (callback) => frames.push(callback),
    onError: (error) => errors.push(error),
  };
  const frame = () => {
    for (const callback of frames.splice(0)) callback();
  };
  const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
  return { deps, log, open, frame, errors, settle };
}

const fader = (db: number): Edit => ({ op: "set_track_mixer", track_id: "trk_1", volume_db: db });

test("moves within a frame collapse to the latest, with one edit in flight", async () => {
  const h = harness();
  const gesture = new LiveGesture(h.deps);
  gesture.move(fader(-1));
  gesture.move(fader(-2));
  expect(h.log).toEqual([]);
  h.frame();
  expect(h.log).toEqual([`send ${JSON.stringify(fader(-2))}`]);
  // While it is in flight, frames pass without another send.
  gesture.move(fader(-3));
  gesture.move(fader(-4));
  h.frame();
  h.frame();
  expect(h.open).toHaveLength(1);
  h.open[0].resolve();
  await h.settle();
  // The latest value goes at the next frame, not at once.
  expect(h.open).toHaveLength(1);
  h.frame();
  expect(h.open.map((d) => d.edit)).toEqual([fader(-2), fader(-4)]);
  expect(gesture.sent).toBe(2);
  expect(gesture.active).toBe(true);
});

test("the release waits for the edit in flight, sends the last value, then seals", async () => {
  const h = harness();
  const gesture = new LiveGesture(h.deps);
  gesture.move(fader(-1));
  h.frame();
  gesture.move(fader(-6));
  const ended = gesture.end();
  expect(gesture.active).toBe(false);
  // A move after the release is ignored.
  gesture.move(fader(-9));
  await h.settle();
  expect(h.log).not.toContain("seal");
  h.open[0].resolve();
  await h.settle();
  expect(h.open.map((d) => d.edit)).toEqual([fader(-1), fader(-6)]);
  h.open[1].resolve();
  await ended;
  expect(h.log).toEqual([
    `send ${JSON.stringify(fader(-1))}`,
    `done ${JSON.stringify(fader(-1))}`,
    `send ${JSON.stringify(fader(-6))}`,
    `done ${JSON.stringify(fader(-6))}`,
    "seal",
  ]);
});

test("a refused move is reported and the gesture goes on", async () => {
  const h = harness();
  const gesture = new LiveGesture(h.deps);
  gesture.move(fader(99));
  h.frame();
  h.open[0].reject({ code: "INVALID_ARGUMENT", message: "volume out of range" });
  await h.settle();
  expect(h.errors).toEqual([{ code: "INVALID_ARGUMENT", message: "volume out of range" }]);
  gesture.move(fader(-3));
  h.frame();
  const ended = gesture.end();
  h.open[1].resolve();
  await ended;
  expect(h.log.at(-1)).toBe("seal");
});

test("a gesture without moves only seals", async () => {
  const h = harness();
  await new LiveGesture(h.deps).end();
  expect(h.log).toEqual(["seal"]);
});

test("timeline and ruler gestures commit once on release, then seal", async () => {
  const h = harness();
  const move: Edit = { op: "move_clip", clip_id: "clp_1", start: 8 };
  const committed = commitOnRelease(h.deps, move);
  h.open[0].resolve();
  expect(await committed).toBe(true);
  expect(h.log).toEqual([`send ${JSON.stringify(move)}`, `done ${JSON.stringify(move)}`, "seal"]);
  const refused = commitOnRelease(h.deps, move);
  h.open[1].reject({ code: "NOT_FOUND", message: "clip 'clp_1' not found" });
  expect(await refused).toBe(false);
  expect(h.errors).toHaveLength(1);
  expect(h.log.at(-1)).toBe("seal");
});
