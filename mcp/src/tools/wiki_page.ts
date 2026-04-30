/**
 * MCP tool: wiki_page
 *
 * Return one auto-wiki page's full markdown body by slug. When `version`
 * is omitted the newest version for that slug is returned. Shape matches
 * the `wiki_pages` row on the Wiki shard.
 *
 * v0.2 (phase-c8 wiring): reads `wiki.db::wiki_pages` directly via
 * `bun:sqlite` (store.ts::wikiPageGet). If the wiki shard isn't built yet
 * or the slug is unknown we return a helpful placeholder page telling the
 * caller to run `wiki_generate` first — never throws.
 *
 * Hot-reload safe: no module-level mutable state.
 */

import {
  WikiPageInput,
  WikiPageOutput,
  type ToolDescriptor,
} from "../types.ts";
import { shardDbPath, wikiPageGet } from "../store.ts";

type Input = ReturnType<typeof WikiPageInput.parse>;
type Output = ReturnType<typeof WikiPageOutput.parse>;

/**
 * Convert a free-form topic string into a slug compatible with the
 * `wiki_pages.slug` column produced by `wiki_generate`. Lowercases,
 * collapses non-alphanumerics to `-`, and trims edge dashes.
 */
function slugify(topic: string): string {
  return topic
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function missingPage(slug: string, reason: string): Output {
  return {
    slug,
    title: slug,
    community_id: -1,
    version: 0,
    markdown: `# ${slug}\n\n_${reason}_\n`,
    risk_score: 0,
    generated_at: new Date().toISOString(),
  };
}

export const tool: ToolDescriptor<Input, Output> = {
  name: "wiki_page",
  description:
    "Fetch a single auto-wiki page's markdown by slug (latest version by default). Slugs come from `wiki_generate`.",
  inputSchema: WikiPageInput,
  outputSchema: WikiPageOutput,
  category: "multimodal",
  async handler(input) {
    // Schema guarantees at least one of slug | topic is set; prefer slug
    // and slugify topic when only topic was provided.
    const slug = input.slug ?? slugify(input.topic ?? "");
    if (!slug) {
      return missingPage(
        "",
        "Either `slug` or `topic` is required.",
      );
    }

    if (!shardDbPath("wiki")) {
      return missingPage(
        slug,
        "Wiki shard not yet built. Run `mneme build .` then `wiki_generate` first.",
      );
    }

    const page = wikiPageGet(slug, input.version ?? null);
    if (!page) {
      return missingPage(
        slug,
        "Page not found. Run `wiki_generate` first.",
      );
    }

    return {
      slug: page.slug,
      title: page.title,
      community_id: page.community_id,
      version: page.version,
      markdown: page.markdown,
      risk_score: page.risk_score,
      generated_at: page.generated_at,
    };
  },
};
