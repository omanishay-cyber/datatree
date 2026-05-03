/**
 * MCP tool: file_intent
 *
 * Phase A J7. Returns the persisted intent annotation for a given file
 * path — `frozen | stable | deferred | experimental | drift | unknown`
 * — with reason, source, and confidence. Sourced from
 * `memory.db.file_intent`, populated by
 * `cli/src/commands/build.rs::run_intent_pass` from `@mneme-intent:`
 * magic comments at file head OR (future) git heuristics / LLM
 * inference.
 *
 * Why this matters: a 5,000-line file frozen-by-intent (e.g. a legacy
 * calculator ported verbatim from VBA) looks identical in the AST to
 * a 5,000-line file deferred-by-want-of-time. Every refactor planner,
 * `god_nodes` consumer, or AI agent should respect intent before
 * recommending action. This is the Phase A §J differentiator.
 *
 * Graceful degrade:
 *   - Memory shard missing → `{ intent: "unknown", source: "missing-shard" }`
 *   - File never annotated → `{ intent: "unknown", source: "no-record" }`
 */

import { z } from "zod";
import {
  type ToolDescriptor,
} from "../types.ts";
import { withShard, shardDbPath } from "../store.ts";

export const FileIntentInput = z.object({
  /** Project-relative file path. Absolute paths also accepted. */
  path: z.string().min(1),
});

export const FileIntentOutput = z.object({
  intent: z.string(),
  reason: z.string().nullable(),
  source: z.string(),
  confidence: z.number().min(0).max(1),
  annotated_at: z.string().nullable(),
  // v0.3.2 hotfix: when no annotation exists, also surface
  // `annotation_found: false` and a `hint` so the caller knows what's
  // happening (silent "unknown" used to make Claude keep re-querying
  // un-annotated files expecting useful results).
  annotation_found: z.boolean().default(true),
  hint: z.string().nullable().default(null),
});

const NO_ANNOTATION_HINT =
  "no @mneme-intent: annotation found in this file. " +
  "Add a JSDoc/comment near the top of the file like " +
  "`// @mneme-intent: payment processing UI` " +
  "(or `# @mneme-intent: ...` for Python/Ruby/shell, " +
  "`/** @mneme-intent: ... */` for JSDoc, etc.) " +
  "to make this file discoverable by intent-based recall and to " +
  "let refactor planners respect freeze/stable/deferred markers. " +
  "Re-run `mneme build` after adding the annotation.";

export const tool: ToolDescriptor<
  ReturnType<typeof FileIntentInput.parse>,
  ReturnType<typeof FileIntentOutput.parse>
> = {
  name: "file_intent",
  description:
    "Get the per-file intent annotation (frozen / stable / deferred / experimental / drift / unknown). Use this BEFORE recommending refactors so you don't propose changes to files explicitly marked frozen-by-intent (verbatim formulas, locked-down API shapes, etc.). Annotations come from `@mneme-intent:` magic comments parsed at build time. When a file has NO annotation, the response sets `annotation_found: false` and a `hint` explaining how to add one — do not call repeatedly on un-annotated files expecting different results; either the file is annotated or it is not.",
  inputSchema: FileIntentInput,
  outputSchema: FileIntentOutput,
  category: "recall",
  async handler(input) {
    if (!shardDbPath("memory")) {
      return {
        intent: "unknown",
        reason: null,
        source: "missing-shard",
        confidence: 0,
        annotated_at: null,
        annotation_found: false,
        hint:
          "memory shard is missing — run `mneme build` first so the " +
          "intent annotations get persisted. " + NO_ANNOTATION_HINT,
      };
    }
    const result = withShard(
      "memory",
      (db) => {
        const row = db
          .prepare(
            "SELECT intent, reason, source, confidence, annotated_at \
             FROM file_intent WHERE file_path = ?1 LIMIT 1",
          )
          .get(input.path) as
          | {
              intent: string;
              reason: string | null;
              source: string;
              confidence: number;
              annotated_at: string;
            }
          | undefined;
        return row ?? null;
      },
      null as null | {
        intent: string;
        reason: string | null;
        source: string;
        confidence: number;
        annotated_at: string;
      },
    );
    if (!result) {
      return {
        intent: "unknown",
        reason: null,
        source: "no-record",
        confidence: 0,
        annotated_at: null,
        annotation_found: false,
        hint: NO_ANNOTATION_HINT,
      };
    }
    return {
      intent: result.intent,
      reason: result.reason,
      source: result.source,
      confidence: result.confidence,
      annotated_at: result.annotated_at,
      annotation_found: true,
      hint: null,
    };
  },
};
