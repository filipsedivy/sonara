import { expect, test } from "@playwright/test";
import { agentLabel, errorText, sessionLabel } from "../../src/lib/status";

test("the agent label says on with the address, or off", () => {
  expect(agentLabel({ on: true, address: "/run/sonara-41.sock" })).toEqual({
    text: "agent: on",
    title: "/run/sonara-41.sock",
  });
  expect(agentLabel({ on: false, address: null })).toEqual({
    text: "agent: off",
    title: "agent connections disabled",
  });
  expect(agentLabel(null).text).toBe("agent: off");
});

test("a recoverable session reads as the prototype's Recovery dialog lists it", () => {
  const session = {
    workspace: "/home/workspaces/new-prj_1",
    name: "Night Drive",
    saved_at: "2026-10-22T12:00:00Z",
  };
  expect(sessionLabel(session)).toBe("Night Drive — autosaved 2026-10-22T12:00:00Z");
  expect(sessionLabel({ ...session, name: null, saved_at: null })).toBe("Untitled — autosaved ?");
});

test("a failed command shows its message", () => {
  expect(errorText("cannot recover x: gone")).toBe("cannot recover x: gone");
  expect(errorText(new Error("boom"))).toBe("boom");
  expect(errorText(42)).toBe("42");
});
