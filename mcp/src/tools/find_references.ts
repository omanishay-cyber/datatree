/**
 * MCP tool: find_references
 *
 * All references (definition + callers + imports + uses) to a symbol.
 *
 * v0.1 (review P2): reads `graph.db` via `bun:sqlite` read-only. Query
 * shape:
 *   - definition: SELECT file_path, line_start FROM nodes WHERE qualified_name=?
 *   - usages:     SELECT source_qualified, kind, file_path, line FROM edges
 *                 WHERE target_qualified = ?
 * Kinds are mapped: 'calls' → 'call', 'imports' → 'import', others → 'usage'.
 * Missing shard → `{ symbol, hits: [] }`.
 */

import {
  FindReferencesInput,
  FindReferencesOutput,
  type ToolDescriptor,
} from "../types.ts";
import { findReferences, shardDbPath } from "../store.ts";

type HitKind = "definition" | "call" | "import" | "usage";

function mapKind(k: string): HitKind {
  if (k === "definition") return "definition";
  if (k === "calls" || k === "call") return "call";
  if (k === "imports" || k === "import") return "import";
  return "usage";
}

export const tool: ToolDescriptor<
  ReturnType<typeof FindReferencesInput.parse>,
  ReturnType<typeof FindReferencesOutput.parse>
> = {
  name: "find_references",
  description:
    "Find all references to a symbol across the project (or workspace). Returns ReferenceHit[] with file, line, column, kind, and surrounding context. Use INSTEAD of Grep when you want structural certainty.",
  inputSchema: FindReferencesInput,
  outputSchema: FindReferencesOutput,
  category: "graph",
  async handler(input) {
    if (!shardDbPath("graph")) {
      return { symbol: input.symbol, hits: [] };
    }
    const rows = findReferences(input.symbol);
    const hits = rows.map((r) => ({
      file: r.file,
      line: r.line,
      column: 0,
      context: r.context,
      kind: mapKind(r.kind),
    }));
    return { symbol: input.symbol, hits };
  },
};
