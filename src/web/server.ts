/**
 * Local web server for the Clean Up browser UI.
 * Serves the single-page HTML frontend and exposes a JSON API
 * for scanners and trash operations.
 *
 * Binds to 127.0.0.1 only — no network exposure.
 */

import type { CliOptions } from "../types";
import { VERSION, BUILD_TIME } from "../version";
import { getAllScanners } from "../scanners/registry";
import { moveMultipleToTrash } from "../utils/trash";
import { getInstalledAppsList, getAppAssociatedData } from "../utils/apps";

// In-memory cache for app icon PNGs (path -> Buffer|null)
const iconCache = new Map<string, Buffer | null>();

// Track when reindex was last triggered so we don't report "up to date"
// before Spotlight actually starts indexing (race condition with mdutil -s)
let lastReindexTime = 0;

async function getAppIconPng(appPath: string): Promise<Buffer | null> {
  if (iconCache.has(appPath)) return iconCache.get(appPath)!;

  try {
    const plistPath = appPath + "/Contents/Info.plist";
    const plistFile = Bun.file(plistPath);
    if (!(await plistFile.exists())) {
      iconCache.set(appPath, null);
      return null;
    }

    // Try to extract CFBundleIconFile using plutil (handles both XML and binary plists)
    let iconFileName = "";
    try {
      const proc = Bun.spawnSync({
        cmd: ["plutil", "-extract", "CFBundleIconFile", "raw", plistPath],
        stdout: "pipe",
        stderr: "pipe",
      });
      iconFileName = new TextDecoder().decode(proc.stdout).trim();
    } catch {}

    if (!iconFileName) {
      iconCache.set(appPath, null);
      return null;
    }

    // Ensure .icns extension
    if (!iconFileName.endsWith(".icns")) iconFileName += ".icns";

    const icnsPath = appPath + "/Contents/Resources/" + iconFileName;
    const icnsFile = Bun.file(icnsPath);
    if (!(await icnsFile.exists())) {
      iconCache.set(appPath, null);
      return null;
    }

    // Convert icns to 64x64 PNG using sips (built-in macOS tool)
    const tmpPng = `/tmp/clean-up-icon-${Date.now()}-${Math.random().toString(36).slice(2)}.png`;
    const sips = Bun.spawnSync({
      cmd: [
        "sips",
        "-s",
        "format",
        "png",
        "-z",
        "64",
        "64",
        icnsPath,
        "--out",
        tmpPng,
      ],
      stdout: "pipe",
      stderr: "pipe",
    });

    if (sips.exitCode !== 0) {
      iconCache.set(appPath, null);
      return null;
    }

    const pngFile = Bun.file(tmpPng);
    if (!(await pngFile.exists())) {
      iconCache.set(appPath, null);
      return null;
    }

    const buf = Buffer.from(await pngFile.arrayBuffer());
    // Clean up temp file
    try {
      require("fs").unlinkSync(tmpPng);
    } catch {}

    iconCache.set(appPath, buf);
    return buf;
  } catch {
    iconCache.set(appPath, null);
    return null;
  }
}

