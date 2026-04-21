import { setMuted as applyMuted, setVolume as applyVolume } from '$lib/audio/context';

// Ephemeral UI state — mute, volume, modal stack, and toasts.
//
// The modal stack exists so Esc can close the top modal before
// exiting fullscreen (spec §17.2). Anything that opens a dialog
// should push its own id at open and pop at close.

export type ToastKind = 'info' | 'success' | 'warn' | 'error';

export interface Toast {
  id: number;
  kind: ToastKind;
  message: string;
}

const DEFAULT_DISMISS_MS: Record<ToastKind, number | null> = {
  info: 2500,
  success: 2000,
  warn: 4000,
  error: null, // errors require explicit dismissal
};

class UiStore {
  muted = $state(false);
  volume = $state(1);
  modalStack = $state<string[]>([]);
  toasts = $state<Toast[]>([]);

  private nextToastId = 1;

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

  setVolume(value: number): void {
    const clamped = Math.max(0, Math.min(1, value));
    this.volume = clamped;
    if (!this.muted) applyVolume(clamped);
  }

  /** Enqueue a toast. Returns its id so callers can dismiss early. */
  showToast(message: string, kind: ToastKind = 'info'): number {
    const id = this.nextToastId++;
    const toast: Toast = { id, kind, message };
    this.toasts = [...this.toasts, toast];

    const dismissAfter = DEFAULT_DISMISS_MS[kind];
    if (dismissAfter !== null) {
      window.setTimeout(() => this.dismissToast(id), dismissAfter);
    }
    return id;
  }

  dismissToast(id: number): void {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }
}

export const ui = new UiStore();
