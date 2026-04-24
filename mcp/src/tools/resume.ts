/**
 * MCP tool: mneme_resume
 *
 * F1 — compaction-survivor. Produces the "resume bundle" the model needs
 * to rebuild mental state after context loss: recent decisions, recent
 * implementations, open questions, and a timeline in chronological order.
 *
 * Prefers the Rust supervisor path (`ledger.resume_summary`); falls back
 * to the shared store helper `ledgerResumeBundle` which reads the per-
 * project tasks.db directly. The shared helper uses the canonical
 * ProjectId (full SHA-256 of canonical path) — the legacy inline
 * `hash.slice(0,16)` resolver pointed at the wrong directory.
 */
import {
  MnemeResumeInput,
  MnemeResumeOutput,
  type LedgerEntry,
  type LedgerKind,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";
import { ledgerResumeBundle, type LedgerRawRow } from "../store.ts";

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
// Local fallback — via shared store helper
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
  const raw = ledgerResumeBundle(sinceMillis, cwd);
  const recent_decisions = raw.recent_decisions.map(rowToEntry);
  const recent_implementations = raw.recent_implementations.map(rowToEntry);
  const timeline = raw.timeline.map(rowToEntry).reverse(); // oldest → newest
  const open_questions = raw.open_questions.map(rowToEntry).filter((e) => {
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
}

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
