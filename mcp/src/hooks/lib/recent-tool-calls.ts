/**
 * recent-tool-calls.ts — query tool_cache.db for blast_radius / file_intent
 * recency within a configurable freshness window.
 *
 * Used by Layer 2 (pretool-edit-write.ts) to decide whether to block an
 * Edit/Write because no blast_radius was run for the target file recently.
 *
 * Architecture note: all DB reads go through the IPC query layer (db.ts).
 * This module NEVER opens SQLite directly. It is a pure query helper with
 * zero side-effects — callers may call it freely.
 *
 * Fail-open guarantee: every exported function returns a safe default
 * (false / empty) on IPC failure. A broken mneme daemon must never block
 * an Edit call.
 */

import { query } from "../../db.ts";
import { homedir } from "node:os";
import { join } from "node:path";
import { readFileSync } from "node:fs";

// ---------------------------------------------------------------------------
// Config resolution — reads [hooks] section from ~/.mneme/config.toml.
// The config is read once at module load and cached for the process lifetime
// (hooks are short-lived single-shot processes, so stale config is fine).
// ---------------------------------------------------------------------------

interface HooksConfig {
  enforce_blast_radius_before_edit: boolean;
  enforce_recall_before_grep: boolean;
  inject_user_prompt_reminder: boolean;
  blast_radius_freshness_seconds: number;
}

const DEFAULT_HOOKS_CONFIG: HooksConfig = {
  enforce_blast_radius_before_edit: true,
  enforce_recall_before_grep: false,
  inject_user_prompt_reminder: true,
  blast_radius_freshness_seconds: 600,
} as const;

/**
 * Parse a raw value as a boolean. Accepts true/false literals, "true"/"false"
 * strings, 0/1 integers. Returns the fallback for anything else.
 */
function parseBool(raw: unknown, fallback: boolean): boolean {
  if (raw === true || raw === "true" || raw === 1) return true;
  if (raw === false || raw === "false" || raw === 0) return false;
  return fallback;
}

/**
 * Parse a raw value as a positive integer. Returns fallback for anything
 * that is not a finite positive number.
 */
function parsePositiveInt(raw: unknown, fallback: number): number {
  const n = Number(raw);
  if (Number.isFinite(n) && n > 0) return Math.floor(n);
  return fallback;
}

/**
 * Load the [hooks] section from ~/.mneme/config.toml.
 *
 * We parse the TOML manually using regex rather than pulling in a TOML
 * dependency: the section is small, flat, and well-specified. Full TOML
 * parsing is overkill and would break the frozen lockfile constraint.
 *
 * Returns DEFAULT_HOOKS_CONFIG on any read/parse error (fail-open).
 */
