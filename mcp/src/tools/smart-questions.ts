/**
 * MCP tool: smart_questions
 *
 * Wave 3.2 — Auto-ranked question generation from graph topology.
 *
 * Given a project's graph.db, computes centrality + complexity + anomaly
 * signals for every node and surfaces the top-N questions an AI should
 * ask about the codebase before working in it.
 *
 * Algorithm (pure TypeScript — no Rust IPC required):
 *   1. Load all nodes + edges from graph.db via store.smartQuestionsData().
 *   2. Compute in-degree per node → z-score (centrality signal).
 *   3. Compute line-span per node → z-score (complexity proxy).
 *   4. Detect anomalies via iterative Tarjan SCC (cycles) + threshold (god
 *      nodes) + zero-degree (orphans).
 *   5. score = centrality_z * 0.4 + complexity_z * 0.3 + anomaly * 0.3
 *   6. Return top-N sorted by score desc.
 *
 * Graceful degrade: missing graph shard → { questions: [] }.
 */

import {
  SmartQuestionsInput,
  SmartQuestionsOutput,
  type ToolDescriptor,
} from "../types.ts";
import { smartQuestionsData, shardDbPath } from "../store.ts";

// ---------------------------------------------------------------------------
// Scoring algorithm (mirrors brain/src/smart_questions.rs in TypeScript)
// ---------------------------------------------------------------------------

interface RawNode {
  qualified_name: string;
  name: string;
  kind: string;
  file_path: string | null;
  line_start: number | null;
  line_end: number | null;
}

interface RawEdge {
  source: string;
  target: string;
  kind: string;
}

interface ScoredQuestion {
  question: string;
  score: number;
  justification: string;
  related_nodes: string[];
}

/** Population z-score normalisation. Returns map: qname → z */
function zScoreMap(nodes: RawNode[], values: number[]): Map<string, number> {
  if (nodes.length === 0) return new Map();
  const n = values.length;
  const mean = values.reduce((a, b) => a + b, 0) / n;
  const variance =
    values.map((v) => (v - mean) ** 2).reduce((a, b) => a + b, 0) / n;
  const std = Math.sqrt(variance);
  const out = new Map<string, number>();
  nodes.forEach((node, i) => {
    out.set(node.qualified_name, std < 1e-9 ? 0 : (values[i]! - mean) / std);
  });
  return out;
}

/** Iterative Tarjan SCC — returns set of qualified_names in any cycle (≥2 members). */
function tarjanCycleMembers(nodes: RawNode[], edges: RawEdge[]): Set<string> {
  const allNames = new Set(nodes.map((n) => n.qualified_name));
  const adj = new Map<string, string[]>();
  for (const e of edges) {
    if (allNames.has(e.source) && allNames.has(e.target)) {
      let list = adj.get(e.source);
      if (!list) {
        list = [];
        adj.set(e.source, list);
      }
      list.push(e.target);
    }
  }

  let idx = 0;
  const indices = new Map<string, number>();
  const lowlink = new Map<string, number>();
  const onStack = new Set<string>();
  const stack: string[] = [];
  const cycleMembers = new Set<string>();

  for (const start of nodes.map((n) => n.qualified_name)) {
    if (indices.has(start)) continue;

    const work: Array<{ v: string; i: number }> = [{ v: start, i: 0 }];
    indices.set(start, idx);
    lowlink.set(start, idx);
    idx++;
    stack.push(start);
    onStack.add(start);

    while (work.length > 0) {
      const frame = work[work.length - 1]!;
      const succs = adj.get(frame.v) ?? [];
      if (frame.i < succs.length) {
        const w = succs[frame.i++]!;
        if (!indices.has(w)) {
          indices.set(w, idx);
          lowlink.set(w, idx);
          idx++;
          stack.push(w);
          onStack.add(w);
          work.push({ v: w, i: 0 });
        } else if (onStack.has(w)) {
          lowlink.set(frame.v, Math.min(lowlink.get(frame.v)!, indices.get(w)!));
        }
      } else {
        work.pop();
        const vLL = lowlink.get(frame.v)!;
        const vIdx = indices.get(frame.v)!;
        const parent = work[work.length - 1];
        if (parent) {
          lowlink.set(parent.v, Math.min(lowlink.get(parent.v)!, vLL));
        }
        if (vLL === vIdx) {
          const scc: string[] = [];
          while (true) {
            const top = stack.pop();
            if (top == null) break;
            onStack.delete(top);
            scc.push(top);
            if (top === frame.v) break;
          }
          if (scc.length >= 2) {
            for (const m of scc) cycleMembers.add(m);
          }
        }
      }
    }
  }

  return cycleMembers;
}

/** 95th percentile of in-degree values, minimum 1 (avoids trivial god-node labelling). */
function percentile95(inDegree: Map<string, number>): number {
  if (inDegree.size === 0) return 1;
  const vals = Array.from(inDegree.values()).sort((a, b) => a - b);
  const idx = Math.min(Math.floor(vals.length * 0.95), vals.length - 1);
  return Math.max(vals[idx] ?? 1, 1);
}

