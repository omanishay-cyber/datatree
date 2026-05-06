// mcp/src/__tests__/federated-similar-cap.test.ts
//
// Coverage for the HIGH-21 fix's 64 KiB cap on `code_snippet`.
//
// Audit fix TEST-NEW-11 (2026-05-06 multi-agent fan-out, testing-
// reviewer): the DoS gate added `z.string().min(1).max(64*1024)` on
// the input schema, but no test pinned the boundary. A regression
// that swaps the comparison operator (`>` vs `>=`) or removes the
// cap silently re-opens the 100MB-blocks-event-loop attack vector.

import { describe, expect, it } from "bun:test";
import { FederatedSimilarInput } from "../tools/federated_similar.ts";

const MAX_BYTES = 64 * 1024; // 65_536

describe("FederatedSimilarInput.code_snippet cap", () => {
  it("accepts a snippet at the boundary (65_536 chars)", () => {
    // Zod's z.string().max(n) is inclusive (n is the maximum
    // accepted length, not the first rejected length). 65_536
    // chars must pass; 65_537 must fail. This pins both halves
    // of that boundary.
    const at = "a".repeat(MAX_BYTES);
    const parsed = FederatedSimilarInput.safeParse({ code_snippet: at });
    expect(parsed.success).toBe(true);
  });

  it("rejects one byte over the cap (65_537 chars)", () => {
    const over = "a".repeat(MAX_BYTES + 1);
    const parsed = FederatedSimilarInput.safeParse({ code_snippet: over });
    expect(parsed.success).toBe(false);
    if (!parsed.success) {
      // The Zod error must reference the size constraint specifically
      // — a future refactor that moves the cap to a different field
      // or removes the .max() entirely would let this test catch
      // the regression by name.
      const message = parsed.error.issues
        .map((i) => `${i.code}:${i.message}`)
        .join("\n");
      expect(message).toMatch(/too_big|too long|max/i);
    }
  });

  it("rejects far-over-cap input without spending O(n) work", () => {
    // 1 MiB input — proves the schema rejects without forcing the
    // downstream tokenize/simhash/minhash loops to run on attacker-
    // sized data. (The Zod check fires BEFORE the handler is
    // entered, so this test runs in microseconds even for very
    // large inputs.)
    const huge = "x".repeat(1_048_576);
    const start = performance.now();
    const parsed = FederatedSimilarInput.safeParse({ code_snippet: huge });
    const elapsed = performance.now() - start;
    expect(parsed.success).toBe(false);
    // Generous bound — sub-100ms on any modern machine. The whole
    // point of the cap is to keep this fast.
    expect(elapsed).toBeLessThan(100);
  });

  it("rejects empty snippet (the .min(1) constraint)", () => {
    const parsed = FederatedSimilarInput.safeParse({ code_snippet: "" });
    expect(parsed.success).toBe(false);
  });

  it("accepts a tiny single-character snippet", () => {
    const parsed = FederatedSimilarInput.safeParse({ code_snippet: "x" });
    expect(parsed.success).toBe(true);
  });
});
