/**
 * Tests: userprompt-submit hook — Layer 1 of v0.4.0 self-ping enforcement.
 *
 * Strategy:
 *   - We do NOT spawn a live daemon. The db.ts layer is mocked so the tests
 *     are fast, isolated, and pass without a running mneme supervisor.
 *   - We test the pure logic: tool ranking, reminder block shape, trespass
 *     log formatting, config-flag gating.
 *   - We test the fail-open contract: any exception path returns empty context,
 *     never throws.
 *
 * Run with: cd mcp && bun test test/hooks/userprompt-submit.test.ts
 */

import { describe, it, expect, beforeEach, mock, afterEach } from "bun:test";
import { _resetHooksConfigCache } from "../../src/hooks/lib/recent-tool-calls.ts";

// ---------------------------------------------------------------------------
// Mock the DB layer BEFORE importing the hook under test.
// We use a call-sequence approach: _selectResponses is a queue. Each call to
// stubSelect returns the first element (or [] if exhausted). This lets
// individual tests produce different results for successive calls.
// ---------------------------------------------------------------------------

let _selectResponses: Array<unknown[]> = [];
let _selectCallCount = 0;
let _selectShouldThrow = false;

const stubSelect = mock(async () => {
  _selectCallCount++;
  if (_selectShouldThrow) {
    throw new Error("IPC down");
  }
  // Pop from queue; fall back to empty if queue is exhausted.
  return _selectResponses.shift() ?? [];
});

mock.module("../../src/db.ts", () => ({
  query: {
    select: stubSelect,
    raw: mock(async () => {
      throw new Error("not implemented in tests");
    }),
  },
  livebus: { emit: mock(async () => {}) },
  _setClient: mock(() => {}),
  shutdown: mock(() => {}),
}));

// Import AFTER mocking so the mock is in place when the hook module loads.
import {
  runUserPromptSubmit,
  type UserPromptSubmitArgs,
} from "../../src/hooks/userprompt-submit.ts";

// ---------------------------------------------------------------------------
// Test setup helpers
// ---------------------------------------------------------------------------

/** Build a minimal args object for the hook. */
function makeArgs(prompt: string, sessionId = "test-session"): UserPromptSubmitArgs {
  return { prompt, sessionId, cwd: "/tmp/test-project" };
}

/** Queue select responses for successive calls. */
function setSelectSequence(...responses: Array<unknown[]>): void {
  _selectResponses = responses;
}

beforeEach(() => {
  // Reset config singleton between tests so MNEME_HOME changes take effect.
  _resetHooksConfigCache();
  // Default state: empty queue, don't throw.
  _selectResponses = [];
  _selectCallCount = 0;
  _selectShouldThrow = false;
  // Point MNEME_HOME at a non-existent path so loadHooksConfig returns defaults.
  process.env["MNEME_HOME"] = "/tmp/mneme-test-nonexistent-" + Date.now();
});

afterEach(() => {
  delete process.env["MNEME_HOME"];
  _resetHooksConfigCache();
  _selectShouldThrow = false;
  _selectResponses = [];
});

// ---------------------------------------------------------------------------
// Layer 1A: Tool ranking
// ---------------------------------------------------------------------------

describe("userprompt-submit — tool ranking", () => {
  it("returns exactly 3 tool recommendations for any prompt", async () => {
    const result = await runUserPromptSubmit(makeArgs("edit the auth module"));
    const ctx = result.hook_specific.additionalContext;
    expect(ctx).toBeTruthy();
    // Count the bullet points in the "Top 3" section.
    const bullets = [...ctx.matchAll(/^\s+•\s+mcp__mneme__/gm)];
    expect(bullets).toHaveLength(3);
  });

  it("puts blast_radius first for edit/change prompts", async () => {
    const result = await runUserPromptSubmit(makeArgs("edit src/auth.ts to fix the login bug"));
    const ctx = result.hook_specific.additionalContext;
    const firstBullet = ctx.match(/•\s+(mcp__mneme__\w+)/)?.[1];
    expect(firstBullet).toBe("mcp__mneme__blast_radius");
  });

  it("puts mneme_recall first for why/history prompts", async () => {
    const result = await runUserPromptSubmit(makeArgs("why was this decision made last time?"));
    const ctx = result.hook_specific.additionalContext;
    const firstBullet = ctx.match(/•\s+(mcp__mneme__\w+)/)?.[1];
    expect(firstBullet).toBe("mcp__mneme__mneme_recall");
  });

  it("includes find_references for rename/symbol prompts", async () => {
    const result = await runUserPromptSubmit(makeArgs("rename the function handleAuth everywhere"));
    const ctx = result.hook_specific.additionalContext;
    expect(ctx).toContain("mcp__mneme__find_references");
  });

  it("emits empty context for a simple-intent prompt (Item #119 contract)", async () => {
    // Audit fix (2026-05-06): Item #119 introduced simple/code/resume
    // tiers; "hello" correctly classifies as "simple" and pays zero
    // tokens. The previous version of this test asserted 3 bullets,
    // which was the pre-Item-#119 contract — every prompt got a
    // reminder block regardless of intent. Updated to match current
    // behaviour. A non-simple prompt's 3-bullet shape is covered by
    // the existing "puts mneme_recall first…" / "ranks edit prompts
    // toward blast_radius" tests.
    const result = await runUserPromptSubmit(makeArgs("hello"));
    expect(result.hook_specific.additionalContext).toBe("");
  });

  it("returns 3 tools for a code-intent generic prompt", async () => {
    // A generic code-flavoured prompt with no specific keyword
    // should still backfill 3 tools by priority order.
    const result = await runUserPromptSubmit(makeArgs("edit something"));
    const ctx = result.hook_specific.additionalContext;
    const bullets = [...ctx.matchAll(/^\s+•\s+mcp__mneme__/gm)];
    expect(bullets).toHaveLength(3);
  });

  it("includes a Why line for each tool recommendation", async () => {
    const result = await runUserPromptSubmit(makeArgs("fix the bug in the parser"));
    const ctx = result.hook_specific.additionalContext;
    const whyLines = [...ctx.matchAll(/^\s+Why:/gm)];
    expect(whyLines).toHaveLength(3);
  });
});

