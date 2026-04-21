import { debug, error, info, warn } from '@tauri-apps/plugin-log';

// Thin wrapper around @tauri-apps/plugin-log so TS call sites read like
// a normal logger. Forwards every call to the Rust side where
// tauri-plugin-log fans out to stdout, the rolling log file, and the
// webview console (configured in src-tauri/src/logging.rs).
//
// Why not `console.*`? Those only land in the webview devtools, which
// are closed during `pnpm tauri dev` unless explicitly opened. Routing
// through plugin-log means every log line shows up in the terminal
// and the persisted log file as well.

type LogData = Record<string, unknown> | undefined;

function format(msg: string, data: LogData): string {
  if (data === undefined) return msg;
  try {
    return `${msg} ${JSON.stringify(data)}`;
  } catch {
    return `${msg} [unserialisable data]`;
  }
}

export const logger = {
  debug: (msg: string, data?: LogData): void => {
    void debug(format(msg, data));
  },
  info: (msg: string, data?: LogData): void => {
    void info(format(msg, data));
  },
  warn: (msg: string, data?: LogData): void => {
    void warn(format(msg, data));
  },
  error: (msg: string, data?: LogData): void => {
    void error(format(msg, data));
  },
};
