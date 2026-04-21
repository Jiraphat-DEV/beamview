import { setMuted as applyMuted } from '$lib/audio/context';

// Ephemeral UI state — mute toggle and the modal stack.
//
// The modal stack exists so Esc can close the top modal before
// exiting fullscreen (spec §17.2). Anything that opens a dialog
// should push its own id at open and pop at close.

class UiStore {
  muted = $state(false);
  modalStack = $state<string[]>([]);

  get modalOpen(): boolean {
    return this.modalStack.length > 0;
  }

  get topModal(): string | null {
    return this.modalStack.at(-1) ?? null;
  }

  pushModal(id: string): void {
    if (this.modalStack.includes(id)) return;
    this.modalStack = [...this.modalStack, id];
  }

  popModal(id?: string): void {
    if (id === undefined) {
      this.modalStack = this.modalStack.slice(0, -1);
    } else {
      this.modalStack = this.modalStack.filter((m) => m !== id);
    }
  }

  toggleMute(): void {
    this.setMuted(!this.muted);
  }

  setMuted(value: boolean): void {
    this.muted = value;
    applyMuted(value);
  }
}

export const ui = new UiStore();
