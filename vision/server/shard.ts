// vision/server/shard.ts
//
// Server-only shard reader. Imported exclusively by `vision/server.ts`
// (Bun runtime). Never imported from `src/` — it pulls `bun:sqlite` +
// `node:*` builtins that must not end up in the browser bundle.
//
// Mirrors the pattern in `mcp/src/store.ts`: derive ProjectId by
// SHA-256-hashing the canonical project root, look up the shard directory
// at `~/.mneme/projects/<project-id>/`, open each `.db` read-only.

import { createHash } from "node:crypto";
import { existsSync, readdirSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { Database } from "bun:sqlite";

import type {
  ShardFileRow,
  ShardFindingRow,
  GraphStatsPayload,
  DaemonHealthPayload,
} from "../src/api/graph";
import type { GraphNode, GraphEdge } from "../src/api";

const MNEME_HOME = join(homedir(), ".mneme");

function projectIdForPath(absPath: string): string {
  return createHash("sha256").update(absPath).digest("hex");
}

function findProjectRoot(start: string): string | null {
  const markers = [".git", ".claude", "package.json", "Cargo.toml", "pyproject.toml"];
  let cur = resolve(start);
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

function resolveShardRoot(): string | null {
  const cwd = process.cwd();
  const fromCwd = findProjectRoot(cwd);
  if (fromCwd) {
    const id = projectIdForPath(fromCwd);
    const dir = join(MNEME_HOME, "projects", id);
    if (existsSync(dir)) return dir;
  }
  const projectsDir = join(MNEME_HOME, "projects");
  if (existsSync(projectsDir)) {
    try {
      const entries = readdirSync(projectsDir);
      if (entries.length === 1 && entries[0]) {
        return join(projectsDir, entries[0]);
      }
    } catch {
      /* ignore */
    }
  }
  return null;
}

function shardDbPath(layer: string): string | null {
  const root = resolveShardRoot();
  if (!root) return null;
  const p = join(root, `${layer}.db`);
  return existsSync(p) ? p : null;
}

function openShard(layer: string): Database | null {
  const p = shardDbPath(layer);
  if (!p) return null;
  try {
    return new Database(p, { readonly: true });
  } catch {
    return null;
  }
}

/** Row count summary for the Vision status bar. */
export function graphStats(): {
  nodes: number;
  edges: number;
  files: number;
  byKind: Record<string, number>;
} {
  const db = openShard("graph");
  if (!db) return { nodes: 0, edges: 0, files: 0, byKind: {} };
  try {
    const nodes =
      (db.prepare("SELECT COUNT(*) AS c FROM nodes").get() as { c: number } | undefined)?.c ?? 0;
    const edges =
      (db.prepare("SELECT COUNT(*) AS c FROM edges").get() as { c: number } | undefined)?.c ?? 0;
    const files =
      (db.prepare("SELECT COUNT(*) AS c FROM nodes WHERE kind='file'").get() as
        | { c: number }
        | undefined)?.c ?? 0;
    const byKind: Record<string, number> = {};
    try {
      for (const row of db
        .prepare("SELECT kind, COUNT(*) AS c FROM nodes GROUP BY kind")
        .all() as Array<{ kind: string; c: number }>) {
        byKind[row.kind] = row.c;
      }
    } catch {
      /* ignore */
    }
    return { nodes, edges, files, byKind };
  } finally {
    try {
      db.close();
    } catch {
      /* ignore */
    }
  }
}

export function fetchGraphNodes(limit = 2000): GraphNode[] {
  const db = openShard("graph");
  if (!db) return [];
  try {
    const rows = db
      .prepare(
        `SELECT qualified_name AS id, name, kind, file_path
         FROM nodes
         ORDER BY id
         LIMIT ?`,
      )
      .all(limit) as Array<{
      id: string;
      name: string | null;
      kind: string;
      file_path: string | null;
    }>;
    return rows.map((r) => ({
      id: r.id,
      label: r.name ?? r.id,
      type: r.kind,
      size: sizeForKind(r.kind),
      color: colorForKind(r.kind),
      meta: { kind: r.kind, file_path: r.file_path, source: "shard" },
    }));
  } finally {
    try {
      db.close();
    } catch {
      /* ignore */
    }
  }
}

export function fetchGraphEdges(limit = 8000): GraphEdge[] {
  const db = openShard("graph");
  if (!db) return [];
  try {
    const rows = db
      .prepare(
        `SELECT id, source_qualified AS source, target_qualified AS target, kind
         FROM edges
         ORDER BY id
         LIMIT ?`,
      )
      .all(limit) as Array<{ id: number; source: string; target: string; kind: string }>;
    return rows.map((r) => ({
      id: String(r.id),
      source: r.source,
      target: r.target,
      type: r.kind,
      weight: 1,
      meta: { kind: r.kind, source: "shard" },
    }));
  } finally {
    try {
      db.close();
    } catch {
      /* ignore */
    }
  }
}

export function fetchFilesForTreemap(limit = 2000): ShardFileRow[] {
  const db = openShard("graph");
  if (!db) return [];
  try {
    return db
      .prepare(
        `SELECT path, language, line_count, byte_count, last_parsed_at
         FROM files
         ORDER BY line_count DESC
         LIMIT ?`,
      )
      .all(limit) as ShardFileRow[];
  } finally {
    try {
      db.close();
    } catch {
      /* ignore */
    }
  }
}

export function fetchFindings(limit = 2000): ShardFindingRow[] {
  const db = openShard("findings");
  if (!db) return [];
  try {
    return db
      .prepare(
        `SELECT id, rule_id, scanner, severity, file, line_start, line_end,
                message, suggestion, created_at
         FROM findings
         WHERE resolved_at IS NULL
         ORDER BY CASE severity
                    WHEN 'critical' THEN 4
                    WHEN 'high'     THEN 3
                    WHEN 'medium'   THEN 2
                    WHEN 'low'      THEN 1
                    ELSE 0 END DESC,
                  created_at DESC
         LIMIT ?`,
      )
      .all(limit) as ShardFindingRow[];
  } finally {
    try {
      db.close();
    } catch {
      /* ignore */
    }
  }
}

export function projectStatus(): {
  project: string | null;
  shardRoot: string | null;
  lastIndexAt: string | null;
} {
  const root = resolveShardRoot();
  if (!root) return { project: null, shardRoot: null, lastIndexAt: null };

  let project: string | null = null;
  let lastIndexAt: string | null = null;

  // Optional metadata.db or metadata table in graph.db.
  const metadb = openShard("metadata") ?? openShard("graph");
  if (metadb) {
    try {
      const row = metadb
        .prepare("SELECT value FROM metadata WHERE key = 'project_name' LIMIT 1")
        .get() as { value: string } | undefined;
      if (row?.value) project = row.value;
    } catch {
      /* ignore */
    }
    try {
      const row = metadb
        .prepare("SELECT value FROM metadata WHERE key = 'last_index_at' LIMIT 1")
        .get() as { value: string } | undefined;
      if (row?.value) lastIndexAt = row.value;
    } catch {
      /* ignore */
    }
    try {
      metadb.close();
    } catch {
      /* ignore */
    }
  }

  // Fallback: newest *.db mtime under the shard directory.
  if (!lastIndexAt && existsSync(root)) {
    try {
      let newest = 0;
      for (const name of readdirSync(root)) {
        if (!name.endsWith(".db")) continue;
        const s = statSync(join(root, name));
        if (s.mtimeMs > newest) newest = s.mtimeMs;
      }
      if (newest > 0) lastIndexAt = new Date(newest).toISOString();
    } catch {
      /* ignore */
    }
  }

  // Fallback project name: last path segment of the detected project root.
  if (!project) {
    const cwdRoot = findProjectRoot(process.cwd());
    if (cwdRoot) {
      const parts = cwdRoot.split(/[\\/]/).filter(Boolean);
      project = parts[parts.length - 1] ?? null;
    }
  }

  return { project, shardRoot: root, lastIndexAt };
}

export async function probeDaemon(url: string): Promise<DaemonHealthPayload> {
  try {
    const ac = new AbortController();
    const timer = setTimeout(() => ac.abort(), 800);
    const res = await fetch(url, { signal: ac.signal });
    clearTimeout(timer);
    if (!res.ok) {
      return { ok: false, status: "error", url, detail: `HTTP ${res.status}` };
    }
    const text = await res.text();
    return { ok: true, status: "running", url, detail: text.slice(0, 200) };
  } catch (err) {
    const msg = (err as Error).message;
    return { ok: false, status: "missing", url, error: msg };
  }
}

export function buildStatusPayload(): GraphStatsPayload {
  try {
    const stats = graphStats();
    const s = projectStatus();
    return {
      ok: Boolean(s.shardRoot),
      project: s.project,
      shardRoot: s.shardRoot,
      nodes: stats.nodes,
      edges: stats.edges,
      files: stats.files,
      byKind: stats.byKind,
      lastIndexAt: s.lastIndexAt,
    };
  } catch (err) {
    return {
      ok: false,
      project: null,
      shardRoot: null,
      nodes: 0,
      edges: 0,
      files: 0,
      byKind: {},
      lastIndexAt: null,
      error: String(err),
    };
  }
}

function sizeForKind(kind: string): number {
  switch (kind) {
    case "file":
      return 6;
    case "function":
      return 4;
    case "class":
      return 5;
    case "module":
      return 7;
    default:
      return 3;
  }
}

function colorForKind(kind: string): string {
  switch (kind) {
    case "file":
      return "#4191e1";
    case "function":
      return "#41e1b5";
    case "class":
      return "#22d3ee";
    case "module":
      return "#f59e0b";
    default:
      return "#7aa7ff";
  }
}
