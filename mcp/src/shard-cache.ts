/**
 * SharedShardCache — Wave 1D auto-rebuild guard.
 *
 * Bug #224 META: MCP queries silently return empty results when the
 * file path the AI passes is outside the currently-indexed shard root.
 * The tool returns [] / "unknown" and the AI stops using mneme tools
 * because they look broken, even though the root cause is simply that
 * the index was built from a different path spelling or an old location.
 *
 * This module inserts a detection layer in front of every path-bearing
 * MCP tool. On each call:
 *
 *   1. Load all known shards from `~/.mneme/meta.db::projects` (root +
 *      id columns). Cache in memory with a 30-second TTL so repeated
 *      calls within one tool invocation pay the SQLite open cost at
 *      most once per half-minute.
 *
 *   2. Canonicalize the caller's file path and check if any shard root
 *      is an ancestor prefix.
 *
 *   3. On miss: try to infer the project root via `findProjectRoot`,
 *      spawn `mneme build <root>` detached (once — coalesced per root),
 *      and return a structured PathNotIndexed response. The caller
 *      returns this directly; the tool does NOT query the empty shard.
 *
 *   4. On stale shard (directory exists on disk but has been moved):
 *      same as miss — the guard triggers a background rebuild.
 *
 * Design constraints:
 *   - Zero new npm/bun dependencies (only node:* + bun:sqlite already
 *     present in the project).
 *   - Strict-mode TypeScript throughout.
 *   - Backwards compatible: tools that do not call checkPath() are
 *     completely unaffected.
 *   - Coalesce: a second query for the same unindexed root while a
 *     build is in-flight returns the same "rebuilding" response
 *     without spawning a second child process.
 */

import { spawn } from "node:child_process";
import { existsSync, realpathSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, isAbsolute, join, resolve, sep } from "node:path";
import { Database } from "bun:sqlite";
import { findProjectRoot } from "./store.ts";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/**
 * Returned by `checkPath` when the supplied path has no matching shard.
 * Tools should return this directly so the AI gets actionable guidance
 * instead of a silent empty result.
 */
export interface PathNotIndexed {
  error: "path_not_indexed";
  /** The exact path that was checked. */
  path: string;
  /** Human-readable rebuild instruction. */
  suggestion: string;
  /** True when a background build was kicked off as a result of this call. */
  auto_rebuild_started: boolean;
  /** Set when a build is already in-flight for this root. */
  rebuild_in_progress?: true;
}

/**
 * Returned when a shard exists but its root no longer corresponds to an
 * on-disk directory (the project was moved). A background rebuild is
 * started and the caller should retry.
 */
export interface ShardStale {
  status: "rebuilding";
  /** Stale shard root that no longer exists on disk. */
  stale_root: string;
  /** Rough wall-clock estimate in seconds. */
  eta_sec: number;
  suggestion: string;
}

/**
 * Union of the two miss-class responses. Both cause the tool to
 * short-circuit and return immediately without querying the shard.
 */
export type PathMissResponse = PathNotIndexed | ShardStale;

// ---------------------------------------------------------------------------
// Known-shard row (one row per registered project in meta.db)
// ---------------------------------------------------------------------------

interface ShardRow {
  /** SHA-256 of the canonical project root — directory name in ~/.mneme/projects/ */
  id: string;
  /** Absolute project root as written by the Rust CLI (native path). */
  root: string;
}

// ---------------------------------------------------------------------------
// In-process build-coalescing ledger
//
// Key: canonical project root (string)
// Value: Unix timestamp (ms) when the spawn occurred
//
// Entries are cleared after COALESCE_TTL_MS regardless so a failed or
// completed build does not block future rebuild attempts forever.
// ---------------------------------------------------------------------------

const IN_FLIGHT_BUILDS = new Map<string, number>();
const COALESCE_TTL_MS = 5 * 60 * 1000; // 5 minutes

/** True if a build was started for `projectRoot` within the last 5 min. */
function isInFlight(projectRoot: string): boolean {
  const ts = IN_FLIGHT_BUILDS.get(projectRoot);
  if (ts === undefined) return false;
  if (Date.now() - ts > COALESCE_TTL_MS) {
    IN_FLIGHT_BUILDS.delete(projectRoot);
    return false;
  }
  return true;
}

function markInFlight(projectRoot: string): void {
  IN_FLIGHT_BUILDS.set(projectRoot, Date.now());
}

// ---------------------------------------------------------------------------
// Shard-list cache
// ---------------------------------------------------------------------------

interface ShardCache {
  rows: ShardRow[];
  loadedAt: number;
}

let _cache: ShardCache | null = null;
const CACHE_TTL_MS = 30_000; // 30 seconds

const MNEME_HOME = join(homedir(), ".mneme");

