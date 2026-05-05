// mcp/src/result-cap.ts
//
// Item #118 (2026-05-05): central tool-result size budget.
//
// Without this, a single tool call can blow the AI's entire turn
// budget on one response. Real example from the CRG-vs-Mneme bench
// (2026-05-02):
//
//   "architecture_overview returned ~232k chars and overflowed the
//    tool result; I'd need to slice it via Bash, which the constraint
//    forbids."
//
// 232K chars ≈ 60K tokens — more than the entire CRG session used in
// 5 queries combined. The tool was silently over-claiming context
// space the agent didn't ask for.
//
// Fix: cap every tool result at a default budget (4 KB) before it
// crosses the MCP wire. Tools that legitimately need more space
// declare an override via MAX_BYTES_OVERRIDES below. Anything past
// the cap gets replaced with a small envelope that includes the
// original size + a hint for how to drill in.
//
// Authors: Anish Trivedi & Kruti Trivedi. Apache-2.0.

/** Default cap. ~4 KB ≈ 1000 tokens. Aggressive enough to prevent
 *  bloat, generous enough to fit a typical recall result + 5-10
 *  hits + a short hint string. */
export const DEFAULT_MAX_BYTES = 4 * 1024;

/** "Detail=full" cap. Tools that opt into this via per-call args get
 *  4× the default. Still capped to prevent the 232K dump. */
export const DETAIL_FULL_MAX_BYTES = 16 * 1024;

/** Per-tool overrides. Most tools use the default. These are the
 *  outliers where the result legitimately needs more headroom — but
 *  every override is bounded; nothing exceeds 32 KB.
 *
 *  When a new tool is added, the default applies automatically.
 *  Override here ONLY if a real benchmark shows the default truncates
 *  meaningful content. The pressure should be on shrinking results,
 *  not enlarging budgets. */
export const MAX_BYTES_OVERRIDES: Record<string, number> = {
  // The headline offender from the CRG bench. 32 KB is enough for the
  // top-level architecture summary + per-package one-liners; deeper
  // detail comes from mneme_context / wiki_page on specific paths.
  architecture_overview: 32 * 1024,
  // Wiki pages are intentionally long-form prose. 24 KB ≈ 6K tokens
  // ≈ 1500-3000 words, the rough size of a finished wiki page.
  wiki_page: 24 * 1024,
  wiki_generate: 16 * 1024,
  // Federated similar can return multiple matched repo summaries.
  federated_similar: 16 * 1024,
  mneme_federated_similar: 16 * 1024,
  // Audit findings can balloon on dirty repos. Cap so a single audit
  // call doesn't poison the agent's context — the AI can drill in
  // with audit_<scanner> + per-finding queries.
  audit: 16 * 1024,
  audit_a11y: 16 * 1024,
  audit_corpus: 16 * 1024,
  audit_perf: 16 * 1024,
  audit_security: 16 * 1024,
  audit_theme: 16 * 1024,
  audit_types: 16 * 1024,
  drift_findings: 16 * 1024,
  // Graphify corpus can return a large set of multimodal artifacts.
  graphify_corpus: 16 * 1024,
  // mneme_context is the kitchen-sink "summarize this file" tool.
  // 16 KB lets it return a meaningful chunk without enabling 232K
  // dumps.
  mneme_context: 16 * 1024,
};

/** Look up the cap for a given tool name. Falls back to DEFAULT_MAX_BYTES
 *  if no override is registered. */
export function maxBytesFor(toolName: string): number {
  return MAX_BYTES_OVERRIDES[toolName] ?? DEFAULT_MAX_BYTES;
}

/** Cap an arbitrary tool result to `maxBytes` of JSON-encoded output.
 *  Returns either the original value (when small enough) or a small
 *  envelope describing the truncation. The envelope is itself JSON-
 *  serializable and includes a preview of the original up to the cap.
 *
 *  Preserves error envelopes (objects with a top-level `error` field)
 *  verbatim regardless of size, since callers depend on that contract.
 */
export function capResult(value: unknown, maxBytes: number): unknown {
  // Pass-through for error envelopes — the agent needs the full error
  // string for actionable diagnosis.
  if (isErrorEnvelope(value)) return value;

  const json = safeStringify(value);
  if (json.length <= maxBytes) return value;

  // Build a preview that's still valid JSON-quoted text. Trim to
  // (maxBytes - envelope overhead) to ensure the wrapped envelope
  // ITSELF stays under the cap.
  const reservedForEnvelope = 256;
  const previewBudget = Math.max(maxBytes - reservedForEnvelope, 512);
  const preview = json.slice(0, previewBudget);

  return {
    _truncated: true,
    original_bytes: json.length,
    max_bytes: maxBytes,
    hint: "tool result exceeded the budget. Re-call with `detail: \"full\"` for more (capped to 16 KB), or use a more specific tool to drill into the part you need.",
    preview,
  };
}

/** True when value looks like `{ error: "..." }` — those bypass the
 *  cap so the agent can read the full failure message. */
function isErrorEnvelope(v: unknown): boolean {
  if (v === null || typeof v !== "object" || Array.isArray(v)) return false;
  const o = v as Record<string, unknown>;
  return typeof o["error"] === "string";
}

/** JSON.stringify wrapper that handles circular references gracefully.
 *  A circular structure shouldn't happen for tool results (they're
 *  serialized over MCP anyway) but defending against it costs nothing
 *  and keeps the cap from throwing. */
function safeStringify(v: unknown): string {
  try {
    return JSON.stringify(v);
  } catch {
    return "<unserializable result>";
  }
}

/** Resolve the effective cap for a (tool, args) pair, honoring the
 *  optional `detail` arg. Tools that don't pre-register an override
 *  get DEFAULT_MAX_BYTES, or DETAIL_FULL_MAX_BYTES if the caller
 *  passed `detail: "full"`. Tools that DO pre-register an override
 *  use their override regardless of `detail` (the override is the
 *  "best size we can defend"). */
export function effectiveMaxBytes(
  toolName: string,
  args: unknown,
): number {
  const override = MAX_BYTES_OVERRIDES[toolName];
  if (override !== undefined) return override;
  if (args && typeof args === "object" && !Array.isArray(args)) {
    const a = args as Record<string, unknown>;
    if (a["detail"] === "full") return DETAIL_FULL_MAX_BYTES;
  }
  return DEFAULT_MAX_BYTES;
}
