/**
 * MCP tool: audit_a11y
 *
 * Accessibility scanner view: aria-label gaps, missing alt text, contrast
 * failures, raw <button>/<input> usage.
 *
 * v0.1 (review P2): reads `findings.db → findings` WHERE scanner='a11y' via
 * `bun:sqlite` read-only. Missing shard → `{ findings: [] }`.
 */

import {
  ScannerInput,
  ScannerOutput,
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
  ReturnType<typeof ScannerInput.parse>,
  ReturnType<typeof ScannerOutput.parse>
> = {
  name: "audit_a11y",
  description:
    "Accessibility audit: missing aria-labels on icon-only buttons, missing alt text, color contrast failures, missing focus rings, raw <button>/<input> usage. Returns Finding[].",
  inputSchema: ScannerInput,
  outputSchema: ScannerOutput,
  category: "drift",
  async handler(input) {
    const t0 = Date.now();
    if (!shardDbPath("findings")) {
      return { findings: [], scanner: "a11y", duration_ms: Date.now() - t0 };
    }
    const rows = scannerFindings(
      ["a11y"],
      undefined,
      input.scope === "file" ? input.file : undefined,
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
    return { findings, scanner: "a11y", duration_ms: Date.now() - t0 };
  },
};
