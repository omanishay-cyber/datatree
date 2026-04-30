/**
 * Self-contained parser tests.
 *
 * These tests run under a minimal vitest-style harness so we don't have
 * to pull vitest/jest/mocha into the extension package. The harness
 * lives at the top of this file and supports describe, it, and basic
 * expect(x).toEqual(y).
 *
 * Run with: `npm test` (from the vscode/ directory) after `tsc -p .`,
 * or directly: `node out/test/parse.test.js`.
 *
 * Exits with code 0 on success, 1 on any failure.
 */

import {
  parseRecallHits,
  parseBlast,
  parseGodNodes,
  parseDrift,
  parseSteps,
  parseDecisions,
  parseShards,
  humanBytes,
  humanAge,
  looksInstalled,
} from "../util/parse";

// ----- Test harness (hoisted; TS strict doesn't allow forward refs
// across module-level const/let, so declarations live up top). -----

interface Expectation {
  toEqual(expected: unknown): void;
}

interface Suite {
  readonly name: string;
  readonly cases: Array<{ readonly label: string; readonly fn: () => void }>;
}

const suites: Suite[] = [];
let current: Suite | null = null;

function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) {
    return true;
  }
  if (typeof a !== typeof b) {
    return false;
  }
  if (a === null || b === null) {
    return false;
  }
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) {
      return false;
    }
    for (let i = 0; i < a.length; i++) {
      if (!deepEqual(a[i], b[i])) {
        return false;
      }
    }
    return true;
  }
  if (typeof a === "object" && typeof b === "object") {
    const ak = Object.keys(a as Record<string, unknown>).sort();
    const bk = Object.keys(b as Record<string, unknown>).sort();
    if (ak.length !== bk.length) {
      return false;
    }
    for (let i = 0; i < ak.length; i++) {
      if (ak[i] !== bk[i]) {
        return false;
      }
      if (
        !deepEqual(
          (a as Record<string, unknown>)[ak[i]],
          (b as Record<string, unknown>)[bk[i]],
        )
      ) {
        return false;
      }
    }
    return true;
  }
  return false;
}

function expect(actual: unknown): Expectation {
  return {
    toEqual(expected: unknown): void {
      if (!deepEqual(actual, expected)) {
        throw new Error(
          `expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`,
        );
      }
    },
  };
}

function describe(name: string, fn: () => void): void {
  const suite: Suite = { name, cases: [] };
  current = suite;
  fn();
  current = null;
  suites.push(suite);
}

function it(label: string, fn: () => void): void {
  if (!current) {
    throw new Error("it() called outside describe()");
  }
  current.cases.push({ label, fn });
}

// ----- Actual tests -----

describe("parseRecallHits", () => {
  it("parses a plain hit with file:line", () => {
    const hits = parseRecallHits("[function] do_thing src/foo.rs:42");
    expect(hits.length).toEqual(1);
    expect(hits[0].kind).toEqual("function");
    expect(hits[0].name).toEqual("do_thing");
    expect(hits[0].file).toEqual("src/foo.rs");
    expect(hits[0].line).toEqual(42);
  });

  it("tolerates missing location", () => {
    const hits = parseRecallHits("[decision] switched to sqlite");
    expect(hits.length).toEqual(1);
    expect(hits[0].kind).toEqual("decision");
    expect(hits[0].file).toEqual(null);
    expect(hits[0].line).toEqual(null);
  });

  it("skips non-matching garbage", () => {
    const hits = parseRecallHits("garbage\n[fn] valid src/x.rs:1\nmore garbage");
    expect(hits.length).toEqual(1);
    expect(hits[0].name).toEqual("valid");
  });

  it("handles path:line:col suffix", () => {
    const hits = parseRecallHits("[fn] foo src/bar.rs:12:4");
    expect(hits[0].file).toEqual("src/bar.rs");
    expect(hits[0].line).toEqual(12);
  });
});

describe("parseBlast", () => {
  it("reads header + sites", () => {
    const raw = [
      "=> callers: 4    transitive: 17",
      "  - src/foo.rs:12  do_thing",
      "  - src/bar.rs:99  helper",
    ].join("\n");
    const result = parseBlast(raw);
    expect(result.directCallers).toEqual(4);
    expect(result.transitiveCallers).toEqual(17);
    expect(result.sites.length).toEqual(2);
    expect(result.sites[0].file).toEqual("src/foo.rs");
    expect(result.sites[0].line).toEqual(12);
    expect(result.sites[0].symbol).toEqual("do_thing");
  });

  it("defaults to zero on empty input", () => {
    const result = parseBlast("");
    expect(result.directCallers).toEqual(0);
    expect(result.transitiveCallers).toEqual(0);
    expect(result.sites.length).toEqual(0);
  });
});

