/**
 * MCP tool: drift_findings
 *
 * Returns the current open drift findings (rule violations actively present
 * in the working tree).
 */

import {
  DriftFindingsInput,
  DriftFindingsOutput,
  type Finding,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

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
    const clauses: string[] = ["resolved_at IS NULL"];
    const params: unknown[] = [];
    if (input.severity) {
      clauses.push("severity = ?");
      params.push(input.severity);
    }
    if (input.scope) {
      clauses.push("(file LIKE ?)");
      params.push(`%${input.scope}%`);
    }
    const where = `${clauses.join(" AND ")} ORDER BY severity DESC, detected_at DESC LIMIT ?`;
    params.push(input.limit);

    const findings = await dbQuery
      .select<Finding>("findings", where, params)
      .catch(() => [] as Finding[]);

    return { findings };
  },
};
