/**
 * MCP tool: surprising_connections
 *
 * Returns high-confidence cross-community edges — connections between
 * concepts that live in different communities and would be hard to find by
 * searching either side individually.
 */

import {
  SurprisingConnectionsInput,
  SurprisingConnectionsOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof SurprisingConnectionsInput.parse>,
  ReturnType<typeof SurprisingConnectionsOutput.parse>
> = {
  name: "surprising_connections",
  description:
    "List high-confidence unexpected edges that bridge two different concept communities. Surfaces non-obvious cross-cutting relationships in the corpus.",
  inputSchema: SurprisingConnectionsInput,
  outputSchema: SurprisingConnectionsOutput,
  category: "multimodal",
  async handler(input) {
    const result = await dbQuery
      .raw<ReturnType<typeof SurprisingConnectionsOutput.parse>>(
        "multimodal.surprising_connections",
        { min_confidence: input.min_confidence, limit: input.limit },
      )
      .catch(() => null);
    return result ?? { surprises: [] };
  },
};
