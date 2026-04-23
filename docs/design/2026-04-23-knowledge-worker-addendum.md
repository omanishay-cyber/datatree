---
name: Datatree Knowledge-Worker Mandate
description: Datatree is not a code-only tool. Designed equally for blog writing, research, note-taking, and any text-heavy knowledge work.
type: design-amendment
date: 2026-04-23
status: approved-by-user-firestart
overrides: 2026-04-23-datatree-design.md
---

# Knowledge-Worker Mandate Addendum

User direction: "even for just typing a blog can use this for refreance and all."

Datatree's core architecture (graph + drift + multimodal + Step Ledger + Command Center) maps perfectly to non-code work. This addendum makes that explicit.

## Use cases beyond code

| Use case | How datatree serves it |
|---|---|
| **Blog writing** | Every draft + every reference saved; "find the paragraph where I argued X 3 weeks ago" → recall_concept; PDFs and screenshots dragged into the project become god-nodes |
| **Long-form research** | Whisper-transcribed lectures + paper PDFs + your notes all in one graph; god_nodes() shows the most-connected ideas; surprising_connections() finds non-obvious links |
| **Note-taking (Obsidian-style)** | Drinks every .md; resolves wikilinks; runs Leiden clustering on note communities; presents in HierarchyTree + Sunburst views |
| **Daily journaling** | Step Ledger keeps your "today/this week/this month" goal stack across sessions; conversation history searchable; insights view auto-generates weekly summaries |
| **Studying / coursework** | Lecture videos transcribed, slide PDFs extracted, your notes linked; "find every place we discussed transformer attention" returns code + slides + notes + your prose |
| **Writing a book / dissertation** | Every chapter + every reference + every interview transcript + every figure indexed; drift detection ensures terminology stays consistent across chapters |
| **Knowledge management for a team** | Per-project shard committed scope: when teammates clone the repo + run `datatree install`, they get the same shared brain |

## What changes in the design

Nothing structurally — the architecture already supports this. We just relax assumptions that were code-centric:

### Smart defaults adapt to project type

On first index, datatree detects the project's flavor:

- **Code project** (presence of `package.json` / `Cargo.toml` / `requirements.txt` / `.git`) → enables theme + types + security + perf + a11y scanners
- **Writing project** (predominance of `.md`, `.docx`, `.pdf`, no programming language files) → enables only markdown-drift scanner; suggests grammar/style scanners (off by default; opt-in)
- **Research project** (mix of PDFs, .ipynb notebooks, transcript audio) → enables multimodal extraction, citation tracking, concept clustering by default
- **Hybrid** (docs/ folder inside a code repo) → enables both modes

### New scanners (writing-mode)

| Scanner | What it flags |
|---|---|
| `style.repetition` | Same word/phrase repeated within N paragraphs (writer's tic detector) |
| `style.consistency` | Terminology drift ("user" vs "customer" vs "client" used interchangeably) |
| `style.readability` | Flesch–Kincaid grade level out of target range |
| `cite.broken` | A `[citation needed]` marker or a citation pointing to a non-existent reference |
| `cite.missing` | Claim made without a citation when the same claim was cited earlier in another doc |
| `quote.unattributed` | A direct quote without a source link |
| `consistency.heading` | Same heading levels used for unequal-importance content |

All off by default unless the writing-mode flavor is detected.

### New MCP tools (writing-mode)

| Tool | Purpose |
|---|---|
| `recall_quote(text)` | Find every place a quoted phrase appears across the project |
| `recall_citation(source)` | Show every doc that cites a given source |
| `compare_drafts(file_a, file_b)` | Semantic diff between two drafts of the same piece |
| `suggest_related(concept)` | Surface notes / PDFs / videos most related to a concept |
| `outline_extract(file)` | Extract heading-tree as a navigable outline |

### New Vision view (writing-mode)

15th view: **WritingMap** — sunburst-style overview of every document in the project, sized by word count, colored by recency, with concept-cluster overlay. Click any document → side panel shows outline + word count + last edit + concept tags.

### Step Ledger for writing tasks

A "publish a blog post" flow becomes a numbered ledger:
```
1.  Draft outline
2.  First-pass writing
3.  Add citations
4.  Self-edit pass 1 (datatree drift check: style.repetition)
5.  Self-edit pass 2 (datatree drift check: style.consistency)
6.  External review
7.  Final read-through
8.  Publish
```

Each step's acceptance check is appropriate: word count threshold, no `[citation needed]` markers remaining, all wikilinks resolve, etc.

### Command Center for writers

The Command Center surfaces:
- Active writing project + chapter + current word count
- Today's writing goal (e.g., 1500 words) and progress
- Currently dirty files (unsaved drafts)
- Drift findings (style/citation issues)
- Last 5 minutes of edits
- "Suggested next move" (e.g., "you're 200 words short of today's goal — open the latest draft?")

### Acceptance criteria added to §19

21. ✅ A non-coder can use datatree for a blog project after running one install command and an onboarding wizard, without needing to read any documentation.
22. ✅ Writing-mode scanners ship in v1.0 (off by default; opt-in via "this is a writing project" prompt during onboarding).
23. ✅ The 15th vision view (WritingMap) ships in v1.0.

---

End of knowledge-worker mandate.
