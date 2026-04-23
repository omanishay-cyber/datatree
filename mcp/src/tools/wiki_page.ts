/**
 * MCP tool: wiki_page
 *
 * Return one auto-wiki page's full markdown body by slug. When `version`
 * is omitted the newest version for that slug is returned. Shape matches
 * the `wiki_pages` row on the Wiki shard.
 *
 * Hot-reload safe: no module-level mutable state.
 */

import {
  WikiPageInput,
  WikiPageOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

type Input = ReturnType<typeof WikiPageInput.parse>;
type Output = ReturnType<typeof WikiPageOutput.parse>;

export const tool: ToolDescriptor<Input, Output> = {
  name: "wiki_page",
  description:
    "Fetch a single auto-wiki page's markdown by slug (latest version by default). Slugs come from `wiki_generate`.",
  inputSchema: WikiPageInput,
  outputSchema: WikiPageOutput,
  category: "multimodal",
  async handler(input) {
    const raw = await dbQuery
      .raw<Partial<Output>>("wiki.get_page", {
        slug: input.slug,
        version: input.version,
      })
      .catch(() => null);

    if (!raw || typeof raw.markdown !== "string") {
      return {
        slug: input.slug,
        title: input.slug,
        community_id: -1,
        version: 0,
        markdown: `# ${input.slug}\n\n_Page not found. Run \`wiki_generate\` first._\n`,
        risk_score: 0,
        generated_at: new Date().toISOString(),
      };
    }
    return {
      slug: raw.slug ?? input.slug,
      title: raw.title ?? input.slug,
      community_id: raw.community_id ?? -1,
      version: raw.version ?? 1,
      markdown: raw.markdown,
      risk_score: raw.risk_score ?? 0,
      generated_at: raw.generated_at ?? new Date().toISOString(),
    };
  },
};
