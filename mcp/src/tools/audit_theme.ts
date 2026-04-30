/**
 * MCP tool: audit_theme
 *
 * Theme scanner view: hardcoded colors, missing dark: variants, raw hex/rgb.
 *
 * v0.1 (review P2): reads `findings.db → findings` WHERE scanner='theme' via
 * `bun:sqlite` read-only. Scope ('project' | 'file' | 'diff') → optional
 * `file = ?` filter. Graceful degrade: missing shard → `{ findings: [] }`.
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
  name: "audit_theme",
  description:
    "Theme audit: hardcoded colors, missing dark: variants, raw hex/rgb usage, missing CSS custom property usage. Returns Finding[] keyed to file + line.",
  inputSchema: ScannerInput,
  outputSchema: ScannerOutput,
  category: "drift",
  async handler(input) {
    const t0 = Date.now();
    if (!shardDbPath("findings")) {
      return { findings: [], scanner: "theme", duration_ms: Date.now() - t0 };
    }
    const rows = scannerFindings(
      ["theme"],
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
    return { findings, scanner: "theme", duration_ms: Date.now() - t0 };
  },
};
