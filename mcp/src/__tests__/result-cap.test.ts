// mcp/src/__tests__/result-cap.test.ts
//
// Coverage for the central tool-result size budget. Item #118
// (2026-05-05). Five axes: pass-through under budget, truncation at
// budget, error envelope passes through, per-tool override applied,
// detail=full bumps the default cap.

import { describe, it, expect } from "bun:test";
import {
  capResult,
  effectiveMaxBytes,
  DEFAULT_MAX_BYTES,
  DETAIL_FULL_MAX_BYTES,
  MAX_BYTES_OVERRIDES,
} from "../result-cap.ts";

describe("capResult", () => {
  it("returns the original value when under budget", () => {
    const small = { items: [1, 2, 3] };
    expect(capResult(small, DEFAULT_MAX_BYTES)).toBe(small);
  });

  it("truncates oversized results into an error-envelope (HIGH-43)", () => {
    // Audit fix HIGH-43 (2026-05-06): the cap previously returned
    // a `{ _truncated, original_bytes, max_bytes, hint, preview }`
    // shape that violated every tool's published outputSchema.
    // Now it returns a `{ error: "..." }` envelope — the same
    // canonical shape used elsewhere in the MCP layer for
    // unrecoverable conditions, and the same shape that ALREADY
    // bypasses the cap at the top of capResult().
    const big = { items: Array.from({ length: 5000 }, (_, i) => `item-${i}`) };
    const capped = capResult(big, 1024) as Record<string, unknown>;
    expect(typeof capped["error"]).toBe("string");
    const err = capped["error"] as string;
    // Error message must surface the original size + cap so the AI
    // host can plan its next call (re-call with detail=full, or
    // narrow the query).
    expect(err).toContain("exceeded");
    expect(err).toContain(String(1024));
    // Must include a preview prefix of the original payload.
    expect(err).toContain("Preview");
    // The envelope itself must still be close to budget — no
    // runaway when the user payload is multi-KB.
    expect(JSON.stringify(capped).length).toBeLessThan(1024 + 512);
  });

  it("passes through error envelopes unchanged regardless of size", () => {
    // Error envelopes bypass the cap so the agent can read the full
    // failure message — actionable diagnosis matters more than tokens
    // when something has actually broken.
    const longErr = "x".repeat(10_000);
    const env = { error: longErr };
    expect(capResult(env, 1024)).toBe(env);
  });

  it("preview text is a non-trivial slice of the original (HIGH-43)", () => {
    // Same shape change as above: preview is now embedded inside
    // the `error` string rather than a separate top-level field.
    // We still verify the non-triviality contract.
    const big = { payload: "a".repeat(20_000) };
    const capped = capResult(big, 2048) as Record<string, unknown>;
    const err = capped["error"] as string;
    expect(typeof err).toBe("string");
    // Cap reserves 256 bytes for envelope, so previewBudget = 1792.
    // Error string carries the preview verbatim plus prose; it must
    // exceed the half-budget.
    expect(err.length).toBeGreaterThan(1024);
    // Error string must reference Preview + the JSON-shaped payload
    // (preview slice begins with `{` since the input is an object).
    expect(err).toContain("Preview");
    expect(err).toContain("{");
  });
});

describe("effectiveMaxBytes", () => {
  it("returns DEFAULT_MAX_BYTES for tools with no override and no detail arg", () => {
    expect(effectiveMaxBytes("recall_concept", { query: "auth" })).toBe(
      DEFAULT_MAX_BYTES,
    );
  });

  it("respects per-tool overrides verbatim", () => {
    expect(effectiveMaxBytes("architecture_overview", {})).toBe(
      (MAX_BYTES_OVERRIDES.architecture_overview ?? 0),
    );
    // architecture_overview is the headline 232K offender — must be
    // generously bumped but still bounded.
    expect((MAX_BYTES_OVERRIDES.architecture_overview ?? 0)).toBeGreaterThan(
      DEFAULT_MAX_BYTES,
    );
    expect((MAX_BYTES_OVERRIDES.architecture_overview ?? 0)).toBeLessThanOrEqual(
      32 * 1024,
    );
  });

  it("`detail: full` bumps non-overridden tools to DETAIL_FULL_MAX_BYTES", () => {
    expect(effectiveMaxBytes("recall_concept", { detail: "full" })).toBe(
      DETAIL_FULL_MAX_BYTES,
    );
  });

  it("`detail: full` does NOT downsize tools whose override is already larger", () => {
    // architecture_overview override is 32 KB; detail=full would
    // ordinarily mean 16 KB. The override wins.
    expect(
      effectiveMaxBytes("architecture_overview", { detail: "full" }),
    ).toBe((MAX_BYTES_OVERRIDES.architecture_overview ?? 0));
  });

  it("non-object args don't crash detail-arg lookup", () => {
    expect(effectiveMaxBytes("recall_concept", null)).toBe(DEFAULT_MAX_BYTES);
    expect(effectiveMaxBytes("recall_concept", "string-args")).toBe(
      DEFAULT_MAX_BYTES,
    );
    expect(effectiveMaxBytes("recall_concept", [1, 2, 3])).toBe(
      DEFAULT_MAX_BYTES,
    );
  });
});
