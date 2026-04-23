/**
 * Direct read-only access to a project's datatree SQLite shards.
 *
 * MCP tools in v0.1 read from graph.db / history.db / findings.db / tasks.db
 * directly via Bun's native bun:sqlite. This is safe because SQLite WAL mode
 * supports unlimited concurrent readers alongside the supervisor's single
 * writer — we never open in write mode from here.
 *
 * Writes still go through the supervisor over IPC (see db.ts).
 */

import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { Database } from "bun:sqlite";

const MNEME_HOME = join(homedir(), ".datatree");

/**
 * Hash an absolute project path to its ProjectId (matches Rust
 * `ProjectId::from_path` which SHA-256s the canonical path).
 */
export function projectIdForPath(absPath: string): string {
  return createHash("sha256").update(absPath).digest("hex");
}

/**
 * Walk up from `start` until we find a project marker (.git / .claude /
 * package.json / Cargo.toml / pyproject.toml). Returns null if none found.
 */
export function findProjectRoot(start: string): string | null {
  const markers = [".git", ".claude", "package.json", "Cargo.toml", "pyproject.toml"];
  let cur = resolve(start);
  // Climb up to 40 levels (protect against symlink loops).
  for (let i = 0; i < 40; i++) {
    for (const m of markers) {
      if (existsSync(join(cur, m))) return cur;
    }
    const parent = dirname(cur);
    if (parent === cur) break;
    cur = parent;
  }
  return null;
}

/**
 * Resolve the active shard root: uses cwd by default, falls back to env,
 * then scans `~/.mneme/projects/*` and returns the newest one if only
 * one exists.
 */
export function resolveShardRoot(cwdOverride?: string): string | null {
  const cwd = cwdOverride ?? process.cwd();
  const fromCwd = findProjectRoot(cwd);
  if (fromCwd) {
    const id = projectIdForPath(fromCwd);
    const dir = join(MNEME_HOME, "projects", id);
    if (existsSync(dir)) return dir;
  }
  // Fallback: if exactly one project exists, use it.
  const projectsDir = join(MNEME_HOME, "projects");
  if (existsSync(projectsDir)) {
    try {
      const { readdirSync } = require("node:fs");
      const entries = readdirSync(projectsDir);
      if (entries.length === 1) return join(projectsDir, entries[0]);
    } catch {
      // ignore
    }
  }
  return null;
}

/** Open a shard's .db file read-only. Throws if the shard isn't built yet. */
export function openShardDb(layer: string, cwdOverride?: string): Database {
  const root = resolveShardRoot(cwdOverride);
  if (!root) {
    throw new Error(
      "datatree shard not found — run `datatree build .` in your project first",
    );
  }
  const path = join(root, `${layer}.db`);
  if (!existsSync(path)) {
    throw new Error(`datatree shard missing ${layer}.db at ${path}`);
  }
  return new Database(path, { readonly: true });
}

/**
 * Quick node count for health reporting. Safe to call even if the DB is
 * empty or freshly created.
 */
export function graphStats(cwdOverride?: string): {
  nodes: number;
  edges: number;
  files: number;
  byKind: Record<string, number>;
} {
  const db = openShardDb("graph", cwdOverride);
  try {
    const nodes =
      (db.prepare("SELECT COUNT(*) AS c FROM nodes").get() as { c: number }).c;
    const edges =
      (db.prepare("SELECT COUNT(*) AS c FROM edges").get() as { c: number }).c;
    const files =
      (db.prepare("SELECT COUNT(*) AS c FROM nodes WHERE kind='file'").get() as {
        c: number;
      }).c;
    const byKind: Record<string, number> = {};
    for (const row of db
      .prepare("SELECT kind, COUNT(*) AS c FROM nodes GROUP BY kind")
      .all() as Array<{ kind: string; c: number }>) {
      byKind[row.kind] = row.c;
    }
    return { nodes, edges, files, byKind };
  } finally {
    db.close();
  }
}

/**
 * Blast radius: every node reachable from `target` via `calls`, `contains`,
 * or `imports` edges within `maxDepth` hops. Returns the qualified names and
 * the depth at which each was discovered.
 */
export function blastRadius(
  target: string,
  maxDepth: number = 2,
  cwdOverride?: string,
): { node: string; depth: number; kind: string }[] {
  const db = openShardDb("graph", cwdOverride);
  try {
    // Recursive CTE: frontier-expand via edges where source = target.
    const sql = `
      WITH RECURSIVE blast(node, depth) AS (
        SELECT qualified_name, 0 FROM nodes WHERE qualified_name = ?
        UNION
        SELECT e.target_qualified, b.depth + 1
        FROM blast b
        JOIN edges e ON e.source_qualified = b.node
        WHERE b.depth < ?
      )
      SELECT b.node, b.depth, COALESCE(n.kind, '?') AS kind
      FROM blast b
      LEFT JOIN nodes n ON n.qualified_name = b.node
      ORDER BY b.depth, b.node
      LIMIT 500
    `;
    const rows = db.prepare(sql).all(target, maxDepth) as Array<{
      node: string;
      depth: number;
      kind: string;
    }>;
    return rows;
  } finally {
    db.close();
  }
}

/**
 * Semantic-ish recall: LIKE-match over qualified_name + name. For v0.1
 * without embeddings this is a simple FTS-style substring scan.
 */
export function recallNode(
  query: string,
  limit: number = 20,
  cwdOverride?: string,
): { qualified_name: string; kind: string; file_path: string | null }[] {
  const db = openShardDb("graph", cwdOverride);
  try {
    const like = `%${query.toLowerCase()}%`;
    const rows = db
      .prepare(
        `SELECT qualified_name, kind, file_path
         FROM nodes
         WHERE lower(name) LIKE ? OR lower(qualified_name) LIKE ?
         LIMIT ?`,
      )
      .all(like, like, limit) as Array<{
      qualified_name: string;
      kind: string;
      file_path: string | null;
    }>;
    return rows;
  } finally {
    db.close();
  }
}

/** Direct callers of a target (incoming `calls` edges). */
export function callersOf(
  target: string,
  limit: number = 100,
  cwdOverride?: string,
): { caller: string; file_path: string | null; line: number | null }[] {
  const db = openShardDb("graph", cwdOverride);
  try {
    const rows = db
      .prepare(
        `SELECT e.source_qualified AS caller, n.file_path, e.line
         FROM edges e
         LEFT JOIN nodes n ON n.qualified_name = e.source_qualified
         WHERE e.target_qualified = ? AND e.kind = 'calls'
         LIMIT ?`,
      )
      .all(target, limit) as Array<{
      caller: string;
      file_path: string | null;
      line: number | null;
    }>;
    return rows;
  } finally {
    db.close();
  }
}
