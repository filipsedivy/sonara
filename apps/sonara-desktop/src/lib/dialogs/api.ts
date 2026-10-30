// The typed client of the dialogs' commands (src-tauri/src/dialogs.rs) and their payloads, as
// sonara-app serializes them (crates/sonara-app/src: projects.rs, settings_dialog.rs,
// export_dialog.rs, about.rs). The forms are checked and the texts worded there; this module only
// calls the commands.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** A dialog the menu asks the page to open (`dialog-command`). */
export type DialogName = "new-project" | "settings" | "export-audio" | "agent-connection" | "about" | "import-audio";

/** One entry of a combo: its text and the value it sends. */
export interface Choice<T> {
  label: string;
  value: T;
}

/** An error of a command: the agent API's code and the message. */
export interface ErrorNotice {
  code: string;
  message: string;
}

// -- New Project ---------------------------------------------------------------------------

/** The New Project dialog's fields (`projects::NewProjectForm`). */
export interface NewProjectForm {
  name: string;
  template: string;
  bpm: number;
  time_signature: string;
  sample_rate: number;
}

/** `new_project_view`. */
export interface NewProjectView {
  form: NewProjectForm;
  templates: { id: string; name: string; description: string }[];
  signatures: string[];
  rates: [string, number][];
  bpm: [number, number];
}

/** A field the dialog got wrong; `form` when the project as a whole cannot be made. */
export interface FieldError extends ErrorNotice {
  field: keyof NewProjectForm | "form";
}

// -- Settings -------------------------------------------------------------------------------

/** The Settings dialog's fields (`settings_dialog::SettingsForm`). */
export interface SettingsForm {
  backend: string;
  output_device: string | null;
  input_device: string | null;
  buffer_frames: number;
  lookahead_ms: number;
  autosave_seconds: number;
  backup_retention: number;
  undo_limit: number;
  theme: string;
  agent_require_confirmation: boolean;
  agent_allowed_paths: string;
}

/** `settings_view`: the fields and the combos' choices (a stored value outside them included). */
export interface SettingsView {
  form: SettingsForm;
  backends: Choice<string>[];
  outputs: Choice<string | null>[];
  inputs: Choice<string | null>[];
  devices_error: string | null;
  buffers: Choice<number>[];
  themes: Choice<string>[];
  lookahead_ms: [number, number];
  autosave_seconds: [number, number];
  backup_retention: [number, number];
  undo_limit: [number, number];
}

// -- Export Audio ---------------------------------------------------------------------------

export type ExportTail = "auto" | "none" | number;

/** The Export Audio dialog's fields (`export_dialog::ExportForm`). */
export interface ExportForm {
  path: string;
  preset: string;
  formats: string[];
  bit_depth: "16" | "24" | "32f";
  sample_rate: number | null;
  channels: 1 | 2;
  bitrate_kbps: number;
  dither: boolean;
  range: "song" | "loop";
  tail: ExportTail;
  normalize: "none" | "peak" | "true_peak" | "loudness";
  target: number;
  ceiling: number;
  stems: boolean;
  through_master: boolean;
  /** The checked stem tracks, in the list's order. */
  stem_tracks: string[];
}

/** `export_view`. */
export interface ExportView {
  form: ExportForm;
  formats: Choice<string>[];
  presets: string[];
  rates: Choice<number | null>[];
  bitrates: Choice<number>[];
  stem_tracks: Choice<string>[];
  target_db: [number, number];
  ceiling_db: [number, number];
}

/** `export_audio`: whether the jobs started (not when the user kept the existing files). */
export interface Exported {
  started: boolean;
  jobs: string[];
}

// -- Help ----------------------------------------------------------------------------

/** `agent_connection`. */
export interface AgentConnection {
  status: string;
  address: string | null;
  cli: string;
  command: string;
  text: string;
  discovery: string | null;
}

/** `about_info`. */
export interface About {
  product: string;
  version: string;
  tagline: string;
  author: string;
  email: string;
  copyright: string;
  system: [string, string][];
  credits: { name: string; license: string; url: string }[];
  lgpl_note: string;
  licenses: string | null;
}

// -- commands ---------------------------------------------------------------------------------------

/** What the menu asks the page to open. */
export function onDialogCommand(handler: (dialog: DialogName) => void): Promise<UnlistenFn> {
  return listen<{ dialog: DialogName }>("dialog-command", (event) => handler(event.payload.dialog));
}

export function newProjectView(): Promise<NewProjectView> {
  return invoke<NewProjectView>("new_project_view");
}

/** MW-01: rejects with a {@link FieldError}; `created` is false when the unsaved prompt was cancelled. */
export function createProject(form: NewProjectForm): Promise<{ created: boolean }> {
  return invoke<{ created: boolean }>("create_project", { form });
}

export function settingsView(): Promise<SettingsView> {
  return invoke<SettingsView>("settings_view");
}

/** MW-14: saves the settings; the autosave timer follows. */
export function applySettings(form: SettingsForm): Promise<{ autosave_seconds: number }> {
  return invoke<{ autosave_seconds: number }>("apply_settings", { form });
}

export function exportView(): Promise<ExportView> {
  return invoke<ExportView>("export_view");
}

/** The fields after choosing `preset` (`_apply_preset`). */
export function exportPreset(form: ExportForm, preset: string): Promise<ExportForm> {
  return invoke<ExportForm>("export_preset", { form, preset });
}

/** Browse…: the save panel; the chosen path without its extension, or null. */
export function exportBrowse(path: string): Promise<string | null> {
  return invoke<string | null>("export_browse", { path });
}

/** MW-09: asks before replacing files (the stems' too), then queues the export. */
export function exportAudio(form: ExportForm): Promise<Exported> {
  return invoke<Exported>("export_audio", { form });
}

export function agentConnection(): Promise<AgentConnection> {
  return invoke<AgentConnection>("agent_connection");
}

export function aboutInfo(): Promise<About> {
  return invoke<About>("about_info");
}

/** MW-21: deletes a track, asking first when it has clips; false when nothing was deleted. */
export function deleteTrack(trackId: string): Promise<boolean> {
  return invoke<boolean>("delete_track", { trackId });
}

/** The message of a rejected command. */
export function errorMessage(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return String(error);
}
