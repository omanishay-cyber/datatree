/**
 * MCP tool: refactor_suggest
 *
 * Asks the Rust supervisor to run the refactor scanner across the project
 * (or a single file) and returns the open proposals.
 *
 * The supervisor-side method `refactor.suggest` performs the scan, writes
 * each finding into `refactor_proposals` on the Refactors shard, and
 * returns the list of proposal rows. On error we degrade gracefully to
 * an empty result so the tool never crashes the MCP loop.
 *
 * Hot-reload safe: no module-level mutable state.
 */

import {
  RefactorSuggestInput,
  RefactorSuggestOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

type Input = ReturnType<typeof RefactorSuggestInput.parse>;
type Output = ReturnType<typeof RefactorSuggestOutput.parse>;

export const tool: ToolDescriptor<Input, Output> = {
  name: "refactor_suggest",
  description:
    "Scan the project (or one file) for safe refactor candidates: unused imports, unreachable functions, unreferenced types, and naming-convention rename suggestions. Returns a list of proposals with exact replacement spans so `refactor_apply` can rewrite them atomically.",
  inputSchema: RefactorSuggestInput,
  outputSchema: RefactorSuggestOutput,
  category: "graph",
  async handler(input) {
    const t0 = Date.now();
    const raw = await dbQuery
      .raw<{
        proposals?: Output["proposals"];
        scanned_files?: number;
      }>("refactor.suggest", {
        scope: input.scope,
        file: input.file,
        kinds: input.kinds,
        limit: input.limit,
      })
      .catch(() => null);

    if (!raw) {
      return { proposals: [], scanned_files: 0, duration_ms: Date.now() - t0 };
    }
    const proposals = raw.proposals ?? [];
    const scanned = raw.scanned_files ?? 0;
    return {
      proposals,
      scanned_files: scanned,
      duration_ms: Date.now() - t0,
    };
  },
};
