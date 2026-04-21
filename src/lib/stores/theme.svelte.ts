import { commands } from '$lib/ipc';
import type { Theme } from '$lib/ipc';

/** UI-level theme store.
 *
 * - `pref` is the user's chosen preference: `'light' | 'dark' | 'system'`.
 * - `resolved` is the concrete mode to paint with — always `'light'` or `'dark'`,
 *   collapsing `system` against the OS media query.
 * - `init()` loads the saved preference from Rust config and starts listening to
 *   the OS theme so `resolved` stays accurate if the user flips macOS dark mode
 *   while Beamview is running.
 * - `set(next)` updates the preference in memory only. Persistence is the
 *   Settings modal's job (explicit save, Milestone 6).
 */
class ThemeStore {
  pref = $state<Theme>('system');
  ready = $state(false);
  private systemDark = $state(false);

  /** Concrete 'light' | 'dark' used by [data-theme] on <html>. */
  get resolved(): 'light' | 'dark' {
    if (this.pref === 'light' || this.pref === 'dark') return this.pref;
    return this.systemDark ? 'dark' : 'light';
  }

  async init(): Promise<void> {
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    this.systemDark = mq.matches;
    mq.addEventListener('change', (e) => {
      this.systemDark = e.matches;
    });

    try {
      const cfg = await commands.loadConfig();
      this.pref = cfg.theme;
    } catch (err) {
      // Keep the `system` default — loading can fail on first run if the
      // config dir is inaccessible, but the UI should still render.
      console.warn('[theme] failed to load config, keeping system default', err);
    }

    this.ready = true;
  }

  set(next: Theme): void {
    this.pref = next;
  }
}

export const theme = new ThemeStore();
