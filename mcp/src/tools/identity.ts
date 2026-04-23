/**
 * MCP tool: mneme_identity
 *
 * Exposes the Project Identity Kernel (blueprint F9). The Rust brain crate
 * also builds this same structure during `mneme build`; this tool is the
 * read path — it queries the on-disk shard plus the live project tree and
 * returns a compact JSON document suitable for injection into any AI
 * harness' SessionStart primer.
 *
 * v0.2: we reconstruct the kernel from local signals (filesystem manifests
 * + README + conventions.db). The supervisor has the authoritative copy
 * once `mneme build` has run; this tool gracefully degrades when the shard
 * isn't populated yet.
 */

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { openShardDb, resolveShardRoot } from "../store.ts";
import type { ToolDescriptor } from "../types.ts";

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

const Technology = z.object({
  name: z.string(),
  version: z.string().nullable(),
  marker: z.string(),
  category: z.enum(["Language", "Runtime", "Framework", "BuildTool", "Tooling"]),
});

const KeyConcept = z.object({
  term: z.string(),
  score: z.number(),
});

const ConventionSummary = z.object({
  id: z.string(),
  kind: z.string(),
  description: z.string(),
  confidence: z.number(),
  evidence_count: z.number().int(),
});

export const IdentityInput = z
  .object({
    project: z.string().optional(),
  })
  .default({});

export const IdentityOutput = z.object({
  name: z.string(),
  stack: z.array(Technology),
  domain_summary: z.string(),
  key_concepts: z.array(KeyConcept),
  conventions: z.array(ConventionSummary),
  recent_goals: z.array(z.string()),
  open_questions: z.array(z.string()),
});

type IdentityInputT = z.infer<typeof IdentityInput>;
type IdentityOutputT = z.infer<typeof IdentityOutput>;
type TechnologyT = z.infer<typeof Technology>;

// ---------------------------------------------------------------------------
// Stack detection — mirrors `brain::identity::detect_stack`
// ---------------------------------------------------------------------------

interface JsFrameworkProbe {
  dep: string;
  name: string;
  category: TechnologyT["category"];
}

const JS_PROBES: JsFrameworkProbe[] = [
  { dep: "electron", name: "electron", category: "Framework" },
  { dep: "react", name: "react", category: "Framework" },
  { dep: "next", name: "next.js", category: "Framework" },
  { dep: "vue", name: "vue", category: "Framework" },
  { dep: "svelte", name: "svelte", category: "Framework" },
  { dep: "solid-js", name: "solid", category: "Framework" },
  { dep: "@tauri-apps/api", name: "tauri", category: "Framework" },
  { dep: "vite", name: "vite", category: "BuildTool" },
  { dep: "webpack", name: "webpack", category: "BuildTool" },
  { dep: "esbuild", name: "esbuild", category: "BuildTool" },
  { dep: "turbo", name: "turborepo", category: "BuildTool" },
  { dep: "typescript", name: "typescript", category: "Language" },
  { dep: "tailwindcss", name: "tailwind", category: "Tooling" },
  { dep: "zustand", name: "zustand", category: "Tooling" },
  { dep: "vitest", name: "vitest", category: "Tooling" },
  { dep: "jest", name: "jest", category: "Tooling" },
  { dep: "playwright", name: "playwright", category: "Tooling" },
];

function pushUnique(arr: TechnologyT[], tech: TechnologyT): void {
  if (!arr.some((t) => t.name === tech.name)) {
    arr.push(tech);
  }
}

