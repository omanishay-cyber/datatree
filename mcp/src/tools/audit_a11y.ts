/**
 * MCP tool: audit_a11y
 *
 * Accessibility scanner: missing aria-labels, contrast failures, keyboard
 * traps, missing alt text, focus-ring violations.
 */

import {
  ScannerInput,
  ScannerOutput,
  type Finding,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

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
    const findings = await dbQuery
      .raw<Finding[]>("scanner.run_one", {
        scanner: "a11y",
        scope: input.scope,
        file: input.file,
      })
      .catch(() => [] as Finding[]);
    return { findings, scanner: "a11y", duration_ms: Date.now() - t0 };
  },
};
