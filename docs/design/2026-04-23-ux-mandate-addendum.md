---
name: Datatree UX Mandate
description: Coding/debugging/editing must feel "easy like cake for anyone." Drives proactive suggestion, plain-English everything, one-keystroke actions.
type: design-amendment
date: 2026-04-23
status: approved-by-user-firestart
overrides: 2026-04-23-datatree-design.md
---

# UX Mandate Addendum

User direction: "i want it so powerfull that coding, debugging, file editing all becomes easy like cake for anyone."

This document specifies the UX behaviors that turn raw capability into ease-of-use.

## Cake principles

1. **Datatree speaks first.** When relevant, surface the next useful action without being asked.
2. **No CLI flags required for 80% of tasks.** Smart defaults; flags exist for power users.
3. **Plain English everywhere.** No jargon in user-facing text. Tooltips explain symbols. Error messages say what to do, not what failed internally.
4. **One-keystroke fixes.** Every drift finding shows a "Fix" button. One key applies the suggestion.
5. **Visual confidence.** The Command Center shows "you're on track" in green or "you drifted" in red — no decoding required.
6. **Resumption is invisible.** After context compaction, the resumption bundle is auto-injected; user types their next message normally and gets correct behavior.

## Concrete features

### A. Proactive Next-Best-Action panel (in Command Center)

After every assistant turn, datatree computes the top 3 most-likely-useful next moves and shows them as buttons:

```
┌─ Next moves (datatree thinks) ───────────────────┐
│  ▶ Run failing tests (3 broken since last edit)  │
│  ▶ Fix 12 hardcoded colors in src/components/    │
│  ▶ Update CLAUDE.md (stale: mentions old auth)   │
└──────────────────────────────────────────────────┘
```

One click → datatree creates a step in the Step Ledger and starts executing.

### B. "Ask in plain English" mode

A persistent input box at the bottom of the Command Center:

```
┌──────────────────────────────────────────────────────┐
│  💬 Tell datatree what you want to do…              │
│                                                      │
│  Examples:                                           │
│   "add dark mode toggle to the settings page"       │
│   "find every place we hardcoded the API URL"       │
│   "explain why the login is slow"                   │
└──────────────────────────────────────────────────────┘
```

Datatree's local LLM converts the request into:
- A roadmap (numbered Step Ledger entries)
- Prefilled MCP tool calls
- Drift checks to run as the work happens
- Files to open/show
- A predicted blast radius

User confirms → datatree starts. No code-fluency required.

### C. One-keystroke drift fixes

Every drift finding row has an inline action:

```
src/components/Header.tsx:45
  ⚠ Hardcoded color #3b82f6 — your tokens file has --color-primary
  [F]ix    [I]gnore once    [I,a]gnore always
```