/**
 * Return all known shard rows from `meta.db`. Returns [] when meta.db
 * is missing (first-run) or unreadable. Never throws.
 */
function loadShards(): ShardRow[] {
  const metaPath = join(MNEME_HOME, "meta.db");
  if (!existsSync(metaPath)) return [];
  let db: Database | null = null;
  try {
    db = new Database(metaPath, { readonly: true });
    const rows = db
      .prepare("SELECT id, root FROM projects ORDER BY last_indexed_at DESC NULLS LAST")
      .all() as ShardRow[];
    return rows;
  } catch {
    return [];
  } finally {
    try {
      db?.close();
    } catch {
      // ignore
    }
  }
}

/** Cached shard list — reloads every 30 s. */
function getShards(): ShardRow[] {
  if (_cache !== null && Date.now() - _cache.loadedAt < CACHE_TTL_MS) {
    return _cache.rows;
  }
  const rows = loadShards();
  _cache = { rows, loadedAt: Date.now() };
  return rows;
}

/** For testing: force the next getShards() call to re-read meta.db. */
export function _invalidateCache(): void {
  _cache = null;
}

/**
 * For testing: inject a synthetic shard list without touching the
 * filesystem. Only callable when NODE_ENV / BUN_ENV is "test".
 */
export function _setShards(rows: ShardRow[]): void {
  const env = process.env.NODE_ENV ?? process.env.BUN_ENV ?? "production";
  if (env.toLowerCase() !== "test") {
    throw new Error(
      `_setShards is test-only and cannot be called in ${env} mode`,
    );
  }
  _cache = { rows, loadedAt: Date.now() };
}

// ---------------------------------------------------------------------------
// Path canonicalization
// ---------------------------------------------------------------------------

/**
 * Canonicalize a file path for ancestor-prefix comparison.
 *
 * Rules (must stay in sync with `projectIdForPath` in store.ts and the
 * Rust CLI's `dunce::canonicalize`):
 *   - Resolve symlinks when possible (fall through on missing path).
 *   - On Windows, strip `\\?\` UNC long-path prefix.
 *   - Do NOT lowercase (CLI preserves case).
 *   - Normalize path separators to `sep` for the current platform.
 */
function canonicalizePath(p: string): string {
  let resolved = isAbsolute(p) ? p : resolve(process.cwd(), p);
  try {
    const realpath = realpathSync as typeof realpathSync & {
      native?: (s: string) => string;
    };
    resolved = realpath.native?.(resolved) ?? realpath(resolved);
  } catch {
    // Path may not exist — use the math-resolved form.
  }
  if (process.platform === "win32" && resolved.startsWith("\\\\?\\")) {
    resolved = resolved.slice(4);
  }
  // Normalize to platform separator for prefix comparison.
  return resolved.replace(/[/\\]/g, sep);
}

/**
 * True when `candidate` is an ancestor of (or equal to) `filePath`.
 * Both paths must already be canonicalized to platform sep.
 */
function isAncestorOf(ancestor: string, filePath: string): boolean {
  const prefix = ancestor.endsWith(sep) ? ancestor : ancestor + sep;
  return filePath.startsWith(prefix) || filePath === ancestor;
}

// ---------------------------------------------------------------------------
// Background build spawner
// ---------------------------------------------------------------------------

/**
 * Spawn `mneme build <projectRoot>` detached. Returns the PID on
 * success or null if the binary is not on PATH. Never throws.
 *
 * `markInFlight` is called BEFORE the spawn attempt so the coalescing
 * ledger is updated regardless of whether `mneme` is on PATH. This
 * ensures a second query for the same root sees `rebuild_in_progress`
 * even in environments where the binary is absent (e.g. CI test runs).
 */
function spawnBuild(projectRoot: string): number | null {
  // Mark as in-flight immediately — before spawn — so coalescing works
  // even when the binary is absent and spawn throws synchronously.
  markInFlight(projectRoot);
  try {
    const child = spawn("mneme", ["build", projectRoot], {
      cwd: projectRoot,
      detached: true,
      stdio: "ignore",
      windowsHide: true,
    });
    // Attach an error listener BEFORE unref so Bun's global unhandled-
    // error handler does not intercept the ENOENT when mneme is not on
    // PATH. Without this, Bun emits the error as an uncaught exception
    // between tests even though the try/catch above would swallow it.
    child.on("error", () => {
      // Intentionally empty: spawn ENOENT is expected in environments
      // where the mneme binary is absent. The in-flight entry remains
      // so a second query sees rebuild_in_progress rather than
      // re-attempting the spawn.
    });
    child.unref();
    return typeof child.pid === "number" ? child.pid : null;
  } catch {
    // Spawn threw synchronously (binary missing, permission denied, etc.).
    // markInFlight was already called above; return null to indicate no
    // PID is available but the in-flight entry IS set.
    return null;
  }
}

