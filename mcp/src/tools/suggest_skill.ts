/**
 * MCP tool: suggest_skill
 *
 * Given a free-form task description, rank the installed mneme plugin
 * skills (~/.mneme/plugin/skills/) by how well their declared YAML
 * `triggers` and `tags` match the task, and return the top few with a
 * confidence label plus the absolute path to each SKILL.md.
 *
 * This is the read-path for "mneme, tell Claude which skill to use":
 *
 *   - The inject hook in cli/src/commands/inject.rs runs the same
 *     matching algorithm (ported into Rust) on every UserPromptSubmit
 *     and surfaces the top hit as a <mneme-skill-prescription> block.
 *   - The assistant can also call this tool directly, e.g. mid-turn
 *     when switching context, and get up to `limit` suggestions with
 *     a reason string it can show the user.
 *
 * # Design
 *
 * 1. On first invocation we scan the skills directory, parse the
 *    YAML frontmatter of each SKILL.md (a tiny hand-rolled parser —
 *    no new runtime dep), and cache the parsed descriptors in a
 *    module-level Map keyed by skill name. Broken / missing files are
 *    logged and skipped, never thrown.
 * 2. Matching is keyword-based: we tokenise the task (lowercase,
 *    drop stopwords and short tokens), then for each skill count how
 *    many `triggers` appear as substrings of the task string and how
 *    many `tags` do. Triggers weigh 2x tags.
 * 3. Confidence thresholds: `high >= 3`, `medium >= 1`, `low > 0`.
 *    Zero-score skills are dropped from the result.
 * 4. `mneme-codewords` is always returned at low confidence when the
 *    raw task contains one of the four codewords literally
 *    (`coldstart`, `hotstart`, `firestart`, `CHS`), even if the
 *    tokeniser would have dropped it.
 * 5. Results are capped at `limit` (default 3).
 *
 * # Why this belongs in mneme
 *
 * Claude Code can't autoload a skill based on a user prompt —
 * skill gating is the assistant's responsibility. Mneme shortens that
 * loop: the inject hook announces "recommended_skill: fireworks-debug"
 * and the assistant can load the SKILL.md with a single cat.
 *
 * # Hot-reload safety
 *
 * The tool file itself must be drop-in replaceable (see
 * `mcp/src/tools/index.ts` — hot reload pattern). The parsed-skill
 * cache is module-level but rebuilt lazily on demand, so a reload
 * starts from an empty cache and repopulates on the next call.
 */

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { z } from "zod";
import type { ToolDescriptor } from "../types.ts";

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

const ConfidenceEnum = z.enum(["high", "medium", "low"]);

export const SuggestSkillInput = z.object({
  task: z.string().min(1),
  limit: z.number().int().positive().max(20).default(3),
});

const Suggestion = z.object({
  skill: z.string(),
  reason: z.string(),
  triggers_matched: z.array(z.string()),
  confidence: ConfidenceEnum,
  source_path: z.string(),
});

export const SuggestSkillOutput = z.object({
  suggestions: z.array(Suggestion),
  ledger: z.object({
    total_skills_checked: z.number().int().nonnegative(),
    strategy: z.literal("keyword_match_and_tag_match"),
  }),
});

type SuggestSkillInputT = z.infer<typeof SuggestSkillInput>;
type SuggestSkillOutputT = z.infer<typeof SuggestSkillOutput>;

// ---------------------------------------------------------------------------
// Stopwords (kept tiny — we only drop glue words that never drive a
// skill choice).
// ---------------------------------------------------------------------------

const STOPWORDS = new Set<string>([
  "a",
  "an",
  "the",
  "and",
  "or",
  "of",
  "to",
  "for",
  "with",
  "in",
  "on",
  "at",
  "is",
  "it",
  "this",
  "that",
  "my",
  "i",
  "me",
  "we",
  "our",
  "please",
  "help",
  "need",
  "want",
  "can",
  "could",
  "you",
  "your",
  "do",
  "does",
  "did",
  "have",
  "has",
  "had",
  "be",
  "been",
  "am",
  "are",
  "was",
  "were",
  "if",
  "then",
  "so",
  "but",
  "not",
  "no",
  "yes",
]);

// ---------------------------------------------------------------------------
// YAML frontmatter parser — scoped to what SKILL.md files actually use.
// ---------------------------------------------------------------------------

interface ParsedSkill {
  name: string;
  description: string;
  triggers: string[];
  tags: string[];
  sourcePath: string;
}

/**
 * Extract the `---` delimited frontmatter block at the top of a
 * SKILL.md. Returns `null` if the file doesn't start with `---`.
 */
