/**
 * MCP tool: drift_findings
 *
 * Returns the current open drift findings (rule violations actively present
 * in the working tree).
 *
 * v0.1 (review P2): reads `findings.db → findings` via `bun:sqlite`
 * read-only. Query shape:
 *   WHERE resolved_at IS NULL
 *     [AND severity = ?]
 *     [AND file LIKE '%scope%']
 *   ORDER BY severity-rank DESC, created_at DESC
 *   LIMIT ?
 *
 * Graceful degrade: missing findings shard → `{ findings: [] }`.
 */

import {
  DriftFindingsInput,
  DriftFindingsOutput,
  type Finding,
  type Severity,
  type ToolDescriptor,
} from "../types.ts";
import { driftFindings, shardDbPath } from "../store.ts";

const ALLOWED_SEVERITY: readonly Severity[] = [
  "info",
  "low",
  "medium",
  "high",
  "critical",
];

function coerceSeverity(s: string): Severity {
  return (ALLOWED_SEVERITY as readonly string[]).includes(s)
    ? (s as Severity)
    : "info";
}

export const tool: ToolDescriptor<
  ReturnType<typeof DriftFindingsInput.parse>,
  ReturnType<typeof DriftFindingsOutput.parse>
> = {
  name: "drift_findings",
  description:
    "Get current open drift findings (rule violations present in the working tree, not yet resolved). Optionally filter by severity or scope.",
  inputSchema: DriftFindingsInput,
  outputSchema: DriftFindingsOutput,
  category: "drift",
  async handler(input) {
    if (!shardDbPath("findings")) {
      return { findings: [] };
    }

    const rows = driftFindings(input.severity, input.scope, input.limit);

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

    return { findings };
  },
};
