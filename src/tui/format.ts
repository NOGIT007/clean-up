/**
 * ANSI formatting utilities — zero dependencies.
 * Provides colors, size formatting, age formatting, and path truncation.
 */

// ANSI escape codes
const ESC = "\x1b[";
const RESET = `${ESC}0m`;

export const colors = {
  bold: (s: string) => `${ESC}1m${s}${RESET}`,
  dim: (s: string) => `${ESC}2m${s}${RESET}`,
  italic: (s: string) => `${ESC}3m${s}${RESET}`,
  underline: (s: string) => `${ESC}4m${s}${RESET}`,
  red: (s: string) => `${ESC}31m${s}${RESET}`,
  green: (s: string) => `${ESC}32m${s}${RESET}`,
  yellow: (s: string) => `${ESC}33m${s}${RESET}`,
  blue: (s: string) => `${ESC}34m${s}${RESET}`,
  magenta: (s: string) => `${ESC}35m${s}${RESET}`,
  cyan: (s: string) => `${ESC}36m${s}${RESET}`,
  white: (s: string) => `${ESC}97m${s}${RESET}`,
  gray: (s: string) => `${ESC}90m${s}${RESET}`,
  // 256-color for the green-teal theme
  teal: (s: string) => `${ESC}38;5;43m${s}${RESET}`,
  mint: (s: string) => `${ESC}38;5;48m${s}${RESET}`,
  softWhite: (s: string) => `${ESC}38;5;252m${s}${RESET}`,
  bgRed: (s: string) => `${ESC}41m${s}${RESET}`,
  bgGreen: (s: string) => `${ESC}42m${s}${RESET}`,
  bgYellow: (s: string) => `${ESC}43m${s}${RESET}`,
  bgBlue: (s: string) => `${ESC}44m${s}${RESET}`,
  bgTeal: (s: string) => `${ESC}48;5;30m${s}${RESET}`,
};

/** Format bytes into human-readable string (e.g. 1.5 GB). */
export function formatBytes(bytes: number): string {
  if (!bytes || !Number.isFinite(bytes) || bytes < 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const val = bytes / Math.pow(1024, i);
  return `${val >= 100 ? val.toFixed(0) : val >= 10 ? val.toFixed(1) : val.toFixed(2)} ${units[i]}`;
}

/** Format age in ms to human-readable string (e.g. "3 months"). */
export function formatAge(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  const months = Math.floor(days / 30);
  const years = Math.floor(days / 365);

  if (years > 0) return `${years} year${years === 1 ? "" : "s"}`;
  if (months > 0) return `${months} month${months === 1 ? "" : "s"}`;
  if (days > 0) return `${days} day${days === 1 ? "" : "s"}`;
  if (hours > 0) return `${hours} hour${hours === 1 ? "" : "s"}`;
  if (minutes > 0) return `${minutes} min${minutes === 1 ? "" : "s"}`;
  return `${seconds} sec${seconds === 1 ? "" : "s"}`;
}

/** Truncate a path to fit within maxLen, keeping the end visible. */
export function truncatePath(filePath: string, maxLen: number = 60): string {
  if (filePath.length <= maxLen) return filePath;

  // Replace home dir with ~
  const home = process.env.HOME ?? "";
  let display = filePath;
  if (home && display.startsWith(home)) {
    display = "~" + display.slice(home.length);
  }

  if (display.length <= maxLen) return display;

  // Truncate from the middle
  const ellipsis = "...";
  const keep = maxLen - ellipsis.length;
  const front = Math.ceil(keep / 2);
  const back = Math.floor(keep / 2);
  return display.slice(0, front) + ellipsis + display.slice(-back);
}

/** ANSI codes to control cursor and screen. */
export const cursor = {
  hide: `${ESC}?25l`,
  show: `${ESC}?25h`,
  up: (n: number = 1) => `${ESC}${n}A`,
  down: (n: number = 1) => `${ESC}${n}B`,
  clearLine: `${ESC}2K`,
  clearDown: `${ESC}J`,
  moveTo: (col: number) => `${ESC}${col}G`,
  saveCursor: `${ESC}s`,
  restoreCursor: `${ESC}u`,
};
