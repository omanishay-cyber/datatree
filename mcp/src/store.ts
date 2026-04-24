/**
 * Direct read-only access to a project's mneme SQLite shards.
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

const MNEME_HOME = join(homedir(), ".mneme");

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
      "mneme shard not found — run `mneme build .` in your project first",
    );
  }
  const path = join(root, `${layer}.db`);
  if (!existsSync(path)) {
    throw new Error(`mneme shard missing ${layer}.db at ${path}`);
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

// ---------------------------------------------------------------------------
// Shard availability + per-shard read helpers
// (Used by recall_file / recall_decision / recall_todo / recall_constraint /
//  recall_conversation / god_nodes / drift_findings / doctor / step_status /
//  step_resume — the 10 tools wired in review P2.)
// ---------------------------------------------------------------------------

/** Returns the absolute .db path for a layer, or null if the shard root or
 *  the specific layer DB file doesn't exist yet. Never throws. */
export function shardDbPath(layer: string, cwdOverride?: string): string | null {
  const root = resolveShardRoot(cwdOverride);
  if (!root) return null;
  const path = join(root, `${layer}.db`);
  return existsSync(path) ? path : null;
}

/** Open a shard read-only if it exists; returns null instead of throwing. */
export function tryOpenShard(layer: string, cwdOverride?: string): Database | null {
  const p = shardDbPath(layer, cwdOverride);
  if (!p) return null;
  try {
    return new Database(p, { readonly: true });
  } catch {
    return null;
  }
}

/** Run `fn` against a read-only connection to `layer`. If the shard is
 *  missing or fn throws, returns `fallback`. Connection is always closed. */
export function withShard<T>(
  layer: string,
  fn: (db: Database) => T,
  fallback: T,
  cwdOverride?: string,
): T {
  const db = tryOpenShard(layer, cwdOverride);
  if (!db) return fallback;
  try {
    return fn(db);
  } catch {
    return fallback;
  } finally {
    try {
      db.close();
    } catch {
      // ignore
    }
  }
}

// -- graph shard -----------------------------------------------------------

/** Look up a file node by exact path + top-N neighbors (callers/callees). */
export function fileNodeState(
  filePath: string,
  neighborLimit: number = 10,
  cwdOverride?: string,
): {
  file_path: string;
  language: string | null;
  sha256: string | null;
  line_count: number | null;
  byte_count: number | null;
  last_parsed_at: string | null;
  neighbors: Array<{ qualified_name: string; edge_kind: string; kind: string | null }>;
} | null {
  return withShard<{
    file_path: string;
    language: string | null;
    sha256: string | null;
    line_count: number | null;
    byte_count: number | null;
    last_parsed_at: string | null;
    neighbors: Array<{ qualified_name: string; edge_kind: string; kind: string | null }>;
  } | null>(
    "graph",
    (db) => {
      const file = db
        .prepare(
          `SELECT path, sha256, language, line_count, byte_count, last_parsed_at
           FROM files WHERE path = ?`,
        )
        .get(filePath) as
        | {
            path: string;
            sha256: string;
            language: string | null;
            line_count: number | null;
            byte_count: number | null;
            last_parsed_at: string;
          }
        | undefined;

      if (!file) return null;

      // Top neighbors: edges that cross this file's boundary (either endpoint
      // lives in this file). We approximate via nodes.file_path match.
      const neighbors = db
        .prepare(
          `SELECT DISTINCT
             CASE WHEN n_src.file_path = ?1 THEN e.target_qualified
                  ELSE e.source_qualified END AS qualified_name,
             e.kind AS edge_kind,
             n_other.kind AS kind
           FROM edges e
           LEFT JOIN nodes n_src ON n_src.qualified_name = e.source_qualified
           LEFT JOIN nodes n_tgt ON n_tgt.qualified_name = e.target_qualified
           LEFT JOIN nodes n_other ON n_other.qualified_name =
             CASE WHEN n_src.file_path = ?1 THEN e.target_qualified
                  ELSE e.source_qualified END
           WHERE n_src.file_path = ?1 OR n_tgt.file_path = ?1
           LIMIT ?2`,
        )
        .all(filePath, neighborLimit) as Array<{
        qualified_name: string;
        edge_kind: string;
        kind: string | null;
      }>;

      return {
        file_path: file.path,
        language: file.language,
        sha256: file.sha256 ?? null,
        line_count: file.line_count,
        byte_count: file.byte_count,
        last_parsed_at: file.last_parsed_at ?? null,
        neighbors,
      };
    },
    null,
    cwdOverride,
  );
}

/** Count inbound+outbound edges that touch any node whose file_path matches. */
export function blastRadiusCount(filePath: string, cwdOverride?: string): number {
  return withShard<number>(
    "graph",
    (db) => {
      const row = db
        .prepare(
          `SELECT COUNT(DISTINCT e.id) AS c FROM edges e
           LEFT JOIN nodes n_src ON n_src.qualified_name = e.source_qualified
           LEFT JOIN nodes n_tgt ON n_tgt.qualified_name = e.target_qualified
           WHERE n_src.file_path = ? OR n_tgt.file_path = ?`,
        )
        .get(filePath, filePath) as { c: number } | undefined;
      return row?.c ?? 0;
    },
    0,
    cwdOverride,
  );
}

/** Top-N most-connected nodes (degree = incoming + outgoing edges). */
export function godNodesTopN(
  topN: number = 10,
  cwdOverride?: string,
): Array<{
  qualified_name: string;
  degree: number;
  out_degree: number;
  in_degree: number;
  kind: string | null;
}> {
  return withShard<
    Array<{
      qualified_name: string;
      degree: number;
      out_degree: number;
      in_degree: number;
      kind: string | null;
    }>
  >(
    "graph",
    (db) => {
      const rows = db
        .prepare(
          `WITH deg AS (
             SELECT source_qualified AS q, COUNT(*) AS out_d, 0 AS in_d FROM edges GROUP BY source_qualified
             UNION ALL
             SELECT target_qualified AS q, 0, COUNT(*)                  FROM edges GROUP BY target_qualified
           )
           SELECT d.q AS qualified_name,
                  SUM(d.out_d + d.in_d) AS degree,
                  SUM(d.out_d)          AS out_degree,
                  SUM(d.in_d)           AS in_degree,
                  (SELECT kind FROM nodes WHERE qualified_name = d.q LIMIT 1) AS kind
           FROM deg d
           GROUP BY d.q
           ORDER BY degree DESC
           LIMIT ?`,
        )
        .all(topN) as Array<{
        qualified_name: string;
        degree: number;
        out_degree: number;
        in_degree: number;
        kind: string | null;
      }>;
      return rows;
    },
    [],
    cwdOverride,
  );
}

