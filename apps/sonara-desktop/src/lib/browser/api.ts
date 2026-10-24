// The typed client of the Browser's commands (src-tauri/src/dialogs.rs; sonara-app `browser.rs`).

import { invoke } from "@tauri-apps/api/core";
import type { BrowserItem, FileEntry } from "./tree";

/** The Devices tab's tree. */
export function browserDevices(): Promise<BrowserItem[]> {
  return invoke<BrowserItem[]>("browser_devices");
}

/** The Files tab's root folder (`~/Music`, else `~`). */
export function browserRoot(): Promise<string> {
  return invoke<string>("browser_root");
}

/** The folders and audio files of `path` (the import's formats). */
export function listDirectory(path: string): Promise<FileEntry[]> {
  return invoke<FileEntry[]>("list_directory", { path });
}
