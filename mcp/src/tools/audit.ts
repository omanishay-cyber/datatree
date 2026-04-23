/**
 * MCP tool: audit
 *
 * Runs all configured scanners over the requested scope (project, file, or
 * uncommitted diff) and returns the union of findings with a summary.
 */

import {
  AuditInput,
  AuditOutput,
  type Finding,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

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
    const findings = await dbQuery
      .raw<Finding[]>("scanner.run_all", {
        scope: input.scope,
        file: input.file,
        scanners: input.scanners,
      })
      .catch(() => [] as Finding[]);

    const bySeverity: Record<string, number> = {};
    const byScanner: Record<string, number> = {};
    for (const f of findings) {
      bySeverity[f.severity] = (bySeverity[f.severity] ?? 0) + 1;
      byScanner[f.scanner] = (byScanner[f.scanner] ?? 0) + 1;
    }

    return {
      findings,
      summary: { total: findings.length, by_severity: bySeverity, by_scanner: byScanner },
    };
  },
};
