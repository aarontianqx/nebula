interface DebouncedPushOptions<T> {
  delayMs: number;
  /** Sends the latest local state; called with no argument so it always snapshots after any wait. */
  send: () => Promise<T>;
  onResult: (result: T) => void;
  onError: (err: unknown) => void;
}

/**
 * Coalesces rapid local edits into one push, never overlapping two pushes: a slow older request
 * must not be able to land after (and overwrite) a newer one.
 */
export class DebouncedPush<T> {
  private timer: ReturnType<typeof setTimeout> | null = null;
  private inFlight: Promise<void> | null = null;
  private dirty = false;
  /** Bumped by `discard`, so a result that arrives for a superseded state is recognised as stale. */
  private generation = 0;

  constructor(private readonly options: DebouncedPushOptions<T>) {}

  schedule(): void {
    this.dirty = true;
    this.clearTimer();
    this.timer = setTimeout(() => void this.push(), this.options.delayMs);
  }

  /** Complete any pending or in-flight push (call before an action that reads the remote state). */
  async flush(): Promise<void> {
    if (this.dirty || this.timer) await this.push();
    else if (this.inFlight) await this.inFlight;
  }

  /** The remote state was replaced: drop pending edits and ignore the result of a push in flight. */
  discard(): void {
    this.clearTimer();
    this.dirty = false;
    this.generation += 1;
  }

  private clearTimer(): void {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
  }

  private async push(): Promise<void> {
    this.clearTimer();
    // `inFlight` never rejects: failures are handled below.
    while (this.inFlight) await this.inFlight;
    // Snapshot after any wait so the push carries every edit made in the meantime. A failure of
    // the push we just waited on is superseded by this snapshot, so its retry mark is cleared.
    this.dirty = false;
    const sentIn = this.generation;
    this.inFlight = this.options
      .send()
      .then((result) => {
        if (this.generation === sentIn) this.options.onResult(result);
      })
      .catch((err) => {
        this.options.onError(err);
        this.dirty = true;
      })
      .finally(() => {
        this.inFlight = null;
      });
    await this.inFlight;
  }
}
