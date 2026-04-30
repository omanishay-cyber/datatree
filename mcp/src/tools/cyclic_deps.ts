/**
 * MCP tool: cyclic_deps
 *
 * Detects circular dependency chains.
 *
 * v0.1 (review P2): reads `graph.db → edges` via `bun:sqlite` read-only
 * and runs an in-process iterative Tarjan SCC. We scope to `kind IN
 * ('imports', 'import')` so only true module-level cycles surface (not
 * mutual recursion through calls). Missing shard → `{ cycles: [],
 * count: 0 }`.
 */

import {
  CyclicDepsInput,
  CyclicDepsOutput,
  type ToolDescriptor,
} from "../types.ts";
import { detectCycles, shardDbPath } from "../store.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof CyclicDepsInput.parse>,
  ReturnType<typeof CyclicDepsOutput.parse>
> = {
  name: "cyclic_deps",
  description:
    "Detect circular dependencies in the project graph. Returns each cycle as an ordered list of file paths. Run after large refactors or before merging a PR that touches imports.",
  inputSchema: CyclicDepsInput,
  outputSchema: CyclicDepsOutput,
  category: "graph",
  async handler() {
    if (!shardDbPath("graph")) {
      return { cycles: [], count: 0 };
    }
    // Use a null kind filter to consider all edges; a typical corpus
    // graph has `imports` edges but older shards used `import`.
    const cyclesImports = detectCycles("imports");
    const cyclesImport = detectCycles("import");
    const cycles = [...cyclesImports, ...cyclesImport];
    return { cycles, count: cycles.length };
  },
};
