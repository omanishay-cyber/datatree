/**
 * MCP tool: audit_theme
 *
 * Runs the theme scanner — finds hardcoded colors and missing dark: variants.
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
  name: "audit_theme",
  description:
    "Theme audit: hardcoded colors, missing dark: variants, raw hex/rgb usage, missing CSS custom property usage. Returns Finding[] keyed to file + line.",
  inputSchema: ScannerInput,
  outputSchema: ScannerOutput,
  category: "drift",
  async handler(input) {
    const t0 = Date.now();
    const findings = await dbQuery
      .raw<Finding[]>("scanner.run_one", {
        scanner: "theme",
        scope: input.scope,
        file: input.file,
      })
      .catch(() => [] as Finding[]);
    return {
      findings,
      scanner: "theme",
      duration_ms: Date.now() - t0,
    };
  },
};
