/**
 * MCP tool: recall_constraint
 *
 * Returns active constraints (rules) for the current project, file, or global
 * scope. Constraints originate from CLAUDE.md, .claude/rules/, and the
 * project's datatree.json.
 */

import {
  RecallConstraintInput,
  RecallConstraintOutput,
  type Constraint,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

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
    const clauses: string[] = [];
    const params: unknown[] = [];
    if (input.scope === "global") {
      clauses.push("scope = 'global'");
    } else if (input.scope === "project") {
      clauses.push("scope IN ('global','project')");
    } else if (input.scope === "file" && input.file) {
      clauses.push("scope IN ('global','project','file')");
      // file-scope filter is best-effort — server-side glob match
      clauses.push("(scope <> 'file' OR file_glob_matches(?, file_glob))");
      params.push(input.file);
    }

    const whereBody = clauses.length > 0 ? clauses.join(" AND ") : "1=1";
    const where = `${whereBody} ORDER BY severity DESC, id ASC LIMIT 50`;
    const constraints = await dbQuery
      .select<Constraint>("constraints", where, params)
      .catch(() => [] as Constraint[]);

    return { constraints };
  },
};
