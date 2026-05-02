/**
 * MCP tool: blast_radius
 *
 * Computes all callers, dependents, and tests affected by changing a target
 * (file or function). v0.1: reads directly from the project's graph.db
 * shard via bun:sqlite for sub-millisecond queries.
 *
 * F7 upgrade: the response now ALSO includes a structured risk report —
 * `direct_consumers`, `transitive_consumers`, `tests_affected`,
 * `decisions_assumed`, and `risk` (one of `low | medium | high | critical`).
 * Added additively: every field on the old shape is preserved so callers
 * that don't know about F7 keep working.
 */

import { z } from "zod";
import {
  BlastRadiusInput,
  type ToolDescriptor,
} from "../types.ts";
import { blastRadius } from "../store.ts";
import { errMsg } from "../errors.ts";

// ---------------------------------------------------------------------------
// Extended schema (additive over BlastRadiusOutput).
// ---------------------------------------------------------------------------

const CodeRef = z.object({
  qualified_name: z.string(),
  file: z.string().nullable(),
  line: z.number().int().nullable(),
  kind: z.string(),
});

const RiskLevel = z.enum(["low", "medium", "high", "critical"]);

const BlastRadiusOutputExtended = z.object({
  // --- original fields ---
  target: z.string(),
  affected_files: z.array(z.string()),
  affected_symbols: z.array(z.string()),
  test_files: z.array(z.string()),
  total_count: z.number().int(),
  critical_paths: z.array(z.string()).default([]),
  // --- F7 additions ---
  direct_consumers: z.array(CodeRef).default([]),
  transitive_consumers: z.array(CodeRef).default([]),
  tests_affected: z.array(CodeRef).default([]),
  decisions_assumed: z.array(z.string()).default([]),
  risk: RiskLevel.default("low"),
});

type BlastRadiusInputT = z.infer<typeof BlastRadiusInput>;
type BlastRadiusOutputExtendedT = z.infer<typeof BlastRadiusOutputExtended>;

// ---------------------------------------------------------------------------
// Mirror of brain/src/blast.rs::compute_risk — kept in the TS layer so the
// tool can return a valid `risk` even when the supervisor is offline.
// ---------------------------------------------------------------------------

function bump(l: z.infer<typeof RiskLevel>): z.infer<typeof RiskLevel> {
  if (l === "low") return "medium";
  if (l === "medium") return "high";
  if (l === "high") return "critical";
  return "critical";
}

function computeRisk(
  direct: number,
  transitive: number,
  tests: number,
  decisions: number,
): z.infer<typeof RiskLevel> {
  if (decisions > 0) return "critical";
  let level: z.infer<typeof RiskLevel> = "low";
  if (direct > 5) level = "medium";
  if (direct > 15 || transitive > 20) level = "high";
  if (direct + transitive > 100) level = "critical";
  if (tests === 0 && direct + transitive > 0) level = bump(level);
  return level;
}

function isTestNode(node: string): boolean {
  return (
    node.includes("test") ||
    node.includes("spec") ||
    node.includes(".test.") ||
    node.includes(".spec.")
  );
}

// ---------------------------------------------------------------------------
// Tool
// ---------------------------------------------------------------------------

export const tool: ToolDescriptor<
  BlastRadiusInputT,
  BlastRadiusOutputExtendedT
> = {
  name: "blast_radius",
  description:
    "Compute the blast radius of a change: every caller, dependent, and test affected. Pass either a file path or a fully-qualified function name. Returns a structured risk report (F7): direct + transitive consumers, affected tests, and a risk level (low/medium/high/critical). Use BEFORE Edit/Write on any file to know what else might break.",
  inputSchema: BlastRadiusInput,
  outputSchema: BlastRadiusOutputExtended,
  category: "graph",
  async handler(input) {
    try {
      const rows = blastRadius(input.target, input.depth ?? 2);

      const affected_files: string[] = [];
      const affected_symbols: string[] = [];
      const test_files: string[] = [];
      const critical_paths: string[] = [];

      const direct_consumers: z.infer<typeof CodeRef>[] = [];
      const transitive_consumers: z.infer<typeof CodeRef>[] = [];
      const tests_affected: z.infer<typeof CodeRef>[] = [];

      for (const r of rows) {
        // Depth 0 is the target itself — skip from consumer lists.
        if (r.depth === 0) continue;

        if (r.kind === "file") {
          affected_files.push(r.node);
          if (isTestNode(r.node)) test_files.push(r.node);
        } else {
          affected_symbols.push(r.node);
          if (r.depth === 1) critical_paths.push(r.node);
        }

        const ref: z.infer<typeof CodeRef> = {
          qualified_name: r.node,
          file: r.kind === "file" ? r.node : null,
          line: null,
          kind: r.kind,
        };
        if (isTestNode(r.node)) tests_affected.push(ref);
        if (r.depth === 1) direct_consumers.push(ref);
        else transitive_consumers.push(ref);
      }

      const risk = computeRisk(
        direct_consumers.length,
        transitive_consumers.length,
        tests_affected.length,
        0,
      );

      return {
        target: input.target,
        affected_files,
        affected_symbols,
        test_files,
        total_count: rows.length,
        critical_paths,
        direct_consumers,
        transitive_consumers,
        tests_affected,
        decisions_assumed: [],
        risk,
      };
    } catch (err) {
      // Graceful: if the shard isn't built yet, return an empty radius
      // with a hint in the symbols list.
      return {
        target: input.target,
        affected_files: [],
        affected_symbols: [
          `(mneme not yet built — run mneme build .\` first; ${errMsg(err)})`,
        ],
        test_files: [],
        total_count: 0,
        critical_paths: [],
        direct_consumers: [],
        transitive_consumers: [],
        tests_affected: [],
        decisions_assumed: [],
        risk: "low",
      };
    }
  },
};
