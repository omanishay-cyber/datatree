---
name: fireworks-research
description: Deep research superbrain — multi-source investigation, codebase mapping, library evaluation, structured findings
version: 1.0.0
author: mneme
tags: [research, investigation, codebase-mapping, library-evaluation, analysis]
triggers: [research, investigate, compare, evaluate, library, which one, how does, documentation, alternatives]
---

# Fireworks Research — Deep Research Superbrain

> Multi-source investigation engine. Codebase mapping. Library evaluation. Structured findings with confidence levels and source citations.

---

## 1. Multi-Source Research Protocol

Research is NOT "I googled it." Research is a structured, multi-source investigation that produces verifiable findings with confidence levels.

### The Research Pipeline

```
Step 1: CODEBASE READING (always first)
  - Search the existing codebase for related code, patterns, prior art
  - Read package.json, config files, existing implementations
  - Understand what already exists before looking externally

Step 2: OFFICIAL DOCS (Context7)
  - Use resolve-library-id + query-docs for any library questions
  - Official documentation is the HIGHEST authority source
  - Never guess API patterns — verify from docs

Step 3: WEB SEARCH (when codebase + docs insufficient)
  - Use WebSearch for current information, comparisons, known issues
  - Use WebFetch for specific pages with detailed information
  - Prefer official sources: GitHub repos, library docs, RFCs

Step 4: CROSS-REFERENCE
  - Verify findings from at least 2 independent sources
  - Flag any contradictions between sources
  - Downgrade confidence if only one source confirms

Step 5: SYNTHESIZE
  - Combine findings into structured report
  - Assign confidence levels based on source quality
  - Produce actionable recommendation
```

### Source Hierarchy (Highest to Lowest Authority)

1. **Official documentation** (Context7, library docs, RFCs) — HIGH confidence
2. **Source code** (the actual implementation, package.json) — HIGH confidence
3. **Official blog posts** (from library maintainers) — HIGH confidence
4. **Reputable tech sites** (MDN, web.dev, Node.js docs) — HIGH confidence
5. **GitHub issues/discussions** (from maintainers) — MEDIUM confidence
6. **Community articles** (dev.to, medium, blogs) — MEDIUM confidence
7. **Stack Overflow answers** (highly voted) — MEDIUM confidence
8. **Single unverified source** — LOW confidence
9. **AI inference without source** — LOW confidence (flag explicitly)

---

## 2. Codebase Deep-Read Methodology

Before researching externally, ALWAYS map what already exists in the codebase.

### Architecture Mapping Steps

```
1. ENTRY POINTS: Find main.ts/index.ts, renderer entry, preload scripts
2. IMPORTS: Trace imports from entry points to build dependency graph
3. LAYERS: Identify UI -> State -> IPC -> Main -> Data layer boundaries
4. PATTERNS: Document naming conventions, error handling, state management
5. DATA FLOW: Trace a user action end-to-end through all layers
```

### What to Look For

- **File organization**: Feature-based? Layer-based? Hybrid?
- **Naming conventions**: camelCase? kebab-case? How are components named?
- **State management**: Zustand stores? Redux? Context? Local state?
- **Error handling**: Try-catch? Error boundaries? Result types?
- **IPC patterns**: Channel naming, typed interfaces, envelope format
- **Testing patterns**: Unit tests? Integration? E2E? What framework?

### Tools for Codebase Mapping

- `Glob` — find files matching patterns (e.g., `**/*.store.ts`, `**/*.test.ts`)
- `Grep` — search for patterns across the codebase (e.g., `ipcMain.handle`, `createStore`)
- `Read` — read specific files to understand implementation
- `npx madge --image graph.svg src/` — generate visual dependency graph
- `npx madge --circular src/` — find circular dependencies

> See `references/codebase-mapping.md` for detailed mapping techniques.

---

## 3. Library Evaluation Framework

When choosing a library, NEVER just pick the first result. Evaluate systematically.

### Evaluation Criteria

| Criterion | Weight | How to Check |
|-----------|--------|-------------|
| Bundle size | High | bundlephobia.com, `npm pack --dry-run` |
| Maintenance | High | Last publish date, open issues, contributors |
| TypeScript | Medium | Built-in types vs @types, quality of types |
| API Design | Medium | Read examples, try simple use case |
| Community | Medium | GitHub stars, npm downloads, Stack Overflow |
| License | Low | Must be MIT/Apache/BSD for commercial use |

### Quick Evaluation Commands

```bash
# Check bundle size impact
npx bundlephobia-cli <package-name>

# Check latest version and publish date
npm view <package-name> time --json | tail -5

# Check TypeScript support
npm view <package-name> types typings

# Check dependencies (fewer = better)
npm view <package-name> dependencies

# Check download trends
# Use WebSearch: "npm trends <package-a> vs <package-b>"
```

### Disqualifiers (Automatic Rejection)

- License incompatible with commercial use (GPL without exception)
- Last publish > 2 years ago with unresolved security issues
- No TypeScript support (no built-in types AND no @types package)
- Known critical CVEs without patches

> See `references/library-evaluation.md` for the full comparison matrix template.

---

## 4. Research Scoping Rules

Research without scope is procrastination. Set boundaries BEFORE starting.

### Time Boxing

| Research Type | Max Time | Example |
|--------------|----------|---------|
| Simple lookup | 5 min | "What's the API for X?" |
| Library comparison | 15 min | "Should we use X or Y?" |
| Architecture decision | 30 min | "How should we structure this?" |
| Deep investigation | 45 min | "Why is X happening across the system?" |

### Stopping Conditions (Stop When ANY Is True)

1. **Answer found with HIGH confidence** from official source
2. **Time box exceeded** — report what you have, note gaps
3. **3 search reformulations failed** — the question may be wrong (see 3-Strike Rule)
4. **Diminishing returns** — new sources confirm what you already know

### Do NOT Research When

- You already know the answer from experience
- You can test it faster than researching it (< 2 minutes to test)
- The question is about the current codebase (just READ the code)
- The user already told you the answer (re-read the conversation)

---

## 5. Structured Findings Report Format

Every research task MUST produce a structured report. No exceptions.

```
RESEARCH FINDINGS
=================
Topic: [what was researched]
Time spent: [minutes]
Scope: [what was included/excluded from research]
Sources: [list of sources consulted]

KEY FINDINGS:
1. [Finding] — Confidence: HIGH/MEDIUM/LOW — Source: [where]
2. [Finding] — Confidence: HIGH/MEDIUM/LOW — Source: [where]
3. [Finding] — Confidence: HIGH/MEDIUM/LOW — Source: [where]

RECOMMENDATION: [what to do based on findings]
  Rationale: [why this recommendation over alternatives]

ALTERNATIVES CONSIDERED:
  - [Alternative A]: [why rejected]
  - [Alternative B]: [why rejected]

GAPS / UNKNOWNS:
  - [What couldn't be determined and why]
  - [What needs further investigation]

NEXT STEPS:
  1. [Concrete action item]
  2. [Concrete action item]
```

### When to Use Short vs Full Report

- **Short report** (3-5 findings): Simple lookup, single-library evaluation
- **Full report** (all sections): Architecture decisions, library comparisons, debugging investigations

> See `references/report-format.md` for examples and templates.

---

---

## Full Reference

For complete patterns, examples, and advanced usage, see [`references/full-guide.md`](./references/full-guide.md).
Read that file when you need deeper context than the summary above.
