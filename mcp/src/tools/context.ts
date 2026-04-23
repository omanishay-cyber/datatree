/**
 * F2 — mneme_context
 *
 * Hybrid-retrieval context pack. Fuses BM25 + semantic + 2-hop graph walk
 * in the Rust `brain` crate (see `brain/src/retrieve.rs`) and returns a
 * token-budget-bounded bundle. This tool is the front door for every
 * "give me the minimum set of things the model needs to act on `task`"
 * request — the idea is NOT to dump everything, but to do *rank → rerank →
 * pack*.
 *
 * The heavy lifting lives in Rust; this tool forwards `task`,
 * `budget_tokens`, and `anchors` over the supervisor IPC channel and
 * re-validates the response against the zod schema below.
 *
 * Fallback: if the supervisor isn't reachable (e.g. daemon not started),
 * we synthesise a minimal empty response rather than throwing — the host
 * harness should still be able to proceed without mneme.
 */

import { z } from "zod";
import type { ToolDescriptor } from "../types.ts";
import { query as dbQuery } from "../db.ts";

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

const ContextInput = z.object({
  task: z.string().min(1),
  budget_tokens: z.number().int().positive().max(32_000).default(2_000),
  anchors: z.array(z.string()).default([]),
});

const RetrievalSource = z.enum(["bm25", "semantic", "graph", "reranker"]);

const ScoredHit = z.object({
  id: z.string(),
  text: z.string(),
  score: z.number().min(0).max(1),
  sources: z.array(RetrievalSource),
});

const ContextOutput = z.object({
  task: z.string(),
  hits: z.array(ScoredHit),
  tokens_used: z.number().int().nonnegative(),
  budget_tokens: z.number().int().positive(),
  latency_ms: z.number().int().nonnegative(),
  formatted: z.string(),
});

type ContextInputT = z.infer<typeof ContextInput>;
type ContextOutputT = z.infer<typeof ContextOutput>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatPack(hits: z.infer<typeof ScoredHit>[], task: string): string {
  const lines: string[] = [];
  lines.push("<mneme-context>");
  lines.push(`Task: ${task}`);
  lines.push("");
  for (const h of hits) {
    const srcTag = h.sources.map((s) => s.toUpperCase()).join("+");
    lines.push(`## [${h.score.toFixed(3)} ${srcTag}] ${h.id}`);
    lines.push(h.text);
    lines.push("");
  }
  lines.push("</mneme-context>");
  return lines.join("\n");
}

// ---------------------------------------------------------------------------
// Tool
// ---------------------------------------------------------------------------

export const tool: ToolDescriptor<ContextInputT, ContextOutputT> = {
  name: "mneme_context",
  description:
    "Hybrid retrieval (BM25 + semantic + 2-hop graph + optional reranker) that returns a token-budgeted context pack for a task. Use this at the start of any non-trivial turn instead of dumping raw files or relying on the model's memory.",
  inputSchema: ContextInput,
  outputSchema: ContextOutput,
  category: "recall",
  async handler(input) {
    const t0 = Date.now();
    try {
      // Forward to the supervisor. The Rust side owns the BM25/graph/embed
      // indexes and the fusion code (brain::retrieve::RetrievalEngine).
      type RetrieveResponse = {
        hits: { id: string; text: string; score: number; sources: string[] }[];
        tokens_used: number;
        budget_tokens: number;
        latency_ms: number;
      };

      const resp = await dbQuery
        .raw<RetrieveResponse>("retrieve.hybrid", {
          task: input.task,
          budget_tokens: input.budget_tokens,
          anchors: input.anchors,
        })
        .catch(() => null);

      const hits = (resp?.hits ?? []).map((h) => ({
        id: h.id,
        text: h.text,
        score: Math.max(0, Math.min(1, h.score)),
        sources: h.sources.filter((s): s is z.infer<typeof RetrievalSource> =>
          (RetrievalSource.options as readonly string[]).includes(s),
        ),
      }));

      return {
        task: input.task,
        hits,
        tokens_used: resp?.tokens_used ?? 0,
        budget_tokens: input.budget_tokens,
        latency_ms: resp?.latency_ms ?? Date.now() - t0,
        formatted: formatPack(hits, input.task),
      };
    } catch (err) {
      return {
        task: input.task,
        hits: [],
        tokens_used: 0,
        budget_tokens: input.budget_tokens,
        latency_ms: Date.now() - t0,
        formatted:
          `<mneme-context>\n` +
          `Task: ${input.task}\n` +
          `(retrieval offline: ${(err as Error).message})\n` +
          `</mneme-context>`,
      };
    }
  },
};
