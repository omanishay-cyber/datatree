/**
 * MCP tool: step_plan_from (phase-c9 wired)
 *
 * Ingest a Markdown roadmap and create a step-ledger tree.
 *
 * Write path (preferred): supervisor IPC verb `step.plan_from_markdown`.
 * That IPC call is the only way to mutate `tasks.db` while preserving
 * the single-writer-per-shard invariant (§3.4).
 *
 * Graceful degrade: when IPC is unavailable (daemon offline), we fall
 * through to `stepPlanDirectWrite` in store.ts which opens tasks.db in
 * WAL mode and inserts rows directly. This is a DEV-TOOL FALLBACK — see
 * the trade-off comment on `stepPlanDirectWrite`. Production flows
 * should always go through IPC.
 *
 * NOTE: as of phase-c9 the supervisor in supervisor/src/ipc.rs does NOT
 * yet route `step.plan_from_markdown` — every call currently takes the
 * fallback path.
 *
 * Parsing rules (match `mneme build` convention):
 *   - A line beginning with `##` opens a new top-level step (parent).
 *   - A line beginning with `- [ ]` (or `- [x]`) under that heading is a
 *     child step. `[x]` is ingested as `completed`.
 *   - Numbered lists (`1.`, `2.`) are accepted as children too.
 */

import { existsSync, readFileSync } from "node:fs";
import { z } from "zod";
import {
  StepPlanFromInput,
  StepPlanFromOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";
import { stepPlanDirectWrite } from "../store.ts";

// Additive extension — keep the original output shape intact.
const StepPlanFromOutputExtended = StepPlanFromOutput.extend({
  note: z.string().optional(),
});

interface ParsedStep {
  description: string;
  status: "not_started" | "completed";
  children: ParsedStep[];
}

function parseRoadmap(md: string): ParsedStep[] {
  const lines = md.split(/\r?\n/);
  const parents: ParsedStep[] = [];
  let current: ParsedStep | null = null;
  for (const raw of lines) {
    const line = raw.trim();
    if (line.startsWith("## ")) {
      current = {
        description: line.slice(3).trim(),
        status: "not_started",
        children: [],
      };
      parents.push(current);
      continue;
    }
    const bulletMatch = line.match(/^-\s+\[( |x|X)\]\s+(.*)$/);
    if (bulletMatch) {
      const [, mark, desc] = bulletMatch;
      const child: ParsedStep = {
        description: (desc ?? "").trim(),
        status:
          mark === "x" || mark === "X" ? "completed" : "not_started",
        children: [],
      };
      if (current) current.children.push(child);
      else
        parents.push({
          description: child.description,
          status: child.status,
          children: [],
        });
      continue;
    }
    const numMatch = line.match(/^\d+\.\s+(.*)$/);
    if (numMatch && numMatch[1]) {
      const child: ParsedStep = {
        description: numMatch[1].trim(),
        status: "not_started",
        children: [],
      };
      if (current) current.children.push(child);
      else parents.push(child);
    }
  }
  return parents;
}

function countSteps(steps: ParsedStep[]): number {
  let n = 0;
  for (const s of steps) {
    n += 1 + countSteps(s.children);
  }
  return n;
}

type Input = ReturnType<typeof StepPlanFromInput.parse>;
type Output = z.infer<typeof StepPlanFromOutputExtended>;

export const tool: ToolDescriptor<Input, Output> = {
  name: "step_plan_from",
  description:
    "Ingest a Markdown roadmap (numbered hierarchical checklist) and create a step-ledger tree. Each numbered item becomes a Step row. Use at the start of any multi-step task to anchor the plan.",
  inputSchema: StepPlanFromInput,
  outputSchema: StepPlanFromOutputExtended,
  category: "step",
  async handler(input, ctx) {
    const sessionId = input.session_id ?? ctx.sessionId;

    // Pre-flight — bail early on an unreadable file so neither the
    // supervisor nor the dev-tool fallback gets called.
    if (!existsSync(input.markdown_path)) {
      return {
        steps_created: 0,
        root_step_id: "",
        note: "fallback:missing-file",
      };
    }
    let md = "";
    try {
      md = readFileSync(input.markdown_path, "utf8");
    } catch {
      return {
        steps_created: 0,
        root_step_id: "",
        note: "fallback:read-error",
      };
    }
    const parsed = parseRoadmap(md);
    const localCount = countSteps(parsed);
    if (parsed.length === 0) {
      return {
        steps_created: 0,
        root_step_id: "",
        note: "fallback:empty-plan",
      };
    }

    // ---- Supervisor path --------------------------------------------------
    const result = await dbQuery
      .raw<{ steps_created: number; root_step_id: string }>(
        "step.plan_from_markdown",
        {
          markdown_path: input.markdown_path,
          session_id: sessionId,
          parsed_steps: parsed,
        },
      )
      .catch(() => null);

    if (result && result.steps_created > 0) {
      return { ...result, note: "supervisor" };
    }

    // ---- Dev-tool fallback: direct write into tasks.db --------------------
    // Breaks the single-writer invariant intentionally; see docstring on
    // stepPlanDirectWrite. Only reached when IPC is offline.
    try {
      const direct = stepPlanDirectWrite(parsed, sessionId);
      if (direct) {
        return {
          steps_created: direct.steps_created,
          root_step_id: direct.root_step_id,
          note: "fallback:direct-write (dev-tool; bypasses single-writer)",
        };
      }
    } catch {
      // fall through to parsed-only
    }

    return {
      steps_created: localCount,
      root_step_id: "",
      note: "fallback:parsed-only",
    };
  },
};