// -- history shard ---------------------------------------------------------

/** FTS5 search over decisions.(topic + problem + chosen + reasoning). */
export function searchDecisions(
  queryText: string,
  limit: number = 10,
  since?: string,
  cwdOverride?: string,
): Array<{
  id: number;
  session_id: string | null;
  topic: string;
  problem: string;
  chosen: string;
  reasoning: string;
  alternatives: string;
  artifacts: string;
  created_at: string;
}> {
  return withShard<
    Array<{
      id: number;
      session_id: string | null;
      topic: string;
      problem: string;
      chosen: string;
      reasoning: string;
      alternatives: string;
      artifacts: string;
      created_at: string;
    }>
  >(
    "history",
    (db) => {
      // Decisions doesn't have an FTS5 index at schema v1 — scan via LIKE.
      const like = `%${queryText.toLowerCase()}%`;
      const params: Array<string | number> = [like, like, like, like];
      let where = `(lower(topic) LIKE ? OR lower(problem) LIKE ?
                    OR lower(chosen) LIKE ? OR lower(reasoning) LIKE ?)`;
      if (since) {
        where += " AND created_at >= ?";
        params.push(since);
      }
      params.push(limit);
      const sql = `SELECT id, session_id, topic, problem, chosen, reasoning,
                          alternatives, artifacts, created_at
                   FROM decisions
                   WHERE ${where}
                   ORDER BY created_at DESC
                   LIMIT ?`;
      return db.prepare(sql).all(...params) as Array<{
        id: number;
        session_id: string | null;
        topic: string;
        problem: string;
        chosen: string;
        reasoning: string;
        alternatives: string;
        artifacts: string;
        created_at: string;
      }>;
    },
    [],
    cwdOverride,
  );
}