describe("parseGodNodes", () => {
  it("parses tab-separated degree/name/kind/file", () => {
    const raw = ["42\tFooBar\tstruct\tsrc/a.rs", "17\tbaz\tfunction\tsrc/b.rs"].join("\n");
    const nodes = parseGodNodes(raw);
    expect(nodes.length).toEqual(2);
    expect(nodes[0].degree).toEqual(42);
    expect(nodes[0].name).toEqual("FooBar");
    expect(nodes[0].kind).toEqual("struct");
    expect(nodes[0].file).toEqual("src/a.rs");
  });

  it("drops lines without a numeric degree", () => {
    const raw = "not-a-number\tFoo\tkind\tpath";
    const nodes = parseGodNodes(raw);
    expect(nodes.length).toEqual(0);
  });
});

describe("parseDrift", () => {
  it("coerces severity and splits messages", () => {
    const raw = [
      "critical\ttheme\tsrc/a.rs:3\thardcoded color",
      "should-fix\ttypes\tsrc/b.ts:99\tany used",
      "info\ta11y\tsrc/c.tsx:42\tmissing aria-label",
    ].join("\n");
    const findings = parseDrift(raw);
    expect(findings.length).toEqual(3);
    expect(findings[0].severity).toEqual("critical");
    expect(findings[1].severity).toEqual("should-fix");
    expect(findings[2].severity).toEqual("info");
    expect(findings[2].message).toEqual("missing aria-label");
  });

  it("normalises alias severities", () => {
    const raw = "error\ttheme\tsrc/a.rs:3\toops";
    const findings = parseDrift(raw);
    expect(findings[0].severity).toEqual("critical");
  });
});

describe("parseSteps", () => {
  it("reads checkbox markers", () => {
    const raw = [
      "[x] 1. Implement auth",
      "[v] 2. Write tests",
      "[ ] 3. Ship",
      "[!] 4. Blocked on CI",
    ].join("\n");
    const steps = parseSteps(raw);
    expect(steps.length).toEqual(4);
    expect(steps[0].status).toEqual("done");
    expect(steps[1].status).toEqual("verified");
    expect(steps[2].status).toEqual("pending");
    expect(steps[3].status).toEqual("blocked");
  });
});

describe("parseDecisions", () => {
  it("parses iso timestamp + summary + transcript path", () => {
    const raw = "2026-04-23T18:00:00Z\tSwitched to sqlite\tdocs/decisions/001.md";
    const decisions = parseDecisions(raw);
    expect(decisions.length).toEqual(1);
    expect(decisions[0].summary).toEqual("Switched to sqlite");
    expect(decisions[0].transcriptPath).toEqual("docs/decisions/001.md");
  });

  it("handles missing transcript", () => {
    const raw = "2026-04-23T18:00:00Z\tNo transcript yet";
    const decisions = parseDecisions(raw);
    expect(decisions[0].transcriptPath).toEqual(null);
  });
});

describe("parseShards", () => {
  it("parses name/size/timestamp/path", () => {
    const raw = "orion\t104857600\t2026-04-24T12:00:00Z\t/home/anish/.mneme/projects/orion";
    const shards = parseShards(raw);
    expect(shards.length).toEqual(1);
    expect(shards[0].name).toEqual("orion");
    expect(shards[0].sizeBytes).toEqual(104857600);
    expect(shards[0].path).toEqual("/home/anish/.mneme/projects/orion");
  });
});

describe("humanBytes", () => {
  it("scales units", () => {
    expect(humanBytes(512)).toEqual("512 B");
    expect(humanBytes(2048)).toEqual("2.0 KB");
    expect(humanBytes(10 * 1024 * 1024)).toEqual("10.0 MB");
  });
});

describe("humanAge", () => {
  it("returns ? for bad input", () => {
    expect(humanAge("not-a-date")).toEqual("?");
  });

  it("returns 'just now' for future timestamps", () => {
    const future = new Date(Date.now() + 60_000).toISOString();
    expect(humanAge(future)).toEqual("just now");
  });
});

describe("looksInstalled", () => {
  it("accepts a valid version banner", () => {
    expect(looksInstalled("mneme 0.3.0")).toEqual(true);
  });

  it("rejects garbage", () => {
    expect(looksInstalled("command not found")).toEqual(false);
  });
});

// ----- Runner -----

function main(): number {
  let passed = 0;
  let failed = 0;
  const failures: Array<{ suite: string; label: string; error: string }> = [];
  for (const suite of suites) {
    for (const c of suite.cases) {
      try {
        c.fn();
        passed++;
      } catch (err) {
        failed++;
        const msg = err instanceof Error ? err.message : String(err);
        failures.push({ suite: suite.name, label: c.label, error: msg });
      }
    }
  }
  const total = passed + failed;
  // eslint-disable-next-line no-console
  console.log(`mneme test: ${passed}/${total} passed`);
  for (const f of failures) {
    // eslint-disable-next-line no-console
    console.log(`  FAIL ${f.suite} > ${f.label}: ${f.error}`);
  }
  return failed === 0 ? 0 : 1;
}

// Auto-run when executed directly.
if (require.main === module) {
  process.exit(main());
}

export { main as runTests };