/** Short name: rightmost `::`- or `.`-separated segment. */
function shortName(qname: string): string {
  return (
    qname.split("::").at(-1)?.split(".").at(-1)?.trim() ?? qname
  );
}

/** Base name from a file path. */
function baseName(filePath: string | null): string {
  if (!filePath) return "";
  return filePath.replace(/\\/g, "/").split("/").at(-1) ?? "";
}

/**
 * Full scoring pipeline. Returns questions sorted by score descending.
 * Returns [] for an empty graph without throwing.
 */
function scoreQuestions(
  nodes: RawNode[],
  edges: RawEdge[],
  limit: number,
  kind: string | undefined,
): ScoredQuestion[] {
  if (nodes.length === 0) return [];

  // --- Degree maps ---
  const inDeg = new Map<string, number>();
  const outDeg = new Map<string, number>();
  for (const n of nodes) {
    inDeg.set(n.qualified_name, 0);
    outDeg.set(n.qualified_name, 0);
  }
  for (const e of edges) {
    inDeg.set(e.target, (inDeg.get(e.target) ?? 0) + 1);
    outDeg.set(e.source, (outDeg.get(e.source) ?? 0) + 1);
  }

  // --- Anomaly detection ---
  const cycleMembers = tarjanCycleMembers(nodes, edges);
  const godThreshold = percentile95(inDeg);

  // --- Complexity: line span ---
  const validSpans: number[] = [];
  for (const n of nodes) {
    if (n.line_start != null && n.line_end != null && n.line_end >= n.line_start) {
      validSpans.push(n.line_end - n.line_start);
    }
  }
  validSpans.sort((a, b) => a - b);
  const medianSpan = validSpans.length > 0
    ? (validSpans[Math.floor(validSpans.length / 2)] ?? 0)
    : 0;
  const complexityValues = nodes.map((n) => {
    if (n.line_start != null && n.line_end != null && n.line_end >= n.line_start) {
      return n.line_end - n.line_start;
    }
    return medianSpan;
  });

  // --- Z-score maps ---
  const centralityValues = nodes.map((n) => inDeg.get(n.qualified_name) ?? 0);
  const centralityZ = zScoreMap(nodes, centralityValues);
  const complexityZ = zScoreMap(nodes, complexityValues);

  // --- Generate per-node candidates ---
  const candidates: ScoredQuestion[] = [];

  for (const n of nodes) {
    const qname = n.qualified_name;
    const inD = inDeg.get(qname) ?? 0;
    const outD = outDeg.get(qname) ?? 0;
    const cz = centralityZ.get(qname) ?? 0;
    const cpz = complexityZ.get(qname) ?? 0;

    const isGod = inD >= godThreshold;
    const isCycle = cycleMembers.has(qname);
    const isOrphan = inD === 0 && outD === 0;

    const anomalyScore = Math.min(
      (isGod ? 1.0 : 0) + (isCycle ? 0.7 : 0) + (isOrphan ? 0.5 : 0),
      1.0,
    );

    const composite = Math.max(cz, 0) * 0.4 + Math.max(cpz, 0) * 0.3 + anomalyScore * 0.3;

    // Kind filter
    const passesFilter =
      kind === "starter"
        ? !isGod && !isCycle && !isOrphan && cz > 0
        : kind === "deep-dive"
          ? cpz > 0.5
          : kind === "anomaly"
            ? isGod || isCycle || isOrphan
            : composite >= 0.01; // "all" — skip negligible-score nodes

    if (!passesFilter) continue;

    // Build question text + justification
    const label = shortName(qname);
    const kindLabel = n.kind;
    const file = baseName(n.file_path);
    const filePart = file ? ` in \`${file}\`` : "";

    let question: string;
    let justification: string;

    if (isOrphan) {
      question =
        `Is \`${label}\`${filePart} dead code? It has no callers and imports nothing — was it intentionally left or accidentally orphaned?`;
      justification =
        `\`${label}\` (${kindLabel}) has in-degree=0 and out-degree=0. Nothing in the indexed graph references it or is referenced by it. Likely dead code or an unwired stub.`;
    } else if (isGod) {
      question =
        `What does \`${label}\` do, and why does everything call it? With ${inD} dependants it is the highest-traffic node — what would break if its signature changed?`;
      justification =
        `\`${label}\` (${kindLabel}) has in-degree=${inD} which exceeds the 95th-percentile threshold of ${godThreshold}. Changes here have the widest blast radius in the project.`;
    } else if (isCycle) {
      question =
        `Why does \`${label}\` participate in a circular dependency? What is the intended ownership boundary and can the cycle be broken by introducing an abstraction?`;
      justification =
        `\`${label}\` (${kindLabel}) is part of a directed cycle (in=${inD}, out=${outD}). Cycles impede testability and clean layering.`;
    } else if (cpz > 1.5) {
      const lineHint =
        n.line_start != null && n.line_end != null
          ? ` (~${n.line_end - n.line_start} lines)`
          : "";
      question =
        `What does \`${label}\`${lineHint} do, and should it be split? It is significantly larger than the codebase median — does it have a single clear responsibility?`;
      justification =
        `\`${label}\` (${kindLabel}) has a line-span ${cpz.toFixed(1)}σ above the corpus median, making it a complexity outlier.`;
    } else {
      question =
        `What is the role of \`${label}\` in this codebase? It has ${inD} upstream dependants — what contract does it expose and who should be notified when it changes?`;
      justification =
        `\`${label}\` (${kindLabel}) has in-degree=${inD} and out-degree=${outD}, placing it in the top centrality tier. Understanding it is essential before any large refactor.`;
    }

    candidates.push({
      question,
      score: Math.min(Math.max(composite, 0), 1),
      justification,
      related_nodes: [qname],
    });
  }

  // --- Add cycle-level questions (multi-node anomaly) ---
  if (kind === undefined || kind === "anomaly") {
    const cycleQs = buildCycleQuestions(nodes, edges, cycleMembers);
    candidates.push(...cycleQs);
  }

  // Sort score desc, then qname asc for stability.
  candidates.sort((a, b) => {
    const ds = b.score - a.score;
    if (Math.abs(ds) > 1e-9) return ds;
    return (a.related_nodes[0] ?? "").localeCompare(b.related_nodes[0] ?? "");
  });

  return candidates.slice(0, limit);
}

