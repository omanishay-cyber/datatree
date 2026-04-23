/**
 * MCP tool: recall_conversation
 *
 * Searches conversation history (assistant + user turns) across the session
 * (or the whole project history) for messages matching `query`.
 */

import {
  RecallConversationInput,
  RecallConversationOutput,
  type ConversationTurn,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

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
    let turns = await dbQuery
      .semanticSearch<ConversationTurn>("history", input.query, input.limit)
      .catch(() => [] as ConversationTurn[]);

    if (input.session_id) {
      turns = turns.filter((t) => t.session_id === input.session_id);
    }
    if (input.since) {
      turns = turns.filter((t) => t.timestamp >= (input.since as string));
    }

    return { turns };
  },
};