function sliceFrontmatter(text: string): string | null {
  // Tolerate a UTF-8 BOM and Windows line endings.
  const normalized = text.replace(/^﻿/, "").replace(/\r\n/g, "\n");
  if (!normalized.startsWith("---")) return null;
  // Find the terminating `---` on its own line after the opener.
  const afterOpen = normalized.slice(3);
  const endIdx = afterOpen.indexOf("\n---");
  if (endIdx < 0) return null;
  // Skip the leading newline of the opener block.
  const block = afterOpen.slice(0, endIdx);
  return block.replace(/^\n/, "");
}

/**
 * Parse a very small subset of YAML: scalar strings, inline arrays
 * (`[a, b, "c d"]`), and block arrays (`- a\n- b`). Enough to handle
 * every SKILL.md in the mneme plugin as of v0.3.0.
 */
function parseTinyYaml(block: string): Record<string, string | string[]> {
  const out: Record<string, string | string[]> = {};
  const lines = block.split("\n");
  let i = 0;
  while (i < lines.length) {
    const raw = lines[i];
    if (raw === undefined) break;
    const line = raw.replace(/\s+$/, "");
    // Skip blank lines and full-line comments.
    if (line.trim() === "" || line.trimStart().startsWith("#")) {
      i += 1;
      continue;
    }
    // Only consider top-level keys (no leading whitespace). SKILL.md
    // frontmatter doesn't nest beyond arrays.
    const kvMatch = line.match(/^([A-Za-z_][A-Za-z0-9_-]*)\s*:\s*(.*)$/);
    if (!kvMatch) {
      i += 1;
      continue;
    }
    const key = kvMatch[1]!;
    const rest = kvMatch[2] ?? "";
    if (rest === "" || rest === ">" || rest === "|") {
      // Block scalar or block array starts on the next line.
      // Peek ahead: is it an indented `- item` array, or folded text?
      const collected: string[] = [];
      let isArray = false;
      let j = i + 1;
      while (j < lines.length) {
        const next = lines[j];
        if (next === undefined) break;
        if (next.trim() === "") {
          j += 1;
          continue;
        }
        if (/^\s*-\s+/.test(next)) {
          isArray = true;
          const itemMatch = next.match(/^\s*-\s+(.*)$/);
          if (itemMatch && itemMatch[1] !== undefined) {
            collected.push(stripYamlScalar(itemMatch[1]));
          }
          j += 1;
          continue;
        }
        // Folded / literal scalar: indented non-dash line.
        if (!isArray && /^\s+\S/.test(next)) {
          collected.push(next.trim());
          j += 1;
          continue;
        }
        break;
      }
      if (isArray) {
        out[key] = collected;
      } else if (collected.length > 0) {
        out[key] = collected.join(" ");
      } else {
        out[key] = "";
      }
      i = j;
      continue;
    }
    // Inline array: `[a, b, "c d"]`.
    if (rest.startsWith("[") && rest.endsWith("]")) {
      const inner = rest.slice(1, -1);
      out[key] = splitInlineArray(inner);
      i += 1;
      continue;
    }
    // Plain scalar.
    out[key] = stripYamlScalar(rest);
    i += 1;
  }
  return out;
}

function stripYamlScalar(s: string): string {
  const trimmed = s.trim();
  if (trimmed.length >= 2) {
    const first = trimmed[0];
    const last = trimmed[trimmed.length - 1];
    if ((first === '"' && last === '"') || (first === "'" && last === "'")) {
      return trimmed.slice(1, -1);
    }
  }
  return trimmed;
}

function splitInlineArray(inner: string): string[] {
  const out: string[] = [];
  let buf = "";
  let quote: string | null = null;
  for (const ch of inner) {
    if (quote) {
      if (ch === quote) {
        quote = null;
      } else {
        buf += ch;
      }
      continue;
    }
    if (ch === '"' || ch === "'") {
      quote = ch;
      continue;
    }
    if (ch === ",") {
      const trimmed = buf.trim();
      if (trimmed) out.push(trimmed);
      buf = "";
      continue;
    }
    buf += ch;
  }
  const tail = buf.trim();
  if (tail) out.push(tail);
  return out;
}

function coerceStringArray(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value.map((v) => String(v)).filter((v) => v.length > 0);
  }
  if (typeof value === "string" && value.trim().length > 0) {
    return [value.trim()];
  }
  return [];
}

// ---------------------------------------------------------------------------
// Skills directory resolution + scan.
// ---------------------------------------------------------------------------

