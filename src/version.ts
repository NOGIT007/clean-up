export const VERSION = "1.0.1";

// Injected at compile time via --define in build.sh; falls back to startup time in dev
declare const __BUILD_TIME__: string | undefined;
export const BUILD_TIME: string =
  typeof __BUILD_TIME__ === "string"
    ? __BUILD_TIME__
    : new Date().toISOString();
