// App-level hotkey registry.
//
// Bindings declare a `match` function that inspects the KeyboardEvent
// and returns `true` if it handled it. The registry calls matchers in
// descending priority order and calls preventDefault() on the first
// match so only one binding fires per event.
//
// Why not `window.addEventListener` scattered across components?
//   1. Spec §17.2 requires an Esc priority stack (modal before
//      fullscreen). One ordered registry makes that trivial.
//   2. Every component binding its own listener would race on Esc.
//   3. Skipping keystrokes while focus is inside an <input> should
//      be a single rule, not repeated at every call site.

export interface HotkeyBinding {
  /** Stable id used as the map key (must be unique). */
  id: string;
  /** Higher priority matches first. Default 0. */
  priority?: number;
  /** Return `true` if this binding handled the event — the registry
   *  will call preventDefault() and stop further dispatch. Return
   *  `false` to let lower-priority bindings see the event. */
  match: (event: KeyboardEvent) => boolean;
}

const bindings = new Map<string, HotkeyBinding>();
let installed = false;

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  return ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName);
}

function keydownHandler(event: KeyboardEvent): void {
  // Keep text editing unobstructed even when a hotkey uses the same
  // key (e.g. Cmd+A vs. select-all in a field). Form fields still see
  // their usual keystrokes; hotkeys fire only from the main UI chrome.
  if (isTypingTarget(event.target)) return;

  const sorted = [...bindings.values()].sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0));
  for (const binding of sorted) {
    if (binding.match(event)) {
      event.preventDefault();
      return;
    }
  }
}

/** Register a binding. Returns an unsubscribe function. */
export function register(binding: HotkeyBinding): () => void {
  bindings.set(binding.id, binding);
  return () => {
    bindings.delete(binding.id);
  };
}

/** Attach the single window-level keydown listener. Idempotent.
 *  Returns an uninstall function the caller can use in a component
 *  onDestroy to fully detach during tests. */
export function install(): () => void {
  if (installed) return () => undefined;
  window.addEventListener('keydown', keydownHandler);
  installed = true;
  return () => {
    window.removeEventListener('keydown', keydownHandler);
    installed = false;
  };
}

// Convenience matchers for the common Mac-style combinations so call
// sites stay readable.
export function isMetaKey(event: KeyboardEvent, key: string): boolean {
  return (
    event.key.toLowerCase() === key.toLowerCase() &&
    event.metaKey &&
    !event.ctrlKey &&
    !event.altKey
  );
}

export function isEscape(event: KeyboardEvent): boolean {
  return event.key === 'Escape' && !event.metaKey && !event.ctrlKey && !event.altKey;
}

export function isF11(event: KeyboardEvent): boolean {
  return event.key === 'F11';
}
