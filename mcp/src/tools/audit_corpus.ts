/**
 * MCP tool: audit_corpus
 *
 * Generates a GRAPH_REPORT.md style report covering god nodes, surprises,
 * suggested questions, and audit warnings.
 */

import {
  AuditCorpusInput,
  AuditCorpusOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof AuditCorpusInput.parse>,
  ReturnType<typeof AuditCorpusOutput.parse>
> = {
  name: "audit_corpus",
  description:
    "Generate a GRAPH_REPORT.md style report covering god nodes, surprising connections, suggested questions, and quality warnings (orphan nodes, low cohesion communities, etc.).",
  inputSchema: AuditCorpusInput,
  outputSchema: AuditCorpusOutput,
  category: "multimodal",
  async handler(input) {
    const result = await dbQuery
      .raw<ReturnType<typeof AuditCorpusOutput.parse>>(
        "multimodal.audit_corpus",
        { path: input.path },
      )
      .catch(() => null);
    return (
      result ?? {
        report_markdown: "# GRAPH_REPORT\n\n(Empty corpus — run graphify_corpus first.)\n",
        report_path: "",
        warnings: ["Corpus shard empty or unavailable."],
      }
    );
  },
};
