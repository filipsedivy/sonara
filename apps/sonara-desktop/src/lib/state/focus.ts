// The live MIDI focus follows the selection's current track. The page sends the current
// track whenever it changes; sonara-app gives the focus only to an instrument track and does
// nothing before the transport exists (the prototype's `_update_midi_focus`). When the transport
// appears later (the live frame's engine leaves "absent"), the focus is sent again.

export class MidiFocus {
  /** The track last sent; `undefined` before the first. */
  private sent: string | null | undefined = undefined;

  constructor(
    private readonly send: (trackId: string | null) => Promise<void>,
    private readonly onError: (error: unknown) => void = () => {},
  ) {}

  /** The selection's current track is `trackId` (or none). */
  follow(trackId: string | null): void {
    if (trackId === this.sent) return;
    this.sent = trackId;
    this.send(trackId).catch(this.onError);
  }

  /** Sends the current track again at the next {@link follow} (the transport was created). */
  reset(): void {
    this.sent = undefined;
  }
}
