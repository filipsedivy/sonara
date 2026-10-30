// The payloads of the desktop's commands and events (apps/sonara-desktop/src-tauri/src), as
// sonara-app serializes them (crates/sonara-app/src: view.rs, events.rs, edit.rs, peaks.rs,
// eq_response.rs).

/** Whether agents can attach (`sonara mcp`), and the IPC address. */
export interface AgentStatus {
  on: boolean;
  address: string | null;
}

/** A status-bar message (`message` event, and the startup's messages). */
export interface StatusMessage {
  text: string;
  duration_ms: number;
  error: boolean;
}

/** One pane of the window: its size in pixels and whether it shows. */
export interface Pane {
  size: number;
  open: boolean;
}

/** The window's panes: the Browser (left), the Inspector (right), the Editors (bottom). */
export interface PaneLayout {
  browser: Pane;
  inspector: Pane;
  editors: Pane;
}

/** `shell_status`: what the window shows at startup. */
export interface ShellStatus {
  agent: AgentStatus;
  messages: StatusMessage[];
  /** The panes as they were saved. */
  panes: PaneLayout;
  /** The audio file suffixes a drop imports (".wav", …). */
  audio_suffixes: string[];
  /** The mixer's own window was open when Sonara quit: it opens again (A10). */
  reopen_mixer?: boolean;
}

/** `view-command`: a View menu item for the page. */
export interface ViewCommand {
  command:
    | "toggle-browser"
    | "toggle-inspector"
    | "toggle-editors"
    | "mixer"
    | "piano-roll"
    | "devices";
}

/** `transport_stop`: what a stop did; a recording that ended counts its takes. */
export interface Stopped {
  was_playing: boolean;
  takes: number | null;
  message: StatusMessage | null;
  status: Record<string, unknown>;
}

/**
 * `transport_status`: the transport's `status()`, null before the transport exists. Without a
 * usable backend `{available: false, backend, reason}`; otherwise the backend and, once a player
 * exists, its status (`device_sample_rate`, `buffer_frames`, …).
 */
export interface EngineStatus {
  available: boolean;
  backend?: string;
  reason?: string | null;
  state?: string;
  [key: string]: unknown;
}

/** An unsaved scratch session that can be recovered. */
export interface RecoverableSession {
  workspace: string;
  name: string | null;
  saved_at: string | null;
}

// -- events ---------------------------------------------------------------------------------------

/** A changed object of a session event. */
export interface ObjectNotice {
  kind: string;
  id: string;
}

/** A session event's change set. */
export interface ChangeNotice {
  created: ObjectNotice[];
  updated: ObjectNotice[];
  deleted: ObjectNotice[];
  /** The change categories, sorted (`clips`, `mixer`, `tracks`, …). */
  flags: string[];
  /** The panels rebuild instead of refreshing in place (the prototype's `structure_changed`). */
  structure_changed: boolean;
}

/** `session-changed`: one session event. */
export interface SessionNotice {
  version: number;
  kind: "command" | "undo" | "redo" | "loaded" | "closed" | "saved";
  label: string;
  source: string;
  changes: ChangeNotice;
}

/** An error with the agent API's code. */
export interface ErrorNotice {
  code: string;
  message: string;
}

/** `job-changed`: a background job's state. */
export interface JobNotice {
  id: string;
  kind: string;
  label: string;
  status: "queued" | "running" | "succeeded" | "failed" | "cancelled";
  finished: boolean;
  progress: number;
  message: string;
  result: Record<string, unknown> | null;
  error: ErrorNotice | null;
}

/** `peaks-ready`: a waveform pyramid became ready, or failed. */
export interface PeaksReady {
  asset_id: string;
  sample_rate: number;
  state: "ready" | "failed";
}

// -- the project view -------------------------------------------------------------------------------

export type ParamValue = boolean | number | string;

export interface TempoView {
  beat: number;
  bpm: number;
  ramp: boolean;
}

export interface TimeSignatureView {
  bar: number;
  beat: number;
  numerator: number;
  denominator: number;
  beats_per_bar: number;
}

export interface TransportView {
  loop_enabled: boolean;
  loop_start: number;
  loop_end: number;
  punch_enabled: boolean;
  punch_in: number;
  punch_out: number;
  metronome: boolean;
  count_in_bars: number;
}

export interface MetadataView {
  title: string;
  artist: string;
  album: string;
  genre: string;
  composer: string;
  comment: string;
}

export interface MixerView {
  volume_db: number;
  pan: number;
  mute: boolean;
  solo: boolean;
  solo_safe: boolean;
}

export interface TargetView {
  id: string;
  name: string;
}

export interface SendView {
  id: string;
  target_id: string;
  level_db: number;
  pre_fader: boolean;
  enabled: boolean;
}

export interface InputView {
  channels: number[];
  armed: boolean;
  monitor: "off" | "auto" | "on";
}

export interface DeviceView {
  id: string;
  plugin_id: string;
  name: string;
  enabled: boolean;
  preset: string | null;
  sidechain_id: string | null;
  available: boolean;
  params: Record<string, ParamValue>;
}

