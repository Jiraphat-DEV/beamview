export { commands } from './commands';
export type { AppConfig, Theme } from './types';
export { DEFAULT_HOTKEYS } from './types';

// M3 — translation IPC
export type { Region, ModelStatus, OcrTranslateResult } from './commands';
export { ocrTranslate, getTranslationModelStatus, downloadTranslationModel } from './commands';
