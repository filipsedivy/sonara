import { expect, test } from "@playwright/test";
import { DATA_LAYER, calls, emit, mockTauri } from "./tauri-mock";

// The empty window against mocked commands: the agent label, the status bar's
// messages (MW-29: an agent's edit), and the recovery offer of scratch sessions.

const SOCKET = "/Users/me/Library/Application Support/Sonara/run/sonara-4242.sock";

test("the status bar shows the agent connection and an agent's edit", async ({ page }) => {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: { agent: { on: true, address: SOCKET }, messages: [] },
    recoverable_sessions: [],
  });
  await page.goto("/");
  const label = page.getByTestId("agent-label");
  await expect(label).toHaveText("agent: on");
  await expect(label).toHaveAttribute("title", SOCKET);
  await emit(page, "message", { text: "agent: Create Track", duration_ms: 3000, error: false });
  await expect(page.getByRole("status")).toHaveText("agent: Create Track");
  await expect(page.getByRole("dialog")).toHaveCount(0);
});

test("an autosave shows Autosaved for 1.5 s", async ({ page }) => {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: { agent: { on: false, address: null }, messages: [] },
    recoverable_sessions: [],
  });
  await page.goto("/");
  await expect(page.getByTestId("agent-label")).toHaveText("agent: off");
  // The desktop's message after each autosave (sonara-app `StatusMessage::autosaved`).
  await emit(page, "message", { text: "Autosaved", duration_ms: 1500, error: false });
  const status = page.getByRole("status").filter({ hasText: "Autosaved" });
  await expect(status).toHaveCount(1);
  const shown = Date.now();
  await expect(status).toHaveCount(0, { timeout: 5000 });
  const gone = Date.now() - shown;
  expect(gone).toBeGreaterThan(1000);
  expect(gone).toBeLessThan(3000);
});

test("without IPC the label says off, and startup errors are shown", async ({ page }) => {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: {
      agent: { on: false, address: null },
      messages: [{ text: "project /x.sonara does not exist", duration_ms: 8000, error: true }],
    },
    recoverable_sessions: [],
  });
  await page.goto("/");
  await expect(page.getByTestId("agent-label")).toHaveText("agent: off");
  await expect(page.getByRole("alert")).toHaveText("project /x.sonara does not exist");
});

test("unsaved scratch sessions are offered for recovery", async ({ page }) => {
  const sessions = [
    { workspace: "/w/new-a", name: "Night Drive", saved_at: "2026-10-22T10:00:00Z" },
    { workspace: "/w/new-b", name: null, saved_at: null },
  ];
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: { agent: { on: true, address: SOCKET }, messages: [] },
    recoverable_sessions: sessions,
    recover_session: null,
  });
  await page.goto("/");
  const dialog = page.getByRole("dialog", { name: "Recover Unsaved Work" });
  await expect(dialog).toBeVisible();
  const list = dialog.getByRole("listbox");
  await expect(list.getByRole("option")).toHaveText([
    "Night Drive — autosaved 2026-10-22T10:00:00Z",
    "Untitled — autosaved ?",
  ]);
  await list.selectOption("/w/new-b");
  await dialog.getByRole("button", { name: "Recover" }).click();
  await expect(dialog).toHaveCount(0);
  expect((await calls(page)).filter((c) => c.cmd === "recover_session")).toEqual([
    { cmd: "recover_session", args: { workspace: "/w/new-b" } },
  ]);
});

test("Not Now dismisses the offer, and a failed recovery is reported", async ({ page }) => {
  const sessions = [{ workspace: "/w/new-a", name: "Song", saved_at: "2026-10-22T10:00:00Z" }];
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: { agent: { on: true, address: SOCKET }, messages: [] },
    recoverable_sessions: sessions,
    recover_session: { reject: "cannot recover /w/new-a: project.json is damaged" },
  });
  await page.goto("/");
  const dialog = page.getByRole("dialog", { name: "Recover Unsaved Work" });
  await dialog.getByRole("button", { name: "Recover" }).click();
  await expect(page.getByRole("alert")).toHaveText(
    "cannot recover /w/new-a: project.json is damaged",
  );
  await page.reload();
  await page.getByRole("dialog").getByRole("button", { name: "Not Now" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect((await calls(page)).map((c) => c.cmd)).not.toContain("recover_session");
});
