// mcp/src/__tests__/tool-cache.test.ts
//
// Coverage for the negative cache that wraps MCP tool invocations.
// Item #121 (2026-05-05). Six axes: hit, miss, mutating-tool bypass,
// TTL expiry, LRU eviction, key stability across argument orderings.

import { describe, it, expect } from "bun:test";
import { ToolCache, NEVER_CACHE, wrapCachedResult } from "../tool-cache.ts";

describe("ToolCache", () => {
  it("returns undefined on miss", () => {
    const c = new ToolCache();
    expect(c.get("recall_concept", { query: "auth" })).toBeUndefined();
  });

  it("returns the value on hit with age_ms = 0", () => {
    const c = new ToolCache();
    c.set("recall_concept", { query: "auth" }, { items: [1, 2, 3] });
    const got = c.get("recall_concept", { query: "auth" });
    expect(got).toBeDefined();
    expect(got?.value).toEqual({ items: [1, 2, 3] });
    // Within the same tick the age should be ~0; allow a small slop
    // for slow runners.
    expect(got?.ageMs ?? Infinity).toBeLessThan(50);
  });

  it("bypasses cache for tools on the NEVER_CACHE list", () => {
    const c = new ToolCache();
    // Pick one mutator from the live list — `audit` is the canonical
    // example that broke #NEW-C.
    expect(NEVER_CACHE.has("audit")).toBe(true);
    c.set("audit", { project: "/tmp/proj" }, { dispatched: true });
    expect(c.get("audit", { project: "/tmp/proj" })).toBeUndefined();
  });

  it("treats {a:1,b:2} and {b:2,a:1} as the same key", () => {
    const c = new ToolCache();
    c.set("call_graph", { fn: "spawn", depth: 2 }, "result-A");
    const flipped = c.get("call_graph", { depth: 2, fn: "spawn" });
    expect(flipped?.value).toBe("result-A");
  });

  it("evicts the oldest entry when over capacity", () => {
    const c = new ToolCache(2); // capacity = 2
    c.set("recall_concept", { q: "a" }, "A");
    c.set("recall_concept", { q: "b" }, "B");
    // Insert third — should evict the oldest (q=a) since q=b was
    // never read after insertion.
    c.set("recall_concept", { q: "c" }, "C");
    expect(c.get("recall_concept", { q: "a" })).toBeUndefined();
    expect(c.get("recall_concept", { q: "b" })?.value).toBe("B");
    expect(c.get("recall_concept", { q: "c" })?.value).toBe("C");
  });

  it("expires entries past their TTL", async () => {
    const c = new ToolCache(256, 5); // 5 ms TTL
    c.set("blast_radius", { file: "x.rs" }, "R");
    expect(c.get("blast_radius", { file: "x.rs" })?.value).toBe("R");
    // Wait past TTL.
    await new Promise((r) => setTimeout(r, 20));
    expect(c.get("blast_radius", { file: "x.rs" })).toBeUndefined();
  });

  it("LRU bumps a hit so subsequent eviction picks the older miss", () => {
    const c = new ToolCache(2);
    c.set("call_graph", { id: 1 }, "one");
    c.set("call_graph", { id: 2 }, "two");
    // Read id:1 — that bumps it past id:2 in recency order.
    c.get("call_graph", { id: 1 });
    // Insert id:3 — should now evict id:2, not id:1.
    c.set("call_graph", { id: 3 }, "three");
    expect(c.get("call_graph", { id: 1 })?.value).toBe("one");
    expect(c.get("call_graph", { id: 2 })).toBeUndefined();
    expect(c.get("call_graph", { id: 3 })?.value).toBe("three");
  });
});

describe("wrapCachedResult", () => {
  it("merges _cache metadata into object results", () => {
    const w = wrapCachedResult({ items: [1, 2] }, 1234);
    expect(w).toEqual({ items: [1, 2], _cache: { hit: true, age_ms: 1234 } });
  });

  it("envelopes primitive results", () => {
    const w = wrapCachedResult(42, 5);
    expect(w).toEqual({ _cache: { hit: true, age_ms: 5 }, value: 42 });
  });

  it("envelopes array results (does not spread into _cache)", () => {
    const w = wrapCachedResult([1, 2, 3], 8);
    expect(w).toEqual({ _cache: { hit: true, age_ms: 8 }, value: [1, 2, 3] });
  });
});