function candidateSkillDirs(): string[] {
  const dirs: string[] = [];
  // Preferred: installed plugin under ~/.mneme/plugin/skills/.
  dirs.push(join(homedir(), ".mneme", "plugin", "skills"));
  // Dev fallback: the repo this MCP file lives in.
  const here =
    typeof import.meta.url === "string" ? fileURLToPath(import.meta.url) : "";
  if (here) {
    // mcp/src/tools/suggest_skill.ts -> repo root is three levels up.
    const repoRoot = resolve(dirname(here), "..", "..", "..");
    dirs.push(join(repoRoot, "plugin", "skills"));
  }
  return dirs;
}

function walkSkillFiles(dir: string): string[] {
  const hits: string[] = [];
  let entries: string[];
  try {
    entries = readdirSync(dir);
  } catch {
    return hits;
  }
  for (const entry of entries) {
    const full = join(dir, entry);
    let s: ReturnType<typeof statSync>;
    try {
      s = statSync(full);
    } catch {
      continue;
    }
    if (s.isDirectory()) {
      const skillMd = join(full, "SKILL.md");
      if (existsSync(skillMd)) hits.push(skillMd);
    } else if (s.isFile() && entry.toLowerCase() === "skill.md") {
      hits.push(full);
    }
  }
  return hits;
}

// Module-level cache. Populated lazily; reloads are cheap (file IO
// + tiny parse). Hot-reload wipes the module so this resets naturally.
let cachedSkills: ParsedSkill[] | null = null;

function loadSkills(): ParsedSkill[] {
  if (cachedSkills !== null) return cachedSkills;
  const out: ParsedSkill[] = [];
  const seen = new Set<string>();
  for (const dir of candidateSkillDirs()) {
    if (!existsSync(dir)) continue;
    for (const file of walkSkillFiles(dir)) {
      let text: string;
      try {
        text = readFileSync(file, "utf-8");
      } catch (err) {
        console.warn(`[mneme-mcp suggest_skill] read failed: ${file}: ${(err as Error).message}`);
        continue;
      }
      const frontmatter = sliceFrontmatter(text);
      if (!frontmatter) continue;
      let parsed: Record<string, string | string[]>;
      try {
        parsed = parseTinyYaml(frontmatter);
      } catch (err) {
        console.warn(`[mneme-mcp suggest_skill] frontmatter parse failed: ${file}: ${(err as Error).message}`);
        continue;
      }
      const name =
        typeof parsed.name === "string" ? parsed.name.trim() : "";
      if (!name) continue;
      // Dedup: first dir wins (installed plugin beats repo fallback).
      if (seen.has(name)) continue;
      seen.add(name);
      const description =
        typeof parsed.description === "string"
          ? parsed.description.trim()
          : Array.isArray(parsed.description)
            ? parsed.description.join(" ").trim()
            : "";
      const triggers = coerceStringArray(parsed.triggers).map((t) =>
        t.toLowerCase(),
      );
      const tags = coerceStringArray(parsed.tags).map((t) => t.toLowerCase());
      out.push({
        name,
        description,
        triggers,
        tags,
        sourcePath: file,
      });
    }
  }
  cachedSkills = out;
  return cachedSkills;
}

// ---------------------------------------------------------------------------
// Matching algorithm.
// ---------------------------------------------------------------------------

const CODEWORDS = ["coldstart", "hotstart", "firestart", "chs"] as const;

interface MatchResult {
  skill: ParsedSkill;
  score: number;
  triggersMatched: string[];
  tagsMatched: string[];
}

function tokenize(task: string): Set<string> {
  const lowered = task.toLowerCase();
  const raw = lowered.split(/[^a-z0-9_+.-]+/g).filter((t) => t.length > 0);
  const kept = new Set<string>();
  for (const tok of raw) {
    if (tok.length < 2) continue;
    if (STOPWORDS.has(tok)) continue;
    kept.add(tok);
  }
  return kept;
}

function triggerMatches(trigger: string, loweredTask: string, tokens: Set<string>): boolean {
  const t = trigger.trim();
  if (t.length === 0) return false;
  // Multi-word triggers (e.g. "system design") match as substrings.
  if (t.includes(" ")) {
    return loweredTask.includes(t);
  }
  // Single-token triggers must match a whole token to avoid false
  // positives like "art" inside "artifact".
  if (tokens.has(t)) return true;
  // But if the trigger contains a hyphen/underscore/digit, allow
  // substring match (these are rarely substrings of unrelated words).
  if (/[-_+.0-9]/.test(t) && loweredTask.includes(t)) return true;
  return false;
}

