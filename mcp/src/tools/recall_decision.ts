/**
 * MCP tool: recall_decision
 *
 * Semantically searches the decisions log shard for past architectural and
 * implementation decisions matching the query.
 */

import {
  RecallDecisionInput,
  RecallDecisionOutput,
  type Decision,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof RecallDecisionInput.parse>,
  ReturnType<typeof RecallDecisionOutput.parse>
> = {
  name: "recall_decision",
  description:
    "Search the per-project decisions log semantically. Returns ranked Decision[] (topic, problem, chosen, reasoning, rejected). Use BEFORE making a new architectural choice to avoid contradicting prior decisions.",
  inputSchema: RecallDecisionInput,
  outputSchema: RecallDecisionOutput,
  category: "recall",
  async handler(input) {
    const t0 = Date.now();
    const where = input.since
      ? "timestamp >= ? ORDER BY timestamp DESC LIMIT ?"
      : "1=1 ORDER BY timestamp DESC LIMIT ?";
    const params: unknown[] = input.since
      ? [input.since, input.limit]
      : [input.limit];

    const semantic = await dbQuery
      .semanticSearch<Decision>("decisions", input.query, input.limit)
      .catch(() => [] as Decision[]);

    // Fall back to recency-based selection if semantic search is unavailable.
    const decisions =
      semantic.length > 0
        ? semantic
        : await dbQuery
            .select<Decision>("decisions", where, params)
            .catch(() => [] as Decision[]);

    return {
      decisions,
      query_id: crypto.randomUUID(),
      latency_ms: Date.now() - t0,
    };
  },
};
