/**
 * MCP tool: drift_findings
 *
 * Returns the current open drift findings (rule violations actively present
 * in the working tree, `resolved_at IS NULL`).
 *
 * v0.2 (task wiring): reads `findings.db → findings` via `bun:sqlite`
 * read-only through `driftFindingsExtended()` in store.ts. Every column the
 * scanners layer writes is surfaced so the output can carry `column`,
 * `scope_snippet`, `first_seen`, and `last_seen` — additive over the base
 * `DriftFindingsOutput` schema, same pattern blast_radius.ts uses.
 *
 * Query shape (inside store.ts):
 *   SELECT ... FROM findings
 *   WHERE resolved_at IS NULL
 *     [AND severity = ?]
 *     [AND file LIKE ?]
 *   ORDER BY created_at DESC, id DESC
 *   LIMIT ?
 *
 * Scope interpretation:
 *   - "project"  → no file filter (project-wide view; the default)
 *   - "diff"     → no file filter (delegated; diff scoping is enforced by
 *                  the scanner pipeline upstream)
 *   - any other  → treated as a LIKE substring against `file`
 *
 * Graceful degrade:
 *   - findings shard missing            → empty list, total 0, zod-valid
 *   - shard present but query throws    → empty list, total 0, zod-valid
 *   - severity not in enum              → coerced to "info" (never throws)
 */

import { z } from "zod";
import {
  DriftFindingsInput,
  Finding,
  SeverityEnum,
  TruncationMixin,
  type Severity,
  type ToolDescriptor,
} from "../types.ts";
import { driftFindingsExtended, shardDbPath } from "../store.ts";

// ---------------------------------------------------------------------------
// Extended output schema (additive over DriftFindingsOutput).
//
// The base `DriftFindingsOutput` is `{ findings: Finding[] }`. The task spec
// asks for four extra per-row fields (`column`, `scope_snippet`, `first_seen`,
// `last_seen`) plus envelope fields (`total_count`, `filtered_by`). We build
// the extended shape locally — identical pattern to blast_radius.ts — so
// existing consumers of the base schema keep working if they narrow.
// ---------------------------------------------------------------------------

const ExtendedFinding = Finding.extend({
  column: z.number().int().nullable(),
  scope_snippet: z.string().nullable(),
  first_seen: z.string(),
  last_seen: z.string().nullable(),
});
type ExtendedFindingT = z.infer<typeof ExtendedFinding>;

const DriftFindingsOutputExtended = z
  .object({
    findings: z.array(ExtendedFinding),
    total_count: z.number().int().nonnegative(),
    filtered_by: z.object({
      severity: SeverityEnum.nullable(),
      scope: z.string().nullable(),
    }),
  })
  .extend(TruncationMixin.shape);

type DriftFindingsInputT = z.infer<typeof DriftFindingsInput>;
type DriftFindingsOutputExtendedT = z.infer<typeof DriftFindingsOutputExtended>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const ALLOWED_SEVERITY: readonly Severity[] = [
  "info",
  "low",
  "medium",
  "high",
  "critical",
];

function coerceSeverity(s: string): Severity {
  return (ALLOWED_SEVERITY as readonly string[]).includes(s)
    ? (s as Severity)
    : "info";
}

/**
 * Map a raw `scope` input onto the SQL `file LIKE ?` parameter.
 * - "project" / "diff" → no filter (undefined)
 * - "file"             → no filter here either; callers that want a specific
 *                        file should pass that file's path as the scope
 *                        string. The enum label alone is not a selector.
 * - any other string   → passed through as a substring match
 */
function scopeToFileFilter(scope: string | undefined): string | undefined {
  if (!scope) return undefined;
  if (scope === "project" || scope === "diff" || scope === "file") {
    return undefined;
  }
  return scope;
}

// ---------------------------------------------------------------------------
// Tool
// ---------------------------------------------------------------------------

export const tool: ToolDescriptor<
  DriftFindingsInputT,
  DriftFindingsOutputExtendedT
> = {
  name: "drift_findings",
  description:
    "Get currently-open drift findings (rule violations present in the working tree, resolved_at IS NULL). Filter by severity and/or scope (a path segment, or one of 'project'|'file'|'diff'). Results are ordered first_seen DESC and include column + scope_snippet + first/last_seen so the model can cite locations precisely.",
  inputSchema: DriftFindingsInput,
  outputSchema: DriftFindingsOutputExtended,
  category: "drift",
  async handler(input) {
    const severityIn = input.severity;
    const scopeIn = input.scope;
    const limitIn = input.limit;

    const filtered_by = {
      severity: severityIn ?? null,
      scope: scopeIn ?? null,
    };

    // Fast-path graceful degrade: shard not yet built.
    if (!shardDbPath("findings")) {
      return {
        findings: [],
        total_count: 0,
        filtered_by,
      };
    }

    const fileFilter = scopeToFileFilter(scopeIn);

    let rows: ReturnType<typeof driftFindingsExtended>["rows"] = [];
    let total_count = 0;
    try {
      const res = driftFindingsExtended(severityIn, fileFilter, limitIn);
      rows = res.rows;
      total_count = res.total_count;
    } catch {
      // Shard corrupt / query failed — degrade to empty, preserve envelope.
      rows = [];
      total_count = 0;
    }

    const findings: ExtendedFindingT[] = rows.map((r) => {
      // column_start is 0-based from tree-sitter; surface as nullable so
      // callers don't assume a specific base. line maps to line_start.
      const column = Number.isFinite(r.column_start) ? r.column_start : null;

      // scope_snippet = the file path + line span; a compact anchor the model
      // can quote when presenting the finding. We deliberately do NOT read
      // the file here (read-only store.ts layer, no fs access on hot path).
      const scope_snippet =
        r.line_start === r.line_end
          ? `${r.file}:${r.line_start}`
          : `${r.file}:${r.line_start}-${r.line_end}`;

      return {
        id: String(r.id),
        scanner: r.scanner,
        severity: coerceSeverity(r.severity),
        file: r.file,
        line: Number.isFinite(r.line_start) ? r.line_start : null,
        rule: r.rule_id,
        message: r.message,
        suggestion: r.suggestion,
        detected_at: r.created_at,
        // --- extended ---
        column,
        scope_snippet,
        first_seen: r.created_at,
        last_seen: r.resolved_at,
      };
    });

    return {
      findings,
      total_count,
      filtered_by,
    };
  },
};
