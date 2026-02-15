/**
 * Custom TUI prompts — zero dependencies.
 * Built from scratch using process.stdin raw mode + ANSI escape codes.
 */

import { colors, cursor, formatBytes } from "./format";

/** Write to stdout without newline. */
function write(s: string): void {
  process.stdout.write(s);
}

/** Write to stdout with newline. */
function writeln(s: string = ""): void {
  process.stdout.write(s + "\n");
}

// ---------------------------------------------------------------------------
// Intro / Outro
// ---------------------------------------------------------------------------

/** Display a styled intro banner. */
export function intro(title: string): void {
  writeln();
  writeln(`  ${colors.bgTeal(colors.bold(colors.white(` ✦ ${title} `)))}`);
  writeln();
}

/** Display a styled outro message. */
export function outro(message: string): void {
  writeln();
  writeln(`  ${colors.teal("✦")} ${colors.bold(colors.green(message))}`);
  writeln();
}

/** Display an informational note. */
export function note(title: string, body: string): void {
  writeln(`  ${colors.teal("│")} ${colors.bold(colors.white(title))}`);
  for (const line of body.split("\n")) {
    writeln(`  ${colors.teal("│")} ${colors.softWhite(line)}`);
  }
  writeln();
}

/** Display a warning. */
export function warn(message: string): void {
  writeln(`  ${colors.yellow("⚠")} ${colors.yellow(message)}`);
}

// ---------------------------------------------------------------------------
// Key reading helpers
// ---------------------------------------------------------------------------

interface KeyPress {
  name: string;
  ctrl: boolean;
  raw: string;
}

function parseKey(data: Buffer): KeyPress {
  const raw = data.toString();
  const ctrl = data.length === 1 && data[0]! < 32;

  // Arrow keys
  if (raw === "\x1b[A") return { name: "up", ctrl: false, raw };
  if (raw === "\x1b[B") return { name: "down", ctrl: false, raw };
  if (raw === "\x1b[C") return { name: "right", ctrl: false, raw };
  if (raw === "\x1b[D") return { name: "left", ctrl: false, raw };

  // Special keys
  if (raw === "\r" || raw === "\n") return { name: "return", ctrl: false, raw };
  if (raw === " ") return { name: "space", ctrl: false, raw };
  if (raw === "\x1b" || raw === "\x1b\x1b")
    return { name: "escape", ctrl: false, raw };
  if (raw === "\x7f") return { name: "backspace", ctrl: false, raw };

  // Ctrl+C
  if (data.length === 1 && data[0] === 3) return { name: "c", ctrl: true, raw };

  // Regular character
  if (ctrl && data.length === 1) {
    const char = String.fromCharCode(data[0]! + 96);
    return { name: char, ctrl: true, raw };
  }

  return { name: raw, ctrl: false, raw };
}

/** Read a single keypress from stdin in raw mode. */
function readKey(): Promise<KeyPress> {
  return new Promise((resolve) => {
    const wasRaw = process.stdin.isRaw;
    process.stdin.setRawMode(true);
    process.stdin.resume();
    process.stdin.once("data", (data: Buffer) => {
      process.stdin.pause();
      process.stdin.setRawMode(wasRaw ?? false);
      resolve(parseKey(data));
    });
  });
}

// ---------------------------------------------------------------------------
// Confirm prompt
// ---------------------------------------------------------------------------

/** Ask a yes/no question. Returns true if user confirms. */
export async function confirm(message: string): Promise<boolean> {
  write(
    `  ${colors.teal("?")} ${colors.bold(colors.white(message))} ${colors.softWhite("(y/N)")} `,
  );

  const key = await readKey();
  const yes = key.name === "y" || key.name === "Y";

  if (key.ctrl && key.name === "c") {
    writeln();
    process.exit(0);
  }

  writeln(
    yes ? colors.bold(colors.green("Yes")) : colors.bold(colors.red("No")),
  );
  return yes;
}

// ---------------------------------------------------------------------------
// Select prompt (single choice)
// ---------------------------------------------------------------------------

export interface SelectOption<T> {
  label: string;
  value: T;
  hint?: string;
}

/** Single-choice select with arrow keys. Returns the chosen value. */
export async function select<T>(
  message: string,
  options: SelectOption<T>[],
): Promise<T> {
  writeln(`  ${colors.teal("?")} ${colors.bold(colors.white(message))}`);

  let cursor_pos = 0;

  function render() {
    for (let i = 0; i < options.length; i++) {
      const opt = options[i]!;
      const active = i === cursor_pos;
      const prefix = active ? colors.teal("❯ ") : "  ";
      const label = active
        ? colors.bold(colors.white(opt.label))
        : colors.softWhite(opt.label);
      const hint = opt.hint ? colors.gray(` (${opt.hint})`) : "";
      write(`${cursor.clearLine}\r  ${prefix}${label}${hint}\n`);
    }
  }

  render();

  while (true) {
    const key = await readKey();

    if (key.ctrl && key.name === "c") {
      write(cursor.show);
      writeln();
      process.exit(0);
    }

    if (key.name === "up" && cursor_pos > 0) {
      cursor_pos--;
    } else if (key.name === "down" && cursor_pos < options.length - 1) {
      cursor_pos++;
    } else if (key.name === "return") {
      // Clear the options and show selected
      write(cursor.up(options.length));
      for (let i = 0; i < options.length; i++) {
        write(`${cursor.clearLine}\n`);
      }
      write(cursor.up(options.length));
      writeln(
        `  ${colors.teal("❯")} ${colors.bold(colors.white(options[cursor_pos]!.label))}`,
      );
      return options[cursor_pos]!.value;
    }

    // Re-render
    write(cursor.up(options.length));
    render();
  }
}

