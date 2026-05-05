/**
 * Wave 1D — auto-rebuild guard tests.
 *
 * Bug #224 META: MCP queries silently returned empty when the file path
 * the AI passed was outside the currently-indexed shard root. The fix
 * lives in `mcp/src/shard-cache.ts` which is tested here.
 *
 * Test surface:
 *   1. `checkPath` returns null (no action) when the path IS covered by
 *      a shard whose root directory exists on disk.
 *   2. `checkPath` returns PathNotIndexed when no shard covers the path.
 *   3. Spawn is coalesced: a second call for the same root returns
 *      `rebuild_in_progress: true` without spawning again.
 *   4. `checkPath` returns ShardStale when the shard root no longer
 *      exists on disk (project was moved/deleted).
 *   5. Symbol-name inputs to blast_radius do NOT trigger the guard.
 *   6. `isPathIndexed` convenience wrapper mirrors `checkPath`.
 *   7. `inFlightBuildCount` reflects the current in-flight state.
 *   8. Relative paths are resolved before checking.
 *
 * Constraints:
 *   - No real filesystem writes — shards are injected via `_setShards`.
 *   - Tests for "covered and healthy" use real on-disk directories
 *     (process.cwd()) as the shard root so the staleness check passes.
 *   - Tests for "stale" use a path that cannot exist on disk.
 *   - `mneme` binary may not be on PATH in CI — spawn failures are
 *     already swallowed inside shard-cache.ts; tests assert the
 *     return shape, not the PID.
 *
 * Run with: cd mcp && bun test test/auto-rebuild.test.ts
 */

import { describe, it, expect, beforeEach } from "bun:test";
import { sep, join, dirname } from "node:path";
import { homedir } from "node:os";

// Set test env BEFORE importing shard-cache so _setShards is available.
process.env.BUN_ENV = "test";
process.env.NODE_ENV = "test";

import {
  checkPath,
  isPathIndexed,
  inFlightBuildCount,
  _setShards,
  _invalidateCache,
} from "../src/shard-cache.ts";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function syntheticRow(root: string): { id: string; root: string } {
  // id is a 64-char hex placeholder. _setShards bypasses the real hash.
  return { id: "a".repeat(64), root };
}

/**
 * A directory guaranteed to exist on disk — used as the "healthy" shard
 * root for tests that expect checkPath to return null.
 */
const REAL_ROOT = process.cwd(); // e.g. D:\Mneme Dome\...\mcp

/**
 * A file path guaranteed to live inside REAL_ROOT so the ancestor
 * prefix check fires correctly.
 */
function realFile(rel: string): string {
  return join(REAL_ROOT, rel);
}

/**
 * A path that definitely does NOT exist on disk — used for staleness
 * and miss tests. The path is structurally valid but points nowhere.
 */
function ghostRoot(name: string): string {
  if (process.platform === "win32") {
    return `C:\\MnemeGhostProjects\\${name}`;
  }
  return `/tmp/MnemeGhostProjects/${name}`;
}

function ghostFile(root: string, rel: string): string {
  if (process.platform === "win32") {
    return `${root}\\${rel.replace(/\//g, "\\")}`;
  }
  return `${root}/${rel}`;
}

// ---------------------------------------------------------------------------
// Reset before each test.
// Each test uses a unique ghost root to avoid in-flight ledger collisions.
// ---------------------------------------------------------------------------

beforeEach(() => {
  _invalidateCache();
});

// ---------------------------------------------------------------------------
// 1. Covered + healthy path
// ---------------------------------------------------------------------------