function detectStack(root: string): TechnologyT[] {
  const out: TechnologyT[] = [];

  // Node ecosystem.
  const pkgJsonPath = join(root, "package.json");
  if (existsSync(pkgJsonPath)) {
    out.push({
      name: "node",
      version: null,
      marker: "package.json",
      category: "Runtime",
    });
    try {
      const pkg = JSON.parse(readFileSync(pkgJsonPath, "utf-8")) as {
        dependencies?: Record<string, string>;
        devDependencies?: Record<string, string>;
      };
      const allDeps: Record<string, string> = {
        ...(pkg.dependencies ?? {}),
        ...(pkg.devDependencies ?? {}),
      };
      for (const probe of JS_PROBES) {
        const raw = allDeps[probe.dep];
        if (raw !== undefined) {
          const version = raw.length > 0 ? raw.replace(/^[~^]/, "") : null;
          pushUnique(out, {
            name: probe.name,
            version,
            marker: "package.json",
            category: probe.category,
          });
        }
      }
    } catch {
      // malformed package.json — keep the node marker but skip probes
    }
  }

  // Bun.
  if (existsSync(join(root, "bun.lockb")) || existsSync(join(root, "bunfig.toml"))) {
    pushUnique(out, {
      name: "bun",
      version: null,
      marker: "bun.lockb",
      category: "Runtime",
    });
  }

  // Deno.
  if (existsSync(join(root, "deno.json")) || existsSync(join(root, "deno.jsonc"))) {
    out.push({
      name: "deno",
      version: null,
      marker: "deno.json",
      category: "Runtime",
    });
  }

  // Rust.
  const cargoPath = join(root, "Cargo.toml");
  if (existsSync(cargoPath)) {
    out.push({
      name: "rust",
      version: null,
      marker: "Cargo.toml",
      category: "Language",
    });
    try {
      const text = readFileSync(cargoPath, "utf-8");
      if (text.includes("tauri")) {
        pushUnique(out, {
          name: "tauri",
          version: null,
          marker: "Cargo.toml",
          category: "Framework",
        });
      }
      if (text.includes("tokio")) {
        pushUnique(out, {
          name: "tokio",
          version: null,
          marker: "Cargo.toml",
          category: "Framework",
        });
      }
    } catch {
      // ignore
    }
  }

  // Python.
  if (existsSync(join(root, "requirements.txt"))) {
    out.push({
      name: "python",
      version: null,
      marker: "requirements.txt",
      category: "Language",
    });
  }
  if (existsSync(join(root, "pyproject.toml"))) {
    pushUnique(out, {
      name: "python",
      version: null,
      marker: "pyproject.toml",
      category: "Language",
    });
  }

  // Go.
  if (existsSync(join(root, "go.mod"))) {
    out.push({
      name: "go",
      version: null,
      marker: "go.mod",
      category: "Language",
    });
  }

  // Ruby / Java / PHP / .NET.
  if (existsSync(join(root, "Gemfile"))) {
    out.push({
      name: "ruby",
      version: null,
      marker: "Gemfile",
      category: "Language",
    });
  }
  if (existsSync(join(root, "pom.xml"))) {
    out.push({
      name: "java",
      version: null,
      marker: "pom.xml",
      category: "Language",
    });
  }
  if (existsSync(join(root, "composer.json"))) {
    out.push({
      name: "php",
      version: null,
      marker: "composer.json",
      category: "Language",
    });
  }
  try {
    const entries = readdirSync(root);
    if (entries.some((e) => e.endsWith(".csproj") || e.endsWith(".sln"))) {
      out.push({
        name: "dotnet",
        version: null,
        marker: ".csproj",
        category: "Runtime",
      });
    }
  } catch {
    // unreadable root — ignore
  }

  return out;
}

// ---------------------------------------------------------------------------
// README first paragraph
// ---------------------------------------------------------------------------

function firstNonHeadingParagraph(text: string): string {
  let buf = "";
  let insideHtml = false;
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (line.startsWith("<") && !line.startsWith("</")) {
      insideHtml = true;
    }
    if (insideHtml) {
      if (line.startsWith("</") || (line.endsWith(">") && line.includes("</"))) {
        insideHtml = false;
      }
      continue;
    }
    if (line === "") {
      if (buf !== "") break;
      continue;
    }
    if (line.startsWith("#")) continue;
    if (line.startsWith("---")) continue;
    if (line.startsWith("![") || line.startsWith("[!")) continue;
    if (line.startsWith("[") && line.endsWith(")") && !line.includes(". ")) continue;
    if (buf !== "") buf += " ";
    buf += line;
  }
  const MAX = 500;
  if (buf.length > MAX) return `${buf.slice(0, MAX)}…`;
  return buf;
}

