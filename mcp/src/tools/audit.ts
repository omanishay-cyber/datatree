/**
 * MCP tool: audit
 *
 * Union of all scanner findings present in the working tree, optionally
 * filtered by scope (project | file | diff) and the list of scanners to
 * include.
 *
 * v0.1 (review P2): reads `findings.db → findings` via `bun:sqlite`
 * read-only. Query shape:
 *   WHERE resolved_at IS NULL
 *     [AND scanner IN (?,?,..)]
 *     [AND file = ?]         // scope='file'
 *   ORDER BY created_at DESC
 *   LIMIT N
 *
 * Summary counts are computed client-side from the returned rows so they
 * always match what the caller sees.
 *
 * Graceful degrade: missing findings shard → `{ findings: [], summary: … }`
 * with zeroed counts. Never throws.
 */

import {
  AuditInput,
  AuditOutput,
  SeverityEnum,
  type Finding,
  type Severity,
  type ToolDescriptor,
} from "../types.ts";
import { scannerFindings, shardDbPath } from "../store.ts";

function coerceSeverity(s: string): Severity {
  const parsed = SeverityEnum.safeParse(s);
  return parsed.success ? parsed.data : "info";
}

export const tool: ToolDescriptor<
  ReturnType<typeof AuditInput.parse>,
  ReturnType<typeof AuditOutput.parse>
> = {
  name: "audit",
  description:
    "Run all enabled scanners (theme, types, security, a11y, perf, ipc, ...) over the chosen scope. Returns Finding[] with a summary by severity and scanner. Use before commit and after major refactors.",
  inputSchema: AuditInput,
  outputSchema: AuditOutput,
  category: "drift",
  async handler(input) {
    if (!shardDbPath("findings")) {
      return {
        findings: [],
        summary: { total: 0, by_severity: {}, by_scanner: {} },
      };
    }

    const rows = scannerFindings(
      input.scanners,
      undefined,
      input.scope === "file" ? input.file : undefined,
      500,
    );

    const findings: Finding[] = rows.map((r) => ({
      id: String(r.id),
      scanner: r.scanner,
      severity: coerceSeverity(r.severity),
      file: r.file,
      line: r.line_start ?? null,
      rule: r.rule_id,
      message: r.message,
      suggestion: r.suggestion,
      detected_at: r.created_at,
    }));

    const bySeverity: Record<string, number> = {};
    const byScanner: Record<string, number> = {};
    for (const f of findings) {
      bySeverity[f.severity] = (bySeverity[f.severity] ?? 0) + 1;
      byScanner[f.scanner] = (byScanner[f.scanner] ?? 0) + 1;
    }

    return {
      findings,
      summary: {
        total: findings.length,
        by_severity: bySeverity,
        by_scanner: byScanner,
      },
    };
  },
};
