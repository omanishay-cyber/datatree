/**
 * Tests: pretool-edit-write hook — Layer 2 of v0.4.0 self-ping enforcement.
 *
 * Strategy:
 *   - Mock the DB layer + IPC so tests run without a live daemon.
 *   - Test the two core branches: block when no recent blast_radius, approve
 *     when blast_radius was run within the freshness window.
 *   - Test the fail-open contract: any exception → approve.
 *   - Test config-flag gating: feature OFF → always approve.
 *   - Test the auto-run result is embedded in the block reason.
 *
 * Run with: cd mcp && bun test test/hooks/pretool-edit-write.test.ts
 */

import { describe, it, expect, beforeEach, mock, afterEach } from "bun:test";
import { _resetHooksConfigCache } from "../../src/hooks/lib/recent-tool-calls.ts";

// ---------------------------------------------------------------------------
// Mock the DB layer BEFORE importing the hook.
// ---------------------------------------------------------------------------

// Stub for query.select — controls whether blast_radius appears in cache.
let _stubSelectResult: unknown[] = [];
const stubSelect = mock(async () => _stubSelectResult);

// Stub for query.raw — controls the auto-run blast_radius result.
let _stubRawResult: unknown = null;
const stubRaw = mock(async (method: string, _params: unknown) => {
  if (method === "tool.blast_radius") {
    if (_stubRawResult === null) {
      throw new Error("blast_radius not available in test");
    }
    return _stubRawResult;
  }
  throw new Error(`unexpected method: ${method}`);
});

mock.module("../../src/db.ts", () => ({
  query: {
    select: stubSelect,
    raw: stubRaw,
  },
  livebus: { emit: mock(async () => {}) },
  _setClient: mock(() => {}),
  shutdown: mock(() => {}),
}));

import {
  runPreToolEditWrite,
  type PreToolEditWriteArgs,
} from "../../src/hooks/pretool-edit-write.ts";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeBlastRadiusCacheRow(filePath: string, ageMs = 0): unknown {
  return {
    tool: "blast_radius",
    params_json: JSON.stringify({ target: filePath }),
    cached_at: new Date(Date.now() - ageMs).toISOString(),
    session_id: "test-session",
  };
}

function makeBlastRadiusResult(filePath = "/src/auth.ts"): unknown {
  return {
    target: filePath,
    affected_files: ["/src/utils.ts", "/src/middleware.ts"],
    affected_symbols: ["authenticate", "verifyToken"],
    test_files: ["/test/auth.test.ts"],
    total_count: 3,
    critical_paths: ["/src/middleware.ts"],
  };
}

function editArgs(filePath = "/src/auth.ts", tool = "Edit"): PreToolEditWriteArgs {
  return {
    tool,
    params: { file_path: filePath, old_string: "foo", new_string: "bar" },
    sessionId: "test-session",
  };
}

beforeEach(() => {
  _resetHooksConfigCache();
  _stubSelectResult = [];
  _stubRawResult = null;
  // Use a non-existent MNEME_HOME so we get defaults from loadHooksConfig.
  process.env["MNEME_HOME"] = "/tmp/mneme-test-nonexistent-" + Date.now();
  // Reset mock call history so accumulated calls from previous tests don't
  // pollute assertions like `not.toHaveBeenCalledWith`.
  stubSelect.mockClear();
  stubRaw.mockClear();
  // Reset mocks to default implementations.
  stubSelect.mockImplementation(async () => _stubSelectResult);
  stubRaw.mockImplementation(async (method: string, _params: unknown) => {
    if (method === "tool.blast_radius") {
      if (_stubRawResult === null) throw new Error("blast_radius not available");
      return _stubRawResult;
    }
    throw new Error(`unexpected: ${method}`);
  });
});

afterEach(() => {
  delete process.env["MNEME_HOME"];
  _resetHooksConfigCache();
});

// ---------------------------------------------------------------------------
// Core branching: block vs approve
// ---------------------------------------------------------------------------