Pressing `F`:
- Datatree generates the patch (replaces #3b82f6 → var(--color-primary))
- Shows a 2-line diff
- Press `Y` to apply, `N` to skip
- Refactor recorded to T layer (instant undo)

### D. Live "what changed" sidebar

The Vision app's sidebar continuously shows:

```
LAST 5 MINUTES
  edit  src/auth/login.ts          (you, 0:42 ago)
  test  failed: auth.test.ts:14   (CI, 1:08 ago)
  drift hardcoded color in 1 file (datatree, 2:15 ago)
  agent claude touched 3 files     (background, 3:01 ago)
```

Click any row → jump to the change in the editor.

### E. Plain-English error tooltips

When any tool fails, datatree wraps the technical error in a plain-English sentence:

| Raw error | Datatree explanation |
|---|---|
| `SQLITE_CORRUPT: database disk image is malformed` | "One of datatree's storage files got damaged. Don't worry — datatree is restoring from yesterday's snapshot. Should be back in 5 seconds." |
| `Tree-sitter: unable to parse src/foo.ts:142` | "Couldn't read line 142 of `src/foo.ts` — looks like a syntax error you just introduced. The graph for the rest of this file is still up to date." |
| `Whisper model not found at ~/.datatree/llm/whisper-base` | "Voice transcription isn't installed. To enable it, run `datatree models install --with-whisper --from <folder>`." |

### F. Onboarding wizard

First time the user opens the Vision app:

```
1. Welcome — what is datatree?
2. Pick the project to index (auto-detected from cwd)
3. Pick which AI tools you use (auto-detected: Claude Code, Cursor, etc.)
4. Confirm install scopes (global / project)
5. Indexing… (progress bar; ~5-30 seconds for typical project)
6. "All set! Try one of these:"
   - Tour the views
   - Ask datatree a question
   - See what's currently broken
```

Total time to first useful answer: under 60 seconds.

### G. Smart defaults (no config required)

Out of the box:
- Drift detector ON for theme + types + security (the rules most projects benefit from)
- Step Ledger ON
- Compaction recovery ON
- Live push channel ON
- All 14 vision views available
- All 5 injection modes ON
- bge-small embeddings ON (because it's already bundled)
- Phi-3 OFF (because 2.4GB download)
- Voice nav OFF (Phase 5)

User can flip any of these via `/dt-config` slash command (interactive UI in Vision app) — no JSON editing required for common changes.

### H. The "explain this code" button

In the Vision app, hovering any function shows a tooltip with the LLM-generated 1-sentence summary. Clicking it opens a side panel with:
- What this function does (1 paragraph, plain English)
- Who calls it
- What it depends on
- What it tests
- Recent edits
- Related decisions
- Related discussions in conversation history

For non-coders / new team members: this is the killer onboarding feature.

### I. Guided debugging mode

When a test fails, datatree's drift hunter writes the failure to errors.db and the Vision app surfaces a guided debug flow:

```
TEST FAILED: auth.test.ts › should reject invalid email
  expected: 400
  received: 200

DATATREE INVESTIGATION (auto-generated)
  This test exercises:    src/auth/validate.ts → validateEmail()
  Recently changed:       src/auth/validate.ts (you, 12 minutes ago)
  Last passing version:   commit abc123 (2 hours ago)
  Blast radius of fix:    7 callers, 3 tests
  Similar past bugs:      "rejected emails were too lenient" — fixed in commit def456

SUGGESTED NEXT STEPS
  ▶ Show me the diff between abc123 and now
  ▶ Open src/auth/validate.ts at the validateEmail function
  ▶ Generate a fix proposal
```

Click any → datatree does it.

### J. "Don't break things" guardrails

Before any tool call that's destructive (rm, force-push, delete, drop-table), datatree intercepts and shows:

```
⚠ DATATREE GUARDRAIL
  About to: rm -rf src/legacy/
  Affects:
    — 47 files
    — 12 callers in non-legacy code (will break)
    — 3 tests reference these files

  This is destructive and might break things.
  Proceed? [y / show-blast-radius / cancel]
```

User picks `show-blast-radius` → vision app opens with the affected files highlighted.

## How these surface across platforms

| Platform | Cake-easy delivery |
|---|---|
| Claude Code | Hooks + `<datatree-context>` block + slash commands; Vision app opens in OS browser/Tauri window |
| Cursor / Windsurf | Same hooks pattern; Vision app same |
| Codex / Aider / Trae | AGENTS.md instructs LLM to use datatree MCP tools first; Vision app accessed via `datatree view` |
| Browser-based agents (Antigravity, Continue) | MCP tools + Vision web UI |
| All | `/dt-explain`, `/dt-fix`, `/dt-debug` slash commands work uniformly |

## Acceptance criteria added to §19

16. ✅ First-time user reaches a useful answer in <60 seconds from running `datatree install`
17. ✅ Drift findings include actionable suggestions for ≥80% of cases
18. ✅ "Ask in plain English" successfully decodes ≥90% of beginner-level requests on a curated test set
19. ✅ Vision app onboarding wizard completes in under 5 clicks
20. ✅ Every error message includes a plain-English explanation + a next-step

---

End of UX mandate.
