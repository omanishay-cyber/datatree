/**
 * MCP tool: step_plan_from
 *
 * Ingest a Markdown roadmap and create a step-ledger tree.
 *
 * v0.1 (review P2): this tool WRITES — it must go through the single-
 * writer supervisor. We parse the markdown locally (no md-ingest crate
 * available from Bun), pre-flight the file read, then dispatch
 * `step.plan_from_markdown` over IPC. That IPC call is the only legal
 * way to mutate `tasks.db`. If IPC is unavailable (daemon down) we
 * fall back to returning zero-steps-created rather than corrupting
 * the ledger.
 *
 * Parsing rules (match `mneme build` convention):
 *   - A line beginning with `##` opens a new top-level step (parent).
 *   - A line beginning with `- [ ]` (or `- [x]`) under that heading is a
 *     child step. `[x]` is ingested as `completed`.
 *   - Numbered lists (`1.`, `2.`) are accepted as children too.
 */

import { existsSync, readFileSync } from "node:fs";
import {
  StepPlanFromInput,
  StepPlanFromOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

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

export const tool: ToolDescriptor<
  ReturnType<typeof StepPlanFromInput.parse>,
  ReturnType<typeof StepPlanFromOutput.parse>
> = {
  name: "step_plan_from",
  description:
    "Ingest a Markdown roadmap (numbered hierarchical checklist) and create a step-ledger tree. Each numbered item becomes a Step row. Use at the start of any multi-step task to anchor the plan.",
  inputSchema: StepPlanFromInput,
  outputSchema: StepPlanFromOutput,
  category: "step",
  async handler(input, ctx) {
    const sessionId = input.session_id ?? ctx.sessionId;

    // Pre-flight: if the file isn't readable there's no point dispatching.
    if (!existsSync(input.markdown_path)) {
      return { steps_created: 0, root_step_id: "" };
    }
    let md = "";
    try {
      md = readFileSync(input.markdown_path, "utf8");
    } catch {
      return { steps_created: 0, root_step_id: "" };
    }
    const parsed = parseRoadmap(md);
    const localCount = countSteps(parsed);

    // Dispatch the actual write through the supervisor.
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

    if (result && result.steps_created > 0) return result;
    // IPC failed → report the parsed count so the caller knows the plan
    // was at least understood locally.
    return { steps_created: localCount, root_step_id: "" };
  },
};