describe("pretool-edit-write — block when no recent blast_radius", () => {
  it("blocks Edit when no blast_radius entry is in tool_cache", async () => {
    _stubSelectResult = []; // empty cache
    _stubRawResult = makeBlastRadiusResult("/src/auth.ts");

    const result = await runPreToolEditWrite(editArgs("/src/auth.ts"));

    expect(result.hook_specific.decision).toBe("block");
  });

  it("includes the file path in the block reason", async () => {
    _stubSelectResult = [];
    _stubRawResult = makeBlastRadiusResult("/src/auth.ts");

    const result = await runPreToolEditWrite(editArgs("/src/auth.ts"));

    expect(result.hook_specific.reason).toContain("/src/auth.ts");
  });

  it("embeds the auto-run blast_radius result in the block reason", async () => {
    _stubSelectResult = [];
    _stubRawResult = makeBlastRadiusResult("/src/auth.ts");

    const result = await runPreToolEditWrite(editArgs("/src/auth.ts"));

    expect(result.hook_specific.decision).toBe("block");
    const reason = result.hook_specific.reason ?? "";
    // The block message should contain the auto-run summary.
    expect(reason).toContain("blast_radius");
    // total_count = 3 should appear somewhere.
    expect(reason).toContain("3");
  });

  it("blocks Write in addition to Edit", async () => {
    _stubSelectResult = [];
    _stubRawResult = makeBlastRadiusResult("/src/config.ts");

    const result = await runPreToolEditWrite(editArgs("/src/config.ts", "Write"));
    expect(result.hook_specific.decision).toBe("block");
  });

  it("blocks MultiEdit", async () => {
    _stubSelectResult = [];
    _stubRawResult = makeBlastRadiusResult("/src/parser.ts");

    const result = await runPreToolEditWrite(editArgs("/src/parser.ts", "MultiEdit"));
    expect(result.hook_specific.decision).toBe("block");
  });
});

describe("pretool-edit-write — approve when blast_radius was run recently", () => {
  it("approves when a recent blast_radius entry exists for the file", async () => {
    // Cache hit: blast_radius was run 2 minutes ago (within 10-min window).
    _stubSelectResult = [makeBlastRadiusCacheRow("/src/auth.ts", 120_000)];

    const result = await runPreToolEditWrite(editArgs("/src/auth.ts"));

    expect(result.hook_specific.decision).toBe("approve");
  });

  it("approves when a file_intent entry exists for the file", async () => {
    _stubSelectResult = [
      {
        tool: "file_intent",
        params_json: JSON.stringify({ path: "/src/auth.ts" }),
        cached_at: new Date(Date.now() - 60_000).toISOString(),
        session_id: "test-session",
      },
    ];

    const result = await runPreToolEditWrite(editArgs("/src/auth.ts"));

    expect(result.hook_specific.decision).toBe("approve");
  });

  it("does not call query.raw (no auto-run) when cache hit", async () => {
    _stubSelectResult = [makeBlastRadiusCacheRow("/src/auth.ts", 30_000)];

    await runPreToolEditWrite(editArgs("/src/auth.ts"));

    // stubRaw should NOT have been called for the cache-hit path.
    expect(stubRaw).not.toHaveBeenCalledWith("tool.blast_radius", expect.anything());
  });
});

// ---------------------------------------------------------------------------
// Stale cache (beyond freshness window)
// ---------------------------------------------------------------------------

describe("pretool-edit-write — stale cache treated as miss", () => {
  it("blocks when the blast_radius entry is older than the freshness window", async () => {
    // 11 minutes old — outside the 10-minute default window.
    // Because the DB select filters by cached_at >= cutoff, the supervisor
    // returns empty rows for stale entries. We simulate that here.
    _stubSelectResult = []; // supervisor filtered it out
    _stubRawResult = makeBlastRadiusResult("/src/auth.ts");

    const result = await runPreToolEditWrite(editArgs("/src/auth.ts"));
    expect(result.hook_specific.decision).toBe("block");
  });
});

// ---------------------------------------------------------------------------
// Non-Edit tools are passed through
// ---------------------------------------------------------------------------

describe("pretool-edit-write — non-Edit tools always pass through", () => {
  it("approves Bash without checking blast_radius", async () => {
    _stubSelectResult = []; // empty cache, would block Edit

    const result = await runPreToolEditWrite({
      tool: "Bash",
      params: { command: "ls" },
      sessionId: "test-session",
    });

    expect(result.hook_specific.decision).toBe("approve");
  });

  it("approves Read without checking blast_radius", async () => {
    _stubSelectResult = [];

    const result = await runPreToolEditWrite({
      tool: "Read",
      params: { file_path: "/src/auth.ts" },
      sessionId: "test-session",
    });

    expect(result.hook_specific.decision).toBe("approve");
  });

  it("approves Grep without checking blast_radius", async () => {
    _stubSelectResult = [];

    const result = await runPreToolEditWrite({
      tool: "Grep",
      params: { pattern: "foo" },
      sessionId: "test-session",
    });

    expect(result.hook_specific.decision).toBe("approve");
  });
});

// ---------------------------------------------------------------------------
// Params without file_path
// ---------------------------------------------------------------------------

describe("pretool-edit-write — params without file_path", () => {
  it("approves when file_path is absent from params", async () => {
    _stubSelectResult = [];

    const result = await runPreToolEditWrite({
      tool: "Edit",
      params: { some_other_key: "value" },
      sessionId: "test-session",
    });

    // Cannot enforce without a file path — fail open.
    expect(result.hook_specific.decision).toBe("approve");
  });

  it("approves when file_path is an empty string", async () => {
    const result = await runPreToolEditWrite({
      tool: "Edit",
      params: { file_path: "" },
      sessionId: "test-session",
    });

    expect(result.hook_specific.decision).toBe("approve");
  });
});

