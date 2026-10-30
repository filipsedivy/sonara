import { expect, test } from "@playwright/test";
import { Composition } from "../../src/lib/chat/ime";

// The guard for the Enter that commits an input method's candidate (WebKit: compositionend comes
// before that keydown, and the keydown has isComposing false and keyCode 229).

const key = (extra: Partial<Pick<KeyboardEvent, "isComposing" | "keyCode">> = {}) => ({ isComposing: false, keyCode: 13, ...extra });
const nextTask = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

test("a plain Enter is not part of a composition", () => {
  expect(new Composition().handles(key())).toBe(false);
});

test("isComposing and keyCode 229 each mark an input method's key", () => {
  const composition = new Composition();
  expect(composition.handles(key({ isComposing: true }))).toBe(true);
  expect(composition.handles(key({ keyCode: 229 }))).toBe(true);
});

test("a key during a composition belongs to it", () => {
  const composition = new Composition();
  composition.start();
  expect(composition.handles(key())).toBe(true);
});

test("the key right after compositionend still belongs to it, a later one does not", async () => {
  const composition = new Composition();
  composition.start();
  composition.end();
  // WebKit's order: compositionend, then the committing Enter, in the same task.
  expect(composition.handles(key())).toBe(true);
  await nextTask();
  await nextTask();
  expect(composition.handles(key())).toBe(false);
});

test("a new composition cancels the pending end, and reset forgets a composition that never ended", async () => {
  const composition = new Composition();
  composition.start();
  composition.end();
  composition.start();
  await nextTask();
  await nextTask();
  expect(composition.handles(key())).toBe(true);
  composition.reset();
  expect(composition.handles(key())).toBe(false);
});
