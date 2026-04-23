/**
 * MCP tool: blast_radius
 *
 * Computes all callers, dependents, and tests affected by changing a target
 * (file or function). v0.1: reads directly from the project's graph.db
 * shard via bun:sqlite for sub-millisecond queries.
 */

import {
  BlastRadiusInput,
  BlastRadiusOutput,
  type ToolDescriptor,
} from "../types.ts";
import { blastRadius } from "../store.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof BlastRadiusInput.parse>,
  ReturnType<typeof BlastRadiusOutput.parse>
> = {
  name: "blast_radius",
  description:
    "Compute the blast radius of a change: every caller, dependent, and test affected. Pass either a file path or a fully-qualified function name. Use BEFORE Edit/Write on any file to know what else might break.",
  inputSchema: BlastRadiusInput,
  outputSchema: BlastRadiusOutput,
  category: "graph",
  async handler(input) {
    try {
      const rows = blastRadius(input.target, input.depth ?? 2);

      const affected_files: string[] = [];
      const affected_symbols: string[] = [];
      const test_files: string[] = [];
      const critical_paths: string[] = [];

      for (const r of rows) {
        if (r.kind === "file") {
          affected_files.push(r.node);
          if (r.node.includes("test") || r.node.includes("spec")) {
            test_files.push(r.node);
          }
        } else {
          affected_symbols.push(r.node);
          if (r.depth === 1) critical_paths.push(r.node);
        }
      }

      return {
        target: input.target,
        affected_files,
        affected_symbols,
        test_files,
        total_count: rows.length,
        critical_paths,
      };
    } catch (err) {
      // Graceful: if the shard isn't built yet, return an empty radius
      // with a hint in the symbols list.
      return {
        target: input.target,
        affected_files: [],
        affected_symbols: [
          `(datatree not yet built — run mneme build .\` first; ${(err as Error).message})`,
        ],
        test_files: [],
        total_count: 0,
        critical_paths: [],
      };
    }
  },
};