function match(task: string): MatchResult[] {
  const loweredTask = task.toLowerCase();
  const tokens = tokenize(task);
  const skills = loadSkills();
  const results: MatchResult[] = [];
  for (const skill of skills) {
    const triggersMatched: string[] = [];
    const tagsMatched: string[] = [];
    for (const trig of skill.triggers) {
      if (triggerMatches(trig, loweredTask, tokens)) {
        triggersMatched.push(trig);
      }
    }
    // De-dup tags against triggers: if a tag string also appears as a
    // trigger on the same skill, don't count it twice. Otherwise skills
    // whose authors mirrored their triggers into tags
    // (e.g. fireworks-test: `tags: [test, tdd, ...]` +
    // `triggers: [test, tdd, ...]`) get an unfair double-score for a
    // single keyword hit.
    const triggerSet = new Set(skill.triggers);
    for (const tag of skill.tags) {
      if (triggerSet.has(tag)) continue;
      if (triggerMatches(tag, loweredTask, tokens)) {
        tagsMatched.push(tag);
      }
    }
    const score = triggersMatched.length * 2 + tagsMatched.length;
    if (score > 0) {
      results.push({ skill, score, triggersMatched, tagsMatched });
    }
  }
  results.sort((a, b) => b.score - a.score);
  return results;
}

function codewordHit(task: string): string | null {
  const lowered = task.toLowerCase();
  // Match whole-word boundaries so "coldstart" hits but "coldstartup" doesn't.
  for (const cw of CODEWORDS) {
    const re = new RegExp(`(^|[^a-z0-9])${cw}([^a-z0-9]|$)`);
    if (re.test(lowered)) return cw;
  }
  return null;
}

function confidenceFor(score: number): z.infer<typeof ConfidenceEnum> {
  if (score >= 3) return "high";
  if (score >= 1) return "medium";
  return "low";
}

function buildReason(triggersMatched: string[], tagsMatched: string[]): string {
  if (triggersMatched.length > 0) {
    return `matched trigger(s): ${triggersMatched.join(", ")}`;
  }
  if (tagsMatched.length > 0) {
    return `tag match: ${tagsMatched.join(", ")}`;
  }
  return "no explicit trigger matched";
}

// ---------------------------------------------------------------------------
// Tool descriptor
// ---------------------------------------------------------------------------

export const tool: ToolDescriptor<SuggestSkillInputT, SuggestSkillOutputT> = {
  name: "suggest_skill",
  description:
    "Given a free-form task description, recommend which mneme plugin skill(s) to load. Scans ~/.mneme/plugin/skills/ for SKILL.md frontmatter, matches the task against each skill's triggers and tags (triggers weigh 2x), and returns up to `limit` ranked suggestions with a confidence label (high/medium/low) and an absolute source_path so the caller can `cat` the SKILL.md directly. Use at the start of any non-trivial task to auto-select the right expert skill instead of guessing.",
  inputSchema: SuggestSkillInput,
  outputSchema: SuggestSkillOutput,
  category: "recall",
  async handler(input): Promise<SuggestSkillOutputT> {
    try {
      const skills = loadSkills();
      const limit = input.limit ?? 3;
      const ranked = match(input.task);

      const suggestions: z.infer<typeof Suggestion>[] = [];

      for (const hit of ranked) {
        suggestions.push({
          skill: hit.skill.name,
          reason: buildReason(hit.triggersMatched, hit.tagsMatched),
          triggers_matched: hit.triggersMatched,
          confidence: confidenceFor(hit.score),
          source_path: hit.skill.sourcePath,
        });
      }

      // Always surface mneme-codewords at low confidence when the task
      // literally contains one of the four codewords.
      const cw = codewordHit(input.task);
      if (cw) {
        const already = suggestions.find((s) => s.skill === "mneme-codewords");
        if (!already) {
          const cwSkill = skills.find((s) => s.name === "mneme-codewords");
          if (cwSkill) {
            suggestions.push({
              skill: "mneme-codewords",
              reason: `codeword literal match: ${cw}`,
              triggers_matched: [cw],
              confidence: "low",
              source_path: cwSkill.sourcePath,
            });
          }
        }
      }

      return {
        suggestions: suggestions.slice(0, limit),
        ledger: {
          total_skills_checked: skills.length,
          strategy: "keyword_match_and_tag_match" as const,
        },
      };
    } catch (err) {
      // Never throw — return an empty result so the hook/inline call
      // never becomes an MCP error.
      console.warn(
        `[mneme-mcp suggest_skill] handler failed: ${(err as Error).message}`,
      );
      return {
        suggestions: [],
        ledger: {
          total_skills_checked: 0,
          strategy: "keyword_match_and_tag_match" as const,
        },
      };
    }
  },
};