export interface FadeView {
  seconds: number;
  curve: "linear" | "equal_power" | "exponential" | "s_curve";
}

export interface NoteView {
  id: string;
  pitch: number;
  start: number;
  length: number;
  velocity: number;
  channel: number;
}

interface ClipBase {
  id: string;
  name: string;
  start: number;
  length: number;
  muted: boolean;
  color: string | null;
}

export interface AudioClipView extends ClipBase {
  type: "audio";
  asset_id: string;
  source_offset: number;
  gain_db: number;
  fade_in: FadeView;
  fade_out: FadeView;
}

export interface MidiClipView extends ClipBase {
  type: "midi";
  content_offset: number;
  loop_length: number | null;
  notes: NoteView[];
}

export type ClipView = AudioClipView | MidiClipView;

export type TrackKind = "audio" | "instrument" | "bus" | "folder" | "master";

export interface TrackView {
  id: string;
  name: string;
  kind: TrackKind;
  color: string | null;
  parent_id: string | null;
  depth: number;
  mixer: MixerView;
  output_id: string | null;
  output_targets: TargetView[];
  sends: SendView[];
  send_targets: TargetView[];
  input: InputView;
  instrument: DeviceView | null;
  inserts: DeviceView[];
  clips: ClipView[];
}

export interface MarkerView {
  id: string;
  beat: number;
  name: string;
  color: string | null;
}

export interface SectionView {
  id: string;
  name: string;
  start: number;
  length: number;
  color: string | null;
}

export interface AssetView {
  id: string;
  kind: "audio" | "midi" | "impulse";
  name: string;
  location: "embedded" | "linked";
  sample_rate: number;
  channels: number;
  frames: number;
  duration_seconds: number;
}

export interface ParamView {
  id: string;
  name: string;
  kind: "float" | "int" | "bool" | "enum";
  default: ParamValue;
  min: number | null;
  max: number | null;
  unit: string;
  choices: string[];
  log_scale: boolean;
  automatable: boolean;
  description: string;
}

export interface PluginView {
  id: string;
  name: string;
  kind: "effect" | "instrument";
  category: string;
  description: string;
  sidechain: boolean;
  presets: string[];
  params: ParamView[];
}

export interface HistoryView {
  undo: string | null;
  redo: string | null;
}

/** The open project as the panels show it. */
export interface OpenProject {
  id: string;
  name: string;
  path: string | null;
  dirty: boolean;
  sample_rate: number;
  length_beats: number | null;
  end_beat: number;
  tempo: TempoView[];
  time_signatures: TimeSignatureView[];
  transport: TransportView;
  metadata: MetadataView;
  tracks: TrackView[];
  master: TrackView;
  markers: MarkerView[];
  sections: SectionView[];
  assets: AssetView[];
  plugins: Record<string, PluginView>;
  history: HistoryView;
  /** The order of a live frame's meters; they apply only when the frame's layout equals it. */
  meter_layout: number;
}

/** `project_view`: the session version and the open project. */
export interface ProjectView {
  version: number;
  project: OpenProject | null;
}

/** `project_tracks`: some tracks, for the refresh of an edit that changed only them. */
export interface TracksView {
  version: number;
  project: {
    id: string;
    dirty: boolean;
    history: HistoryView;
    meter_layout: number;
    /** In the project's order, the master last. */
    tracks: TrackView[];
  } | null;
}

/** `drum_map`: the drum kit's plugin and the GM names of its notes (the piano roll's keyboard). */
export interface DrumMap {
  plugin_id: string;
  names: { pitch: number; label: string }[];
}

/** `project_clips`: some MIDI clips, for the refresh of an edit that changed only their notes. */
export interface ClipsView {
  version: number;
  project: {
    id: string;
    dirty: boolean;
    history: HistoryView;
    meter_layout: number;
    /** In the project's order, each with its track. */
    clips: { track_id: string; clip: ClipView }[];
  } | null;
}

/** `project_devices`: some devices, for the refresh of an edit that changed only their state. */
export interface DevicesView {
  version: number;
  project: {
    id: string;
    dirty: boolean;
    history: HistoryView;
    meter_layout: number;
    /** In the project's order (the master's last), each with its track. */
    devices: { track_id: string; device: DeviceView }[];
  } | null;
}

// -- edits --------------------------------------------------------------------------------------------

/** A new note of `add_notes`. */
export interface NewNote {
  pitch: number;
  start: number;
  length: number;
  velocity: number;
  channel?: number;
}

/** A change of one note of `edit_notes`; a missing field keeps the value. */
export interface NoteChange {
  note_id: string;
  pitch?: number;
  start?: number;
  length?: number;
  velocity?: number;
  channel?: number;
}

export interface FadeEdit {
  seconds: number;
  curve: FadeView["curve"];
}

