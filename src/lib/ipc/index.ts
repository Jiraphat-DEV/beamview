export { commands } from './commands';
export type { AppConfig, Theme, TranslationConfig, ConfigRegion } from './types';
export { DEFAULT_HOTKEYS, DEFAULT_TRANSLATION_CONFIG } from './types';

// M3 — translation IPC
export type { Region, ModelStatus, OcrTranslateResult } from './commands';
export { ocrTranslate, getTranslationModelStatus, downloadTranslationModel } from './commands';