/** FTS5 search over turns.content with optional session + since filters. */
export function searchConversation(
  queryText: string,
  limit: number = 10,
  sessionId?: string,
  since?: string,
  cwdOverride?: string,
): Array<{
  id: number;
  session_id: string;
  role: string;
  content: string;
  timestamp: string;
}> {
  return withShard<
    Array<{
      id: number;
      session_id: string;
      role: string;
      content: string;
      timestamp: string;
    }>
  >(
    "history",
    (db) => {
      const params: Array<string | number> = [];
      // Try FTS5 first; if the query has special chars, fall back to LIKE.
      const safeQuery = queryText.replace(/["']/g, " ").trim();
      const useFts = safeQuery.length > 0 && !/[^\w\s]/.test(safeQuery);

      let sql: string;
      if (useFts) {
        sql = `SELECT t.id, t.session_id, t.role, t.content, t.timestamp
               FROM turns_fts f
               JOIN turns t ON t.id = f.rowid
               WHERE turns_fts MATCH ?`;
        params.push(safeQuery);
      } else {
        sql = `SELECT id, session_id, role, content, timestamp
               FROM turns
               WHERE lower(content) LIKE ?`;
        params.push(`%${queryText.toLowerCase()}%`);
      }
      if (sessionId) {
        sql += ` AND ${useFts ? "t." : ""}session_id = ?`;
        params.push(sessionId);
      }
      if (since) {
        sql += ` AND ${useFts ? "t." : ""}timestamp >= ?`;
        params.push(since);
      }
      sql += ` ORDER BY ${useFts ? "t." : ""}timestamp DESC LIMIT ?`;
      params.push(limit);

      return db.prepare(sql).all(...params) as Array<{
        id: number;
        session_id: string;
        role: string;
        content: string;
        timestamp: string;
      }>;
    },
    [],
    cwdOverride,
  );
}

// -- tasks shard -----------------------------------------------------------

/** Open ledger entries used as "todos" (open_question + unresolved impl). */
export function openReminders(
  limit: number = 200,
  tag?: string,
  since?: string,
  cwdOverride?: string,
): Array<{
  id: string;
  session_id: string;
  kind: string;
  summary: string;
  rationale: string | null;
  touched_files: string;
  touched_concepts: string;
  timestamp: number;
}> {
  return withShard<
    Array<{
      id: string;
      session_id: string;
      kind: string;
      summary: string;
      rationale: string | null;
      touched_files: string;
      touched_concepts: string;
      timestamp: number;
    }>
  >(
    "tasks",
    (db) => {
      const clauses: string[] = ["kind = 'open_question'"];
      const params: Array<string | number> = [];
      if (tag) {
        clauses.push("(lower(summary) LIKE ? OR lower(touched_concepts) LIKE ?)");
        const like = `%${tag.toLowerCase()}%`;
        params.push(like, like);
      }
      if (since) {
        // since is RFC3339 — convert to unix millis for ledger_entries.
        const ms = Date.parse(since);
        if (!Number.isNaN(ms)) {
          clauses.push("timestamp >= ?");
          params.push(ms);
        }
      }
      params.push(limit);
      const sql = `SELECT id, session_id, kind, summary, rationale,
                          touched_files, touched_concepts, timestamp
                   FROM ledger_entries
                   WHERE ${clauses.join(" AND ")}
                   ORDER BY timestamp DESC
                   LIMIT ?`;
      return db.prepare(sql).all(...params) as Array<{
        id: string;
        session_id: string;
        kind: string;
        summary: string;
        rationale: string | null;
        touched_files: string;
        touched_concepts: string;
        timestamp: number;
      }>;
    },
    [],
    cwdOverride,
  );
}

/** All steps for a session, ordered by creation (parent-first). */
export function sessionSteps(
  sessionId: string,
  cwdOverride?: string,
): Array<{
  step_id: string;
  parent_step_id: string | null;
  session_id: string;
  description: string;
  acceptance_cmd: string | null;
  acceptance_check: string;
  status: string;
  started_at: string | null;
  completed_at: string | null;
  verification_proof: string | null;
  artifacts: string;
  notes: string;
  blocker: string | null;
  drift_score: number;
}> {
  return withShard<
    Array<{
      step_id: string;
      parent_step_id: string | null;
      session_id: string;
      description: string;
      acceptance_cmd: string | null;
      acceptance_check: string;
      status: string;
      started_at: string | null;
      completed_at: string | null;
      verification_proof: string | null;
      artifacts: string;
      notes: string;
      blocker: string | null;
      drift_score: number;
    }>
  >(
    "tasks",
    (db) => {
      return db
        .prepare(
          `SELECT step_id, parent_step_id, session_id, description,
                  acceptance_cmd, acceptance_check, status, started_at,
                  completed_at, verification_proof, artifacts, notes,
                  blocker, drift_score
           FROM steps
           WHERE session_id = ?
           ORDER BY CASE WHEN parent_step_id IS NULL THEN 0 ELSE 1 END,
                    step_id ASC`,
        )
        .all(sessionId) as Array<{
        step_id: string;
        parent_step_id: string | null;
        session_id: string;
        description: string;
        acceptance_cmd: string | null;
        acceptance_check: string;
        status: string;
        started_at: string | null;
        completed_at: string | null;
        verification_proof: string | null;
        artifacts: string;
        notes: string;
        blocker: string | null;
        drift_score: number;
      }>;
    },
    [],
    cwdOverride,
  );
}

/** Recent ledger entries for a session (used by step_resume). */
export function recentLedger(
  sessionId: string | undefined,
  sinceMs: number,
  limit: number = 100,
  cwdOverride?: string,
): Array<{
  id: string;
  session_id: string;
  kind: string;
  summary: string;
  rationale: string | null;
  timestamp: number;
}> {
  return withShard<
    Array<{
      id: string;
      session_id: string;
      kind: string;
      summary: string;
      rationale: string | null;
      timestamp: number;
    }>
  >(
    "tasks",
    (db) => {
      const params: Array<string | number> = [sinceMs];
      let sql = `SELECT id, session_id, kind, summary, rationale, timestamp
                 FROM ledger_entries
                 WHERE timestamp >= ?`;
      if (sessionId) {
        sql += ` AND session_id = ?`;
        params.push(sessionId);
      }
      sql += ` ORDER BY timestamp DESC LIMIT ?`;
      params.push(limit);
      return db.prepare(sql).all(...params) as Array<{
        id: string;
        session_id: string;
        kind: string;
        summary: string;
        rationale: string | null;
        timestamp: number;
      }>;
    },
    [],
    cwdOverride,
  );
}

// -- findings shard --------------------------------------------------------

export function driftFindings(
  severity: string | undefined,
  scope: string | undefined,
  limit: number = 50,
  cwdOverride?: string,
): Array<{
  id: number;
  rule_id: string;
  scanner: string;
  severity: string;
  file: string;
  line_start: number;
  line_end: number;
  message: string;
  suggestion: string | null;
  created_at: string;
}> {
  return withShard<
    Array<{
      id: number;
      rule_id: string;
      scanner: string;
      severity: string;
      file: string;
      line_start: number;
      line_end: number;
      message: string;
      suggestion: string | null;
      created_at: string;
    }>
  >(
    "findings",
    (db) => {
      const clauses: string[] = ["resolved_at IS NULL"];
      const params: Array<string | number> = [];
      if (severity) {
        clauses.push("severity = ?");
        params.push(severity);
      }
      if (scope) {
        clauses.push("file LIKE ?");
        params.push(`%${scope}%`);
      }
      params.push(limit);
      const sql = `SELECT id, rule_id, scanner, severity, file,
                          line_start, line_end, message, suggestion, created_at
                   FROM findings
                   WHERE ${clauses.join(" AND ")}
                   ORDER BY CASE severity
                              WHEN 'critical' THEN 4
                              WHEN 'high'     THEN 3
                              WHEN 'medium'   THEN 2
                              WHEN 'low'      THEN 1
                              ELSE 0 END DESC,
                            created_at DESC
                   LIMIT ?`;
      return db.prepare(sql).all(...params) as Array<{
        id: number;
        rule_id: string;
        scanner: string;
        severity: string;
        file: string;
        line_start: number;
        line_end: number;
        message: string;
        suggestion: string | null;
        created_at: string;
      }>;
    },
    [],
    cwdOverride,
  );
}

// -- findings shard (audit helpers) ----------------------------------------

/**
 * Return open findings, optionally filtered by a list of scanner names and
 * a scope glob (matched LIKE '%scope%' against `file`).
 *
 * Used by `audit` and every `audit_<scanner>` tool. Severity + scanner
 * breakdowns are computed by the caller.
 */
export function scannerFindings(
  scanners: string[] | undefined,
  scope: string | undefined,
  file: string | undefined,
  limit: number = 500,
  cwdOverride?: string,
): Array<{
  id: number;
  rule_id: string;
  scanner: string;
  severity: string;
  file: string;
  line_start: number;
  line_end: number;
  message: string;
  suggestion: string | null;
  created_at: string;
}> {
  return withShard<
    Array<{
      id: number;
      rule_id: string;
      scanner: string;
      severity: string;
      file: string;
      line_start: number;
      line_end: number;
      message: string;
      suggestion: string | null;
      created_at: string;
    }>
  >(
    "findings",
    (db) => {
      const clauses: string[] = ["resolved_at IS NULL"];
      const params: Array<string | number> = [];
      if (scanners && scanners.length > 0) {
        const placeholders = scanners.map(() => "?").join(",");
        clauses.push(`scanner IN (${placeholders})`);
        for (const s of scanners) params.push(s);
      }
      if (file) {
        clauses.push("file = ?");
        params.push(file);
      } else if (scope) {
        clauses.push("file LIKE ?");
        params.push(`%${scope}%`);
      }
      params.push(limit);
      const sql = `SELECT id, rule_id, scanner, severity, file,
                          line_start, line_end, message, suggestion, created_at
                   FROM findings
                   WHERE ${clauses.join(" AND ")}
                   ORDER BY created_at DESC
                   LIMIT ?`;
      return db.prepare(sql).all(...params) as Array<{
        id: number;
        rule_id: string;
        scanner: string;
        severity: string;
        file: string;
        line_start: number;
        line_end: number;
        message: string;
        suggestion: string | null;
        created_at: string;
      }>;
    },
    [],
    cwdOverride,
  );
}

/** Aggregated stats for `audit_corpus`: counts by scanner × severity. */
export function findingsCorpusStats(cwdOverride?: string): {
  total: number;
  by_severity: Record<string, number>;
  by_scanner: Record<string, number>;
  by_scanner_severity: Record<string, Record<string, number>>;
} {
  return withShard<{
    total: number;
    by_severity: Record<string, number>;
    by_scanner: Record<string, number>;
    by_scanner_severity: Record<string, Record<string, number>>;
  }>(
    "findings",
    (db) => {
      const rows = db
        .prepare(
          `SELECT scanner, severity, COUNT(*) AS c
           FROM findings
           WHERE resolved_at IS NULL
           GROUP BY scanner, severity`,
        )
        .all() as Array<{ scanner: string; severity: string; c: number }>;

      const by_severity: Record<string, number> = {};
      const by_scanner: Record<string, number> = {};
      const by_scanner_severity: Record<string, Record<string, number>> = {};
      let total = 0;
      for (const r of rows) {
        total += r.c;
        by_severity[r.severity] = (by_severity[r.severity] ?? 0) + r.c;
        by_scanner[r.scanner] = (by_scanner[r.scanner] ?? 0) + r.c;
        const sev = by_scanner_severity[r.scanner] ?? {};
        sev[r.severity] = (sev[r.severity] ?? 0) + r.c;
        by_scanner_severity[r.scanner] = sev;
      }
      return { total, by_severity, by_scanner, by_scanner_severity };
    },
    { total: 0, by_severity: {}, by_scanner: {}, by_scanner_severity: {} },
    cwdOverride,
  );
}

// -- graph shard (call graph / cycles / deps / references) -----------------

export interface GraphEdgeRow {
  source: string;
  target: string;
  kind: string;
  file: string | null;
  line: number | null;
}

/** BFS call-graph expansion. direction picks edge orientation. */
export function callGraphBfs(
  fn: string,
  direction: "callers" | "callees" | "both",
  depth: number,
  cwdOverride?: string,
): {
  nodes: Array<{ id: string; label: string; file: string; line: number }>;
  edges: Array<{ source: string; target: string; call_count: number }>;
} {
  return withShard<{
    nodes: Array<{ id: string; label: string; file: string; line: number }>;
    edges: Array<{ source: string; target: string; call_count: number }>;
  }>(
    "graph",
    (db) => {
      const visited = new Set<string>([fn]);
      const edgePairs = new Map<string, number>(); // "src->tgt" -> count
      const pickCallees = direction === "callees" || direction === "both";
      const pickCallers = direction === "callers" || direction === "both";

      const calleeStmt = db.prepare(
        `SELECT target_qualified AS tgt, file_path AS file, line
         FROM edges WHERE kind = 'calls' AND source_qualified = ?`,
      );
      const callerStmt = db.prepare(
        `SELECT source_qualified AS src, file_path AS file, line
         FROM edges WHERE kind = 'calls' AND target_qualified = ?`,
      );

      let frontier: string[] = [fn];
      for (let d = 0; d < depth && frontier.length > 0; d++) {
        const next: string[] = [];
        for (const cur of frontier) {
          if (pickCallees) {
            const rows = calleeStmt.all(cur) as Array<{
              tgt: string;
              file: string | null;
              line: number | null;
            }>;
            for (const r of rows) {
              const key = `${cur}->${r.tgt}`;
              edgePairs.set(key, (edgePairs.get(key) ?? 0) + 1);
              if (!visited.has(r.tgt)) {
                visited.add(r.tgt);
                next.push(r.tgt);
              }
            }
          }
          if (pickCallers) {
            const rows = callerStmt.all(cur) as Array<{
              src: string;
              file: string | null;
              line: number | null;
            }>;
            for (const r of rows) {
              const key = `${r.src}->${cur}`;
              edgePairs.set(key, (edgePairs.get(key) ?? 0) + 1);
              if (!visited.has(r.src)) {
                visited.add(r.src);
                next.push(r.src);
              }
            }
          }
        }
        frontier = next;
      }

      const nodeMetaStmt = db.prepare(
        `SELECT qualified_name, name, file_path, line_start
         FROM nodes WHERE qualified_name = ?`,
      );
      const nodes = Array.from(visited).map((q) => {
        const m = nodeMetaStmt.get(q) as
          | {
              qualified_name: string;
              name: string;
              file_path: string | null;
              line_start: number | null;
            }
          | undefined;
        return {
          id: q,
          label: m?.name ?? q,
          file: m?.file_path ?? "",
          line: m?.line_start ?? 0,
        };
      });
      const edges = Array.from(edgePairs.entries()).map(([k, count]) => {
        const [source, target] = k.split("->");
        return {
          source: source ?? "",
          target: target ?? "",
          call_count: count,
        };
      });
      return { nodes, edges };
    },
    { nodes: [], edges: [] },
    cwdOverride,
  );
}

/** Tarjan strongly-connected-components over `edges` table. Returns cycles
 *  (components with >= 2 nodes) as ordered lists of qualified names. */
export function detectCycles(
  kindFilter: string | null,
  cwdOverride?: string,
): string[][] {
  return withShard<string[][]>(
    "graph",
    (db) => {
      // Build adjacency map from edges.
      const where = kindFilter ? `WHERE kind = ?` : "";
      const rows = (
        kindFilter
          ? db.prepare(
              `SELECT source_qualified AS s, target_qualified AS t FROM edges ${where}`,
            ).all(kindFilter)
          : db.prepare(
              `SELECT source_qualified AS s, target_qualified AS t FROM edges`,
            ).all()
      ) as Array<{ s: string; t: string }>;

      const adj = new Map<string, string[]>();
      const allNodes = new Set<string>();
      for (const r of rows) {
        let list = adj.get(r.s);
        if (!list) {
          list = [];
          adj.set(r.s, list);
        }
        list.push(r.t);
        allNodes.add(r.s);
        allNodes.add(r.t);
      }

      // Iterative Tarjan.
      let index = 0;
      const indices = new Map<string, number>();
      const lowlink = new Map<string, number>();
      const onStack = new Set<string>();
      const stack: string[] = [];
      const out: string[][] = [];

      const strongconnect = (v0: string): void => {
        // Iterative DFS with a work stack.
        const work: Array<{ v: string; i: number }> = [{ v: v0, i: 0 }];
        indices.set(v0, index);
        lowlink.set(v0, index);
        index++;
        stack.push(v0);
        onStack.add(v0);

        while (work.length > 0) {
          const frame = work[work.length - 1];
          if (!frame) break;
          const successors = adj.get(frame.v) ?? [];
          if (frame.i < successors.length) {
            const w = successors[frame.i];
            frame.i++;
            if (w == null) continue;
            if (!indices.has(w)) {
              indices.set(w, index);
              lowlink.set(w, index);
              index++;
              stack.push(w);
              onStack.add(w);
              work.push({ v: w, i: 0 });
            } else if (onStack.has(w)) {
              const cur = lowlink.get(frame.v);
              const wIdx = indices.get(w);
              if (cur != null && wIdx != null) {
                lowlink.set(frame.v, Math.min(cur, wIdx));
              }
            }
          } else {
            // Done with this vertex — emit SCC if root.
            if (lowlink.get(frame.v) === indices.get(frame.v)) {
              const comp: string[] = [];
              while (true) {
                const w = stack.pop();
                if (w == null) break;
                onStack.delete(w);
                comp.push(w);
                if (w === frame.v) break;
              }
              if (comp.length >= 2) out.push(comp);
            }
            work.pop();
            if (work.length > 0) {
              const parent = work[work.length - 1];
              if (parent) {
                const parentLow = lowlink.get(parent.v);
                const frameLow = lowlink.get(frame.v);
                if (parentLow != null && frameLow != null) {
                  lowlink.set(parent.v, Math.min(parentLow, frameLow));
                }
              }
            }
          }
        }
      };

      for (const v of allNodes) {
        if (!indices.has(v)) strongconnect(v);
      }
      return out;
    },
    [],
    cwdOverride,
  );
}

/** BFS over `imports` edges to collect forward + reverse dependencies. */
export function dependencyChain(
  file: string,
  direction: "forward" | "reverse" | "both",
  cwdOverride?: string,
): { forward: string[]; reverse: string[] } {
  return withShard<{ forward: string[]; reverse: string[] }>(
    "graph",
    (db) => {
      const fwd = new Set<string>();
      const rev = new Set<string>();

      // Forward = files that `file` imports (transitively).
      if (direction === "forward" || direction === "both") {
        const stmt = db.prepare(
          `SELECT DISTINCT e.target_qualified AS tgt, n.file_path AS file
           FROM edges e
           LEFT JOIN nodes n ON n.qualified_name = e.target_qualified
           WHERE e.kind IN ('imports', 'import') AND e.file_path = ?`,
        );
        let frontier: string[] = [file];
        for (let d = 0; d < 10 && frontier.length > 0; d++) {
          const next: string[] = [];
          for (const f of frontier) {
            const rows = stmt.all(f) as Array<{
              tgt: string;
              file: string | null;
            }>;
            for (const r of rows) {
              const t = r.file ?? r.tgt;
              if (t && !fwd.has(t) && t !== file) {
                fwd.add(t);
                next.push(t);
              }
            }
          }
          frontier = next;
        }
      }

      // Reverse = files that import anything in `file`.
      if (direction === "reverse" || direction === "both") {
        const stmt = db.prepare(
          `SELECT DISTINCT e.file_path AS file
           FROM edges e
           LEFT JOIN nodes n ON n.qualified_name = e.target_qualified
           WHERE e.kind IN ('imports', 'import')
             AND n.file_path = ?`,
        );
        let frontier: string[] = [file];
        for (let d = 0; d < 10 && frontier.length > 0; d++) {
          const next: string[] = [];
          for (const f of frontier) {
            const rows = stmt.all(f) as Array<{ file: string | null }>;
            for (const r of rows) {
              if (r.file && !rev.has(r.file) && r.file !== file) {
                rev.add(r.file);
                next.push(r.file);
              }
            }
          }
          frontier = next;
        }
      }

      return { forward: Array.from(fwd), reverse: Array.from(rev) };
    },
    { forward: [], reverse: [] },
    cwdOverride,
  );
}

/** All references to a symbol: edges WHERE target_qualified = ?. */
export function findReferences(
  symbol: string,
  cwdOverride?: string,
): Array<{
  file: string;
  line: number;
  kind: string;
  source: string;
  context: string;
}> {
  return withShard<
    Array<{
      file: string;
      line: number;
      kind: string;
      source: string;
      context: string;
    }>
  >(
    "graph",
    (db) => {
      const rows = db
        .prepare(
          `SELECT e.source_qualified AS source,
                  e.kind               AS kind,
                  COALESCE(e.file_path, n.file_path) AS file,
                  COALESCE(e.line, n.line_start)      AS line,
                  n.signature          AS signature
           FROM edges e
           LEFT JOIN nodes n ON n.qualified_name = e.source_qualified
           WHERE e.target_qualified = ?
           ORDER BY e.kind, file
           LIMIT 500`,
        )
        .all(symbol) as Array<{
        source: string;
        kind: string;
        file: string | null;
        line: number | null;
        signature: string | null;
      }>;

      // Definitions = node rows where qualified_name = symbol.
      const defRows = db
        .prepare(
          `SELECT file_path AS file, line_start AS line, signature
           FROM nodes WHERE qualified_name = ?`,
        )
        .all(symbol) as Array<{
        file: string | null;
        line: number | null;
        signature: string | null;
      }>;

      const defs = defRows.map((d) => ({
        file: d.file ?? "",
        line: d.line ?? 0,
        kind: "definition",
        source: symbol,
        context: d.signature ?? symbol,
      }));

      const usages = rows.map((r) => ({
        file: r.file ?? "",
        line: r.line ?? 0,
        kind: r.kind,
        source: r.source,
        context: r.signature ?? r.source,
      }));

      return [...defs, ...usages];
    },
    [],
    cwdOverride,
  );
}

// -- tasks shard (single-step lookup) --------------------------------------

/** Lookup one step row by step_id. */
export function singleStep(
  stepId: string,
  cwdOverride?: string,
): {
  step_id: string;
  parent_step_id: string | null;
  session_id: string;
  description: string;
  acceptance_cmd: string | null;
  acceptance_check: string;
  status: string;
  started_at: string | null;
  completed_at: string | null;
  verification_proof: string | null;
  artifacts: string;
  notes: string;
  blocker: string | null;
  drift_score: number;
} | null {
  return withShard<{
    step_id: string;
    parent_step_id: string | null;
    session_id: string;
    description: string;
    acceptance_cmd: string | null;
    acceptance_check: string;
    status: string;
    started_at: string | null;
    completed_at: string | null;
    verification_proof: string | null;
    artifacts: string;
    notes: string;
    blocker: string | null;
    drift_score: number;
  } | null>(
    "tasks",
    (db) => {
      const r = db
        .prepare(
          `SELECT step_id, parent_step_id, session_id, description,
                  acceptance_cmd, acceptance_check, status, started_at,
                  completed_at, verification_proof, artifacts, notes,
                  blocker, drift_score
           FROM steps WHERE step_id = ? LIMIT 1`,
        )
        .get(stepId) as
        | {
            step_id: string;
            parent_step_id: string | null;
            session_id: string;
            description: string;
            acceptance_cmd: string | null;
            acceptance_check: string;
            status: string;
            started_at: string | null;
            completed_at: string | null;
            verification_proof: string | null;
            artifacts: string;
            notes: string;
            blocker: string | null;
            drift_score: number;
          }
        | undefined;
      return r ?? null;
    },
    null,
    cwdOverride,
  );
}

// -- snapshots (filesystem) ------------------------------------------------

/** List available snapshots from the project's snapshot dir. Returns
 *  [] when missing. Each snapshot is a sibling directory whose name is
 *  the timestamp id. */
export function listSnapshotsFs(cwdOverride?: string): Array<{
  id: string;
  path: string;
  bytes: number;
  captured_at: string;
}> {
  const root = resolveShardRoot(cwdOverride);
  if (!root) return [];
  const snapDir = join(root, "snapshots");
  if (!existsSync(snapDir)) return [];
  try {
    const { readdirSync, statSync } = require("node:fs") as typeof import("node:fs");
    const entries = readdirSync(snapDir);
    const out: Array<{
      id: string;
      path: string;
      bytes: number;
      captured_at: string;
    }> = [];
    for (const name of entries) {
      const p = join(snapDir, name);
      let bytes = 0;
      try {
        const st = statSync(p);
        if (!st.isDirectory()) continue;
        for (const sub of readdirSync(p)) {
          try {
            const subst = statSync(join(p, sub));
            if (subst.isFile()) bytes += subst.size;
          } catch {
            // skip
          }
        }
        out.push({
          id: name,
          path: p,
          bytes,
          captured_at: st.mtime.toISOString(),
        });
      } catch {
        // skip
      }
    }
    out.sort((a, b) => b.id.localeCompare(a.id));
    return out;
  } catch {
    return [];
  }
}

/** Return absolute path to a snapshot's <layer>.db file, or null if missing. */
export function snapshotLayerPath(
  snapshotId: string,
  layer: string,
  cwdOverride?: string,
): string | null {
  const root = resolveShardRoot(cwdOverride);
  if (!root) return null;
  const p = join(root, "snapshots", snapshotId, `${layer}.db`);
  return existsSync(p) ? p : null;
}

/** Open a snapshot's layer DB read-only (returns null if missing). */
export function openSnapshotShard(
  snapshotId: string,
  layer: string,
  cwdOverride?: string,
): Database | null {
  const p = snapshotLayerPath(snapshotId, layer, cwdOverride);
  if (!p) return null;
  try {
    return new Database(p, { readonly: true });
  } catch {
    return null;
  }
}

// -- memory shard ----------------------------------------------------------

export function activeConstraints(
  scope: "global" | "project" | "file",
  file: string | undefined,
  limit: number = 50,
  cwdOverride?: string,
): Array<{
  id: number;
  scope: string;
  rule_id: string;
  rule: string;
  why: string;
  how_to_apply: string;
  applies_to: string;
  source: string | null;
  created_at: string;
}> {
  return withShard<
    Array<{
      id: number;
      scope: string;
      rule_id: string;
      rule: string;
      why: string;
      how_to_apply: string;
      applies_to: string;
      source: string | null;
      created_at: string;
    }>
  >(
    "memory",
    (db) => {
      // Scope hierarchy: global ⊂ project ⊂ file. "project" scope returns
      // global + project; "file" scope returns all three, and additionally
      // client-side filters file-scope rows whose applies_to contains `file`.
      let allowed: string[];
      if (scope === "global") allowed = ["global"];
      else if (scope === "project") allowed = ["global", "project"];
      else allowed = ["global", "project", "file"];

      const placeholders = allowed.map(() => "?").join(",");
      const rows = db
        .prepare(
          `SELECT id, scope, rule_id, rule, why, how_to_apply, applies_to,
                  source, created_at
           FROM constraints
           WHERE scope IN (${placeholders})
           ORDER BY created_at DESC
           LIMIT ?`,
        )
        .all(...allowed, limit) as Array<{
        id: number;
        scope: string;
        rule_id: string;
        rule: string;
        why: string;
        how_to_apply: string;
        applies_to: string;
        source: string | null;
        created_at: string;
      }>;

      if (scope === "file" && file) {
        return rows.filter((r) => {
          if (r.scope !== "file") return true;
          try {
            const globs = JSON.parse(r.applies_to) as unknown;
            if (!Array.isArray(globs)) return true;
            return globs.some((g) => {
              if (typeof g !== "string") return false;
              // very light glob — contains or suffix match
              if (g === "*") return true;
              if (g.startsWith("*.") && file.endsWith(g.slice(1))) return true;
              return file.includes(g.replace(/\*/g, ""));
            });
          } catch {
            return true;
          }
        });
      }
      return rows;
    },
    [],
    cwdOverride,
  );
}

// -- doctor: cross-shard health sweep --------------------------------------

export interface ShardHealth {
  layer: string;
  exists: boolean;
  path: string | null;
  row_counts: Record<string, number>;
  integrity_ok: boolean;
  error: string | null;
}

const DOCTOR_SHARDS: Array<{ layer: string; tables: string[] }> = [
  { layer: "graph", tables: ["nodes", "edges", "files"] },
  { layer: "history", tables: ["turns", "decisions"] },
  { layer: "tasks", tables: ["steps", "ledger_entries"] },
  { layer: "findings", tables: ["findings"] },
  { layer: "memory", tables: ["constraints"] },
  { layer: "semantic", tables: ["embeddings", "concepts", "communities"] },
];

export function doctorShardSweep(cwdOverride?: string): ShardHealth[] {
  const out: ShardHealth[] = [];
  for (const s of DOCTOR_SHARDS) {
    const p = shardDbPath(s.layer, cwdOverride);
    if (!p) {
      out.push({
        layer: s.layer,
        exists: false,
        path: null,
        row_counts: {},
        integrity_ok: false,
        error: "shard not yet created (run `mneme build .`)",
      });
      continue;
    }
    const db = tryOpenShard(s.layer, cwdOverride);
    if (!db) {
      out.push({
        layer: s.layer,
        exists: true,
        path: p,
        row_counts: {},
        integrity_ok: false,
        error: "could not open shard read-only",
      });
      continue;
    }
    try {
      const row_counts: Record<string, number> = {};
      for (const t of s.tables) {
        try {
          const r = db.prepare(`SELECT COUNT(*) AS c FROM ${t}`).get() as
            | { c: number }
            | undefined;
          row_counts[t] = r?.c ?? 0;
        } catch {
          row_counts[t] = -1;
        }
      }
      let integrity_ok = false;
      try {
        const ic = db.prepare("PRAGMA integrity_check").get() as
          | { integrity_check: string }
          | undefined;
        integrity_ok = ic?.integrity_check === "ok";
      } catch {
        integrity_ok = false;
      }
      out.push({
        layer: s.layer,
        exists: true,
        path: p,
        row_counts,
        integrity_ok,
        error: null,
      });
    } finally {
      try {
        db.close();
      } catch {
        // ignore
      }
    }
  }
  return out;
}

// --- doctor.ts helpers ---------------------------------------------------
// Used by mcp/src/tools/doctor.ts. Do not remove without updating that tool.

/**
 * Read the current schema_version from each shard. Returns one entry per
 * shard the sweep knows about; `version` is null when the shard is missing,
 * when the table doesn't exist yet, or when the read fails. Never throws —
 * the doctor tool should surface failures as individual checks, not as
 * an exception out of the whole probe.
 */
export function shardSchemaVersions(
  cwdOverride?: string,
): Array<{ layer: string; version: number | null; error: string | null }> {
  const out: Array<{ layer: string; version: number | null; error: string | null }> = [];
  for (const s of DOCTOR_SHARDS) {
    const db = tryOpenShard(s.layer, cwdOverride);
    if (!db) {
      out.push({ layer: s.layer, version: null, error: "shard not open" });
      continue;
    }
    try {
      let version: number | null = null;
      let error: string | null = null;
      try {
        const row = db
          .prepare(
            `SELECT version FROM schema_version ORDER BY version DESC LIMIT 1`,
          )
          .get() as { version: number } | undefined;
        if (row && typeof row.version === "number") {
          version = row.version;
        } else {
          error = "schema_version row missing";
        }
      } catch (err) {
        error = (err as Error).message;
      }
      out.push({ layer: s.layer, version, error });
    } finally {
      try {
        db.close();
      } catch {
        // ignore
      }
    }
  }
  return out;
}

// --- god_nodes.ts helpers ------------------------------------------------
// Used by mcp/src/tools/god_nodes.ts. Do not remove without updating that tool.

/**
 * Bulk-lookup community_id from the `semantic` shard's `community_membership`
 * table for a set of node qualified_names. Returns a map of qualified_name →
 * community_id. Missing shard, missing table, or missing rows all resolve
 * to an empty map — never throws. god_nodes falls back to `null` per node.
 */
export function nodeCommunityIds(
  qualifiedNames: string[],
  cwdOverride?: string,
): Record<string, number> {
  if (qualifiedNames.length === 0) return {};
  return withShard<Record<string, number>>(
    "semantic",
    (db) => {
      const placeholders = qualifiedNames.map(() => "?").join(",");
      const rows = db
        .prepare(
          `SELECT node_qualified AS q, community_id AS c
           FROM community_membership
           WHERE node_qualified IN (${placeholders})`,
        )
        .all(...qualifiedNames) as Array<{ q: string; c: number }>;
      const map: Record<string, number> = {};
      for (const r of rows) {
        map[r.q] = r.c;
      }
      return map;
    },
    {},
    cwdOverride,
  );
}

// --- drift_findings helpers ---------------------------------------------------
// Used by mcp/src/tools/drift_findings.ts. Exposes the full `findings` row
// shape (including column_start, created_at, resolved_at) that the tool maps
// onto its extended output schema. Kept separate from `driftFindings` so the
// existing callers (which use only the narrow shape) are not broken.
//
// Schema reference — mirrors `scanners/src/findings_writer.rs` exactly:
//   id          INTEGER PRIMARY KEY
//   rule_id     TEXT          scanner.rule
//   scanner     TEXT          derived from rule_id prefix
//   severity    TEXT          "info"|"low"|"medium"|"high"|"critical"
//   file        TEXT          absolute file path
//   line_start  INTEGER
//   line_end    INTEGER
//   column_start INTEGER
//   column_end   INTEGER
//   message     TEXT
//   suggestion  TEXT NULL
//   auto_fixable INTEGER (0|1)
//   created_at  TEXT          RFC3339 (first_seen)
//   resolved_at TEXT NULL     RFC3339 (set when rule stops firing — last_seen)

/**
 * Extended drift-findings query: same filters as `driftFindings` but returns
 * every column the scanners layer writes. Also returns the unfiltered total
 * count of currently-open findings so the tool can populate `total_count`
 * without a second round-trip.
 *
 * Filtering:
 *   - severity: exact match on the `severity` column (assumed already
 *     lower-cased and in the allowed enum before being passed in).
 *   - scope: interpreted as a LIKE substring against `file`. The tool layer
 *     passes "project" → undefined (no filter), or an explicit path/segment.
 *   - limit: clamped to 1-500; defaults to 50.
 *
 * Ordering: `created_at DESC` (task spec — "first_seen DESC"), tiebreak by id
 * DESC so the newest autoincrement row wins deterministically on the same
 * timestamp.
 */
export function driftFindingsExtended(
  severity: string | undefined,
  scope: string | undefined,
  limit: number = 50,
  cwdOverride?: string,
): {
  rows: Array<{
    id: number;
    rule_id: string;
    scanner: string;
    severity: string;
    file: string;
    line_start: number;
    line_end: number;
    column_start: number;
    column_end: number;
    message: string;
    suggestion: string | null;
    auto_fixable: number;
    created_at: string;
    resolved_at: string | null;
  }>;
  total_count: number;
} {
  return withShard<{
    rows: Array<{
      id: number;
      rule_id: string;
      scanner: string;
      severity: string;
      file: string;
      line_start: number;
      line_end: number;
      column_start: number;
      column_end: number;
      message: string;
      suggestion: string | null;
      auto_fixable: number;
      created_at: string;
      resolved_at: string | null;
    }>;
    total_count: number;
  }>(
    "findings",
    (db) => {
      const clauses: string[] = ["resolved_at IS NULL"];
      const params: Array<string | number> = [];
      if (severity) {
        clauses.push("severity = ?");
        params.push(severity);
      }
      if (scope) {
        clauses.push("file LIKE ?");
        params.push(`%${scope}%`);
      }
      const where = clauses.join(" AND ");
      // Clamp limit: zod already validated (1..=500) but defensive.
      const lim = Math.max(1, Math.min(500, Math.floor(limit)));

      const rows = db
        .prepare(
          `SELECT id, rule_id, scanner, severity, file,
                  line_start, line_end, column_start, column_end,
                  message, suggestion, auto_fixable,
                  created_at, resolved_at
             FROM findings
            WHERE ${where}
            ORDER BY created_at DESC, id DESC
            LIMIT ?`,
        )
        .all(...params, lim) as Array<{
        id: number;
        rule_id: string;
        scanner: string;
        severity: string;
        file: string;
        line_start: number;
        line_end: number;
        column_start: number;
        column_end: number;
        message: string;
        suggestion: string | null;
        auto_fixable: number;
        created_at: string;
        resolved_at: string | null;
      }>;

      // Unfiltered total of currently-open findings. Explicitly not
      // filtered so the caller can present "showing N of M".
      const totalRow = db
        .prepare(
          `SELECT COUNT(*) AS c FROM findings WHERE resolved_at IS NULL`,
        )
        .get() as { c: number } | undefined;

      return { rows, total_count: totalRow?.c ?? 0 };
    },
    { rows: [], total_count: 0 },
    cwdOverride,
  );
}

// ---------------------------------------------------------------------------
// --- step ledger helpers ---
//
// Read-only shard access for the Step Ledger killer feature (step_status /
// step_resume). Mirrors the Rust `SqliteLedger` reader shape (see
// brain/src/ledger.rs, `LEDGER_INIT_SQL`) but never opens a writable
// connection — the supervisor remains the single writer.
//
// Schema reference (tasks.db::ledger_entries, from ledger.rs):
//   id TEXT PRIMARY KEY
//   session_id TEXT NOT NULL
//   timestamp INTEGER NOT NULL      -- unix millis
//   kind TEXT NOT NULL              -- decision | impl | bug | open_question
//                                   -- | refactor | experiment
//   summary TEXT NOT NULL
//   rationale TEXT
//   touched_files TEXT DEFAULT '[]' -- JSON string[]
//   touched_concepts TEXT DEFAULT '[]'
//   transcript_ref TEXT             -- JSON {session_id, turn_index?, message_id?}
//   kind_payload TEXT NOT NULL      -- JSON { kind: "...", ...details }
//
// Every helper is defensive: returns []/null on missing shard, bad JSON,
// or SQL error — the tools graceful-degrade instead of throwing.
// ---------------------------------------------------------------------------

/** Row shape for `ledger_entries` selects that need JSON side columns. */
export interface LedgerEntryRow {
  id: string;
  session_id: string;
  kind: string;
  summary: string;
  rationale: string | null;
  touched_files: string;
  touched_concepts: string;
  transcript_ref: string | null;
  kind_payload: string;
  timestamp: number;
}

/** Parse a JSON column into a string[]; returns [] on any error. */
export function safeJsonStringArray(raw: string | null | undefined): string[] {
  if (raw == null || raw === "") return [];
  try {
    const v = JSON.parse(raw) as unknown;
    if (!Array.isArray(v)) return [];
    return v.filter((x): x is string => typeof x === "string");
  } catch {
    return [];
  }
}

/** Parse a JSON column into an object; returns null on any error. */
export function safeJsonRecord(
  raw: string | null | undefined,
): Record<string, unknown> | null {
  if (raw == null || raw === "") return null;
  try {
    const v = JSON.parse(raw) as unknown;
    if (v && typeof v === "object" && !Array.isArray(v)) {
      return v as Record<string, unknown>;
    }
    return null;
  } catch {
    return null;
  }
}

/**
 * Ledger entries enriched with JSON side-columns. Feeds step_resume's
 * `transcript_refs`, `touched_files`, and kind-specific payloads.
 */
export function ledgerEntriesWithRefs(
  sessionId: string | undefined,
  sinceMs: number,
  limit: number = 50,
  kinds: string[] = [],
  cwdOverride?: string,
): LedgerEntryRow[] {
  return withShard<LedgerEntryRow[]>(
    "tasks",
    (db) => {
      const clauses: string[] = ["timestamp >= ?"];
      const params: Array<string | number> = [sinceMs];
      if (sessionId) {
        clauses.push("session_id = ?");
        params.push(sessionId);
      }
      if (kinds.length > 0) {
        clauses.push(`kind IN (${kinds.map(() => "?").join(",")})`);
        for (const k of kinds) params.push(k);
      }
      params.push(limit);
      const sql = `SELECT id, session_id, kind, summary, rationale,
                          touched_files, touched_concepts, transcript_ref,
                          kind_payload, timestamp
                   FROM ledger_entries
                   WHERE ${clauses.join(" AND ")}
                   ORDER BY timestamp DESC
                   LIMIT ?`;
      return db.prepare(sql).all(...params) as LedgerEntryRow[];
    },
    [],
    cwdOverride,
  );
}

/**
 * Best-effort "what is the session's goal?" resolver.
 *
 * Resolution order:
 *   1. Root step (`parent_step_id IS NULL`) description — the plan's anchor
 *      when `step_plan_from` seeded the session.
 *   2. Most recent `decision` ledger entry summary — decisions establish
 *      intent after a pivot.
 *   3. Most recent entry summary of any kind.
 *   4. `null` when the ledger is empty.
 */
export function goalForSession(
  sessionId: string,
  cwdOverride?: string,
): string | null {
  return withShard<string | null>(
    "tasks",
    (db) => {
      const root = db
        .prepare(
          `SELECT description FROM steps
           WHERE session_id = ? AND parent_step_id IS NULL
           ORDER BY step_id ASC LIMIT 1`,
        )
        .get(sessionId) as { description: string } | undefined;
      if (root?.description) return root.description;

      const decision = db
        .prepare(
          `SELECT summary FROM ledger_entries
           WHERE session_id = ? AND kind = 'decision'
           ORDER BY timestamp DESC LIMIT 1`,
        )
        .get(sessionId) as { summary: string } | undefined;
      if (decision?.summary) return decision.summary;

      const any = db
        .prepare(
          `SELECT summary FROM ledger_entries
           WHERE session_id = ?
           ORDER BY timestamp DESC LIMIT 1`,
        )
        .get(sessionId) as { summary: string } | undefined;
      return any?.summary ?? null;
    },
    null,
    cwdOverride,
  );
}

/**
 * Derive the verification gate for the current step — the
 * `acceptance_cmd` the model is expected to pass before closing the step.
 * Prefers `in_progress` over `blocked`. Returns null when no active step,
 * the step has no `acceptance_cmd`, or the shard is missing.
 */
export function verificationGateForSession(
  sessionId: string,
  cwdOverride?: string,
): string | null {
  return withShard<string | null>(
    "tasks",
    (db) => {
      const row = db
        .prepare(
          `SELECT acceptance_cmd FROM steps
           WHERE session_id = ? AND status IN ('in_progress','blocked')
           ORDER BY CASE status WHEN 'in_progress' THEN 0 ELSE 1 END,
                    step_id ASC
           LIMIT 1`,
        )
        .get(sessionId) as { acceptance_cmd: string | null } | undefined;
      return row?.acceptance_cmd ?? null;
    },
    null,
    cwdOverride,
  );
}
