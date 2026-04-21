// Types that mirror the Rust AppConfig / Theme in src-tauri/src/config.rs.
// Keep field names in snake_case to match the JSON wire format written
// by serde; Svelte stores can adapt these into camelCase if desired.

export type Theme = 'light' | 'dark' | 'system';

export interface AppConfig {
  schema_version: number;
  last_video_device_id: string | null;
  last_audio_device_id: string | null;
  theme: Theme;
  hotkeys: Record<string, string>;
}

export const DEFAULT_HOTKEYS: Readonly<Record<string, string>> = Object.freeze({
  fullscreen: 'Cmd+F',
  mute: 'Cmd+M',
  settings: 'Cmd+,',
  quit: 'Cmd+Q',
});
