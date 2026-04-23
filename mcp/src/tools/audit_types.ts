/**
 * MCP tool: audit_types
 *
 * Type-quality scanner: bare `any`, non-null assertions, default exports,
 * disabled strict-mode, missing return types on exported functions.
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
  name: "audit_types",
  description:
    "Type-safety audit: bare 'any', non-null assertions (!), default exports, disabled strict-mode, missing return types on exported functions. Returns Finding[].",
  inputSchema: ScannerInput,
  outputSchema: ScannerOutput,
  category: "drift",
  async handler(input) {
    const t0 = Date.now();
    const findings = await dbQuery
      .raw<Finding[]>("scanner.run_one", {
        scanner: "types",
        scope: input.scope,
        file: input.file,
      })
      .catch(() => [] as Finding[]);
    return { findings, scanner: "types", duration_ms: Date.now() - t0 };
  },
};
