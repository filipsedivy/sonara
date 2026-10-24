// The Edit, Track and Transport menus' commands (`menu-command`, sonara-app's
// `menus::PageCommand`): the main window's page runs them as its own keys and gestures do, and
// declines every one while a dialog is open (the prototype's dialogs were modal). The desktop sends
// them only to the main window, and declines them itself while a native prompt or a dialog is open.

import type { Edit } from "../types";
import type { EditCommand } from "./editing";
import type { TransportShortcut } from "./shortcuts";

/** A command of the Edit, Track or Transport menu. */
export type MenuCommand =
  | { readonly command: EditCommand }
  | {
      readonly command: "new-instrument-track" | "new-drum-track" | "new-audio-track" | "new-bus" | "new-folder";
      /** MW-20: the track's `create_track` edit, as sonara-app names it. */
      readonly edit: Edit;
    }
  | { readonly command: "delete-track" }
  | { readonly command: "play-stop" | "return-to-start" | "record" | "loop" | "metronome" };

/** What the page does for the menu's commands. */
export interface MenuCommandHandlers {
  /** Whether a dialog is open: then nothing runs. */
  modal(): boolean;
  /** G9: the Edit menu's item on the focused editor. */
  edit(command: EditCommand): void;
  /** MW-20: runs `edit` and selects the new track. */
  newTrack(edit: Edit): void;
  /** MW-21: deletes the selection's current track, asking first when it has clips. */
  deleteTrack(): void;
  /** TB-01–TB-06: the transport's own action. */
  transport(action: TransportShortcut): void;
}

/** The transport bar's action of each Transport item. */
const TRANSPORT: Record<string, TransportShortcut> = {
  "play-stop": "play",
  "return-to-start": "home",
  record: "record",
  loop: "loop",
  metronome: "metronome",
};

/** Runs `command`; whether it was one the page knows (a dialog's being open declines it, `true`). */
export function runMenuCommand(command: MenuCommand, handlers: MenuCommandHandlers): boolean {
  const known =
    command.command === "split-clips" ||
    command.command === "duplicate-clips" ||
    command.command === "delete-clips" ||
    command.command === "delete-track" ||
    "edit" in command ||
    command.command in TRANSPORT;
  if (!known || handlers.modal()) return known;
  if (command.command === "split-clips" || command.command === "duplicate-clips" || command.command === "delete-clips") {
    handlers.edit(command.command);
  } else if ("edit" in command) {
    handlers.newTrack(command.edit);
  } else if (command.command === "delete-track") {
    handlers.deleteTrack();
  } else {
    handlers.transport(TRANSPORT[command.command]);
  }
  return true;
}
