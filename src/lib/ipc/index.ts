export { commands } from './commands';
export type { AppConfig, Theme, TranslationConfig, ConfigRegion } from './types';
export { DEFAULT_HOTKEYS, DEFAULT_TRANSLATION_CONFIG } from './types';

// Translation IPC
export type { Region, ModelStatus, OcrTranslateResult, ModelInfo } from './commands';
export {
  ocrTranslate,
  getTranslationModelStatus,
  downloadTranslationModel,
  listTranslationModels,
  deleteTranslationModel,
  setActiveTranslationModel,
} from './commands';
