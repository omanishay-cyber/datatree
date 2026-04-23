/**
 * MCP tool: recall_constraint
 *
 * Returns active constraints (rules) for the current project, file, or global
 * scope. Constraints originate from CLAUDE.md, .claude/rules/, and the
 * project's datatree.json.
 *
 * v0.1 (review P2): reads `memory.db → constraints` via `bun:sqlite`
 * read-only. Query shape: scope-filtered WHERE + client-side glob match over
 * `applies_to` for file-scoped rules. Scope hierarchy expands downward:
 *   - global  → {scope = 'global'}
 *   - project → {scope IN ('global','project')}
 *   - file    → {all three}, plus `applies_to` glob match against `input.file`
 *
 * Severity + enforcement derivation: the schema stores neither explicitly
 * (`store/src/schema.rs`), so we infer severity from the `rule_id` / `source`
 * (anything with "security" or "MUST" → "high", otherwise "medium") and
 * default enforcement to "warn".
 *
 * Graceful degrade: missing memory shard → `{ constraints: [] }`.
 */

import {
  RecallConstraintInput,
  RecallConstraintOutput,
  type Constraint,
  type Severity,
  type ToolDescriptor,
} from "../types.ts";
import { activeConstraints, shardDbPath } from "../store.ts";

function inferSeverity(rule: string, source: string | null): Severity {
  const blob = `${rule} ${source ?? ""}`.toLowerCase();
  if (blob.includes("security") || blob.includes("never") || blob.includes("must not")) {
    return "high";
  }
  if (blob.includes("must") || blob.includes("always")) return "medium";
  return "low";
}

export const tool: ToolDescriptor<
  ReturnType<typeof RecallConstraintInput.parse>,
  ReturnType<typeof RecallConstraintOutput.parse>
> = {
  name: "recall_constraint",
  description:
    "Get active constraints (rules) for the current scope. Constraints are sourced from CLAUDE.md, .claude/rules/, and project datatree.json. Use BEFORE any Edit/Write to check what rules apply to the file you are about to change.",
  inputSchema: RecallConstraintInput,
  outputSchema: RecallConstraintOutput,
  category: "recall",
  async handler(input) {
    if (!shardDbPath("memory")) {
      return { constraints: [] };
    }

    const rows = activeConstraints(input.scope, input.file, 50);

    const constraints: Constraint[] = rows.map((r) => ({
      id: String(r.id),
      rule: r.rule,
      scope: r.scope,
      source: r.source ?? "unknown",
      severity: inferSeverity(r.rule, r.source),
      enforcement: "warn" as const,
    }));

    return { constraints };
  },
};