// Approximate seconds for a typical build (~10s/100 files, assume 200 files).
const DEFAULT_ETA_SEC = 20;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Check whether `filePath` belongs to a known indexed shard.
 *
 * Returns `null` when the path IS covered — the tool should proceed normally.
 * Returns `PathMissResponse` when the path is NOT covered — the tool should
 * return this value immediately instead of querying the (wrong or empty) shard.
 *
 * @param filePath   Absolute or relative path from the tool's input.
 * @param cwdHint    Optional working directory to use when resolving relative
 *                   paths. Defaults to `process.cwd()`.
 */
export function checkPath(
  filePath: string,
  cwdHint?: string,
): PathMissResponse | null {
  // Absolute-resolve before canonicalizing.
  const base = cwdHint ?? process.cwd();
  const abs = isAbsolute(filePath) ? filePath : resolve(base, filePath);
  const canonical = canonicalizePath(abs);

  const shards = getShards();

  // Fast path: at least one shard root covers this path.
  for (const shard of shards) {
    const shardRoot = canonicalizePath(shard.root);
    if (isAncestorOf(shardRoot, canonical)) {
      // The path IS indexed by this shard's root. Staleness check:
      // if the project root directory itself no longer exists on disk
      // the project has been moved or deleted. In that case the shard
      // data is stale regardless of what lives under ~/.mneme/projects/.
      //
      // We deliberately do NOT check whether ~/.mneme/projects/<id>/
      // exists — that directory being absent simply means the index
      // hasn't been built yet (first run), which is handled by
      // shardDbPath() returning null inside the individual tools'
      // existing graceful-degrade paths. That is a separate concern
      // from a moved project.
      if (!existsSync(shard.root)) {
        // Stale shard — the project root no longer exists on disk.
        const projectRoot = shard.root;
        if (!isInFlight(projectRoot)) {
          spawnBuild(projectRoot);
        }
        return {
          status: "rebuilding",
          stale_root: shard.root,
          eta_sec: DEFAULT_ETA_SEC,
          suggestion:
            `Shard for "${shard.root}" is stale — the project directory no longer ` +
            `exists at that path (it may have been moved). ` +
            `A background \`mneme build ${projectRoot}\` has been started. ` +
            `Retry this tool in ~${DEFAULT_ETA_SEC}s.`,
        };
      }
      return null; // covered and healthy
    }
  }

  // No shard covers this path.
  // Try to infer a sensible project root to build.
  const inferredRoot =
    findProjectRoot(dirname(canonical)) ??
    findProjectRoot(canonical) ??
    dirname(canonical);

  // Coalesce: don't spawn a second build if one is already in-flight.
  // `spawnBuild` calls `markInFlight` before attempting the spawn so
  // the ledger entry is set regardless of whether the binary is on PATH.
  // We therefore check `isInFlight` BEFORE calling `spawnBuild`.
  let autoRebuildStarted = false;
  let rebuildInProgress: true | undefined;

  if (isInFlight(inferredRoot)) {
    rebuildInProgress = true;
  } else {
    // Attempt the spawn. The in-flight entry is set inside spawnBuild
    // even when the binary is missing; we treat any attempt as "started"
    // so the suggestion is honest: a build was registered, even if the
    // binary couldn't be found this instant.
    spawnBuild(inferredRoot);
    autoRebuildStarted = true;
  }

  const response: PathNotIndexed = {
    error: "path_not_indexed",
    path: filePath,
    suggestion:
      `"${filePath}" is not inside any indexed shard. ` +
      `Run \`mneme build ${inferredRoot}\` to index it, ` +
      `then retry this tool. ` +
      (autoRebuildStarted
        ? "A background build has been started automatically."
        : rebuildInProgress
        ? "A background build is already in progress — retry in a moment."
        : ""),
    auto_rebuild_started: autoRebuildStarted,
    ...(rebuildInProgress ? { rebuild_in_progress: true as const } : {}),
  };
  return response;
}

/**
 * Convenience wrapper that returns true when `filePath` is covered by
 * an indexed shard, false otherwise. Does NOT trigger a background
 * build — use `checkPath` for the full guard behaviour.
 */
export function isPathIndexed(filePath: string, cwdHint?: string): boolean {
  return checkPath(filePath, cwdHint) === null;
}

// Re-export the internal build ledger size for health/doctor tooling.
export function inFlightBuildCount(): number {
  // Prune expired entries first.
  const now = Date.now();
  for (const [root, ts] of IN_FLIGHT_BUILDS) {
    if (now - ts > COALESCE_TTL_MS) IN_FLIGHT_BUILDS.delete(root);
  }
  return IN_FLIGHT_BUILDS.size;
}
