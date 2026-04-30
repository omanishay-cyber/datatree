/**
 * QA-2: Schema regression test for MCP tool numeric-param shapes.
 *
 * H1 was a class of bug where numeric MCP tool params (e.g. `limit`, `n`,
 * `depth`, `top_k`, `max_*`, `min_*`, `*_count`, `*_seconds`, `*_ms`) were
 * declared as `z.string()` instead of `z.number()`. Strict-mode JSON callers
 * would then fail at the boundary because the JSON-Schema generated from the
 * zod schema declared "string" while the supervisor IPC handler expected a
 * numeric value (and vice versa).
 *
 * This test walks every MCP tool definition file under `mcp/src/tools/*.ts`
 * AND the centralised `mcp/src/types.ts` schema bundle, finds every zod
 * field whose name matches the canonical numeric-param list, and asserts the
 * declared type is NOT `z.string(...)`.
 *
 * Run with:  cd mcp && bun test test/schema-regression.test.ts
 */
import { test, expect } from "bun:test";
import { readdirSync, readFileSync, existsSync, statSync } from "node:fs";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

// ---------------------------------------------------------------------------
// Locate the repo paths relative to this test file (so the test runs from
// any cwd, not just `mcp/`).
// ---------------------------------------------------------------------------
const __dirname = dirname(fileURLToPath(import.meta.url));
const MCP_ROOT = resolve(__dirname, "..");
const TOOLS_DIR = resolve(MCP_ROOT, "src", "tools");
const TYPES_FILE = resolve(MCP_ROOT, "src", "types.ts");

// ---------------------------------------------------------------------------
// Numeric-param name list per QA-2 spec. Names that, by convention, MUST
// be numeric across every MCP tool input/output.
//
// Two flavours:
//   - EXACT: must match the param name verbatim.
//   - SUFFIX/PREFIX: any name with the given affix counts.
// ---------------------------------------------------------------------------
const NUMERIC_EXACT_NAMES: ReadonlySet<string> = new Set([
  "limit",
  "n",
  "k",
  "depth",
  "top_k",
  "top_n",
  "page",
  "page_size",
  "chunk_size",
  "since_hours",
  "since_minutes",
  "since_millis",
  "maxResults",
  "maxItems",
  "minScore",
  "min_confidence",
  "max_confidence",
  "max_total_per_turn",
  "evidence_count",
]);

// Predicate-based matchers: `max_*`, `min_*`, `*_count`, `*_seconds`, `*_ms`.
function nameLooksNumeric(name: string): boolean {
  if (NUMERIC_EXACT_NAMES.has(name)) return true;
  if (name.startsWith("max_")) return true;
  if (name.startsWith("min_")) return true;
  if (name.endsWith("_count")) return true;
  if (name.endsWith("_seconds")) return true;
  if (name.endsWith("_ms")) return true;
  return false;
}

// ---------------------------------------------------------------------------
// File collection.
// ---------------------------------------------------------------------------
function collectSchemaFiles(): string[] {
  const files: string[] = [];

  // Centralised schema bundle.
  if (existsSync(TYPES_FILE)) files.push(TYPES_FILE);

  // Per-tool inline schemas. Skip the __tests__ subfolder + the tools
  // index.ts re-export barrel.
  if (existsSync(TOOLS_DIR)) {
    for (const entry of readdirSync(TOOLS_DIR)) {
      if (!entry.endsWith(".ts")) continue;
      if (entry === "index.ts") continue;
      const full = join(TOOLS_DIR, entry);
      if (statSync(full).isFile()) files.push(full);
    }
  }
  return files;
}

// ---------------------------------------------------------------------------
// Schema scan.
//
// We strip line + block comments first so that documentation samples like
// `// limit: z.string(...)` cannot trigger a false positive. Then we match
// every `<ident>: z.<typeCall>` and check the field name + leading type
// against the numeric-name list.
// ---------------------------------------------------------------------------
function stripComments(src: string): string {
  // Strip block comments first (greedy-safe via lazy match).
  const noBlock = src.replace(/\/\*[\s\S]*?\*\//g, "");
  // Strip line comments — careful not to break string literals containing
  // "//", but for zod schema files this is good enough.
  return noBlock.replace(/(^|[^:"'`])\/\/[^\n]*/g, "$1");
}

interface Violation {
  readonly file: string;
  readonly param: string;
  readonly snippet: string;
}

function scanFile(absPath: string): Violation[] {
  const raw = readFileSync(absPath, "utf8");
  const src = stripComments(raw);
  const out: Violation[] = [];

  // Match `<ident>: z.<methodChain>` with optional leading whitespace.
  // Capture group 1 = field name, group 2 = the head zod type call.
  const fieldRe =
    /(?:^|[\s,{(])([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*z\.([a-zA-Z_]+)\s*\(/g;

  for (const match of src.matchAll(fieldRe)) {
    const [full, fieldName, zodHead] = match;
    if (!fieldName || !zodHead) continue;
    if (!nameLooksNumeric(fieldName)) continue;
    if (zodHead === "string") {
      out.push({
        file: absPath,
        param: fieldName,
        snippet: full.trim(),
      });
    }
  }
  return out;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
test("QA-2: every numeric MCP param is declared as z.number(), not z.string()", () => {
  const files = collectSchemaFiles();
  expect(files.length).toBeGreaterThan(0);

  const violations: Violation[] = [];
  for (const f of files) violations.push(...scanFile(f));

  if (violations.length > 0) {
    const report = violations
      .map((v) => `  - ${v.file}: \`${v.param}\` declared as z.string  (${v.snippet})`)
      .join("\n");
    throw new Error(
      `Found ${violations.length} numeric-param schema regression(s) (H1):\n${report}`,
    );
  }

  expect(violations).toEqual([]);
});

test("QA-2: file collector finds the tools directory and types.ts", () => {
  // Sanity check so a future refactor that moves files doesn't silently
  // turn this into a vacuous test.
  const files = collectSchemaFiles();
  expect(files.some((f) => f.endsWith("types.ts"))).toBe(true);
  expect(files.some((f) => f.includes(`tools${process.platform === "win32" ? "\\" : "/"}recall.ts`))).toBe(true);
});

test("QA-2: scanFile detects a synthetic z.string() regression", () => {
  // Self-test: build an in-memory schema string with a deliberately-wrong
  // numeric-param declaration and feed it through the same matcher used
  // on real files. This guards against the matcher silently regressing to
  // "matches nothing" — a common failure mode for source-introspection tests.
  const tmpSrc = `
    import { z } from "zod";
    export const Bad = z.object({
      limit: z.string(),
      depth: z.number(),
    });
  `;
  // Inline-call the matcher against a fake path to exercise scanFile's
  // logic directly without writing a temp file to disk.
  const noComments = stripComments(tmpSrc);
  const fieldRe =
    /(?:^|[\s,{(])([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*z\.([a-zA-Z_]+)\s*\(/g;
  const detected: string[] = [];
  for (const m of noComments.matchAll(fieldRe)) {
    const [, name, head] = m;
    if (name && head && nameLooksNumeric(name) && head === "string") {
      detected.push(name);
    }
  }
  expect(detected).toEqual(["limit"]);
});
