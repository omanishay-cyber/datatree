/**
 * MCP tool: audit_corpus
 *
 * Project-wide scanner report: counts of open findings bucketed by scanner
 * × severity, plus a short markdown rollup suitable for pasting into a PR
 * description.
 *
 * v0.1 (review P2): reads `findings.db → findings` via `bun:sqlite`. Query
 * shape: `SELECT scanner, severity, COUNT(*) FROM findings WHERE
 * resolved_at IS NULL GROUP BY scanner, severity`. Missing shard degrades
 * to an empty report with a warning.
 */

import {
  AuditCorpusInput,
  AuditCorpusOutput,
  type ToolDescriptor,
} from "../types.ts";
import { findingsCorpusStats, shardDbPath } from "../store.ts";

function renderReport(stats: ReturnType<typeof findingsCorpusStats>): string {
  const lines: string[] = [];
  lines.push("# GRAPH_REPORT");
  lines.push("");
  lines.push(`Total open findings: **${stats.total}**`);
  lines.push("");

  if (Object.keys(stats.by_severity).length > 0) {
    lines.push("## By severity");
    for (const sev of ["critical", "high", "medium", "low", "info"]) {
      const n = stats.by_severity[sev] ?? 0;
      if (n > 0) lines.push(`- ${sev}: ${n}`);
    }
    lines.push("");
  }

  if (Object.keys(stats.by_scanner).length > 0) {
    lines.push("## By scanner");
    const scanners = Object.keys(stats.by_scanner).sort();
    for (const s of scanners) {
      const sev = stats.by_scanner_severity[s] ?? {};
      const breakdown = Object.entries(sev)
        .map(([k, v]) => `${k}=${v}`)
        .join(", ");
      lines.push(`- **${s}** (${stats.by_scanner[s]}): ${breakdown}`);
    }
    lines.push("");
  }

  if (stats.total === 0) {
    lines.push("_No open findings._");
  }
  return lines.join("\n") + "\n";
}

export const tool: ToolDescriptor<
  ReturnType<typeof AuditCorpusInput.parse>,
  ReturnType<typeof AuditCorpusOutput.parse>
> = {
  name: "audit_corpus",
  description:
    "Generate a GRAPH_REPORT.md style report covering god nodes, surprising connections, suggested questions, and quality warnings (orphan nodes, low cohesion communities, etc.).",
  inputSchema: AuditCorpusInput,
  outputSchema: AuditCorpusOutput,
  category: "multimodal",
  async handler() {
    if (!shardDbPath("findings")) {
      return {
        report_markdown:
          "# GRAPH_REPORT\n\n(Findings shard not yet created — run `mneme build .`)\n",
        report_path: "",
        warnings: ["Findings shard not yet created."],
      };
    }
    const stats = findingsCorpusStats();
    const warnings: string[] = [];
    if ((stats.by_severity["critical"] ?? 0) > 0) {
      warnings.push(
        `${stats.by_severity["critical"]} critical findings open — address before merge.`,
      );
    }
    if (stats.total === 0) {
      warnings.push("No findings present — graphify may not have run yet.");
    }
    return {
      report_markdown: renderReport(stats),
      report_path: "",
      warnings,
    };
  },
};
