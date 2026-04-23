/**
 * MCP tool: mneme_recall
 *
 * F1 — semantic + keyword recall against the persistent Step Ledger.
 * Returns the top-K entries matching the free-form query, optionally
 * filtered by kind and session. Output includes a ready-to-paste markdown
 * string so the model can quote it verbatim without re-formatting.
 *
 * Data path: the tool asks the Rust supervisor over IPC via the
 * `ledger.recall` method (added in F1). If the supervisor is unreachable
 * (offline daemon), the tool falls back to reading the per-project
 * tasks.db directly via `bun:sqlite`. This keeps the tool useful even
 * when the daemon is down — crash-safe by design.
 */

import { Database } from "bun:sqlite";
import { join } from "node:path";
import { homedir } from "node:os";
import { createHash } from "node:crypto";
import {
  MnemeRecallInput,
  MnemeRecallOutput,
  type LedgerEntry,
  type LedgerKind,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof MnemeRecallInput.parse>,
  ReturnType<typeof MnemeRecallOutput.parse>
> = {
  name: "mneme_recall",
  description:
    "Semantic + keyword recall against the persistent Step Ledger (F1). " +
    "Returns past decisions, implementations, bugs, refactors, and open questions " +
    "relevant to the query. Survives context compaction — this is the source of truth " +
    "for 'what did we do last session?'. Filter by kinds (decision|impl|bug|open_question|refactor|experiment).",
  inputSchema: MnemeRecallInput,
  outputSchema: MnemeRecallOutput,
  category: "recall",
  async handler(input, ctx) {
    const sessionId = input.session_id ?? ctx.sessionId;
    const sinceMillis =
      input.since_hours !== undefined
        ? Date.now() - input.since_hours * 3600 * 1000
        : undefined;

    // 1) Preferred path: ask the Rust supervisor via IPC.
    let entries: LedgerEntry[] = [];
    try {
      entries = await dbQuery.raw<LedgerEntry[]>("ledger.recall", {
        text: input.query,
        kinds: input.kinds,
        limit: input.limit,
        since_millis: sinceMillis ?? null,
        session_id: sessionId,
      });
    } catch {
      // 2) Fallback: open tasks.db directly.
      entries = localRecall({
        query: input.query,
        kinds: input.kinds,
        limit: input.limit,
        sinceMillis,
        sessionId,
        cwd: ctx.cwd,
      });
    }

    return {
      entries,
      formatted: formatEntries(input.query, entries),
    };
  },
};

// ---------------------------------------------------------------------------
// Local fallback — reads the tasks.db corresponding to the active project.
// ---------------------------------------------------------------------------

interface LocalRecallArgs {
  query: string;
  kinds: LedgerKind[];
  limit: number;
  sinceMillis: number | undefined;
  sessionId: string;
  cwd: string;
}

function localRecall(args: LocalRecallArgs): LedgerEntry[] {
  const dbPath = tasksDbPath(args.cwd);
  if (!dbPath) return [];
  let db: Database;
  try {
    db = new Database(dbPath, { readonly: true });
  } catch {
    return [];
  }
  try {
    const conds: string[] = ["1=1"];
    const params: unknown[] = [];
    if (args.kinds.length > 0) {
      conds.push(`kind IN (${args.kinds.map(() => "?").join(",")})`);
      params.push(...args.kinds);
    }
    if (args.sinceMillis !== undefined) {
      conds.push("timestamp >= ?");
      params.push(args.sinceMillis);
    }

    const text = args.query.trim();
    if (text.length > 0) {
      // Best-effort FTS; gracefully degrade to a LIKE scan if the virtual
      // table is missing or the match expression blows up.
      try {
        const ftsStmt = db.query<{ id: string }, unknown[]>(
          "SELECT ledger_entries.id AS id FROM ledger_entries_fts " +
            "JOIN ledger_entries ON ledger_entries._rowid_ = ledger_entries_fts.rowid " +
            "WHERE ledger_entries_fts MATCH ?",
        );
        const hitIds = ftsStmt.all(sanitizeFts(text)).map((r) => r.id);
        if (hitIds.length > 0) {
          conds.push(`id IN (${hitIds.map(() => "?").join(",")})`);
          params.push(...hitIds);
        } else {
          conds.push("(summary LIKE ? OR rationale LIKE ?)");
          const like = `%${text.replace(/[%_]/g, "")}%`;
          params.push(like, like);
        }
      } catch {
        conds.push("(summary LIKE ? OR rationale LIKE ?)");
        const like = `%${text.replace(/[%_]/g, "")}%`;
        params.push(like, like);
      }
    }

    const sql =
      "SELECT id, session_id, timestamp, kind, summary, rationale, " +
      "touched_files, touched_concepts, transcript_ref, kind_payload " +
      `FROM ledger_entries WHERE ${conds.join(" AND ")} ` +
      "ORDER BY timestamp DESC LIMIT ?";
    params.push(args.limit);
    const rows = db.query<Record<string, unknown>, unknown[]>(sql).all(...params);
    return rows.map(rowToEntry);
  } finally {
    db.close();
  }
}

/** Resolve the tasks.db path for the cwd's project. */
function tasksDbPath(cwd: string): string | null {
  const home = homedir();
  // ProjectId = first 16 hex chars of sha256(absolute_path) — matches Rust
  // `ProjectId::from_path`. Keep this in sync with common/src/ids.rs.
  const hash = createHash("sha256").update(cwd).digest("hex");
  const projectId = hash.slice(0, 16);
  return join(home, ".mneme", "projects", projectId, "tasks.db");
}

function rowToEntry(row: Record<string, unknown>): LedgerEntry {
  const safeParse = <T>(v: unknown, fallback: T): T => {
    if (typeof v !== "string" || v.length === 0) return fallback;
    try {
      return JSON.parse(v) as T;
    } catch {
      return fallback;
    }
  };
  const tsMillis = Number(row.timestamp ?? 0);
  const kindPayload = safeParse<Record<string, unknown>>(row.kind_payload, {});
  return {
    id: String(row.id ?? ""),
    session_id: String(row.session_id ?? ""),
    timestamp: new Date(tsMillis).toISOString(),
    kind: (row.kind as LedgerKind) ?? "impl",
    summary: String(row.summary ?? ""),
    rationale: row.rationale == null ? null : String(row.rationale),
    touched_files: safeParse<string[]>(row.touched_files, []),
    touched_concepts: safeParse<string[]>(row.touched_concepts, []),
    transcript_ref: safeParse<LedgerEntry["transcript_ref"]>(
      row.transcript_ref,
      null,
    ),
    kind_payload: kindPayload,
  };
}

function sanitizeFts(input: string): string {
  const cleaned = input.replace(/[^a-zA-Z0-9 ]+/g, " ").trim();
  if (cleaned.length === 0) return "*";
  return cleaned
    .split(/\s+/)
    .map((w) => `${w}*`)
    .join(" OR ");
}

// ---------------------------------------------------------------------------
// Formatter
// ---------------------------------------------------------------------------

function formatEntries(query: string, entries: LedgerEntry[]): string {
  if (entries.length === 0) {
    return `# recall: ${query}\n\n_no matching ledger entries._`;
  }
  const lines: string[] = [];
  lines.push(`# recall: ${query}`);
  lines.push("");
  for (const e of entries) {
    lines.push(`## ${e.kind} — ${e.summary}`);
    lines.push(`- id: \`${e.id.slice(0, 12)}\``);
    lines.push(`- when: ${e.timestamp}`);
    if (e.rationale) lines.push(`- rationale: ${e.rationale}`);
    if (e.touched_files.length > 0) {
      lines.push(`- files: ${e.touched_files.join(", ")}`);
    }
    lines.push("");
  }
  return lines.join("\n");
}