/**
 * Build one question per detected cycle (shortest path BFS).
 * Capped at 3 distinct cycles to avoid flooding the output.
 */
function buildCycleQuestions(
  nodes: RawNode[],
  edges: RawEdge[],
  cycleMembers: Set<string>,
): ScoredQuestion[] {
  if (cycleMembers.size === 0) return [];

  const allNames = new Set(nodes.map((n) => n.qualified_name));
  const adj = new Map<string, string[]>();
  for (const e of edges) {
    if (cycleMembers.has(e.source) && cycleMembers.has(e.target) &&
        allNames.has(e.source) && allNames.has(e.target)) {
      let list = adj.get(e.source);
      if (!list) { list = []; adj.set(e.source, list); }
      list.push(e.target);
    }
  }

  const seenCanonical = new Set<string>();
  const out: ScoredQuestion[] = [];

  for (const start of Array.from(cycleMembers).slice(0, 10)) {
    if (out.length >= 3) break;
    const path = bfsShortestCycle(start, adj);
    if (!path) continue;
    const canonical = [...path].sort().join(",");
    if (seenCanonical.has(canonical)) continue;
    seenCanonical.add(canonical);

    const chain = path.map(shortName).join(" → ");
    const first = shortName(path[0] ?? "?");
    out.push({
      question:
        `Should the circular dependency ${chain} → ${first} be broken? If so, which edge should be removed or inverted?`,
      score: 0.85,
      justification:
        `Detected a directed cycle of length ${path.length}. Cycles prevent clean layering and make initialization order, testing, and refactoring significantly harder.`,
      related_nodes: path,
    });
  }

  return out;
}

/** BFS from `start` within `adj`; returns shortest cycle path or null. */
function bfsShortestCycle(
  start: string,
  adj: Map<string, string[]>,
): string[] | null {
  const parent = new Map<string, string>();
  parent.set(start, start);
  const queue: string[] = [start];

  while (queue.length > 0) {
    const node = queue.shift()!;
    for (const next of adj.get(node) ?? []) {
      if (next === start) {
        // Reconstruct.
        const path: string[] = [start];
        let cur = node;
        while (cur !== start) {
          path.push(cur);
          cur = parent.get(cur) ?? start;
        }
        path.reverse();
        return path;
      }
      if (!parent.has(next)) {
        parent.set(next, node);
        queue.push(next);
      }
    }
  }
  return null;
}

// ---------------------------------------------------------------------------
// Tool descriptor
// ---------------------------------------------------------------------------

export const tool: ToolDescriptor<
  ReturnType<typeof SmartQuestionsInput.parse>,
  ReturnType<typeof SmartQuestionsOutput.parse>
> = {
  name: "smart_questions",
  description:
    "Generate the top-N questions an AI should ask about this codebase before working in it. Questions are auto-ranked by graph topology: in-degree centrality (40%), function complexity (30%), and structural anomalies — god nodes, cyclic dependencies, orphaned dead code (30%). Pass `kind: 'starter'` for orientation questions, `kind: 'deep-dive'` for complexity outliers, or `kind: 'anomaly'` to focus on structural problems. Run this at the start of any session on an unfamiliar codebase.",
  inputSchema: SmartQuestionsInput,
  outputSchema: SmartQuestionsOutput,
  category: "graph",
  async handler(input) {
    if (!shardDbPath("graph")) {
      return { questions: [] };
    }

    const { nodes, edges } = smartQuestionsData();

    if (nodes.length === 0) {
      return { questions: [] };
    }

    const questions = scoreQuestions(nodes, edges, input.limit, input.kind);

    return { questions };
  },
};
