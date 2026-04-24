/**
 * MCP tool: wiki_generate
 *
 * Regenerate the project's knowledge wiki. The supervisor collects every
 * Leiden community, picks god-nodes as entry points, pulls file paths and
 * risk context, and asks `brain::wiki::WikiBuilder` to produce one
 * markdown page per community. Pages are persisted to the Wiki shard
 * (append-only; a new version row per regeneration) and summarised here.
 *
 * `force = true` regenerates even when no upstream graph changes were
 * detected since the last run.
 *
 * v0.2 (phase-c8 wiring): regeneration itself requires the supervisor
 * (brain::wiki runs there). When the daemon is offline we fall back to
 * reading the cached snapshot from `wiki.db::wiki_pages` so callers still
 * see the previously-generated page index. The `duration_ms` for the
 * fallback reflects only the cache read, which is what the user sees.
 *
 * Hot-reload safe: no module-level mutable state.
 */

import {
  WikiGenerateInput,
  WikiGenerateOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";
import { shardDbPath, wikiPagesLatest } from "../store.ts";

type Input = ReturnType<typeof WikiGenerateInput.parse>;
type Output = ReturnType<typeof WikiGenerateOutput.parse>;

export const tool: ToolDescriptor<Input, Output> = {
  name: "wiki_generate",
  description:
    "Regenerate the auto-wiki: one markdown page per Leiden community, anchored by its god-nodes. Returns the slug + community_id + risk_score of every page. Use `wiki_page` to fetch an individual page's markdown body.",
  inputSchema: WikiGenerateInput,
  outputSchema: WikiGenerateOutput,
  category: "multimodal",
  async handler(input) {
    const t0 = Date.now();
    const raw = await dbQuery
      .raw<{
        pages?: Output["pages"];
        total_pages?: number;
      }>("wiki.generate", {
        project: input.project,
        force: input.force,
      })
      .catch(() => null);

    if (raw) {
      const pages = raw.pages ?? [];
      return {
        pages,
        total_pages: raw.total_pages ?? pages.length,
        duration_ms: Date.now() - t0,
      };
    }

    // Cached fallback — daemon offline or no regeneration available.
    if (!shardDbPath("wiki")) {
      return { pages: [], total_pages: 0, duration_ms: Date.now() - t0 };
    }
    const cached = wikiPagesLatest();
    const pages = cached.map((p) => ({
      slug: p.slug,
      title: p.title,
      community_id: p.community_id,
      risk_score: p.risk_score,
      file_count: p.file_count,
      entry_point_count: p.entry_point_count,
    }));
    return {
      pages,
      total_pages: pages.length,
      duration_ms: Date.now() - t0,
    };
  },
};
