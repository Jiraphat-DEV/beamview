// Types that mirror the Rust AppConfig / Theme in src-tauri/src/config.rs.
// Keep field names in snake_case to match the JSON wire format written
// by serde; Svelte stores can adapt these into camelCase if desired.

export type Theme = 'light' | 'dark' | 'system';

/**
 * A rectangular region in video native-coordinate space.
 * Mirrors `ConfigRegion` in src-tauri/src/config.rs.
 */
export interface ConfigRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Translation feature configuration (added in schema v2).
 * Mirrors `TranslationConfig` in src-tauri/src/config.rs.
 */
export interface TranslationConfig {
  enabled: boolean;
  region: ConfigRegion | null;
  /** 0.5 | 1.0 | 2.0 fps. Default 1.0. */
  fps: number;
  show_english_caption: boolean;
}

export const DEFAULT_TRANSLATION_CONFIG: TranslationConfig = {
  enabled: false,
  region: null,
  fps: 1.0,
  show_english_caption: false,
};

export interface AppConfig {
  schema_version: number;
  last_video_device_id: string | null;
  last_audio_device_id: string | null;
  theme: Theme;
  hotkeys: Record<string, string>;
  /** True once the user has completed the first-run Welcome flow. */
  welcome_dismissed: boolean;
  /** Translation feature settings (added in schema v2). */
  translation: TranslationConfig;
}

export const DEFAULT_HOTKEYS: Readonly<Record<string, string>> = Object.freeze({
  fullscreen: 'Cmd+F',
  mute: 'Cmd+M',
  settings: 'Cmd+,',
  quit: 'Cmd+Q',
});
