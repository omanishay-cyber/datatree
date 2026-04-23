/**
 * MCP tool: mneme_resume
 *
 * F1 — compaction-survivor. Produces the "resume bundle" the model needs
 * to rebuild mental state after context loss: recent decisions, recent
 * implementations, open questions, and a timeline in chronological order.
 *
 * Prefers the Rust supervisor path (`ledger.resume_summary`); falls back
 * to direct `bun:sqlite` reads of the per-project tasks.db so the tool
 * still works when the daemon is down.
 */

import { Database } from "bun:sqlite";
import { join } from "node:path";
import { homedir } from "node:os";
import { createHash } from "node:crypto";
import {
  MnemeResumeInput,
  MnemeResumeOutput,
  type LedgerEntry,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof MnemeResumeInput.parse>,
  ReturnType<typeof MnemeResumeOutput.parse>
> = {
  name: "mneme_resume",
  description:
    "Return the Step Ledger resume bundle: recent decisions, recent implementations, " +
    "open questions, and a timeline. Use after compaction or when starting a new session " +
    "to reload context. Survives restarts — this is the source of truth.",
  inputSchema: MnemeResumeInput,
  outputSchema: MnemeResumeOutput,
  category: "recall",
  async handler(input, ctx) {
    const sinceMillis = Date.now() - input.since_hours * 3600 * 1000;
    const sessionId = input.session_id ?? ctx.sessionId;

    type Bundle = {
      session_id: string;
      generated_at: string;
      recent_decisions: LedgerEntry[];
      recent_implementations: LedgerEntry[];
      open_questions: LedgerEntry[];
      timeline: LedgerEntry[];
    };

    let bundle: Bundle | null = null;
    try {
      bundle = await dbQuery.raw<Bundle>("ledger.resume_summary", {
        since_millis: sinceMillis,
        session_id: sessionId,
      });
    } catch {
      bundle = null;
    }

    if (!bundle) {
      bundle = localResume(sinceMillis, ctx.cwd);
    }

    return {
      ...bundle,
      formatted: formatBundle(bundle),
    };
  },
};

// ---------------------------------------------------------------------------
// Local fallback
// ---------------------------------------------------------------------------

function localResume(
  sinceMillis: number,
  cwd: string,
): {
  session_id: string;
  generated_at: string;
  recent_decisions: LedgerEntry[];
  recent_implementations: LedgerEntry[];
  open_questions: LedgerEntry[];
  timeline: LedgerEntry[];
} {
  const emptyBundle = {
    session_id: "",
    generated_at: new Date().toISOString(),
    recent_decisions: [],
    recent_implementations: [],
    open_questions: [],
    timeline: [],
  };
  const dbPath = tasksDbPath(cwd);
  if (!dbPath) return emptyBundle;
  let db: Database;
  try {
    db = new Database(dbPath, { readonly: true });
  } catch {
    return emptyBundle;
  }
  try {
    const pick = (kinds: string[], limit: number): LedgerEntry[] => {
      const sql =
        "SELECT id, session_id, timestamp, kind, summary, rationale, " +
        "touched_files, touched_concepts, transcript_ref, kind_payload " +
        `FROM ledger_entries WHERE timestamp >= ? ` +
        (kinds.length > 0 ? `AND kind IN (${kinds.map(() => "?").join(",")}) ` : "") +
        "ORDER BY timestamp DESC LIMIT ?";
      const params: unknown[] = [sinceMillis, ...kinds, limit];
      return db
        .query<Record<string, unknown>, unknown[]>(sql)
        .all(...params)
        .map(rowToEntry);
    };
    const timeline = pick([], 50).reverse();
    const recent_decisions = pick(["decision"], 10);
    const recent_implementations = pick(["impl", "refactor"], 10);
    const open_questions = db
      .query<Record<string, unknown>, unknown[]>(
        "SELECT id, session_id, timestamp, kind, summary, rationale, " +
          "touched_files, touched_concepts, transcript_ref, kind_payload " +
          "FROM ledger_entries WHERE kind = 'open_question' ORDER BY timestamp DESC LIMIT 50",
      )
      .all()
      .map(rowToEntry)
      .filter((e) => {
        const payload = (e.kind_payload ?? {}) as Record<string, unknown>;
        return payload.resolved_by == null;
      });
    const session_id = timeline[timeline.length - 1]?.session_id ?? "";
    return {
      session_id,
      generated_at: new Date().toISOString(),
      recent_decisions,
      recent_implementations,
      open_questions,
      timeline,
    };
  } finally {
    db.close();
  }
}

function tasksDbPath(cwd: string): string | null {
  const home = homedir();
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
  return {
    id: String(row.id ?? ""),
    session_id: String(row.session_id ?? ""),
    timestamp: new Date(tsMillis).toISOString(),
    kind: (row.kind as LedgerEntry["kind"]) ?? "impl",
    summary: String(row.summary ?? ""),
    rationale: row.rationale == null ? null : String(row.rationale),
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

function formatBundle(b: {
  session_id: string;
  generated_at: string;
  recent_decisions: LedgerEntry[];
  recent_implementations: LedgerEntry[];
  open_questions: LedgerEntry[];
  timeline: LedgerEntry[];
}): string {
  const lines: string[] = [];
  lines.push("# resume bundle");
  lines.push(`- session: ${b.session_id || "(none)"}`);
  lines.push(`- generated_at: ${b.generated_at}`);
  lines.push("");

  const section = (title: string, xs: LedgerEntry[]) => {
    lines.push(`## ${title}`);
    if (xs.length === 0) {
      lines.push("_none_");
      lines.push("");
      return;
    }
    for (const e of xs) {
      lines.push(`- [${e.kind}] ${e.summary} — \`${e.id.slice(0, 12)}\``);
    }
    lines.push("");
  };

  section("recent decisions", b.recent_decisions);
  section("recent implementations / refactors", b.recent_implementations);
  section("open questions", b.open_questions);

  lines.push("## timeline (oldest → newest)");
  if (b.timeline.length === 0) {
    lines.push("_empty_");
  } else {
    for (const e of b.timeline) {
      lines.push(`- ${e.timestamp} [${e.kind}] ${e.summary}`);
    }
  }
  return lines.join("\n");
}
