/**
 * MCP tool: mneme_why
 *
 * F6 — the Why-Chain. Given a natural-language "why did we pick X?"
 * question, returns:
 *
 *   - decisions + refactors from the Step Ledger,
 *   - git commits whose message matches the question (via `git log --grep`),
 *   - related concept ids referenced by those entries.
 *
 * The formatted blob is suitable for direct inclusion in the assistant's
 * reply. Output is deterministic (no model call) so it's cheap and
 * quotable.
 */

import { Database } from "bun:sqlite";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { homedir } from "node:os";
import { join } from "node:path";

import {
  MnemeWhyInput,
  MnemeWhyOutput,
  type LedgerEntry,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof MnemeWhyInput.parse>,
  ReturnType<typeof MnemeWhyOutput.parse>
> = {
  name: "mneme_why",
  description:
    "Why-Chain (F6): decision trace for a natural-language question. " +
    "Blends the Step Ledger (decisions + refactors + rationale + rejected alternatives), " +
    "`git log --grep`, and the concept graph to explain 'why did we pick X?'.",
  inputSchema: MnemeWhyInput,
  outputSchema: MnemeWhyOutput,
  category: "recall",
  async handler(input, ctx) {
    // 1) Ledger — prefer supervisor IPC, fall back to local read.
    let decisions: LedgerEntry[] = [];
    try {
      decisions = await dbQuery.raw<LedgerEntry[]>("ledger.recall", {
        text: input.question,
        kinds: ["decision", "refactor"],
        limit: input.limit,
      });
    } catch {
      decisions = localRecall(input.question, input.limit, ctx.cwd);
    }

    // 2) git log --grep — best-effort; empty on failure.
    const git_commits = gitLogGrep(ctx.cwd, input.question, 5);

    // 3) Related concepts — union of concept ids mentioned by the ledger hits.
    const related_concepts = Array.from(
      new Set(decisions.flatMap((d) => d.touched_concepts ?? [])),
    ).sort();

    return {
      question: input.question,
      decisions,
      git_commits,
      related_concepts,
      formatted: formatWhy({
        question: input.question,
        decisions,
        git_commits,
        related_concepts,
      }),
    };
  },
};

// ---------------------------------------------------------------------------
// Local helpers
// ---------------------------------------------------------------------------

function localRecall(query: string, limit: number, cwd: string): LedgerEntry[] {
  const dbPath = tasksDbPath(cwd);
  if (!dbPath) return [];
  let db: Database;
  try {
    db = new Database(dbPath, { readonly: true });
  } catch {
    return [];
  }
  try {
    const like = `%${query.replace(/[%_]/g, "")}%`;
    const sql =
      "SELECT id, session_id, timestamp, kind, summary, rationale, " +
      "touched_files, touched_concepts, transcript_ref, kind_payload " +
      "FROM ledger_entries WHERE kind IN ('decision','refactor') " +
      "AND (summary LIKE ? OR rationale LIKE ?) ORDER BY timestamp DESC LIMIT ?";
    return db
      .query<Record<string, unknown>, [string, string, number]>(sql)
      .all(like, like, limit)
      .map(rowToEntry);
  } finally {
    db.close();
  }
}

function gitLogGrep(
  cwd: string,
  query: string,
  limit: number,
): Array<{ sha: string; date: string; subject: string }> {
  try {
    const result = spawnSync(
      "git",
      [
        "-C",
        cwd,
        "log",
        `--grep=${query}`,
        `-n${limit}`,
        "--pretty=format:%H\t%ad\t%s",
        "--date=short",
      ],
      { encoding: "utf8", timeout: 5000 },
    );
    if (result.status !== 0 || typeof result.stdout !== "string") return [];
    const out: Array<{ sha: string; date: string; subject: string }> = [];
    for (const line of result.stdout.split("\n")) {
      const [sha, date, ...rest] = line.split("\t");
      if (sha && date) {
        out.push({ sha, date, subject: rest.join("\t") });
      }
    }
    return out;
  } catch {
    return [];
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

function formatWhy(args: {
  question: string;
  decisions: LedgerEntry[];
  git_commits: Array<{ sha: string; date: string; subject: string }>;
  related_concepts: string[];
}): string {
  const lines: string[] = [];
  lines.push(`# why: ${args.question}`);
  lines.push("");
  if (args.decisions.length === 0 && args.git_commits.length === 0) {
    lines.push("_no matching decisions, refactors, or commits found._");
    return lines.join("\n");
  }
  if (args.decisions.length > 0) {
    lines.push("## decisions from the step ledger");
    for (const d of args.decisions) {
      lines.push("");
      lines.push(`### ${d.id.slice(0, 12)}`);
      lines.push(`- summary: ${d.summary}`);
      lines.push(`- when: ${d.timestamp}`);
      if (d.rationale) lines.push(`- rationale: ${d.rationale}`);
      const payload = (d.kind_payload ?? {}) as Record<string, unknown>;
      if (d.kind === "decision" && payload.chosen) {
        lines.push(`- chosen: ${String(payload.chosen)}`);
        const rejected = payload.rejected;
        if (Array.isArray(rejected) && rejected.length > 0) {
          lines.push(`- rejected: ${rejected.map(String).join(", ")}`);
        }
      }
      if (d.kind === "refactor") {
        if (payload.before) lines.push(`- before: ${String(payload.before)}`);
        if (payload.after) lines.push(`- after: ${String(payload.after)}`);
      }
      if (d.touched_files.length > 0) {
        lines.push(`- files: ${d.touched_files.join(", ")}`);
      }
    }
  }
  if (args.git_commits.length > 0) {
    lines.push("");
    lines.push("## git commits mentioning the query");
    for (const c of args.git_commits) {
      lines.push(`- \`${c.sha.slice(0, 10)}\` (${c.date}) ${c.subject}`);
    }
  }
  if (args.related_concepts.length > 0) {
    lines.push("");
    lines.push("## related concepts");
    for (const c of args.related_concepts) {
      lines.push(`- ${c}`);
    }
  }
  return lines.join("\n");
}
