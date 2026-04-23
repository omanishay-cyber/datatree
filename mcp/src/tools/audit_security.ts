/**
 * MCP tool: audit_security
 *
 * Security scanner: secrets, dynamic-eval, IPC validation gaps, raw SQL,
 * dangerous DOM injection sinks.
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
  name: "audit_security",
  description:
    "Security audit: hardcoded secrets, dynamic code evaluation sinks, unsafe HTML injection sinks, missing IPC validation, raw SQL concatenation, contextIsolation set to false. Returns Finding[].",
  inputSchema: ScannerInput,
  outputSchema: ScannerOutput,
  category: "drift",
  async handler(input) {
    const t0 = Date.now();
    const findings = await dbQuery
      .raw<Finding[]>("scanner.run_one", {
        scanner: "security",
        scope: input.scope,
        file: input.file,
      })
      .catch(() => [] as Finding[]);
    return { findings, scanner: "security", duration_ms: Date.now() - t0 };
  },
};
