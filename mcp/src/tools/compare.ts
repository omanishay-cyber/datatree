/**
 * MCP tool: compare
 *
 * Diff two snapshots.
 *
 * v0.1 (review P2): each snapshot directory contains per-layer `<l>.db`
 * files. We open snapshot A and snapshot B's `graph.db` read-only and
 * diff their `files.path` sets to derive `files_added/removed/modified`
 * (hash-mismatch on same path ⇒ modified). For decisions/findings we
 * count rows in each snapshot's `history.db`/`findings.db` and report
 * the delta. Missing snapshots or missing layers gracefully degrade to
 * the neutral zero diff.
 */

import { Database } from "bun:sqlite";
import {
  CompareInput,
  CompareOutput,
  type ToolDescriptor,
} from "../types.ts";
import { snapshotLayerPath } from "../store.ts";

interface FileRow {
  path: string;
  sha256: string;
}

function readFileSet(graphDb: string | null): Map<string, string> {
  const out = new Map<string, string>();
  if (!graphDb) return out;
  const db = new Database(graphDb, { readonly: true });
  try {
    const rows = db
      .prepare("SELECT path, sha256 FROM files")
      .all() as FileRow[];
    for (const r of rows) out.set(r.path, r.sha256);
  } catch {
    // ignore — empty set
  } finally {
    try {
      db.close();
    } catch {
      // ignore
    }
  }
  return out;
}

function countRows(dbPath: string | null, table: string): number {
  if (!dbPath) return 0;
  const db = new Database(dbPath, { readonly: true });
  try {
    const r = db.prepare(`SELECT COUNT(*) AS c FROM ${table}`).get() as
      | { c: number }
      | undefined;
    return r?.c ?? 0;
  } catch {
    return 0;
  } finally {
    try {
      db.close();
    } catch {
      // ignore
    }
  }
}

export const tool: ToolDescriptor<
  ReturnType<typeof CompareInput.parse>,
  ReturnType<typeof CompareOutput.parse>
> = {
  name: "compare",
  description:
    "Diff two snapshots. Returns files added/removed/modified counts, decisions added, findings resolved/introduced. Use to understand what changed across a range of time.",
  inputSchema: CompareInput,
  outputSchema: CompareOutput,
  category: "time",
  async handler(input) {
    const graphA = snapshotLayerPath(input.snapshot_a, "graph");
    const graphB = snapshotLayerPath(input.snapshot_b, "graph");
    const filesA = readFileSet(graphA);
    const filesB = readFileSet(graphB);

    const files_added: string[] = [];
    const files_removed: string[] = [];
    const files_modified: string[] = [];
    for (const [path, hashB] of filesB) {
      const hashA = filesA.get(path);
      if (hashA == null) files_added.push(path);
      else if (hashA !== hashB) files_modified.push(path);
    }
    for (const path of filesA.keys()) {
      if (!filesB.has(path)) files_removed.push(path);
    }

    const historyA = snapshotLayerPath(input.snapshot_a, "history");
    const historyB = snapshotLayerPath(input.snapshot_b, "history");
    const decisionsA = countRows(historyA, "decisions");
    const decisionsB = countRows(historyB, "decisions");

    const findingsAPath = snapshotLayerPath(input.snapshot_a, "findings");
    const findingsBPath = snapshotLayerPath(input.snapshot_b, "findings");
    const findingsA = countRows(findingsAPath, "findings");
    const findingsB = countRows(findingsBPath, "findings");

    return {
      diff: {
        files_added,
        files_removed,
        files_modified,
        decisions_added: Math.max(0, decisionsB - decisionsA),
        findings_resolved: Math.max(0, findingsA - findingsB),
        findings_introduced: Math.max(0, findingsB - findingsA),
      },
    };
  },
};
