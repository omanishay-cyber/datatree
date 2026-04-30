/**
 * MCP tool: recall_conversation
 *
 * Searches conversation history (assistant + user turns) across the session
 * (or the whole project history) for messages matching `query`.
 *
 * v0.1 (review P2): reads `history.db → turns` via `bun:sqlite` read-only.
 * Query shape: FTS5 `turns_fts MATCH ?` when the query is plain-word, else
 * LIKE fallback. Optional `session_id` and `since` filters apply on top.
 *
 * Graceful degrade: missing history shard → `{ turns: [] }`.
 */

import {
  RecallConversationInput,
  RecallConversationOutput,
  type ConversationTurn,
  type ToolDescriptor,
} from "../types.ts";
import { searchConversation, shardDbPath } from "../store.ts";

type TurnRole = "user" | "assistant" | "system" | "tool";

function coerceRole(role: string): TurnRole {
  if (role === "user" || role === "assistant" || role === "system" || role === "tool") {
    return role;
  }
  return "system";
}

export const tool: ToolDescriptor<
  ReturnType<typeof RecallConversationInput.parse>,
  ReturnType<typeof RecallConversationOutput.parse>
> = {
  name: "recall_conversation",
  description:
    "Semantic search across conversation history. Returns matching ConversationTurn[] with role, content, and similarity score. Use to recover decisions and reasoning from earlier in long sessions.",
  inputSchema: RecallConversationInput,
  outputSchema: RecallConversationOutput,
  category: "recall",
  async handler(input) {
    if (!shardDbPath("history")) {
      return { turns: [] };
    }

    const rows = searchConversation(
      input.query,
      input.limit,
      input.session_id,
      input.since,
    );

    const turns: ConversationTurn[] = rows.map((r) => ({
      turn_id: String(r.id),
      session_id: r.session_id,
      role: coerceRole(r.role),
      content: r.content,
      tool_calls: [],
      timestamp: r.timestamp,
      similarity: undefined,
    }));

    return { turns };
  },
};