function loadHooksConfig(): HooksConfig {
  // Allow MNEME_HOME override (mirrors PathManager::default_root in Rust).
  const mnemeHome = process.env["MNEME_HOME"] ?? join(homedir(), ".mneme");
  const configPath = join(mnemeHome, "config.toml");

  let text: string;
  try {
    text = readFileSync(configPath, "utf8");
  } catch {
    // File absent on fresh install — use defaults.
    return { ...DEFAULT_HOOKS_CONFIG };
  }

  // Extract the [hooks] section: everything between "[hooks]" and the next
  // "[" section header (or end of file). Note: \z is not valid in JS regex;
  // use `$` (non-multiline, matches end of string) combined with `\n\[` for
  // the next-section lookahead so both cases are handled correctly.
  const hooksMatch = /\[hooks\]([\s\S]*?)(?=\n\[|$)/.exec(text);
  if (!hooksMatch || !hooksMatch[1]) {
    return { ...DEFAULT_HOOKS_CONFIG };
  }

  const section = hooksMatch[1];
  const cfg = { ...DEFAULT_HOOKS_CONFIG };

  // Parse key = value lines. We only care about the four documented keys.
  const keyValue = /^\s*(\w+)\s*=\s*(.+?)\s*$/gm;
  let m: RegExpExecArray | null;
  while ((m = keyValue.exec(section)) !== null) {
    const key = m[1] as string;
    const raw = m[2] as string;
    // Strip inline comments and surrounding quotes.
    const val: string = raw.replace(/#.*$/, "").trim().replace(/^["']|["']$/g, "");

    switch (key) {
      case "enforce_blast_radius_before_edit":
        cfg.enforce_blast_radius_before_edit = parseBool(
          val === "true" ? true : val === "false" ? false : val,
          DEFAULT_HOOKS_CONFIG.enforce_blast_radius_before_edit,
        );
        break;
      case "enforce_recall_before_grep":
        cfg.enforce_recall_before_grep = parseBool(
          val === "true" ? true : val === "false" ? false : val,
          DEFAULT_HOOKS_CONFIG.enforce_recall_before_grep,
        );
        break;
      case "inject_user_prompt_reminder":
        cfg.inject_user_prompt_reminder = parseBool(
          val === "true" ? true : val === "false" ? false : val,
          DEFAULT_HOOKS_CONFIG.inject_user_prompt_reminder,
        );
        break;
      case "blast_radius_freshness_seconds":
        cfg.blast_radius_freshness_seconds = parsePositiveInt(
          val,
          DEFAULT_HOOKS_CONFIG.blast_radius_freshness_seconds,
        );
        break;
    }
  }

  return cfg;
}

// Singleton — loaded once per hook process invocation.
let _cachedConfig: HooksConfig | null = null;

export function getHooksConfig(): HooksConfig {
  if (_cachedConfig === null) {
    _cachedConfig = loadHooksConfig();
  }
  return _cachedConfig;
}

/**
 * Exposed for tests: reset the singleton so tests can inject different configs
 * by manipulating MNEME_HOME before calling getHooksConfig().
 */
export function _resetHooksConfigCache(): void {
  _cachedConfig = null;
}

// ---------------------------------------------------------------------------
// Tool-call recency queries
// ---------------------------------------------------------------------------

/**
 * Result of a recency check.
 *
 * The `found` field uses a three-state discriminant:
 *   true    — a qualifying call was found within the freshness window.
 *   false   — no qualifying call was found (genuine cache miss).
 *   'error' — the DB query failed (IPC down, daemon absent, etc.).
 *             Callers MUST treat 'error' as fail-open: allow the action
 *             through rather than blocking it. A broken mneme daemon must
 *             never prevent the user from editing.
 */
export interface RecencyResult {
  found: boolean | "error";
  /**
   * ISO-8601 timestamp of the most recent qualifying call. Null when not
   * found, or when the DB query failed.
   */
  lastCalledAt: string | null;
  /** How many seconds ago the qualifying call happened. Null when not found. */
  secondsAgo: number | null;
}

/**
 * Check whether `blast_radius` or `file_intent` was called for `filePath`
 * within the last `freshnessSeconds` seconds in `sessionId`.
 *
 * The tool_cache.db schema stores rows with:
 *   tool        TEXT  — tool name (e.g. "blast_radius")
 *   params_json TEXT  — JSON of the input params
 *   cached_at   TEXT  — ISO-8601 timestamp
 *   session_id  TEXT
 *
 * We look for any row whose tool is blast_radius / file_intent, whose
 * params_json contains the file path string, and whose cached_at is within
 * the window. The JSON containment check is intentionally loose (LIKE %path%)
 * to handle both absolute and relative path representations.
 *
 * Fail-open: on any IPC error this returns { found: false, ... } so the
 * caller can pass through the edit.
 */
export async function wasBlastRadiusRunFor(
  filePath: string,
  sessionId: string,
  freshnessSeconds?: number,
): Promise<RecencyResult> {
  const cfg = getHooksConfig();
  const windowSeconds = freshnessSeconds ?? cfg.blast_radius_freshness_seconds;
  const windowMs = windowSeconds * 1000;
  const cutoffIso = new Date(Date.now() - windowMs).toISOString();

  try {
    type CacheRow = { tool: string; params_json: string; cached_at: string };
    const rows = await query.select<CacheRow>(
      "tool_cache",
      `tool IN ('blast_radius', 'file_intent')
       AND params_json LIKE ?
       AND cached_at >= ?
       AND session_id = ?
       ORDER BY cached_at DESC
       LIMIT 1`,
      [`%${filePath}%`, cutoffIso, sessionId],
    );

    if (rows.length === 0) {
      return { found: false, lastCalledAt: null, secondsAgo: null };
    }

    const row = rows[0];
    if (!row) {
      return { found: false, lastCalledAt: null, secondsAgo: null };
    }

    const calledMs = new Date(row.cached_at).getTime();
    const secondsAgo = Math.floor((Date.now() - calledMs) / 1000);

    return {
      found: true,
      lastCalledAt: row.cached_at,
      secondsAgo,
    };
  } catch (err) {
    // Fail open — IPC down, mneme unhealthy, DB missing. Return 'error' so
    // the caller can distinguish "genuinely not found" from "could not check".
    // Callers that enforce blast_radius MUST treat 'error' as pass-through.
    console.error(
      "[mneme-mcp/recent-tool-calls] blast_radius recency query failed (fail-open):",
      err,
    );
    return { found: "error", lastCalledAt: null, secondsAgo: null };
  }
}

/**
 * Check whether any mneme recall tool was called for a given query/path
 * within the freshness window. Used by Layer 3 (pretool-grep-read) to decide
 * whether to surface a redirect nudge.
 *
 * `searchTerm` is the grep pattern or file path being read. We look for
 * recent mneme_recall / recall_concept / blast_radius calls whose params_json
 * contains the search term.
 *
 * Fail-open: returns { found: false } on error.
 */
export async function wasMnemeRecallRunFor(
  searchTerm: string,
  sessionId: string,
  freshnessSeconds?: number,
): Promise<RecencyResult> {
  const cfg = getHooksConfig();
  const windowSeconds = freshnessSeconds ?? cfg.blast_radius_freshness_seconds;
  const windowMs = windowSeconds * 1000;
  const cutoffIso = new Date(Date.now() - windowMs).toISOString();

  try {
    type CacheRow = { tool: string; params_json: string; cached_at: string };
    const rows = await query.select<CacheRow>(
      "tool_cache",
      `tool IN ('mneme_recall', 'recall_concept', 'blast_radius', 'find_references')
       AND params_json LIKE ?
       AND cached_at >= ?
       AND session_id = ?
       ORDER BY cached_at DESC
       LIMIT 1`,
      [`%${searchTerm}%`, cutoffIso, sessionId],
    );

    if (rows.length === 0) {
      return { found: false, lastCalledAt: null, secondsAgo: null };
    }

    const row = rows[0];
    if (!row) {
      return { found: false, lastCalledAt: null, secondsAgo: null };
    }

    const calledMs = new Date(row.cached_at).getTime();
    const secondsAgo = Math.floor((Date.now() - calledMs) / 1000);

    return {
      found: true,
      lastCalledAt: row.cached_at,
      secondsAgo,
    };
  } catch (err) {
    console.error(
      "[mneme-mcp/recent-tool-calls] recall recency query failed (fail-open):",
      err,
    );
    return { found: "error", lastCalledAt: null, secondsAgo: null };
  }
}

/**
 * Return all mneme tool call trespasses in this session: Grep/Read calls that
 * had no prior mneme lookup within the freshness window. Used by Layer 1 to
 * build the trespass log shown in reminder blocks.
 *
 * A "trespass" = a Grep or Read tool call in tool_cache whose params_json
 * refers to a file/pattern that was NOT preceded by a mneme query within the
 * window.
 *
 * We keep this surface lightweight — max 5 trespasses, most recent first.
 * Returns empty array on any failure (fail-open).
 */
export async function getSessionTrespasses(
  sessionId: string,
  limit = 5,
): Promise<Array<{ tool: string; path: string; calledAt: string }>> {
  try {
    type CacheRow = { tool: string; params_json: string; cached_at: string };
    const rows = await query.select<CacheRow>(
      "tool_cache",
      `tool IN ('Grep', 'Read', 'Glob')
       AND session_id = ?
       ORDER BY cached_at DESC
       LIMIT ?`,
      [sessionId, limit * 3], // over-fetch since we filter below
    );

    const trespasses: Array<{ tool: string; path: string; calledAt: string }> = [];

    for (const row of rows) {
      if (trespasses.length >= limit) break;

      // Extract the file_path or pattern from params_json.
      let pathOrPattern = "";
      try {
        const params = JSON.parse(row.params_json) as Record<string, unknown>;
        const fp = params["file_path"] ?? params["pattern"] ?? params["path"] ?? "";
        pathOrPattern = String(fp);
      } catch {
        continue;
      }

      if (!pathOrPattern) continue;

      // Check if mneme was consulted for this path within the window.
      const mnemeWasUsed = await wasMnemeRecallRunFor(pathOrPattern, sessionId);
      if (!mnemeWasUsed.found) {
        trespasses.push({
          tool: row.tool,
          path: pathOrPattern,
          calledAt: row.cached_at,
        });
      }
    }

    return trespasses;
  } catch (err) {
    console.error(
      "[mneme-mcp/recent-tool-calls] trespass query failed (fail-open):",
      err,
    );
    return [];
  }
}
