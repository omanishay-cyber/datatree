/**
 * MCP tool: recall_decision
 *
 * Searches the per-project decisions log for past architectural and
 * implementation decisions matching the query.
 *
 * v0.1 (review P2): reads `history.db → decisions` via `bun:sqlite` read-only.
 * Query shape: LIKE-match over (topic + problem + chosen + reasoning), with
 * optional `since` timestamp filter, ordered by created_at DESC.
 *
 * Graceful degrade: if the history shard isn't built yet, returns
 * `{ decisions: [], ... }` without throwing.
 */

import {
  RecallDecisionInput,
  RecallDecisionOutput,
  type Decision,
  type ToolDescriptor,
} from "../types.ts";
import { searchDecisions, shardDbPath } from "../store.ts";

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
    const queryId = crypto.randomUUID();

    if (!shardDbPath("history")) {
      return { decisions: [], query_id: queryId, latency_ms: Date.now() - t0 };
    }

    const rows = searchDecisions(input.query, input.limit, input.since);

    const decisions: Decision[] = rows.map((r) => {
      let rejected: string[] = [];
      try {
        const parsed = JSON.parse(r.alternatives) as unknown;
        if (Array.isArray(parsed)) {
          rejected = parsed.filter((x): x is string => typeof x === "string");
        }
      } catch {
        // ignore malformed payload
      }
      return {
        id: String(r.id),
        topic: r.topic,
        problem: r.problem,
        chosen: r.chosen,
        reasoning: r.reasoning,
        rejected,
        timestamp: r.created_at,
        source_file: null,
        confidence: 1,
      };
    });

    return {
      decisions,
      query_id: queryId,
      latency_ms: Date.now() - t0,
    };
  },
};
