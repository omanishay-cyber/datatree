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
 * tasks.db directly via the shared store helper (which uses the canonical
 * ProjectId = full SHA-256 hash of the canonical project path — matches
 * the Rust path resolver; the legacy inline 16-char slice was wrong).
 */

import {
  MnemeRecallInput,
  MnemeRecallOutput,
  type LedgerEntry,
  type LedgerKind,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";
import { ledgerRecall, type LedgerRawRow } from "../store.ts";

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
      // 2) Fallback: read tasks.db directly via the shared store helper.
      const rows = ledgerRecall(
        {
          query: input.query,
          kinds: input.kinds,
          limit: input.limit,
          ...(sinceMillis !== undefined ? { sinceMillis } : {}),
          ...(sessionId !== undefined ? { sessionId } : {}),
        },
        ctx.cwd,
      );
      entries = rows.map(rowToEntry);
    }

    return {
      entries,
      formatted: formatEntries(input.query, entries),
    };
  },
};

// ---------------------------------------------------------------------------
// Row mapping
// ---------------------------------------------------------------------------

function rowToEntry(row: LedgerRawRow): LedgerEntry {
  const safeParse = <T>(v: string | null | undefined, fallback: T): T => {
    if (v == null || v.length === 0) return fallback;
    try {
      return JSON.parse(v) as T;
    } catch {
      return fallback;
    }
  };
  return {
    id: row.id,
    session_id: row.session_id,
    timestamp: new Date(row.timestamp).toISOString(),
    kind: (row.kind as LedgerKind) ?? "impl",
    summary: row.summary,
    rationale: row.rationale,
    touched_files: safeParse<string[]>(row.touched_files, []),
    touched_concepts: safeParse<string[]>(row.touched_concepts, []),
    transcript_ref: safeParse<LedgerEntry["transcript_ref"]>(
      row.transcript_ref,
      null,
    ),
    kind_payload: safeParse<Record<string, unknown>>(row.kind_payload, {}),
  };
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