function readDomainSummary(root: string): string {
  for (const name of ["README.md", "Readme.md", "readme.md"]) {
    const p = join(root, name);
    if (existsSync(p)) {
      try {
        return firstNonHeadingParagraph(readFileSync(p, "utf-8"));
      } catch {
        // ignore
      }
    }
  }
  return "";
}

// ---------------------------------------------------------------------------
// Project name
// ---------------------------------------------------------------------------

function inferProjectName(root: string): string {
  const pkg = join(root, "package.json");
  if (existsSync(pkg)) {
    try {
      const v = JSON.parse(readFileSync(pkg, "utf-8")) as { name?: string };
      if (v.name) return v.name;
    } catch {
      // ignore
    }
  }
  const cargo = join(root, "Cargo.toml");
  if (existsSync(cargo)) {
    try {
      const text = readFileSync(cargo, "utf-8");
      let inPackage = false;
      for (const line of text.split(/\r?\n/)) {
        const t = line.trim();
        if (t.startsWith("[")) {
          inPackage = t === "[package]";
          continue;
        }
        if (inPackage && t.startsWith("name")) {
          const m = t.match(/=\s*"([^"]+)"/);
          if (m && m[1]) return m[1];
        }
      }
    } catch {
      // ignore
    }
  }
  const parts = root.split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] ?? "project";
}

// ---------------------------------------------------------------------------
// Conventions read from the shard
// ---------------------------------------------------------------------------

interface ConventionRow {
  id: string;
  pattern_kind: string;
  pattern_json: string;
  confidence: number;
  evidence_count: number;
}

function readTopConventions(
  cwd: string,
  limit: number,
): z.infer<typeof ConventionSummary>[] {
  try {
    const db = openShardDb("conventions", cwd);
    try {
      const rows = db
        .prepare(
          `SELECT id, pattern_kind, pattern_json, confidence, evidence_count
             FROM conventions
             ORDER BY confidence DESC, evidence_count DESC
             LIMIT ?`,
        )
        .all(limit) as ConventionRow[];
      return rows.map((r) => ({
        id: r.id,
        kind: r.pattern_kind,
        description: describeConvention(r.pattern_kind, r.pattern_json),
        confidence: r.confidence,
        evidence_count: r.evidence_count,
      }));
    } finally {
      db.close();
    }
  } catch {
    return [];
  }
}

function describeConvention(kind: string, patternJson: string): string {
  try {
    const p = JSON.parse(patternJson) as Record<string, unknown>;
    switch (kind) {
      case "naming": {
        const scope = p.scope as string | undefined;
        const style = p.style as string | undefined;
        return `${scope ?? "?"} uses ${style ?? "?"}`;
      }
      case "import_order": {
        const order = (p.order as string[] | undefined) ?? [];
        return `import order: ${order.join(" → ")}`;
      }
      case "error_handling":
        return `errors: ${(p.pattern as string) ?? "?"}`;
      case "test_layout": {
        const loc = (p.colocated as boolean) ? "colocated" : "separate dir";
        return `tests are ${loc} (${(p.naming as string) ?? "?"})`;
      }
      case "dependency":
        return `prefers ${(p.prefers as string) ?? "?"}`;
      case "component_shape":
        return `components: ${(p.prefers as string) ?? "?"}`;
      default:
        return kind;
    }
  } catch {
    return kind;
  }
}

// ---------------------------------------------------------------------------
// Concept extraction (ported from brain::concept::deterministic)
// ---------------------------------------------------------------------------

const STOP = new Set([
  "the",
  "a",
  "an",
  "of",
  "to",
  "for",
  "and",
  "or",
  "is",
  "in",
  "on",
  "at",
  "as",
  "if",
  "by",
  "be",
  "fn",
  "def",
  "self",
  "this",
  "that",
  "do",
  "it",
]);

