import { invoke } from '@tauri-apps/api/core';
import type { AppConfig } from './types';

// Typed wrappers around the Rust `#[tauri::command]` handlers in
// src-tauri/src/commands.rs. Each call returns a Promise and rejects
// with the string body emitted by `Err(String)` on the Rust side.

export const commands = {
  loadConfig: (): Promise<AppConfig> => invoke<AppConfig>('load_config'),

  saveConfig: (config: AppConfig): Promise<void> => invoke<void>('save_config', { config }),

  resetConfig: (): Promise<AppConfig> => invoke<AppConfig>('reset_config'),

  getAppVersion: (): Promise<string> => invoke<string>('get_app_version'),

  quitApp: (): Promise<void> => invoke<void>('quit_app'),
};
