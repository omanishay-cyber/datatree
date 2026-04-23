/**
 * MCP tool: recall_todo
 *
 * Returns open TaskCreate items from the tasks shard, optionally filtered.
 */

import {
  RecallTodoInput,
  RecallTodoOutput,
  type Todo,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof RecallTodoInput.parse>,
  ReturnType<typeof RecallTodoOutput.parse>
> = {
  name: "recall_todo",
  description:
    "Get open TaskCreate items from this project's task shard, optionally filtered by status, tag, or since-date. Returns Todo[]. Use at session start and after compaction to know what is unfinished.",
  inputSchema: RecallTodoInput,
  outputSchema: RecallTodoOutput,
  category: "recall",
  async handler(input) {
    const f = input.filter;
    const clauses: string[] = [];
    const params: unknown[] = [];
    if (f.status === "open") clauses.push("status = 'open'");
    else if (f.status === "completed") clauses.push("status = 'completed'");
    if (f.tag) {
      clauses.push("tags LIKE ?");
      params.push(`%${f.tag}%`);
    }
    if (f.since) {
      clauses.push("created_at >= ?");
      params.push(f.since);
    }
    const where = `${
      clauses.length ? clauses.join(" AND ") : "1=1"
    } ORDER BY created_at DESC LIMIT 200`;

    const todos = await dbQuery
      .select<Todo>("tasks", where, params)
      .catch(() => [] as Todo[]);

    return { todos };
  },
};