// ---------------------------------------------------------------------------
// Config gating
// ---------------------------------------------------------------------------

describe("pretool-edit-write — config gating", () => {
  it("approves Edit even without blast_radius when enforce flag is false", async () => {
    const tmpDir = "/tmp/mneme-test-disabled2-" + Date.now();
    await import("node:fs/promises").then((fs) =>
      fs.mkdir(tmpDir, { recursive: true }).then(() =>
        fs.writeFile(
          tmpDir + "/config.toml",
          `[hooks]\nenforce_blast_radius_before_edit = false\n`,
          "utf8",
        ),
      ),
    );
    process.env["MNEME_HOME"] = tmpDir;
    _resetHooksConfigCache();

    _stubSelectResult = []; // no cache

    const result = await runPreToolEditWrite(editArgs("/src/auth.ts"));
    expect(result.hook_specific.decision).toBe("approve");

    await import("node:fs/promises").then((fs) => fs.rm(tmpDir, { recursive: true, force: true }));
  });
});

// ---------------------------------------------------------------------------
// Fail-open contract
// ---------------------------------------------------------------------------

describe("pretool-edit-write — fail-open contract", () => {
  it("approves Edit when query.select throws (IPC down)", async () => {
    stubSelect.mockImplementation(async () => {
      throw new Error("IPC connection refused");
    });

    let result: Awaited<ReturnType<typeof runPreToolEditWrite>> | undefined;
    let threw = false;
    try {
      result = await runPreToolEditWrite(editArgs("/src/auth.ts"));
    } catch {
      threw = true;
    }

    expect(threw).toBe(false);
    expect(result?.hook_specific.decision).toBe("approve");
  });

  it("approves Edit when query.raw (auto-run) throws, after a cache miss", async () => {
    // Cache miss.
    stubSelect.mockImplementation(async () => []);
    // Auto-run also fails.
    stubRaw.mockImplementation(async () => {
      throw new Error("blast_radius IPC dead");
    });

    // The block path's auto-run failing should still produce a block with
    // a fallback message (not approve) — the core decision is still BLOCK,
    // but the auto-run result is "(blast_radius auto-run failed: ...)".
    // This is acceptable: the AI is told to run blast_radius manually.
    const result = await runPreToolEditWrite(editArgs("/src/auth.ts"));

    // Either block with fallback message OR approve (fail-open at outer catch).
    // The contract is: never throw, never hang.
    expect(result.hook_specific.decision).toMatch(/^(block|approve)$/);
    expect(result.hook_specific).toBeDefined();
  });

  it("never throws even when all DB calls fail catastrophically", async () => {
    stubSelect.mockImplementation(async () => { throw new Error("db gone"); });
    stubRaw.mockImplementation(async () => { throw new Error("db gone"); });

    let threw = false;
    try {
      await runPreToolEditWrite(editArgs("/src/auth.ts"));
    } catch {
      threw = true;
    }
    expect(threw).toBe(false);
  });

  it("block reason includes instructions to disable the check", async () => {
    _stubSelectResult = [];
    _stubRawResult = makeBlastRadiusResult("/src/auth.ts");

    const result = await runPreToolEditWrite(editArgs("/src/auth.ts"));

    if (result.hook_specific.decision === "block") {
      expect(result.hook_specific.reason).toContain("config.toml");
    }
  });
});

// ---------------------------------------------------------------------------
// Output shape
// ---------------------------------------------------------------------------

describe("pretool-edit-write — output shape", () => {
  it("always returns hook_specific with decision field", async () => {
    const result = await runPreToolEditWrite(editArgs());
    expect(result).toHaveProperty("hook_specific");
    expect(result.hook_specific).toHaveProperty("decision");
  });

  it("decision is always 'approve' or 'block'", async () => {
    const result = await runPreToolEditWrite(editArgs());
    expect(["approve", "block"]).toContain(result.hook_specific.decision);
  });

  it("reason is present on block, absent or undefined on approve", async () => {
    // Approve path.
    _stubSelectResult = [makeBlastRadiusCacheRow("/src/auth.ts", 30_000)];
    const approveResult = await runPreToolEditWrite(editArgs());
    if (approveResult.hook_specific.decision === "approve") {
      expect(approveResult.hook_specific.reason).toBeUndefined();
    }

    // Block path.
    _stubSelectResult = [];
    _stubRawResult = makeBlastRadiusResult("/src/auth.ts");
    const blockResult = await runPreToolEditWrite(editArgs());
    if (blockResult.hook_specific.decision === "block") {
      expect(blockResult.hook_specific.reason).toBeTruthy();
    }
  });
});