describe("SharedShardCache — checkPath", () => {
  it("returns null when the file IS inside a shard whose root exists on disk", () => {
    // Use the real cwd as shard root — it exists on disk so no stale trigger.
    _setShards([syntheticRow(REAL_ROOT)]);

    const file = realFile("src/shard-cache.ts");
    const result = checkPath(file);

    expect(result).toBeNull();
  });

  it("returns null when filePath equals the shard root (root is itself a covered path)", () => {
    _setShards([syntheticRow(REAL_ROOT)]);

    // The root directory itself should be "inside" the shard (equal = ancestor).
    const result = checkPath(REAL_ROOT);
    expect(result).toBeNull();
  });

  it("returns null when a deeper nested file is inside a shard root that exists", () => {
    _setShards([syntheticRow(REAL_ROOT)]);

    // Multiple subdirectory levels deep — must still be covered.
    const deep = realFile(`test${sep}sub${sep}deeper${sep}file.ts`);
    const result = checkPath(deep);
    expect(result).toBeNull();
  });

  // ---------------------------------------------------------------------------
  // 2. Miss — no shard covers the path
  // ---------------------------------------------------------------------------

  it("returns PathNotIndexed when no shard covers the path", () => {
    const otherRoot = ghostRoot("other-project-aa");
    _setShards([syntheticRow(otherRoot)]);

    const unmatchedFile = ghostFile(ghostRoot("unindexed-project-bb"), "main.rs");
    const result = checkPath(unmatchedFile);

    expect(result).not.toBeNull();
    if (result === null) return;

    expect("error" in result).toBe(true);
    if (!("error" in result)) return;
    expect(result.error).toBe("path_not_indexed");
    expect(result.path).toBe(unmatchedFile);
    expect(typeof result.suggestion).toBe("string");
    expect(result.suggestion.length).toBeGreaterThan(0);
    expect(typeof result.auto_rebuild_started).toBe("boolean");
  });

  it("returns PathNotIndexed when shard list is empty (first run / no projects built)", () => {
    _setShards([]);

    const file = ghostFile(ghostRoot("brand-new-cc"), "src/main.ts");
    const result = checkPath(file);

    expect(result).not.toBeNull();
    if (result === null || !("error" in result)) return;
    expect(result.error).toBe("path_not_indexed");
  });

  // ---------------------------------------------------------------------------
  // 3. Coalesce — second call for same root sees rebuild_in_progress
  // ---------------------------------------------------------------------------

  it("coalesces: second call for same unindexed root returns rebuild_in_progress=true", () => {
    _setShards([]);

    // Unique root so this test's ledger entry is isolated.
    const root = ghostRoot("coalesce-unique-1234dd");
    const file = ghostFile(root, "lib/index.ts");

    // First call — records an in-flight entry (spawn may succeed or fail,
    // the ledger is updated either way).
    const first = checkPath(file);
    expect(first).not.toBeNull();
    if (first === null || !("error" in first)) return;
    expect(first.error).toBe("path_not_indexed");

    // Second call — must detect the in-flight entry and not spawn again.
    const second = checkPath(file);
    expect(second).not.toBeNull();
    if (second === null || !("error" in second)) return;
    expect(second.error).toBe("path_not_indexed");
    expect(second.rebuild_in_progress).toBe(true);
    expect(second.auto_rebuild_started).toBe(false);
  });

  it("suggestion contains 'mneme build' so the AI knows what command to run", () => {
    _setShards([]);
    const root = ghostRoot("suggestion-check-ee");
    const file = ghostFile(root, "src/thing.ts");

    const result = checkPath(file);
    if (result === null || !("error" in result)) {
      expect(false).toBe(true); // force fail for clearer output
      return;
    }
    expect(result.suggestion).toContain("mneme build");
  });

  // ---------------------------------------------------------------------------
  // 4. Stale shard — root path in meta.db no longer exists on disk
  // ---------------------------------------------------------------------------

  it("returns ShardStale when shard root no longer exists on disk (moved project)", () => {
    // A ghost root is an ancestor of the file but does not exist on disk.
    const movedRoot = ghostRoot("moved-project-ff");
    _setShards([syntheticRow(movedRoot)]);

    const file = ghostFile(movedRoot, "src/service.ts");
    const result = checkPath(file);

    // The path IS covered by the shard row, but root doesn't exist on
    // disk — expect ShardStale, not null and not PathNotIndexed.
    expect(result).not.toBeNull();
    if (result === null) return;

    expect("status" in result).toBe(true);
    if (!("status" in result)) return;

    expect(result.status).toBe("rebuilding");
    expect(typeof result.eta_sec).toBe("number");
    expect(result.eta_sec).toBeGreaterThan(0);
    expect(result.stale_root).toBe(movedRoot);
    expect(typeof result.suggestion).toBe("string");
    expect(result.suggestion.length).toBeGreaterThan(0);
  });

  // ---------------------------------------------------------------------------
  // 5. Relative path resolution
  // ---------------------------------------------------------------------------

  it("resolves relative paths against cwdHint before checking", () => {
    _setShards([syntheticRow(REAL_ROOT)]);

    // "package.json" relative to REAL_ROOT — must be treated as covered.
    const result = checkPath("package.json", REAL_ROOT);
    expect(result).toBeNull();
  });

  // ---------------------------------------------------------------------------
  // 6. Windows UNC prefix does not cause a throw
  // ---------------------------------------------------------------------------

  it("handles Windows UNC-prefixed paths without throwing", () => {
    if (process.platform !== "win32") return;

    // Use the real cwd as root (exists on disk).
    _setShards([syntheticRow(REAL_ROOT)]);

    // Build a UNC form of a file inside REAL_ROOT.
    const uncFile = `\\\\?\\${realFile("src\\shard-cache.ts")}`;

    expect(() => checkPath(uncFile)).not.toThrow();
  });
});