function extractConcepts(text: string): z.infer<typeof KeyConcept>[] {
  const bag = new Map<string, number>();
  const addTerm = (term: string, score: number): void => {
    const norm = term
      .trim()
      .toLowerCase()
      .split(/\s+/)
      .filter((w) => w.length > 1 && !STOP.has(w))
      .join(" ");
    if (!norm) return;
    const prev = bag.get(norm) ?? 0;
    if (score > prev) bag.set(norm, score);
  };

  // Headings.
  for (const m of text.matchAll(/(?:^|\n)\s{0,3}#{1,6}\s+(.+?)\s*#*\s*(?:\n|$)/g)) {
    if (m[1]) addTerm(m[1], 0.85);
  }

  // Capitalised noun phrases (1-3 words).
  for (const m of text.matchAll(/\b([A-Z][a-z]+(?:\s+[A-Z][a-z]+){0,2})\b/g)) {
    if (m[1]) addTerm(m[1], 0.55);
  }

  // Identifier declarations.
  for (const m of text.matchAll(
    /\b(?:fn|def|class|struct|enum|interface|trait|impl)\s+([A-Za-z_][A-Za-z0-9_]*)/g,
  )) {
    if (m[1]) {
      const words = m[1]
        .replace(/([a-z])([A-Z])/g, "$1 $2")
        .replace(/_/g, " ")
        .toLowerCase()
        .split(/\s+/)
        .filter((w) => w.length > 1 && !STOP.has(w));
      if (words.length > 0) addTerm(words.join(" "), 0.7);
    }
  }

  return [...bag.entries()]
    .map(([term, score]) => ({ term, score }))
    .sort((a, b) => b.score - a.score)
    .slice(0, 10);
}

// ---------------------------------------------------------------------------
// Tool descriptor
// ---------------------------------------------------------------------------

export const tool: ToolDescriptor<IdentityInputT, IdentityOutputT> = {
  name: "mneme_identity",
  description:
    "Return the Project Identity Kernel for the active project: name, detected stack, short domain summary from the README, key concepts, top conventions, recent goals, and open questions. Seeds any AI coding assistant with a high-signal view of the codebase. Pure-local, no LLM, no network.",
  inputSchema: IdentityInput,
  outputSchema: IdentityOutput,
  category: "recall",
  async handler(input, ctx): Promise<IdentityOutputT> {
    const root = input.project ?? ctx.cwd;

    const name = inferProjectName(root);
    const stack = detectStack(root);
    const domain_summary = readDomainSummary(root);

    // Concept corpus = README + stack names (same as brain::identity).
    const readmePath = ["README.md", "Readme.md", "readme.md"]
      .map((n) => join(root, n))
      .find((p) => existsSync(p));
    let readmeText = "";
    if (readmePath !== undefined) {
      try {
        readmeText = readFileSync(readmePath, "utf-8");
      } catch {
        readmeText = "";
      }
    }
    const corpus = `${readmeText}\n${stack.map((s) => s.name).join("\n")}`;
    const key_concepts = extractConcepts(corpus);

    // Conventions + Step Ledger data come from the shard. Degrade gracefully
    // when the shard hasn't been built yet.
    const conventions = readTopConventions(root, 5);
    const recent_goals = readRecentGoals(root);
    const open_questions = readOpenQuestions(root);

    return {
      name,
      stack,
      domain_summary,
      key_concepts,
      conventions,
      recent_goals,
      open_questions,
    };
  },
};

// ---------------------------------------------------------------------------
// Step Ledger helpers
// ---------------------------------------------------------------------------

function readRecentGoals(cwd: string): string[] {
  try {
    // Quick check: do we have a built shard at all?
    if (resolveShardRoot(cwd) === null) return [];
    const db = openShardDb("tasks", cwd);
    try {
      const rows = db
        .prepare(
          `SELECT description FROM steps
             WHERE status IN ('completed','in_progress')
             ORDER BY COALESCE(completed_at, started_at) DESC
             LIMIT 5`,
        )
        .all() as Array<{ description: string }>;
      return rows.map((r) => r.description);
    } finally {
      db.close();
    }
  } catch {
    return [];
  }
}

function readOpenQuestions(cwd: string): string[] {
  try {
    if (resolveShardRoot(cwd) === null) return [];
    const db = openShardDb("tasks", cwd);
    try {
      const rows = db
        .prepare(
          `SELECT description FROM steps
             WHERE status = 'blocked' OR blocker IS NOT NULL
             ORDER BY started_at DESC
             LIMIT 5`,
        )
        .all() as Array<{ description: string }>;
      return rows.map((r) => r.description);
    } finally {
      db.close();
    }
  } catch {
    return [];
  }
}
