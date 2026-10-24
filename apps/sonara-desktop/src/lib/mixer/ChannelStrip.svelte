<script lang="ts">
  import * as backend from "../backend";
  import type { GestureDeps } from "../gesture";
  import type { Session } from "../session.svelte";
  import ContextMenu from "../shell/ContextMenu.svelte";
  import type { MenuItem } from "../shell/menu";
  import { toHex, trackColor } from "../timeline/colors";
  import type { DeviceView, Edit, EditOutcome, OpenProject, SendView, TrackView } from "../types";
  import { FADER_SPEC, PAN_SPEC, dbText, formatDb, formatPan, parseDb } from "./controls";
  import Fader from "./Fader.svelte";
  import Knob from "./Knob.svelte";
  import { FLOOR_DB, TOP_DB } from "./meter";
  import { meterCanvas } from "./meterCanvas";
  import {
    NEW_SEND_DB,
    STRIP_WIDTH,
    canArm,
    colorIndex,
    dropIndex,
    effectMenu,
    moveIndex,
    sendMenu,
    subtitle,
  } from "./strip";
  import Toggle from "./Toggle.svelte";

  // One channel strip (the prototype's `ChannelStrip`): the name, the subtitle and
  // the colour bar; the inserts; the sends (not on the master); pan;
  // the fader with its meter; the typed dB; M, S and R;
  // the output. A click on the strip selects its track, a double-click shows its devices.
  // The strip is keyed by its track, so a project change refreshes it in place and a
  // control the user drives keeps its own value.

  let { session, track, project, master = false }: {
    session: Session;
    track: TrackView;
    project: OpenProject;
    master?: boolean;
  } = $props();

  /** The height of an insert row, in pixels. */
  const INSERT_ROW = 18;

  const deps = (): GestureDeps => ({
    send: backend.edit,
    seal: backend.sealUndo,
    nextFrame: (callback) => requestAnimationFrame(() => callback()),
    onError: (error) => session.fail(error),
  });

  const color = $derived(toHex(trackColor(colorIndex(project, track), track.color)));
  const selected = $derived(session.selection.tracks.includes(track.id));
  const devices = $derived<DeviceView[]>(track.instrument ? [track.instrument, ...track.inserts] : track.inserts);

  let fader: Fader | undefined = $state();
  let faderShown = $state(0);
  /** The typed dB's text while it is edited; null shows the fader's value. */
  let dbDraft: string | null = $state(null);
  let menu: { x: number; y: number; items: MenuItem[]; label: string } | null = $state(null);
  let insertList: HTMLElement | undefined = $state();
  let insertPress: { device: DeviceView; instrument: boolean; y: number; moved: boolean } | null = null;
  let dropRow: number | null = $state(null);

  async function run(edit: Edit): Promise<EditOutcome | null> {
    try {
      return await backend.edit(edit);
    } catch (error) {
      session.fail(error);
      return null;
    }
  }

  async function seal() {
    try {
      await backend.sealUndo();
    } catch (error) {
      session.fail(error);
    }
  }

  function showMenu(anchor: { x: number; y: number }, label: string, items: MenuItem[]) {
    menu = { ...anchor, label, items };
  }

  function below(event: MouseEvent) {
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    return { x: box.left, y: box.bottom };
  }

  // -- the strip -----------------------------------------------------------------------

  /** Whether a press on `target` belongs to one of the strip's controls rather than the strip. */
  function onControl(target: EventTarget | null): boolean {
    return target instanceof Element && target.closest("button, input, select, [role=slider], .inserts, .send, canvas") !== null;
  }

  function stripDown(event: PointerEvent) {
    if (event.button !== 0 || event.detail >= 2 || onControl(event.target)) return;
    session.selectTrack(track.id, event.shiftKey || event.metaKey);
  }

  function stripDoubleClick(event: MouseEvent) {
    if (onControl(event.target)) return;
    session.showDevices(track.id);
  }

  // -- inserts -------------------------------------------------------------------

  async function addEffect(pluginId: string) {
    const outcome = await run({ op: "add_device", track_id: track.id, plugin_id: pluginId });
    if (outcome?.committed) {
      const deviceId = outcome.result.device_id;
      if (typeof deviceId === "string" && deviceId) session.setCurrentDevice(deviceId);
      session.showDevices(track.id);
    }
  }

  /** MX-09: the device becomes the current one, and the Devices tab shows it. */
  function editDevice(device: DeviceView) {
    session.selectTrack(track.id);
    session.setCurrentDevice(device.id);
    session.showDevices(track.id);
  }

  async function toggleDevice(device: DeviceView, enabled: boolean) {
    await run({ op: "set_device_enabled", device_id: device.id, enabled });
    await seal();
  }

  function rowAt(clientY: number): number {
    if (!insertList) return -1;
    const box = insertList.getBoundingClientRect();
    const row = Math.floor((clientY - box.top + insertList.scrollTop) / INSERT_ROW);
    return row >= 0 && row < devices.length ? row : -1;
  }

  function insertDown(event: PointerEvent, device: DeviceView, instrument: boolean) {
    if (event.button !== 0 || event.ctrlKey) return;
    insertPress = { device, instrument, y: event.clientY, moved: false };
    window.addEventListener("pointermove", insertMove);
    window.addEventListener("pointerup", insertUp, { once: true });
  }

  function insertMove(event: PointerEvent) {
    if (!insertPress || insertPress.instrument) return;
    if (!insertPress.moved && Math.abs(event.clientY - insertPress.y) < 4) return;
    insertPress.moved = true;
    const row = rowAt(event.clientY);
    dropRow = row < 0 ? devices.length : row;
  }

  function insertUp(event: PointerEvent) {
    window.removeEventListener("pointermove", insertMove);
    const press = insertPress;
    insertPress = null;
    dropRow = null;
    if (!press) return;
    if (!press.moved) {
      editDevice(press.device);
      return;
    }
    // The list never moves an item itself; the project's order is the truth.
    const index = dropIndex(rowAt(event.clientY), devices.length, track.instrument !== null);
    const next = moveIndex(
      track.inserts.map((d) => d.id),
      press.device.id,
      index,
    );
    if (next !== null) void run({ op: "move_device", device_id: press.device.id, index: next });
  }

  function insertMenu(event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    const row = rowAt(event.clientY);
    const device = row >= 0 ? devices[row] : null;
    const items: MenuItem[] = [];
    if (device) {
      const instrument = track.instrument?.id === device.id;
      items.push({ label: "Edit…", run: () => editDevice(device) });
      items.push({
        label: device.enabled ? "Bypass" : "Enable",
        run: () => void run({ op: "set_device_enabled", device_id: device.id, enabled: !device.enabled }),
      });
      if (!instrument) items.push({ label: "Remove", run: () => void run({ op: "remove_device", device_id: device.id }) });
      items.push("separator");
    }
    items.push({ label: "Add Effect", items: effectMenu(session.effectGroups, (id) => void addEffect(id)) });
    showMenu({ x: event.clientX, y: event.clientY }, "Inserts", items);
  }

  // -- sends --------------------------------------------------------------------

  function targetName(send: SendView): string {
    return project.tracks.find((t) => t.id === send.target_id)?.name ?? "?";
  }

  function addSend(busId: string) {
    void run({ op: "add_send", track_id: track.id, target_id: busId, level_db: NEW_SEND_DB });
  }

  async function togglePre(send: SendView) {
    await run({ op: "update_send", track_id: track.id, send_id: send.id, pre_fader: !send.pre_fader });
    await seal();
  }

  function sendContext(event: MouseEvent, send: SendView) {
    event.preventDefault();
    event.stopPropagation();
    showMenu({ x: event.clientX, y: event.clientY }, "Send", [
      { label: "Remove Send", run: () => void run({ op: "remove_send", track_id: track.id, send_id: send.id }) },
    ]);
  }

  // -- the typed dB, the buttons and the output ------------------------

  function commitDb(event: FocusEvent) {
    const text = (event.currentTarget as HTMLInputElement).value;
    dbDraft = null;
    const db = parseDb(text);
    if (db === null) {
      session.message(`'${text}' is not a dB value`);
      return;
    }
    if (Math.abs(faderShown - db) > 1e-9) void fader?.commit(db);
  }

  async function chooseOutput(event: Event) {
    const select = event.currentTarget as HTMLSelectElement;
    const outcome = await run({ op: "set_track_output", track_id: track.id, output_id: select.value });
    // A refused route (ROUTING_CYCLE) leaves the model's output; the combo shows it again.
    if (!outcome) select.value = track.output_id ?? "";
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="strip"
  class:master
  class:selected
  role="group"
  aria-label="Channel strip {track.name}"
  data-track={track.id}
  style:width="{STRIP_WIDTH + (master ? 16 : 0)}px"
  onpointerdown={stripDown}
  ondblclick={stripDoubleClick}
>
  <div class="color" style:background={color}></div>
  <div class="name" title={track.name}>{track.name}</div>
  <div class="subtitle">{subtitle(track, project)}</div>

  <div class="head">
    <span class="section">Inserts</span>
    <button
      type="button"
      class="add"
      title="Add effect"
      aria-label="Add effect"
      onclick={(event) =>
        showMenu(below(event), "Add effect", effectMenu(session.effectGroups, (id) => void addEffect(id)))}>+</button
    >
  </div>
  <ul
    class="inserts"
    bind:this={insertList}
    aria-label="Inserts"
    title="Inserts: click to edit, checkbox bypasses, drag to reorder, right-click for more"
    oncontextmenu={insertMenu}
  >
    {#each devices as device (device.id)}
      {@const instrument = track.instrument?.id === device.id}
      <li
        class:current={device.id === session.selection.currentDevice}
        class:bypassed={!device.enabled}
        title="{device.name} ({device.plugin_id}){device.enabled ? '' : ' — bypassed'}"
        onpointerdown={(event) => insertDown(event, device, instrument)}
      >
        <input
          type="checkbox"
          checked={device.enabled}
          aria-label="{device.enabled ? 'Bypass' : 'Enable'} {device.name}"
          onpointerdown={(event) => event.stopPropagation()}
          onchange={(event) => void toggleDevice(device, event.currentTarget.checked)}
        />
        <span>{instrument ? "♪ " : ""}{device.name}</span>
      </li>
    {/each}
    {#if dropRow !== null}<li class="drop" aria-hidden="true" style:top="{dropRow * INSERT_ROW}px"></li>{/if}
  </ul>

  <div class="sends-box" class:hidden={master} aria-hidden={master}>
    <div class="head">
      <span class="section">Sends</span>
      <button
        type="button"
        class="add"
        title="Add send to a bus"
        aria-label="Add send to a bus"
        onclick={(event) => showMenu(below(event), "Add send", sendMenu(track, addSend))}>+</button
      >
    </div>
    <div class="sends">
      {#each track.sends as send (send.id)}
        {@const target = targetName(send)}
        <div class="send" role="group" aria-label="Send to {target}" oncontextmenu={(event) => sendContext(event, send)}>
          <span class="send-name" class:dim={!send.enabled} title="Send to {target}">{target}</span>
          <Knob
            value={send.level_db}
            spec={FADER_SPEC}
            name="Send to {target}"
            format={dbText}
            diameter={18}
            showValue={false}
            minWidth={20}
            {deps}
            edit={(db) => ({ op: "update_send", track_id: track.id, send_id: send.id, level_db: db })}
          />
          <Toggle label="Pre" pressed={send.pre_fader} title="Pre-fader send" width={26} onclick={() => void togglePre(send)} />
        </div>
      {/each}
    </div>
  </div>

  <div class="pan">
    <Knob
      value={track.mixer.pan}
      spec={PAN_SPEC}
      name="Pan"
      format={formatPan}
      bipolar
      diameter={26}
      {deps}
      edit={(pan) => ({ op: "set_track_mixer", track_id: track.id, pan })}
    />
  </div>

  <div class="fader-row">
    <Fader
      bind:this={fader}
      bind:shown={faderShown}
      value={track.mixer.volume_db}
      name="{track.name} volume"
      {deps}
      edit={(db) => ({ op: "set_track_mixer", track_id: track.id, volume_db: db })}
    />
    <!-- A meter for assistive technology: the bank sets its value a few times a second. -->
    <div
      class="meter"
      role="meter"
      aria-label="{track.name} level"
      aria-valuemin={FLOOR_DB}
      aria-valuemax={TOP_DB}
      aria-valuenow={FLOOR_DB}
      aria-valuetext="silent"
      title="Peak / RMS level (dBFS). Click to reset the clip indicator."
      use:meterCanvas={{ bank: session.meters, trackId: track.id }}
    >
      <canvas data-meter={track.id} aria-hidden="true"></canvas>
    </div>
  </div>

  <input
    class="db"
    type="text"
    aria-label="Volume dB"
    title="Volume in dB (type a value, '-inf' for silence)"
    value={dbDraft ?? formatDb(faderShown)}
    onfocus={(event) => (dbDraft = event.currentTarget.value)}
    oninput={(event) => (dbDraft = event.currentTarget.value)}
    onkeydown={(event) => {
      if (event.key === "Enter") event.currentTarget.blur();
    }}
    onblur={commitDb}
  />

  <div class="buttons">
    <Toggle
      label="M"
      color="mute"
      title="Mute"
      pressed={track.mixer.mute}
      onclick={() => void run({ op: "set_track_mixer", track_id: track.id, mute: !track.mixer.mute })}
    />
    {#if !master}
      <Toggle
        label="S"
        color="solo"
        title="Solo"
        pressed={track.mixer.solo}
        onclick={() => void run({ op: "set_track_mixer", track_id: track.id, solo: !track.mixer.solo })}
      />
    {/if}
    {#if canArm(track)}
      <Toggle
        label="R"
        color="arm"
        title="Arm for recording"
        pressed={track.input.armed}
        onclick={() => void run({ op: "set_track_input", track_id: track.id, armed: !track.input.armed })}
      />
    {/if}
  </div>

  {#if master}
    <div class="main-output">Main output</div>
  {:else}
    <select class="output" aria-label="Output routing" title="Output routing" value={track.output_id} onchange={chooseOutput}>
      {#each track.output_targets as target (target.id)}
        <option value={target.id}>→ {target.name}</option>
      {/each}
    </select>
  {/if}
</div>
{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} label={menu.label} onclose={() => (menu = null)} />
{/if}

<style>
  .strip {
    position: relative;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 100%;
    padding: 8px 6px 6px;
    border: 1px solid var(--border);
    background: var(--panel);
    overflow: hidden;
    /* A change inside a strip lays out and paints that strip only, not the row of strips, and
       the strips scrolled out of sight are not rendered at all (about 12 of 101 show). */
    contain: layout paint style;
    content-visibility: auto;
    contain-intrinsic-size: auto 520px;
  }
  .strip.selected {
    border: 1.5px solid var(--accent);
    background: var(--panel-alt);
  }
  .color {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 4px;
  }
  .name {
    overflow: hidden;
    font-size: 12px;
    font-weight: 700;
    text-align: center;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .subtitle {
    overflow: hidden;
    color: var(--text-dim);
    font-size: 10px;
    text-align: center;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 16px;
  }
  .section {
    color: var(--text-dim);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 1px;
    text-transform: uppercase;
  }
  .add {
    width: 18px;
    height: 16px;
    padding: 0;
    border: 0;
    border-radius: 3px;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-weight: 700;
  }
  .add:hover {
    background: var(--panel-alt);
  }
  .inserts {
    position: relative;
    flex: none;
    height: 88px;
    margin: 0;
    padding: 0;
    overflow-x: hidden;
    overflow-y: auto;
    list-style: none;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--lane);
    font-size: 11px;
  }
  .inserts li {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 18px;
    padding: 1px 2px;
    overflow: hidden;
    white-space: nowrap;
  }
  .inserts li span {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .inserts li.current {
    background: var(--accent);
    color: #fff;
  }
  .inserts li.bypassed span {
    color: var(--text-dim);
  }
  .inserts li.drop {
    position: absolute;
    left: 2px;
    right: 2px;
    height: 2px;
    padding: 0;
    background: var(--accent);
    pointer-events: none;
  }
  .inserts input {
    margin: 0 2px 0 0;
  }
  .sends-box {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
    height: 86px;
  }
  .sends-box.hidden {
    visibility: hidden;
  }
  .sends {
    height: 68px;
    overflow-x: hidden;
    overflow-y: auto;
  }
  .send {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 22px;
  }
  .send-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    font-size: 10px;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .send-name.dim {
    color: var(--text-dim);
  }
  .pan {
    display: flex;
    justify-content: center;
  }
  .fader-row {
    flex: 1;
    display: flex;
    justify-content: center;
    gap: 3px;
    min-height: 120px;
  }
  .meter {
    flex: none;
    width: 16px;
    height: 100%;
    min-height: 80px;
  }
  .meter canvas {
    display: block;
    width: 100%;
    height: 100%;
  }
  .db {
    height: 20px;
    width: 100%;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--bg);
    color: var(--text);
    font-family: Menlo, monospace;
    font-size: 11px;
    text-align: center;
    -webkit-user-select: text;
    user-select: text;
  }
  .buttons {
    display: flex;
    justify-content: center;
    gap: 3px;
  }
  .output {
    height: 20px;
    width: 100%;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: 10px;
  }
  .main-output {
    height: 20px;
    color: var(--text-dim);
    font-size: 10px;
    line-height: 20px;
    text-align: center;
  }
</style>
