/**
 * MCP tool: refactor_suggest
 *
 * Asks the Rust supervisor to run the refactor scanner across the project
 * (or a single file) and returns the open proposals.
 *
 * The supervisor-side method `refactor.suggest` performs the scan, writes
 * each finding into `refactor_proposals` on the Refactors shard, and
 * returns the list of proposal rows.
 *
 * v0.2 (phase-c8 wiring): when the supervisor is unreachable we fall back
 * to reading cached open proposals (`applied_at IS NULL`) from
 * `refactors.db` directly. This keeps the tool useful for browsing existing
 * proposals even when the daemon is down; a fresh scan still requires the
 * brain crate. `scanned_files` is zero on the cache path because we did
 * not actually run a new scan.
 *
 * Hot-reload safe: no module-level mutable state.
 */

import {
  RefactorSuggestInput,
  RefactorSuggestOutput,
  SeverityEnum,
  type RefactorProposal,
  type Severity,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";
import { refactorProposalsOpen, shardDbPath } from "../store.ts";

type Input = ReturnType<typeof RefactorSuggestInput.parse>;
type Output = ReturnType<typeof RefactorSuggestOutput.parse>;

const ALLOWED_KINDS = new Set<RefactorProposal["kind"]>([
  "unused-import",
  "unreachable-function",
  "unreferenced-type",
  "rename-function",
  "rename-variable",
  "rename-type",
]);

function coerceKind(k: string): RefactorProposal["kind"] | null {
  return (ALLOWED_KINDS as Set<string>).has(k)
    ? (k as RefactorProposal["kind"])
    : null;
}

function coerceSeverity(s: string): Severity {
  const parsed = SeverityEnum.safeParse(s);
  return parsed.success ? parsed.data : "info";
}

export const tool: ToolDescriptor<Input, Output> = {
  name: "refactor_suggest",
  description:
    "Scan the project (or one file) for safe refactor candidates: unused imports, unreachable functions, unreferenced types, and naming-convention rename suggestions. Returns a list of proposals with exact replacement spans so `refactor_apply` can rewrite them atomically.",
  inputSchema: RefactorSuggestInput,
  outputSchema: RefactorSuggestOutput,
  category: "graph",
  async handler(input) {
    const t0 = Date.now();
    const raw = await dbQuery
      .raw<{
        proposals?: Output["proposals"];
        scanned_files?: number;
      }>("refactor.suggest", {
        scope: input.scope,
        file: input.file,
        kinds: input.kinds,
        limit: input.limit,
      })
      .catch(() => null);

    if (raw) {
      const proposals = raw.proposals ?? [];
      const scanned = raw.scanned_files ?? 0;
      return {
        proposals,
        scanned_files: scanned,
        duration_ms: Date.now() - t0,
      };
    }

    // Cached fallback: return currently-open proposals from the shard.
    if (!shardDbPath("refactors")) {
      return { proposals: [], scanned_files: 0, duration_ms: Date.now() - t0 };
    }
    const rows = refactorProposalsOpen(
      input.scope === "file" ? input.file : undefined,
      input.kinds,
      input.limit,
    );
    const proposals: RefactorProposal[] = [];
    for (const r of rows) {
      const kind = coerceKind(r.kind);
      if (!kind) continue; // drop proposals with kinds the schema no longer supports
      proposals.push({
        proposal_id: r.proposal_id,
        kind,
        file: r.file,
        line_start: r.line_start,
        line_end: r.line_end,
        column_start: r.column_start,
        column_end: r.column_end,
        symbol: r.symbol,
        original_text: r.original_text,
        replacement_text: r.replacement_text,
        rationale: r.rationale,
        severity: coerceSeverity(r.severity),
        confidence: Math.max(0, Math.min(1, r.confidence)),
      });
    }
    return {
      proposals,
      scanned_files: 0,
      duration_ms: Date.now() - t0,
    };
  },
};
