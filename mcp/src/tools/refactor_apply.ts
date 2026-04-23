/**
 * MCP tool: refactor_apply
 *
 * Apply a single refactor proposal by id. The Rust supervisor method
 * `refactor.apply` performs the atomic rewrite: it writes a `.bak` file
 * alongside the original, performs a single-shot `fs::write` of the
 * rewritten content, marks the proposal row `applied_at = now()`, and
 * returns a summary diff string plus the backup path.
 *
 * Dry-run mode skips the write and returns the diff only.
 *
 * Hot-reload safe: no module-level mutable state.
 */

import {
  RefactorApplyInput,
  RefactorApplyOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

type Input = ReturnType<typeof RefactorApplyInput.parse>;
type Output = ReturnType<typeof RefactorApplyOutput.parse>;

export const tool: ToolDescriptor<Input, Output> = {
  name: "refactor_apply",
  description:
    "Apply a single refactor proposal by id, rewriting the target file atomically (backup first). Set dry_run=true to preview the diff without touching disk. Returns the backup path and a summary of bytes written.",
  inputSchema: RefactorApplyInput,
  outputSchema: RefactorApplyOutput,
  category: "graph",
  async handler(input) {
    const raw = await dbQuery
      .raw<{
        applied?: boolean;
        backup_path?: string | null;
        diff_summary?: string;
        bytes_written?: number;
      }>("refactor.apply", {
        proposal_id: input.proposal_id,
        dry_run: input.dry_run,
      })
      .catch((err: unknown) => {
        const message = err instanceof Error ? err.message : String(err);
        return {
          applied: false,
          backup_path: null,
          diff_summary: `error: ${message}`,
          bytes_written: 0,
        };
      });

    return {
      proposal_id: input.proposal_id,
      applied: raw.applied ?? false,
      backup_path: raw.backup_path ?? null,
      diff_summary: raw.diff_summary ?? "",
      bytes_written: raw.bytes_written ?? 0,
    };
  },
};
