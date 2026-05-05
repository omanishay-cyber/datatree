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

  it("truncates oversized results into a _truncated envelope", () => {
    // Build something definitely past 4 KB.
    const big = { items: Array.from({ length: 5000 }, (_, i) => `item-${i}`) };
    const capped = capResult(big, 1024) as Record<string, unknown>;
    expect(capped["_truncated"]).toBe(true);
    expect(typeof capped["original_bytes"]).toBe("number");
    expect((capped["original_bytes"] as number) > 1024).toBe(true);
    expect(capped["max_bytes"]).toBe(1024);
    expect(typeof capped["preview"]).toBe("string");
    // The wrapped envelope itself must stay close to budget — no
    // runaway. Allow ~256 bytes of envelope overhead (the constant
    // matches result-cap.ts's reservedForEnvelope).
    expect(JSON.stringify(capped).length).toBeLessThan(1024 + 256);
  });

  it("passes through error envelopes unchanged regardless of size", () => {
    // Error envelopes bypass the cap so the agent can read the full
    // failure message — actionable diagnosis matters more than tokens
    // when something has actually broken.
    const longErr = "x".repeat(10_000);
    const env = { error: longErr };
    expect(capResult(env, 1024)).toBe(env);
  });

  it("preview text is a non-trivial slice of the original", () => {
    const big = { payload: "a".repeat(20_000) };
    const capped = capResult(big, 2048) as Record<string, unknown>;
    const preview = capped["preview"] as string;
    expect(preview.length).toBeGreaterThan(512);
    // Preview is a JSON-string slice so it should start with the
    // serialized form's opening — not a meaningful invariant on its
    // own, but a useful sanity check.
    expect(preview.startsWith("{")).toBe(true);
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
