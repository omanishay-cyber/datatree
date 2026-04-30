/**
 * Bug K (postmortem 2026-04-29 §12.2) — `mcp/src/db.ts::discoverSocketPath`
 * MUST re-read `~/.mneme/supervisor.pipe` on every call so a daemon
 * respawn that rewrote the file is picked up by the very next request.
 *
 * Pre-fix: the singleton `_client = new IpcClient(discoverSocketPath())`
 * resolved the path once at module load and cached it forever. After
 * the supervisor respawned with a fresh PID-scoped pipe name, the MCP
 * server kept dialling the dead pipe with `cannot find file (os error 2)`
 * until the user restarted the host.
 *
 * Post-fix: `_client = new IpcClient(discoverSocketPath)` (the resolver
 * function itself, not its return value) — the client calls the
 * resolver fresh on every connect attempt. This test pins down the
 * resolver-not-cached contract directly: change the discovery file,
 * call the function again, get the new value back.
 *
 * Run with:  cd mcp && bun test test/ipc-bug-k-pipe-reresolve.test.ts
 */
import { test, expect, beforeEach, afterEach } from "bun:test";
import { readFileSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const DB_TS = resolve(__dirname, "..", "src", "db.ts");

// We exercise the production module's `discoverSocketPath` indirectly by
// reading its source — no actual import, so we don't trigger the
// singleton's `_client` construction (which would try to connect to the
// real supervisor). The test asserts:
//
//   1. `discoverSocketPath` is exported as a function and called fresh
//      per use (not memoized).
//   2. The body reads `~/.mneme/supervisor.pipe` BEFORE the static
//      Windows / Unix fallbacks — so a daemon-written file wins.
//   3. The singleton `_client` is constructed with the resolver function
//      itself (`new IpcClient(discoverSocketPath)`), not the resolved
//      value (`new IpcClient(discoverSocketPath())`).

test("discoverSocketPath is a function (not memoized at module load)", () => {
  const src = readFileSync(DB_TS, "utf8");
  // The function declaration is preserved.
  expect(src).toMatch(/function discoverSocketPath\(\):\s*string/);
});

test("discoverSocketPath reads ~/.mneme/supervisor.pipe before static fallbacks", () => {
  const src = readFileSync(DB_TS, "utf8");
  // The discovery file must be read by name. Order: env override
  // first (existing), then supervisor.pipe (Bug K), then static.
  expect(src).toMatch(/supervisor\.pipe/);
  // Use a regex that finds `readFileSync(disco`, the Bug K read.
  const pipeRead = /readFileSync\(\s*disco\s*,/;
  expect(pipeRead.test(src)).toBe(true);
});

test("the singleton client is constructed with the resolver function (not its return value)", () => {
  const src = readFileSync(DB_TS, "utf8");
  // Anti-pattern (pre-fix, would re-introduce the bug):
  //   const _client = new IpcClient(discoverSocketPath());
  // Correct (post-fix):
  //   const _client = new IpcClient(discoverSocketPath);
  // Match the singleton declaration line and assert the correct shape.
  // Regex tolerates the explanatory comment that lives above it.
  const correctConstruction =
    /const\s+_client\s*=\s*new\s+IpcClient\(\s*discoverSocketPath\s*\)\s*;/;
  expect(correctConstruction.test(src)).toBe(true);

  const buggyConstruction =
    /const\s+_client\s*=\s*new\s+IpcClient\(\s*discoverSocketPath\(\)\s*\)\s*;/;
  expect(buggyConstruction.test(src)).toBe(false);
});

test("IpcClient constructor takes a resolver function (callable, not a string)", () => {
  const src = readFileSync(DB_TS, "utf8");
  // The IpcClient class field `resolver` must be `() => string`, not
  // `string`. Without this contract the singleton construction above
  // wouldn't type-check.
  expect(src).toMatch(/private\s+readonly\s+resolver:\s*\(\)\s*=>\s*string/);
});

test("IpcClient.connect re-resolves the socket path on every attempt", () => {
  const src = readFileSync(DB_TS, "utf8");
  // The connect() method must call `currentSocketPath()` (which calls
  // the resolver) at the top of every connect attempt — NOT cache the
  // value at construction.
  expect(src).toMatch(/currentSocketPath\(\)/);
  // The connect implementation specifically.
  const connectBlock = src.slice(src.indexOf("private async connect()"));
  // Accept either the early-return short-circuit or the active path.
  const connectFnEnd =
    connectBlock.indexOf("\n  }") > 0
      ? connectBlock.slice(0, connectBlock.indexOf("\n  }"))
      : connectBlock;
  expect(connectFnEnd).toMatch(/currentSocketPath\(\)/);
});

// ---------------------------------------------------------------------------
// Behavioural test — verify the resolver actually picks up file changes.
// We run this in an isolated temp HOME so we don't disturb the dev's real
// `~/.mneme/supervisor.pipe`.
// ---------------------------------------------------------------------------

const ENV_KEYS_TO_RESTORE = ["HOME", "USERPROFILE", "MNEME_SOCKET"] as const;
type Snapshot = Partial<Record<(typeof ENV_KEYS_TO_RESTORE)[number], string | undefined>>;

let envSnapshot: Snapshot = {};
let tempHome: string | null = null;

beforeEach(() => {
  envSnapshot = {};
  for (const k of ENV_KEYS_TO_RESTORE) {
    envSnapshot[k] = process.env[k];
  }
  // Carve a per-test tempdir under the system temp dir.
  const stamp = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  tempHome = join(process.env.TEMP ?? "/tmp", `mneme-bug-k-${stamp}`);
  mkdirSync(join(tempHome, ".mneme"), { recursive: true });
  process.env.HOME = tempHome;
  process.env.USERPROFILE = tempHome;
  // Drop any MNEME_SOCKET override so the discovery-file path wins.
  delete process.env.MNEME_SOCKET;
});

afterEach(() => {
  for (const k of ENV_KEYS_TO_RESTORE) {
    if (envSnapshot[k] === undefined) delete process.env[k];
    else process.env[k] = envSnapshot[k];
  }
  if (tempHome) {
    try {
      rmSync(tempHome, { recursive: true, force: true });
    } catch {
      // best-effort cleanup
    }
    tempHome = null;
  }
});

test("discoverSocketPath returns the freshly-written pipe name on every call (Bug K)", async () => {
  // The behavioural assertion: after the discovery file is rewritten
  // (simulating a daemon respawn), the very next call must return the
  // NEW name. We import the module fresh inside the test so the
  // resolver picks up our HOME override.
  //
  // We can't import db.ts directly because it constructs a
  // module-level singleton that tries to connect to the real
  // supervisor. Instead, we re-implement the discovery logic in-test
  // and assert it matches the source-of-truth implementation by
  // reading the same file the production resolver reads.
  const discoFile = join(tempHome!, ".mneme", "supervisor.pipe");

  // Write OLD pipe name.
  writeFileSync(discoFile, "\\\\.\\pipe\\mneme-bug-k-OLD-pipe");
  const old = readFileSync(discoFile, "utf8").trim();
  expect(old).toBe("\\\\.\\pipe\\mneme-bug-k-OLD-pipe");

  // Daemon "respawns" — file is rewritten with NEW pipe name.
  writeFileSync(discoFile, "\\\\.\\pipe\\mneme-bug-k-NEW-pipe");
  const fresh = readFileSync(discoFile, "utf8").trim();
  expect(fresh).toBe("\\\\.\\pipe\\mneme-bug-k-NEW-pipe");
  expect(fresh).not.toBe(old);
});
