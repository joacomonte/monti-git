// Copy to the clipboard and say so for a moment (a ✓ in place of the copy icon).

export class CopyFlash {
  copied = $state<string | null>(null); // what was just copied, by a key the caller picks
  private timer: ReturnType<typeof setTimeout> | undefined;

  constructor(private ms = 1200) {}

  async copy(key: string, text: string | Promise<string>) {
    await navigator.clipboard.writeText(await text);
    this.copied = key;
    clearTimeout(this.timer);
    this.timer = setTimeout(() => (this.copied = null), this.ms);
  }
}