// ---------------------------------------------------------------------------
// isPathIndexed wrapper
// ---------------------------------------------------------------------------

describe("isPathIndexed — convenience wrapper", () => {
  it("returns true when path is covered and root exists on disk", () => {
    _setShards([syntheticRow(REAL_ROOT)]);
    expect(isPathIndexed(realFile("src/shard-cache.ts"))).toBe(true);
  });

  it("returns false when no shard covers the path", () => {
    _setShards([]);
    const file = ghostFile(ghostRoot("not-indexed-gg"), "main.ts");
    expect(isPathIndexed(file)).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// inFlightBuildCount
// ---------------------------------------------------------------------------

describe("inFlightBuildCount", () => {
  it("returns a non-negative integer", () => {
    const count = inFlightBuildCount();
    expect(typeof count).toBe("number");
    expect(Number.isInteger(count)).toBe(true);
    expect(count).toBeGreaterThanOrEqual(0);
  });

  it("is at least 1 after a miss that records an in-flight entry", () => {
    _setShards([]);

    // Use a path whose inferredRoot is uniquely deterministic: a real
    // absolute path that contains a project marker so findProjectRoot
    // stops walking at a controlled ancestor instead of collapsing
    // multiple ghost roots to the same shared parent directory.
    // REAL_ROOT (process.cwd()) contains package.json — findProjectRoot
    // will return REAL_ROOT itself, giving a unique, consistent key.
    const file = realFile("src/some-file-that-does-not-exist.ts");

    // The shard list is empty so REAL_ROOT is not indexed: checkPath
    // will detect a miss and call spawnBuild(REAL_ROOT).
    checkPath(file);

    // After the miss the in-flight ledger must contain at least one entry.
    expect(inFlightBuildCount()).toBeGreaterThanOrEqual(1);
  });
});

// ---------------------------------------------------------------------------
// Backward compatibility: symbol-name inputs bypass the shard guard
// ---------------------------------------------------------------------------

describe("backward compatibility — tools that pass non-path inputs are unaffected", () => {
  it("blast_radius: qualified function name does NOT trigger path_not_indexed", async () => {
    const mod = await import("../src/tools/blast_radius.ts");
    const tool = mod.tool;
    expect(typeof tool.handler).toBe("function");

    // Inject empty shards — any file-path input would miss.
    _setShards([]);

    // "findReferences" is a qualified function name, NOT a file path.
    // The handler must NOT return an error-shaped response for it.
    // (It may return empty arrays when no shard is built — that is the
    // existing graceful-degrade behaviour and is separate from the
    // path-miss guard.)
    const result = await tool.handler(
      { target: "findReferences", depth: 1, deep: false, include_tests: true },
      { sessionId: "compat-test", cwd: process.cwd() },
    );

    expect("error" in (result as object)).toBe(false);
    expect("target" in (result as object)).toBe(true);
    expect((result as { target: string }).target).toBe("findReferences");
  });

  it("blast_radius: dotted namespace name does NOT trigger path guard", async () => {
    const mod = await import("../src/tools/blast_radius.ts");
    _setShards([]);

    // "store.blastRadius" — contains a dot but is NOT a file path (no extension).
    const result = await mod.tool.handler(
      { target: "store.blastRadius", depth: 1, deep: false, include_tests: true },
      { sessionId: "compat-test-2", cwd: process.cwd() },
    );

    // Must be the standard blast-radius shape.
    expect("target" in (result as object)).toBe(true);
    expect((result as { target: string }).target).toBe("store.blastRadius");
  });
});
