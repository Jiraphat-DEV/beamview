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

/** Mirrors `SubtitlePosition` in src-tauri/src/config.rs. */
export type SubtitlePosition = 'panel_below' | 'overlay_bottom';

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
  /** Where the subtitle renders. Default `panel_below` (non-blocking). */
  subtitle_position: SubtitlePosition;
}

export const DEFAULT_TRANSLATION_CONFIG: TranslationConfig = {
  enabled: false,
  region: null,
  fps: 1.0,
  // Default ON — helps the user pair EN→TH visually despite the ~1–2 s
  // translation lag (by the time TH appears, the on-video EN has
  // usually changed).
  show_english_caption: true,
  // Default `panel_below` so the translation sits beneath the video
  // without covering game content.  Users who prefer the compact
  // overlay-on-video layout can switch in Settings → การแปล.
  subtitle_position: 'panel_below',
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