// ---------------------------------------------------------------------------
// Multiselect prompt
// ---------------------------------------------------------------------------

export interface MultiselectOption<T> {
  label: string;
  value: T;
  hint?: string;
  selected?: boolean;
}

/**
 * Multi-choice select with space to toggle, enter to confirm.
 * Returns array of selected values.
 */
export async function multiselect<T>(
  message: string,
  options: MultiselectOption<T>[],
): Promise<T[]> {
  writeln(
    `  ${colors.teal("?")} ${colors.bold(colors.white(message))} ${colors.gray("(space to toggle, enter to confirm)")}`,
  );

  let cursor_pos = 0;
  const selected = new Set<number>(
    options.map((o, i) => (o.selected ? i : -1)).filter((i) => i >= 0),
  );

  function render() {
    for (let i = 0; i < options.length; i++) {
      const opt = options[i]!;
      const active = i === cursor_pos;
      const checked = selected.has(i);
      const checkbox = checked ? colors.green("◉") : colors.gray("○");
      const prefix = active ? colors.teal("❯ ") : "  ";
      const label = active
        ? colors.bold(colors.white(opt.label))
        : checked
          ? colors.softWhite(opt.label)
          : colors.gray(opt.label);
      const hint = opt.hint ? colors.gray(` ${opt.hint}`) : "";
      write(`${cursor.clearLine}\r  ${prefix}${checkbox} ${label}${hint}\n`);
    }
  }

  render();

  while (true) {
    const key = await readKey();

    if (key.ctrl && key.name === "c") {
      write(cursor.show);
      writeln();
      process.exit(0);
    }

    if (key.name === "up" && cursor_pos > 0) {
      cursor_pos--;
    } else if (key.name === "down" && cursor_pos < options.length - 1) {
      cursor_pos++;
    } else if (key.name === "space") {
      if (selected.has(cursor_pos)) {
        selected.delete(cursor_pos);
      } else {
        selected.add(cursor_pos);
      }
    } else if (key.name === "a") {
      // Toggle all
      if (selected.size === options.length) {
        selected.clear();
      } else {
        for (let i = 0; i < options.length; i++) selected.add(i);
      }
    } else if (key.name === "return") {
      // Clear and show selections
      write(cursor.up(options.length));
      for (let i = 0; i < options.length; i++) {
        write(`${cursor.clearLine}\n`);
      }
      write(cursor.up(options.length));
      const labels = [...selected].sort().map((i) => options[i]!.label);
      writeln(
        `  ${colors.teal("❯")} ${colors.bold(colors.white(labels.join(", ") || "none"))}`,
      );
      return [...selected].sort().map((i) => options[i]!.value);
    }

    // Re-render
    write(cursor.up(options.length));
    render();
  }
}

// ---------------------------------------------------------------------------
// Spinner
// ---------------------------------------------------------------------------

const SPINNER_FRAMES = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

export interface SpinnerHandle {
  update(message: string): void;
  stop(finalMessage?: string): void;
}

/** Show a spinner with a message. Call .stop() when done. */
export function spinner(message: string): SpinnerHandle {
  let frame = 0;
  let currentMessage = message;
  let stopped = false;

  write(cursor.hide);
  write(
    `\r  ${colors.teal(SPINNER_FRAMES[0]!)} ${colors.softWhite(currentMessage)}`,
  );

  const interval = setInterval(() => {
    if (stopped) return;
    frame = (frame + 1) % SPINNER_FRAMES.length;
    write(
      `\r${cursor.clearLine}  ${colors.teal(SPINNER_FRAMES[frame]!)} ${colors.softWhite(currentMessage)}`,
    );
  }, 80);

  return {
    update(msg: string) {
      currentMessage = msg;
    },
    stop(finalMessage?: string) {
      stopped = true;
      clearInterval(interval);
      const display = finalMessage ?? currentMessage;
      write(
        `\r${cursor.clearLine}  ${colors.green("✔")} ${colors.white(display)}\n`,
      );
      write(cursor.show);
    },
  };
}

// ---------------------------------------------------------------------------
// Summary table
// ---------------------------------------------------------------------------

export interface SummaryItem {
  label: string;
  size: number;
}

/** Display a summary table of items with sizes. */
export function summary(title: string, items: SummaryItem[]): void {
  if (items.length === 0) return;

  writeln();
  writeln(`  ${colors.teal("┌─")} ${colors.bold(colors.white(title))}`);

  const totalSize = items.reduce((sum, item) => sum + item.size, 0);

  for (const item of items) {
    const size = formatBytes(item.size);
    writeln(
      `  ${colors.teal("│")}   ${colors.softWhite(item.label)}  ${colors.yellow(size)}`,
    );
  }

  writeln(`  ${colors.teal("│")}`);
  writeln(
    `  ${colors.teal("└─")} ${colors.bold(colors.white("Total:"))} ${colors.bold(colors.green(formatBytes(totalSize)))}`,
  );
  writeln();
}
