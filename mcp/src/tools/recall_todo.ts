/**
 * MCP tool: recall_todo
 *
 * Returns open TODO items from the tasks shard, optionally filtered.
 *
 * v0.1 (review P2): reads `tasks.db → ledger_entries` WHERE `kind =
 * 'open_question'` via `bun:sqlite` read-only. "Open" = kind='open_question'
 * that hasn't yet been superseded by a `decision` entry. (The ledger is
 * append-only per `store/src/schema.rs`, so status is inferred from kind.)
 *
 * Filter semantics:
 *   - `status = 'completed'` → no rows (open_question entries are always
 *     "open" by definition; closure creates a new `decision` entry).
 *   - `status = 'all'`       → include completed decisions too.
 *   - `tag`                  → substring match over summary + touched_concepts
 *   - `since`                → RFC3339 → unix millis filter on `timestamp`
 *
 * Graceful degrade: missing tasks shard → `{ todos: [] }`.
 */

import {
  RecallTodoInput,
  RecallTodoOutput,
  type Todo,
  type ToolDescriptor,
} from "../types.ts";
import { openReminders, shardDbPath } from "../store.ts";

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
    if (!shardDbPath("tasks")) {
      return { todos: [] };
    }

    const f = input.filter;
    // "completed" status makes no sense for append-only open_questions.
    if (f.status === "completed") return { todos: [] };

    const rows = openReminders(200, f.tag, f.since);

    const todos: Todo[] = rows.map((r) => {
      let tags: string[] = [];
      try {
        const parsed = JSON.parse(r.touched_concepts) as unknown;
        if (Array.isArray(parsed)) {
          tags = parsed.filter((x): x is string => typeof x === "string");
        }
      } catch {
        // ignore
      }
      return {
        id: r.id,
        text: r.summary,
        status: "open" as const,
        created_at: new Date(r.timestamp).toISOString(),
        completed_at: null,
        source_file: null,
        tags,
      };
    });

    return { todos };
  },
};