/** Launch the web UI server and open the browser. */
export async function startWebServer(options: CliOptions): Promise<void> {
  const scanners = getAllScanners();

  // Resolve ui.html — check multiple locations:
  // 1. Same dir as this module (dev: src/web/)
  // 2. Next to the compiled binary (MacOS/)
  // 3. ../Resources/ relative to binary (compiled .app bundle)
  const binDir = import.meta.dir;
  const exeDir = require("path").dirname(process.execPath);
  const candidates = [
    binDir + "/ui.html",
    exeDir + "/ui.html",
    exeDir + "/../Resources/ui.html",
  ];
  let htmlPath: string | null = null;
  for (const p of candidates) {
    const f = Bun.file(p);
    if (await f.exists()) {
      htmlPath = p;
      break;
    }
  }
  if (!htmlPath) {
    console.error("Error: Could not find ui.html");
    process.exit(1);
  }
  // Read HTML once into memory to avoid BunFile stream exhaustion
  const htmlContent = await Bun.file(htmlPath).text();

  const server = Bun.serve({
    hostname: "127.0.0.1",
    port: 0, // OS picks a free port

    async fetch(req) {
      const url = new URL(req.url);

      // --- Static: serve the HTML frontend ---
      if (url.pathname === "/" || url.pathname === "/index.html") {
        return new Response(htmlContent, {
          headers: { "Content-Type": "text/html; charset=utf-8" },
        });
      }

      // --- API: list scanners ---
      if (url.pathname === "/api/scanners" && req.method === "GET") {
        return Response.json(
          scanners.map((s) => ({
            id: s.id,
            name: s.name,
            description: s.description,
          })),
        );
      }

      // --- API: version info ---
      if (url.pathname === "/api/version" && req.method === "GET") {
        return Response.json({ version: VERSION, built: BUILD_TIME });
      }

      // --- API: check dry-run ---
      if (url.pathname === "/api/dry-run" && req.method === "GET") {
        return Response.json({ dryRun: options.dryRun });
      }

      // --- API: run selected scanners ---
      if (url.pathname === "/api/scan" && req.method === "POST") {
        const body = (await req.json()) as { scannerIds?: string[] };
        const ids = new Set(body.scannerIds ?? scanners.map((s) => s.id));
        const selected = scanners.filter((s) => ids.has(s.id));

        const results = await Promise.all(
          selected.map(async (s) => {
            try {
              return await s.scan();
            } catch (err) {
              return {
                scannerName: s.name,
                findings: [],
                totalSize: 0,
                duration: 0,
                error: String(err),
              };
            }
          }),
        );

        return Response.json(results);
      }

      // --- API: trash selected items ---
      if (url.pathname === "/api/trash" && req.method === "POST") {
        if (options.dryRun) {
          return Response.json(
            { error: "Dry-run mode — trashing disabled" },
            { status: 400 },
          );
        }

        const body = (await req.json()) as { paths?: string[] };
        const paths = body.paths ?? [];
        const results = await moveMultipleToTrash(paths);

        return Response.json({ results });
      }

      // --- API: app icon (PNG) ---
      if (url.pathname === "/api/app-icon" && req.method === "GET") {
        const appPath = url.searchParams.get("path");
        if (!appPath) {
          return new Response("Missing path param", { status: 400 });
        }
        const png = await getAppIconPng(appPath);
        if (!png) {
          return new Response("No icon", { status: 404 });
        }
        return new Response(png, {
          headers: {
            "Content-Type": "image/png",
            "Cache-Control": "public, max-age=3600",
          },
        });
      }

      // --- API: list installed apps ---
      if (url.pathname === "/api/apps" && req.method === "GET") {
        try {
          const apps = await getInstalledAppsList();
          return Response.json(apps);
        } catch (err) {
          return Response.json({ error: String(err) }, { status: 500 });
        }
      }

      // --- API: get associated data for an app ---
      if (url.pathname === "/api/app-data" && req.method === "POST") {
        const body = (await req.json()) as {
          bundleId?: string;
          appName?: string;
        };
        if (!body.bundleId || !body.appName) {
          return Response.json(
            { error: "bundleId and appName required" },
            { status: 400 },
          );
        }
        try {
          const data = await getAppAssociatedData(body.bundleId, body.appName);
          return Response.json(data);
        } catch (err) {
          return Response.json({ error: String(err) }, { status: 500 });
        }
      }

      // --- API: reindex Spotlight (requires admin password via native dialog) ---
      if (url.pathname === "/api/reindex-spotlight" && req.method === "POST") {
        try {
          const script =
            'do shell script "mdutil -E /" with administrator privileges';
          const proc = Bun.spawn(["osascript", "-e", script], {
            stdout: "pipe",
            stderr: "pipe",
          });
          const exitCode = await proc.exited;
          if (exitCode === 0) {
            lastReindexTime = Date.now();
            return Response.json({ ok: true });
          }
          const stderr = new TextDecoder().decode(
            await new Response(proc.stderr).arrayBuffer(),
          );
          // User cancelled the password dialog
          if (stderr.includes("-128")) {
            return Response.json({ error: "cancelled" }, { status: 499 });
          }
          return Response.json(
            { error: stderr || "Reindex failed" },
            { status: 500 },
          );
        } catch (err) {
          return Response.json({ error: String(err) }, { status: 500 });
        }
      }

      // --- API: Spotlight indexing status ---
      if (url.pathname === "/api/spotlight-status" && req.method === "GET") {
        try {
          const proc = Bun.spawn(["mdutil", "-s", "/"], {
            stdout: "pipe",
            stderr: "pipe",
          });
          const output = new TextDecoder().decode(
            await new Response(proc.stdout).arrayBuffer(),
          );
          await proc.exited;
          // Typical output:
          //   /: Indexing enabled.
          //   /: Indexing and calculation in progress.  (or just "Indexing enabled.")
          let indexing = /indexing/i.test(output) && /progress/i.test(output);
          const enabled = /enabled/i.test(output);

          // Grace period: if we recently triggered a reindex but mdutil -s
          // doesn't show "in progress" yet, report indexing anyway. Spotlight
          // can take several seconds to start after mdutil -E returns.
          const GRACE_MS = 5 * 60 * 1000; // 5 minutes
          if (
            !indexing &&
            lastReindexTime > 0 &&
            Date.now() - lastReindexTime < GRACE_MS
          ) {
            indexing = true;
          }
          // Clear the flag once Spotlight confirms it's no longer indexing
          // and the grace period is over
          if (!indexing && lastReindexTime > 0) {
            lastReindexTime = 0;
          }

          return Response.json({ indexing, enabled, raw: output.trim() });
        } catch (err) {
          return Response.json({ error: String(err) }, { status: 500 });
        }
      }

      // --- API: quit the server ---
      if (url.pathname === "/api/quit" && req.method === "POST") {
        setTimeout(() => {
          server.stop();
          process.exit(0);
        }, 100);
        return Response.json({ ok: true });
      }

      return new Response("Not found", { status: 404 });
    },
  });

  const url = `http://127.0.0.1:${server.port}`;
  console.log(`\nclean-up web UI running at ${url}\n`);

  // Auto-open in default browser (macOS)
  Bun.spawn(["open", url], { stdout: "ignore", stderr: "ignore" });

  // Keep the process alive — Bun.serve() already does this,
  // but handle SIGINT gracefully.
  process.on("SIGINT", () => {
    console.log("\nShutting down web server...");
    server.stop();
    process.exit(0);
  });
}