// ---------------------------------------------------------------------------
// Layer 1B: Trespass log
// ---------------------------------------------------------------------------

describe("userprompt-submit — trespass log", () => {
  it("shows no trespass section when DB returns empty rows", async () => {
    // Empty queue — all calls return [].
    const result = await runUserPromptSubmit(makeArgs("edit the parser"));
    const ctx = result.hook_specific.additionalContext;
    expect(ctx).not.toContain("Trespass log");
  });

  it("shows trespass section when Grep calls without prior mneme exist", async () => {
    // getSessionTrespasses does two kinds of DB calls:
    //   1. select for Grep/Read rows (returns our rows)
    //   2. for each row, select to check if mneme was called for that path (returns [])
    // We must model both: first call returns rows, subsequent calls return [].

    const grepRows = [
      {
        tool: "Grep",
        params_json: JSON.stringify({ pattern: "handleAuth" }),
        cached_at: new Date(Date.now() - 60_000).toISOString(),
        session_id: "test-session",
      },
    ];

    // First call: returns the grep row. Second call (mneme recall check): empty.
    setSelectSequence(grepRows, []);

    const result = await runUserPromptSubmit(makeArgs("fix auth", "test-session"));
    const ctx = result.hook_specific.additionalContext;
    expect(ctx).toContain("Trespass log");
  });

  it("includes the tool name and path in each trespass entry", async () => {
    const readRows = [
      {
        tool: "Read",
        params_json: JSON.stringify({ file_path: "/src/auth.ts" }),
        cached_at: new Date(Date.now() - 30_000).toISOString(),
        session_id: "test-session",
      },
    ];
    // First call: rows. Second call (mneme check per row): empty.
    setSelectSequence(readRows, []);

    const result = await runUserPromptSubmit(makeArgs("look at auth", "test-session"));
    const ctx = result.hook_specific.additionalContext;
    if (ctx.includes("Trespass log")) {
      expect(ctx).toContain("Read");
      expect(ctx).toContain("/src/auth.ts");
    }
  });
});

// ---------------------------------------------------------------------------
// Layer 1C: Config gating
// ---------------------------------------------------------------------------

describe("userprompt-submit — config gating", () => {
  it("returns empty context when inject_user_prompt_reminder = false", async () => {
    const tmpDir = "/tmp/mneme-test-disabled-" + Date.now();
    const fsp = await import("node:fs/promises");
    await fsp.mkdir(tmpDir, { recursive: true });
    await fsp.writeFile(tmpDir + "/config.toml", "[hooks]\ninject_user_prompt_reminder = false\n", "utf8");

    // Must set MNEME_HOME and reset cache AFTER writing the file.
    process.env["MNEME_HOME"] = tmpDir;
    _resetHooksConfigCache();

    const result = await runUserPromptSubmit(makeArgs("edit the parser"));
    expect(result.hook_specific.additionalContext).toBe("");

    await fsp.rm(tmpDir, { recursive: true, force: true });
  });
});

// ---------------------------------------------------------------------------
// Layer 1D: Block shape
// ---------------------------------------------------------------------------

describe("userprompt-submit — output shape", () => {
  it("always returns hook_specific.additionalContext key", async () => {
    const result = await runUserPromptSubmit(makeArgs("edit src/foo.ts"));
    expect(result).toHaveProperty("hook_specific");
    expect(result.hook_specific).toHaveProperty("additionalContext");
  });

  it("wraps output in mneme-self-ping XML tags when enabled", async () => {
    const result = await runUserPromptSubmit(makeArgs("edit src/foo.ts"));
    const ctx = result.hook_specific.additionalContext;
    if (ctx.length > 0) {
      expect(ctx.trim()).toMatch(/^<mneme-self-ping>/);
      expect(ctx.trim()).toMatch(/<\/mneme-self-ping>$/);
    }
  });
});

// ---------------------------------------------------------------------------
// Layer 1E: Fail-open
// ---------------------------------------------------------------------------

describe("userprompt-submit — fail-open contract", () => {
  it("does not throw when DB query throws", async () => {
    _selectShouldThrow = true;

    let result: Awaited<ReturnType<typeof runUserPromptSubmit>> | undefined;
    let threw = false;
    try {
      result = await runUserPromptSubmit(makeArgs("edit src/auth.ts"));
    } catch {
      threw = true;
    }

    expect(threw).toBe(false);
    expect(result).toBeDefined();
    expect(result?.hook_specific).toHaveProperty("additionalContext");
    // Tool ranking does NOT require DB — reminder block still shows.
    // The hook must not propagate the DB error.
  });

  it("never exits with a non-zero code from the hook logic itself", async () => {
    const result = await runUserPromptSubmit({
      prompt: "",
      sessionId: "test-session",
      cwd: "",
    });
    expect(result.hook_specific).toBeDefined();
  });

  it("returns a result (possibly with tool list) even when trespass query fails", async () => {
    // DB throws for trespass query — tool ranking (pure function) still works.
    _selectShouldThrow = true;

    const result = await runUserPromptSubmit(makeArgs("edit the auth module"));
    // Either empty string (fail-open) or the tool reminder.
    expect(typeof result.hook_specific.additionalContext).toBe("string");
    expect(result.hook_specific.additionalContext).toBeDefined();
  });
});
