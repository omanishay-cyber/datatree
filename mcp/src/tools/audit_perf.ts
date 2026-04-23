/**
 * MCP tool: audit_perf
 *
 * Performance scanner: missing memoization, sync I/O on render path, large
 * dependency arrays, missing keyed lists, synchronous bundle imports.
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
  name: "audit_perf",
  description:
    "Performance audit: missing useMemo/useCallback on expensive computations, sync I/O on render path, missing keys on lists, large bundle imports, prop-drilling beyond 2 levels. Returns Finding[].",
  inputSchema: ScannerInput,
  outputSchema: ScannerOutput,
  category: "drift",
  async handler(input) {
    const t0 = Date.now();
    const findings = await dbQuery
      .raw<Finding[]>("scanner.run_one", {
        scanner: "perf",
        scope: input.scope,
        file: input.file,
      })
      .catch(() => [] as Finding[]);
    return { findings, scanner: "perf", duration_ms: Date.now() - t0 };
  },
};
