/**
 * MCP tool: surprising_connections (phase-c9 wired)
 *
 * Returns high-confidence cross-community edges — connections between
 * concepts that live in different communities and would be hard to find
 * by searching either side individually.
 *
 * Write/compute path: supervisor IPC verb
 * `multimodal.surprising_connections`. The Rust `brain::multimodal` crate
 * owns the Leiden-community + edge-weight analytics.
 *
 * Graceful degrade: when IPC is down we fall back to
 * `surprisingPairsFallback` in store.ts which scans `graph.db` edges for
 * pairs whose endpoints map to different `community_membership` rows in
 * `semantic.db` and have low direct-edge counts. Same shape, weaker
 * ranking, non-empty `note`.
 *
 * NOTE: the supervisor in supervisor/src/ipc.rs does NOT yet route
 * `multimodal.surprising_connections` — every call takes the fallback
 * until that verb is added.
 */

import { z } from "zod";
import {
  SurprisingConnectionsInput,
  SurprisingConnectionsOutput,
  Surprise,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";
import { surprisingPairsFallback } from "../store.ts";

// Additive extension — keep the original output shape intact for callers
// that don't know about the c9 `note` field.
const SurprisingConnectionsOutputExtended = SurprisingConnectionsOutput.extend({
  note: z.string().optional(),
});

type Input = ReturnType<typeof SurprisingConnectionsInput.parse>;
type Output = z.infer<typeof SurprisingConnectionsOutputExtended>;

export const tool: ToolDescriptor<Input, Output> = {
  name: "surprising_connections",
  description:
    "List high-confidence unexpected edges that bridge two different concept communities. Surfaces non-obvious cross-cutting relationships in the corpus.",
  inputSchema: SurprisingConnectionsInput,
  outputSchema: SurprisingConnectionsOutputExtended,
  category: "multimodal",
  async handler(input) {
    // ---- Supervisor path --------------------------------------------------
    const result = await dbQuery
      .raw<ReturnType<typeof SurprisingConnectionsOutput.parse>>(
        "multimodal.surprising_connections",
        { min_confidence: input.min_confidence, limit: input.limit },
      )
      .catch(() => null);

    if (result && Array.isArray(result.surprises)) {
      return { surprises: result.surprises, note: "supervisor" };
    }

    // ---- Fallback: cross-community edge scan ------------------------------
    try {
      const rows = surprisingPairsFallback(input.min_confidence, input.limit);
      const surprises: z.infer<typeof Surprise>[] = rows.map((r) => ({
        source: r.source,
        target: r.target,
        relation: r.relation,
        confidence: r.confidence,
        source_community: r.source_community,
        target_community: r.target_community,
        reasoning: r.reasoning,
      }));
      return {
        surprises,
        note:
          surprises.length > 0
            ? "fallback:cross-community-pairs"
            : "fallback:empty",
      };
    } catch {
      return { surprises: [], note: "fallback:empty" };
    }
  },
};