/** One edit (sonara-app `edit::Edit`); the inventory ID of each is in its Rust docs. */
export type Edit =
  | {
      op: "create_track";
      name: string;
      kind: "audio" | "instrument" | "bus" | "folder";
      instrument?: string | null;
      preset?: string | null;
    }
  | { op: "delete_track"; track_id: string }
  | { op: "rename_track"; track_id: string; name: string }
  | { op: "set_track_color"; track_id: string; color: string | null }
  | { op: "move_track"; track_id: string; index: number }
  | {
      op: "set_track_mixer";
      track_id: string;
      volume_db?: number;
      pan?: number;
      mute?: boolean;
      solo?: boolean;
    }
  | {
      op: "set_track_input";
      track_id: string;
      armed?: boolean;
      monitor?: InputView["monitor"];
      channels?: number[];
    }
  | { op: "set_track_output"; track_id: string; output_id: string }
  | { op: "add_send"; track_id: string; target_id: string; level_db: number; pre_fader?: boolean }
  | {
      op: "update_send";
      track_id: string;
      send_id: string;
      level_db?: number;
      pre_fader?: boolean;
    }
  | { op: "remove_send"; track_id: string; send_id: string }
  | { op: "add_device"; track_id: string; plugin_id: string; preset?: string; index?: number }
  | { op: "remove_device"; device_id: string }
  | { op: "move_device"; device_id: string; index: number }
  | { op: "set_device_enabled"; device_id: string; enabled: boolean }
  | { op: "set_device_params"; device_id: string; values: Record<string, ParamValue> }
  | { op: "reset_device"; device_id: string }
  | { op: "apply_preset"; device_id: string; preset: string }
  | { op: "set_instrument"; track_id: string; plugin_id: string }
  | { op: "set_sidechain"; device_id: string; source_track_id: string | null }
  | { op: "create_midi_clip"; track_id: string; start: number; length: number; name: string }
  | { op: "move_clip"; clip_id: string; start?: number; track_id?: string }
  | { op: "move_clips"; clip_ids: string[]; delta_beats: number }
  | { op: "trim_clip"; clip_id: string; start?: number; end?: number }
  | { op: "split_clips"; clip_ids: string[]; at: number }
  | { op: "duplicate_clips"; clip_ids: string[] }
  | { op: "delete_clips"; clip_ids: string[] }
  | {
      op: "set_clip_properties";
      clip_id: string;
      name?: string;
      muted?: boolean;
      gain_db?: number;
      fade_in?: FadeEdit;
      fade_out?: FadeEdit;
      /** `null` ends the loop; a missing field keeps it. */
      loop_length?: number | null;
    }
  | { op: "add_notes"; clip_id: string; notes: NewNote[] }
  | { op: "edit_notes"; clip_id: string; edits: NoteChange[] }
  | { op: "delete_notes"; clip_id: string; note_ids: string[] }
  | { op: "quantize_notes"; clip_id: string; note_ids: string[]; grid: number; strength: number }
  | {
      op: "set_transport";
      loop_enabled?: boolean;
      loop_start?: number;
      loop_end?: number;
      metronome?: boolean;
      count_in_bars?: number;
    }
  | { op: "set_tempo"; bpm: number }
  | { op: "set_time_signature"; numerator: number; denominator: number }
  | { op: "rename_project"; name: string }
  | { op: "set_project_length"; length_beats: number | null }
  | {
      op: "set_metadata";
      values: { title?: string; artist?: string; album?: string; genre?: string };
    };

/** What an edit, undo or redo did. */
export interface EditOutcome {
  committed: boolean;
  version: number;
  label: string;
  changes: ChangeNotice;
  result: Record<string, unknown>;
}

// -- computed data ------------------------------------------------------------------------------------

/** `peaks`: a waveform pyramid's state and shape; its tiles come from the `peaks:` scheme. */
export interface PeaksInfo {
  state: "ready" | "pending" | "failed";
  sample_rate: number;
  frames: number;
  missing: boolean;
  level_bins: number[];
  bin_frames: number;
  tile_bins: number;
}

/** One entry of a plugin picker (sonara-app `catalog::PluginItem`). */
export interface PluginItem {
  id: string;
  name: string;
  description: string;
}

/** One category of a plugin picker: its submenu's title and its plugins (`catalog::PluginGroup`). */
export interface PluginGroup {
  category: string;
  title: string;
  plugins: PluginItem[];
}

/** One playable preset among the sampler's instrument choices (`instruments::PresetChoice`). */
export interface PresetChoice {
  /** The preset ID, the value the `instrument` parameter stores. */
  id: string;
  /** The preset's name, with its path in the library when another preset there shares it. */
  label: string;
}

/** `sampler_instruments`: one library's presets among the sampler's choices (`instruments::LibraryChoices`). */
export interface LibraryChoices {
  id: string;
  name: string;
  presets: PresetChoice[];
}

/** `eq_response`: the parametric EQ's curve. */
export interface EqResponse {
  freqs: number[];
  db: number[];
  bands: { freq: number; gain_db: number; on: boolean }[];
}
