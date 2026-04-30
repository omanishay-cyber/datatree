/**
 * MCP tool: mneme_conventions
 *
 * Read path for the Convention Learner (blueprint F3). Returns the top-N
 * inferred project conventions by confidence, so Claude Code (and other
 * harnesses) can cite them before generating code.
 *
 * The learner itself runs inside the Rust `brain` crate during `mneme
 * build`; this tool only reads the append-only `conventions.db` shard.
 */

import { z } from "zod";
import { openShardDb } from "../store.ts";
import type { ToolDescriptor } from "../types.ts";

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

const ConventionOut = z.object({
  id: z.string(),
  kind: z.string(),
  description: z.string(),
  pattern: z.record(z.string(), z.unknown()),
  confidence: z.number().min(0).max(1),
  evidence_count: z.number().int().nonnegative(),
  updated_at: z.number().int().nonnegative(),
});

export const ConventionsInput = z
  .object({
    limit: z.number().int().positive().max(100).default(20),
    min_confidence: z.number().min(0).max(1).default(0.8),
    kind: z
      .enum([
        "naming",
        "import_order",
        "error_handling",
        "test_layout",
        "dependency",
        "component_shape",
      ])
      .optional(),
  })
  .default({});

export const ConventionsOutput = z.object({
  conventions: z.array(ConventionOut),
  total: z.number().int().nonnegative(),
});

type ConventionsInputT = z.infer<typeof ConventionsInput>;
type ConventionsOutputT = z.infer<typeof ConventionsOutput>;

interface Row {
  id: string;
  pattern_kind: string;
  pattern_json: string;
  confidence: number;
  evidence_count: number;
  updated_at: number;
}

function describe(kind: string, pattern: Record<string, unknown>): string {
  switch (kind) {
    case "naming":
      return `${String(pattern.scope ?? "?")} uses ${String(pattern.style ?? "?")}`;
    case "import_order":
      return `import order: ${((pattern.order as string[] | undefined) ?? []).join(" → ")}`;
    case "error_handling":
      return `errors: ${String(pattern.pattern ?? "?")}`;
    case "test_layout": {
      const loc = pattern.colocated === true ? "colocated" : "separate dir";
      return `tests are ${loc} (${String(pattern.naming ?? "?")})`;
    }
    case "dependency":
      return `prefers ${String(pattern.prefers ?? "?")}`;
    case "component_shape":
      return `components: ${String(pattern.prefers ?? "?")}`;
    default:
      return kind;
  }
}

export const tool: ToolDescriptor<ConventionsInputT, ConventionsOutputT> = {
  name: "mneme_conventions",
  description:
    "Return the top inferred project conventions (naming style, import order, test layout, component shape, etc.) ranked by confidence. Use before writing new code to stay consistent with the existing codebase. Produced offline by the Convention Learner during `mneme build`.",
  inputSchema: ConventionsInput,
  outputSchema: ConventionsOutput,
  category: "recall",
  async handler(input, ctx): Promise<ConventionsOutputT> {
    const limit = input.limit ?? 20;
    const minConf = input.min_confidence ?? 0.8;

    let rows: Row[] = [];
    try {
      const db = openShardDb("conventions", ctx.cwd);
      try {
        const params: Array<string | number> = [minConf];
        let sql = `SELECT id, pattern_kind, pattern_json, confidence, evidence_count, updated_at
                   FROM conventions
                   WHERE confidence >= ?`;
        if (input.kind) {
          sql += ` AND pattern_kind = ?`;
          params.push(input.kind);
        }
        sql += ` ORDER BY confidence DESC, evidence_count DESC LIMIT ?`;
        params.push(limit);
        rows = db.prepare(sql).all(...params) as Row[];
      } finally {
        db.close();
      }
    } catch {
      // Shard not built yet — return empty rather than error.
      return { conventions: [], total: 0 };
    }

    const conventions = rows.map((r) => {
      let pattern: Record<string, unknown> = {};
      try {
        const parsed = JSON.parse(r.pattern_json) as unknown;
        if (parsed !== null && typeof parsed === "object") {
          pattern = parsed as Record<string, unknown>;
        }
      } catch {
        pattern = {};
      }
      return {
        id: r.id,
        kind: r.pattern_kind,
        description: describe(r.pattern_kind, pattern),
        pattern,
        confidence: r.confidence,
        evidence_count: r.evidence_count,
        updated_at: r.updated_at,
      };
    });

    return { conventions, total: conventions.length };
  },
};
