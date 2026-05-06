// mcp/src/tool-cache.ts
//
// LOW fix (2026-05-06 audit): the v0.4.0 plan (Item #121) and the
// historical comment here called this a "negative cache". That's
// wrong — a negative cache memoises "this lookup found nothing"
// markers (e.g. DNS NXDOMAIN, query-with-zero-results) so repeat
// queries skip the upstream call. This module memoises SUCCESSFUL
// tool outputs and explicitly skips errors ("Errors do NOT get
// cached"), which is the textbook definition of a positive
// (response) cache. The semantics were always correct; only the
// label was wrong. Updated to match what the code actually does.
//
// Positive response cache for MCP tool invocations. Token-savings
// work per the v0.4.0 plan (Item #121): Claude often re-asks the
// same recall / blast / graph question within a few minutes. Each
// repeat burns the same daemon SQL + the same response tokens.
// With this LRU in front of the registry, repeats become free.
//
// Cache semantics:
//   - Keyed on (toolName, deterministic JSON.stringify(args)).
//   - Default TTL 5 minutes — long enough for "five minutes ago I
//     asked X, now I'm asking X again" but short enough that a fresh
//     `mneme build` invalidates by aging out (the build itself is
//     usually >> 5 min on real repos, and even when faster, the next
//     unique query naturally evicts the stale entry).
//   - Hits return the cached value WRAPPED with a `_cache` envelope
//     so the AI knows it's cached and can decide whether to re-query
//     for fresh data ("re-run with skip-cache=true" pattern, future).
//   - Mutating tools bypass the cache entirely. The bypass list is
//     a conservative explicit set rather than a heuristic — any new
//     mutating tool must be added here in the same PR that introduces
//     it (mirrors the bumper-script-as-canonical-index pattern).
//   - Errors do NOT get cached (cache only successes) so transient
//     failures don't pin themselves into the LRU.
//
// Sizing:
//   - Default 256 entries. At ~4 KB average per entry that's ~1 MB
//     resident, negligible vs the daemon's overall footprint.
//   - LRU eviction picks the oldest-accessed entry when full.
//
// Author: Anish Trivedi & Kruti Trivedi. Apache-2.0.

/** Tools whose outputs MUST NOT be cached because they mutate
 *  state, kick async work, or carry timestamps the caller relies on
 *  being fresh. */
const NEVER_CACHE = new Set<string>([
  // --- step ledger writes ---
  "step_complete",
  "step_plan_from",
  "step_resume",
  "step_show", // shows current state -- always wants fresh
  "step_status",
  "step_verify",
  // --- writes the snapshot store ---
  "snapshot",
  // --- triggers async work ---
  "audit",
  "audit_a11y",
  "audit_corpus",
  "audit_perf",
  "audit_security",
  "audit_theme",
  "audit_types",
  "graphify_corpus",
  "wiki_generate",
  "rebuild",
  // --- mutating refactor ---
  "refactor_apply",
  // --- health/identity carry "now" timestamps the caller may compare ---
  "health",
  "doctor",
  // --- conversation / transcripts always evolve ---
  "recall_conversation",
  "recall_todo",
]);

/** Per-entry cache record. */
interface CacheEntry {
  /** Cached tool result. Stored verbatim — no defensive copy. */
  value: unknown;
  /** Wall-clock millis when this entry was inserted. */
  insertedAt: number;
  /** Wall-clock millis of the most recent read (for LRU bump). */
  lastReadAt: number;
  /** TTL in millis applied to THIS entry. Per-entry so future per-tool
   *  overrides can extend or shrink (e.g. file_intent could TTL longer
   *  since intent annotations change rarely). */
  ttlMs: number;
}

/** Default 5-minute TTL. */
export const DEFAULT_TTL_MS = 5 * 60 * 1000;

/** Default LRU capacity. */
export const DEFAULT_CAPACITY = 256;

/** Cache key shape: tool name + canonical-JSON args. */
function cacheKey(toolName: string, args: unknown): string {
  // JSON.stringify with sorted keys so {a:1,b:2} and {b:2,a:1} share
  // a key. Recursion handles nested objects.
  return `${toolName}::${stableStringify(args)}`;
}

function stableStringify(v: unknown): string {
  if (v === null || typeof v !== "object") return JSON.stringify(v);
  if (Array.isArray(v)) {
    return `[${v.map(stableStringify).join(",")}]`;
  }
  const obj = v as Record<string, unknown>;
  const keys = Object.keys(obj).sort();
  return `{${keys.map((k) => JSON.stringify(k) + ":" + stableStringify(obj[k])).join(",")}}`;
}

/** A small LRU with TTL and a hot opt-out path. */
export class ToolCache {
  private readonly entries = new Map<string, CacheEntry>();
  private readonly capacity: number;
  private readonly defaultTtlMs: number;

  constructor(capacity = DEFAULT_CAPACITY, defaultTtlMs = DEFAULT_TTL_MS) {
    this.capacity = capacity;
    this.defaultTtlMs = defaultTtlMs;
  }

  /** Look up a cached value. Returns `undefined` on miss, expired entry,
   *  or any tool on the NEVER_CACHE list. Successful hits are LRU-bumped. */
  get(toolName: string, args: unknown): { value: unknown; ageMs: number } | undefined {
    if (NEVER_CACHE.has(toolName)) return undefined;
    const key = cacheKey(toolName, args);
    const entry = this.entries.get(key);
    if (!entry) return undefined;
    const now = Date.now();
    if (now - entry.insertedAt > entry.ttlMs) {
      // Expired — evict and miss.
      this.entries.delete(key);
      return undefined;
    }
    // LRU bump — re-insert at the tail by deleting + setting.
    this.entries.delete(key);
    entry.lastReadAt = now;
    this.entries.set(key, entry);
    return { value: entry.value, ageMs: now - entry.insertedAt };
  }

  /** Store a tool result. Mutating tools are silently skipped so the
   *  caller doesn't have to branch — the cache is always safe to call. */
  set(toolName: string, args: unknown, value: unknown, ttlMs?: number): void {
    if (NEVER_CACHE.has(toolName)) return;
    const key = cacheKey(toolName, args);
    const now = Date.now();
    const entry: CacheEntry = {
      value,
      insertedAt: now,
      lastReadAt: now,
      ttlMs: ttlMs ?? this.defaultTtlMs,
    };
    // Evict oldest entry when full. Map iteration order is insertion
    // order, so the FIRST key returned by .keys() is the oldest after
    // an LRU bump. .next() is O(1).
    if (this.entries.size >= this.capacity) {
      const oldestKey = this.entries.keys().next().value;
      if (typeof oldestKey === "string") this.entries.delete(oldestKey);
    }
    this.entries.set(key, entry);
  }

  /** For tests + diagnostics. */
  size(): number {
    return this.entries.size;
  }

  /** Drop everything. */
  clear(): void {
    this.entries.clear();
  }
}

/** Wrap a fresh tool result in a cache envelope so the AI client can
 *  see this came from cache and how stale it is. */
export function wrapCachedResult(value: unknown, ageMs: number): unknown {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return {
      ...(value as Record<string, unknown>),
      _cache: { hit: true, age_ms: ageMs },
    };
  }
  // Primitive / array — wrap in an envelope so the marker is always present.
  return { _cache: { hit: true, age_ms: ageMs }, value };
}

// Re-export for tests that want to manipulate the bypass list.
export { NEVER_CACHE };
