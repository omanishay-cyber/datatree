---
name: Datatree v1 — Final Design (F++++++)
description: Persistent multi-process AI superbrain — per-project SQLite memory, 30+ MCP tools, 14-view live graph, multimodal corpus engine, compaction-resilient command center, marketplace-installable Claude Code plugin.
type: design
status: draft-pending-user-signoff
date: 2026-04-23
author: Anish Trivedi
reviewers: [self]
---

# Datatree — The AI Superbrain

> Tagline: **"Claude never starts cold. Claude never loses its place."**

## 0. North Star

Datatree is a per-project, multi-process, fault-tolerant memory and analysis daemon that turns every Claude Code session into a continuation of every prior session. It captures everything the user types, everything Claude does, and everything the codebase contains — into 27 specialized SQLite-backed storage layers — then composes a precise context bundle for each turn so Claude's working memory never gets bloated and never goes blank after compaction.

The single most important property: **the loss of Claude's working memory does not cause the loss of Claude's working knowledge.**

Secondary properties:
- **Never fails** — multi-process supervision, fault domains, WAL replay, hourly snapshots, watchdog self-test
- **Never freezes** — per-tool timeouts, queue backpressure, single-writer locks, async on hot path
- **Sugar in drink** — installs in one command, drinks every `.md` file like CLAUDE.md does, becomes part of Claude's main context without explicit tool calls
- **Verifiable** — every step in every plan has an acceptance check, datatree won't advance until it passes
- **Extensible** — add MCP tools by dropping `.ts` files; hot-reload without restart
- **Marketplace-distributable** — Claude Code plugin installable at global / user / project scope

---

## 1. Glossary

| Term | Meaning |
|---|---|
| **Daemon** | The long-running supervised process tree (supervisor + workers + MCP server) |
| **Supervisor** | Rust binary that watches all child processes and restarts crashed ones in <100ms |
| **Shard** | One SQLite database per project, stored at `~/.datatree/projects/<project_hash>/` |
| **Bundle** | The compressed context block injected into Claude's turn (1-5K tokens) |
| **Step Ledger** | Numbered, verified plan-tracking system inside Command Center |
| **Drift** | Divergence between Claude's current behavior and the active goal/constraints |
| **Sugar mode** | Default zero-config behavior: works invisibly out of the box |
| **CRG-mode** | Deterministic AST-based code structure analysis (inherited from code-review-graph) |
| **Graphify-mode** | LLM-assisted multimodal extraction with EXTRACTED/INFERRED/AMBIGUOUS tagging |
| **God node** | Most-connected concept in the graph (graphify terminology) |

---

## 2. The 27 Storage Layers

Each layer is a logical concern; physically they group into ~8 SQLite files per project shard for performance.

| # | Layer | DB file | Purpose |
|---|---|---|---|
| A | Code structure (Tree-sitter graph) | `graph.db` | Functions, classes, imports, calls, inheritance, blast-radius |
| B | File state | `graph.db` | Hash + last-read timestamp + summary per file; never re-read unchanged files |
| C | Conversation history | `history.db` | Verbatim user turns + assistant turns + system reminders |
| D | Decisions & learnings | `history.db` | Problem → root cause → solution records, per project |
| E | Tool-call cache | `tool_cache.db` | Bash/Grep/WebFetch outputs keyed by input hash; <1ms identical-call replay |
| F | Todo/roadmap state | `tasks.db` | TaskCreate items + roadmap snapshots; cross-session continuity |
| G | Semantic embeddings | `semantic.db` | Vector embeddings of code/docs/conversations (mmap'd) |
| H | Git history snapshot | `git.db` | Recent commits, blame, indexed for instant queries without `git` shell |
| I | User feedback memory | `memory.db` | Rule + Why + How-to-apply, per project |
| J | Error registry | `errors.db` | Error message + stack + fix + file; "same bug never debugged twice" |
| K | Screenshot/CHS history | `multimodal.db` | Indexed by timestamp; OCR text, detected UI elements |
| L | Dependency graph | `deps.db` | package.json/requirements.txt parsed; versions, vulnerabilities, last-upgrade |
| M | Test coverage map | `tests.db` | Function ↔ test mapping; last run; flaky-test history |
| N | Performance baselines | `perf.db` | Bundle size, build time, test runtime, FPS samples; flag regressions |
| O | Secret/security scan log | `findings.db` | Hardcoded keys, CSP checks, encryption-at-rest verifications |
| P | Theme/design-token registry | `findings.db` | CSS variables, colors, dark/light pairs, glassmorphism classes |
| Q | External docs cache | `cache/docs/` | Context7/WebFetch/library docs cached locally; cross-project sharing |
| R | Cross-session todo continuity | `tasks.db` | Roadmap from every session; "what did we plan in October?" |
| S | Agent collaboration log | `agents.db` | Subagent transcripts + decisions; main Claude knows what each parallel agent did |
| T | Refactor history | `refactors.db` | Rename/move/delete with before/after snapshots; instant undo |
| U | UI screenshot diff timeline | `multimodal.db` | Auto-snapshot UI per commit; visual regression detection |
| V | API contract registry | `contracts.db` | Every IPC channel, REST endpoint, GraphQL schema; consumers/producers |
| W | Auto-generated insight summaries | `insights.db` | Weekly "what was learned / what's stuck / what's next" rollups |
| X | Live file watcher state | `livestate.db` | What changed in last N seconds; who edited (you vs Claude vs git) |
| Y | Multi-project link graph | `~/.datatree/meta.db` | Projects sharing deps/code linked; fixes propagate awareness |
| Z | AI prompt/response telemetry | `telemetry.db` | Every system prompt + response token-counted; cost + latency tracked |
| AA | Multimodal corpus | `corpus.db` | PDF/image/audio/video extracted text + concepts (graphify) |

### 2.1 Per-project shard layout (on disk)

```
~/.datatree/
├── projects/
│   └── <sha256-of-project-path>/
│       ├── graph.db              ← layers A, B
│       ├── history.db            ← layers C, D
│       ├── tool_cache.db         ← layer E
│       ├── tasks.db              ← layers F, R
│       ├── semantic.db           ← layer G
│       ├── git.db                ← layer H
│       ├── memory.db             ← layer I
│       ├── errors.db             ← layer J
│       ├── multimodal.db         ← layers K, U
│       ├── deps.db               ← layer L
│       ├── tests.db              ← layer M
│       ├── perf.db               ← layer N
│       ├── findings.db           ← layers O, P
│       ├── agents.db             ← layer S
│       ├── refactors.db          ← layer T
│       ├── contracts.db          ← layer V
│       ├── insights.db           ← layer W
│       ├── livestate.db          ← layer X
│       ├── telemetry.db          ← layer Z
│       ├── corpus.db             ← layer AA
│       ├── snapshots/            ← hourly snapshots of each .db
│       │   └── 2026-04-23-14/
│       │       └── *.db
│       └── wal/                  ← write-ahead logs (one per .db)
├── meta.db                       ← layer Y (cross-project)
├── cache/
│   ├── docs/                     ← layer Q
│   └── embed/                    ← embedding vectors mmap'd
├── llm/                          ← optional local model weights
├── bin/
│   ├── datatree-supervisor.exe
│   └── datatree-view.exe
├── crashes/                      ← minidumps
└── supervisor.log
```

### 2.2 Why per-project sharding

- One bad project DB cannot kill the others
- Snapshots/WAL logs scoped per project for fast partial recovery
- Disk space accountable per project (audit + cleanup commands)
- Privacy: a single project can be fully removed by deleting its shard folder

---

## 3. Architecture (Multi-Process Supervisor Model)

### 3.1 Process tree

```
datatree-supervisor.exe (Rust, Windows service: DatatreeDaemon)
├── store-worker            (Rust)   — writes only; single-writer per shard
├── parse-worker × N        (Rust)   — Tree-sitter pool; N = CPU cores
├── scan-worker × M         (Rust)   — drift/security/perf/a11y scanners pool
├── md-ingest-worker        (Rust)   — markdown ingestion
├── multimodal-worker       (Python sidecar) — PDF/image/Whisper/OCR
├── brain-worker            (Mixed)  — embeddings + concept extraction + Leiden cluster
├── livebus-worker          (Rust)   — SSE/WebSocket push channel
├── mcp-server              (Bun TS) — JSON-RPC over stdio + hot-reload
├── vision-server           (Bun TS) — HTTP server for vision app on :7777
└── health-watchdog         (Rust)   — 60s self-test loop + SLA dashboard
```

### 3.2 Why this stack split

| Component | Language | Reason |
|---|---|---|
| Supervisor | Rust | Must never crash; `panic = abort`; deterministic restart |
| Storage | Rust + rusqlite | Sub-millisecond DB access; SQLite WAL mode; no GC pauses |
| Parsers | Rust + Tree-sitter | Native bindings; parallel safe; segfault contained per worker |
| Scanners | Rust | CPU-bound; benefit from native speed |
| Multimodal | Python | Whisper, PDF libs, CLIP — Python ecosystem unmatched |
| Brain | Mixed | Rust for embedding storage, Python sidecar for model inference |
| MCP server | Bun + TS | Hot-reloadable tool definitions; matches user's stack; `bun:sqlite` is the fastest SQLite binding |
| Vision app | Bun + TS + Tauri | Same stack as MCP; native desktop window via Tauri |
| Live bus | Rust | Low-latency push channel; tokio + axum |

### 3.3 Fault domains

Each worker is a separate OS process with:

- Memory limit (RSS cap; OOM-killer + restart)
- File descriptor limit
- CPU quota (cgroup on Linux, job object on Windows, libproc on macOS)
- 30s default tool-call timeout (configurable)
- Watchdog heartbeat every 1s

If any worker crashes / OOMs / hangs / panics:
1. Supervisor's epoll/kqueue/IOCP detects the exit
2. Crash dump captured if available
3. Backoff: 100ms → 500ms → 2s → 10s (exponential, max 5 restarts/min)
4. If exceeds 5 restarts/min: worker marked degraded, downstream tools fall back to "best-effort cached" mode
5. SLA dashboard updates uptime metric; alert pushed on live bus if `degraded`

### 3.4 Single-writer constraint per shard

Only the `store-worker` writes to a project's SQLite files. All other workers communicate write requests via an MPSC channel to `store-worker`. Reads are unrestricted (multi-reader via SQLite WAL).

This eliminates the entire class of SQLite "database is locked" errors and lost-update bugs.

---

## 4. The Five Injection Modes (toggleable per scope)

Datatree integrates with Claude Code via 5 mechanisms simultaneously:

### 4.1 Mode A — SessionStart Primer
Hook fires when Claude Code launches. Datatree composes a 1-2K token "project primer" injected as a system reminder:
- Active goal (from Step Ledger)
- Top 3 active constraints from `.claude/rules/`
- Open TaskCreate items
- Top 3 recent decisions
- Currently dirty files (uncommitted)
- Recent drift findings (red only)

### 4.2 Mode B — UserPromptSubmit Smart Inject
Hook fires on every user message. Datatree:
1. Embeds the user message
2. Queries `semantic.db` for top 5-10 relevant facts (code, decisions, prior bugs, .md content)
3. Composes a `<datatree-context>` block (~1-3K tokens)
4. Prepends to the prompt context

### 4.3 Mode C — PreToolUse Enrichment
Hook fires before any Claude tool call. Datatree:
- For Read: if file unchanged since last read in this session, returns cached summary (saves a Read tool call entirely)
- For Edit/Write: pre-injects relevant constraints (e.g., "no hardcoded colors", "no `any` types")
- For Bash: checks `tool_cache.db` for identical recent output; returns cached if hit
- For Grep/Glob: checks if equivalent query exists in cache

### 4.4 Mode D — PostToolUse Capture
Hook fires after every tool call. Datatree records:
- Tool name + parameters (verbatim)
- Full result
- File diffs (if Edit/Write)
- Timestamp + session_id
Written to `tool_cache.db` and `history.db`.

### 4.5 Mode E — On-demand MCP Tools
Even with hooks off, Claude can call any of 30+ MCP tools (see §5).

### 4.6 Mode toggles

```jsonc
// ~/.claude/plugins/datatree/settings.json
{
  "injection": {
    "session_primer": true,
    "smart_inject": true,
    "pre_tool_enrich": true,
    "post_tool_capture": true,
    "mcp_tools": true
  },
  "primer_token_budget": 1500,
  "smart_inject_token_budget": 2500,
  "max_total_overhead_per_turn": 5000
}
```

---

## 5. MCP Tool Catalog (30+ tools)

### 5.1 Recall & Search
| Tool | Purpose |
|---|---|
| `recall_decision(query)` | Search decisions log semantically |
| `recall_conversation(query, since?)` | Search conversation history |
| `recall_concept(query)` | Semantic search across all extracted concepts |
| `recall_file(path)` | Get full file state: hash, summary, last-read, blast-radius |
| `recall_todo(filter?)` | Open TaskCreate items, optionally filtered |
| `recall_constraint(scope?)` | Active constraints for current project/file |

### 5.2 Code Graph (CRG-mode)
| Tool | Purpose |
|---|---|
| `blast_radius(file_or_function)` | All callers/dependents/tests affected by a change |
| `call_graph(function)` | Direct + transitive call graph |
| `find_references(symbol)` | All usages |
| `dependency_chain(file)` | Forward + reverse import chain |
| `cyclic_deps()` | Detect circular dependencies |

### 5.3 Multimodal (Graphify-mode)
| Tool | Purpose |
|---|---|
| `graphify_corpus(path?)` | Run full multimodal extraction pass |
| `god_nodes(project?)` | Top N most-connected concepts |
| `surprising_connections()` | High-confidence unexpected edges |
| `audit_corpus()` | Generate `GRAPH_REPORT.md` style report |

### 5.4 Drift & Audit
| Tool | Purpose |
|---|---|
| `audit(scope?)` | Run all scanners; return findings list |
| `drift_findings(severity?)` | Current rule violations |
| `audit_theme()` | Hardcoded colors, missing dark: variants |
| `audit_security()` | Secrets, eval, IPC validation gaps |
| `audit_a11y()` | Missing aria-labels, contrast failures |
| `audit_perf()` | Missing memoization, sync I/O on render |
| `audit_types()` | `any`, non-null assertions, default exports |

### 5.5 Step Ledger (Command Center)
| Tool | Purpose |
|---|---|
| `step_status()` | Current step + ledger snapshot |
| `step_show(step_id)` | Detail of one step |
| `step_verify(step_id)` | Run acceptance check |
| `step_complete(step_id)` | Mark complete (only if verify passes) |
| `step_resume()` | Emit resumption bundle |
| `step_plan_from(markdown_path)` | Ingest md roadmap → ledger |

### 5.6 Time Machine
| Tool | Purpose |
|---|---|
| `snapshot()` | Manual snapshot of current shard |
| `compare(snapshot_a, snapshot_b)` | Diff two snapshots |
| `rewind(file, when)` | Show file content at a past time |

### 5.7 Health
| Tool | Purpose |
|---|---|
| `health()` | Full SLA snapshot |
| `doctor()` | Run self-test, return diagnostics |
| `rebuild(scope?)` | Re-parse from scratch (last resort) |

---

## 6. Hooks Specification

### 6.1 SessionStart
```bash
# command registered in plugin manifest
datatree session-prime --project="$CWD" --session-id="$SESSION_ID"
```
Output: JSON containing `additional_context` field for system reminder injection.

### 6.2 UserPromptSubmit
```bash
datatree inject --prompt="$USER_PROMPT" --session-id="$SESSION_ID" --cwd="$CWD"
```
Output: JSON with `additional_context` (the smart-inject bundle).

### 6.3 PreToolUse
```bash
datatree pre-tool --tool="$TOOL_NAME" --params="$TOOL_PARAMS" --session-id="$SESSION_ID"
```
Output: JSON; can short-circuit (`{"skip": true, "result": "<cached>"}`) or pass through with enrichment.

### 6.4 PostToolUse
```bash
datatree post-tool --tool="$TOOL_NAME" --result-file="$TOOL_RESULT_PATH" --session-id="$SESSION_ID"
```
Output: empty (fire-and-forget capture).

### 6.5 Stop (between turns)
```bash
datatree turn-end --session-id="$SESSION_ID"
```
Triggers summarizer; updates Step Ledger drift score.

### 6.6 SessionEnd
```bash
datatree session-end --session-id="$SESSION_ID"
```
Final flush; manifest update.

---

## 7. The Command Center & Step Ledger (the killer feature)

### 7.1 Compaction-resilience contract

**Guarantee**: If you give Claude a numbered task spanning N steps, and context compaction occurs at step K (K < N), Claude resumes at step K+1 (not 1, not K-Δ) within 1 user-prompt turn.

### 7.2 Step ledger schema (`tasks.db`)

```sql
CREATE TABLE steps (
  step_id          TEXT PRIMARY KEY,        -- hierarchical: "1", "1.1", "1.1.1"
  parent_step_id   TEXT REFERENCES steps(step_id),
  session_id       TEXT NOT NULL,
  description      TEXT NOT NULL,            -- verbatim
  acceptance_cmd   TEXT,                     -- shell command that returns 0 on success
  acceptance_check JSON,                     -- structured check (e.g., {"file_exists": "..."})
  status           TEXT NOT NULL,            -- not_started|in_progress|completed|blocked|failed
  started_at       TIMESTAMP,
  completed_at     TIMESTAMP,
  verification_proof TEXT,                   -- captured stdout of acceptance check
  artifacts        JSON,                     -- {"files_created": [...], "files_modified": [...]}
  notes            TEXT,                     -- accumulated decisions/reasoning
  blocker          TEXT,                     -- if blocked, reason
  drift_score      INTEGER DEFAULT 0
);

CREATE INDEX idx_steps_session ON steps(session_id, status);
CREATE INDEX idx_steps_parent ON steps(parent_step_id);
```

### 7.3 Resumption bundle (auto-fired after compaction)

Composed by `step_resume()`:

```
<datatree-resume>
You are paused at STEP <K> of <N>.

## Original goal (verbatim from session start)
<verbatim user message that started this work>

## Goal stack (root → current leaf)
<rendered hierarchical list>

## Completed steps (1..K-1)
<each step: id, description, proof, key artifacts>

## YOU ARE HERE — Step <K>
Description: <verbatim>
Started: <timestamp>
Last action: <last tool call or note>
Stuck on: <if blocked>
Next move: <from notes>
Acceptance: <command that must pass>

## Planned steps (K+1..N)
<each step: id, description, deps>

## Active constraints (must honor)
<list from constraints.db>

## Verification gates
<acceptance check for current step>
</datatree-resume>
```

### 7.4 Drift detection

After every assistant response, `livebus-worker` classifies the response topic vs `goal_at_top_of_stack`. If divergence detected for 2+ consecutive turns:
- Increment `drift_score` on current step
- On next UserPromptSubmit, prepend a `<datatree-redirect>` block before the smart-inject

### 7.5 Constraint enforcement

`PreToolUse` hook checks proposed Edit/Write against `constraints.db`. Examples:
- Editing `.tsx` → inject "no hardcoded colors, dark: variants required, named exports only"
- Writing new component → inject "use functional + hooks, no classes"
- Running `git push --force` → BLOCK; require explicit user re-confirmation

---

## 8. Markdown Ingestion ("drinks .md like Claude Code does")

### 8.1 File patterns recognized

| Pattern | Treatment |
|---|---|
| `CLAUDE.md`, `CLAUDE.local.md` | Highest-priority rules; fed to drift detector |
| `AGENTS.md`, `GEMINI.md`, `.cursorrules`, `.windsurfrules`, `.aiderrules` | Cross-platform rules absorbed identically |
| `~/.claude/memory/*.md` | Global user facts; loaded into `memory.db` (global scope) |
| `<project>/.claude/memory/*.md` | Project-scoped memory |
| `<project>/.claude/rules/*.md` | Hard rules, drift-enforced |
| `<project>/.claude/skills/**/SKILL.md` | Skill registry; indexed by name + description |
| `<project>/.claude/agents/*.md` | Agent registry |
| `README.md`, `CONTRIBUTING.md`, `ARCHITECTURE.md`, `SECURITY.md` | Project docs; concept-extracted |
| `docs/**/*.md`, `spec/**/*.md`, `RFC/**/*.md`, `ADR/**/*.md` | Decision records |
| `docs/superpowers/specs/*.md` | Brainstorming specs; linked to implementation |
| `docs/solutions/*.md` | Past solutions; searched before debugging |

### 8.2 Per-file processing pipeline

1. Hash check (skip if unchanged)
2. Frontmatter parse (YAML; extract `name`, `description`, `type`, `tags`)
3. Heading tree extracted (becomes navigable structure)
4. Code blocks extracted (linked to language; fed to parsers)
5. Internal links resolved (linked nodes in graph)
6. External links cached (Q layer)
7. Mermaid diagrams parsed → embedded in vision layer
8. Embedding generated (G layer)
9. Concept extraction (B/AA layers; LLM-assisted in graphify-mode)
10. Drift check (does .md content match observed code?)

### 8.3 Drift detection examples

- README says "the auth flow lives in `src/auth/`" but no such folder exists → drift finding
- ARCHITECTURE.md mentions component "X" not present in code → drift finding
- CLAUDE.md says "always use cn()" but `<file>` uses raw `className` concatenation → drift finding

---

## 9. Vision Layer (14 view modes)

### 9.1 Renderer

- **Sigma.js v3** (WebGL) for force-directed and arc views
- **deck.gl** for 3D galaxy view
- **D3 v7** for hierarchical/sunburst/treemap (SVG, simpler interactions)
- **Three.js** for 3D galaxy
- All views share a unified data layer (graphology) so switching views is instant

### 9.2 The 14 views

1. **Force-Galaxy** (CRG-style, GPU-rendered, 100k+ nodes @ 60fps)
2. **Hierarchy Tree** (folder structure left-rail)
3. **Sunburst** (proportional rings, color = churn)
4. **Treemap** (sized by complexity)
5. **Sankey — Type Flow** (type lifecycle visualization)
6. **Sankey — Domain Flow** (auth/sync/etc.)
7. **Arc/Chord** (cyclic dependencies)
8. **Timeline** (X = git history, Y = files; heat dots)
9. **Heatmap Grid** (file × metric)
10. **Layered Architecture** (stacked planes)
11. **3D Project Galaxy** (cosmos → planet semantic zoom)
12. **Theme Palette** (CSS variables as swatches with WCAG badges)
13. **Test Coverage Map** (function grid colored by coverage %)
14. **Risk Dashboard** (combined scoring + top-10 cards)

### 9.3 Interactions (every view)

- Hover → tooltip with file/lines/last-commit/blast-radius
- Click → side panel with file content + summary + tests + history
- Right-click → context menu (open in editor, find references, run audit)
- Cmd+click → multi-select for combined blast radius
- Drag → physics ripple through dependents
- Lasso → "audit this region against my rules"

### 9.4 Live updates

- WebSocket connection to `livebus-worker`
- Edit a file → node pulses (yellow ring) within 50ms
- Test fails → node turns red, badge appears, push notification
- Subagent edits → cyan flash
- Drift violation → red glow on offending node

### 9.5 Time machine

- Bottom slider scrubs through git history
- Play button → animated time-lapse
- Compare mode → side-by-side or diff overlay
- Future projection (LLM-assisted) → predicted graph after a planned change

### 9.6 AI overlays (toggleable)

- Concept clusters (auto-colored by embedding)
- Drift heatmap (red = many violations)
- Risk heatmap (churn × complexity × coverage)
- Hot-now pulse (auto-highlights current focus)
- Knowledge gaps (untouched-but-depended-on files)

### 9.7 Command Center tab

A dedicated view at `/command-center` with:
- Goal stack visualization
- Step ledger timeline (with compaction event markers)
- Active constraints panel
- Files-touched list
- Decisions log
- Drift indicator
- Search across full session history

### 9.8 Delivery modes

| Mode | Command | Output |
|---|---|---|
| Native desktop | `datatree view` | Tauri window, OS-integrated |
| Web | `datatree serve --web` | `localhost:7777` |
| Standalone HTML | `datatree export --view <name> --filter <q>` | Self-contained file |

---

## 10. Multimodal Engine (graphify-style)

### 10.1 Pipeline

```
File scan → categorize by type → dispatch to extractor → store in corpus.db → concept extraction → graph integration
```

### 10.2 Extractors

| Type | Extractor | Notes |
|---|---|---|
| `.pdf` | PyMuPDF + Tesseract fallback | Page-by-page text + figure detection |
| `.png/.jpg/.jpeg/.webp` | Tesseract OCR + CLIP element detection | UI element extraction |
| `.gif` | First-frame Tesseract | |
| `.mp4/.mov/.webm` | faster-whisper | Domain-aware prompt from corpus god-nodes |
| `.mp3/.wav/.m4a/.flac` | faster-whisper | Same |
| `.ipynb` | nbformat parser | Code + markdown cells |
| `.docx` | python-docx | |
| `.xlsx` | openpyxl | Sheet → table extraction |

### 10.3 Concept extraction

- Phase 1: Tree-sitter AST (deterministic, no LLM)
- Phase 2: Whisper for av files (caches transcripts)
- Phase 3: Claude subagent (or local LLM) for concepts + relationships
- Phase 4: Merge into NetworkX graph (Rust port: petgraph + igraph)
- Phase 5: Leiden community detection
- Phase 6: Tag every edge `EXTRACTED` / `INFERRED` / `AMBIGUOUS`

### 10.4 Audit report (`audit_corpus()`)

Generates a markdown report:
- God nodes (top-10 most connected concepts)
- Surprising connections (high-confidence unexpected edges)
- Suggested questions (LLM-generated based on graph topology)
- Drift findings (between docs and code)

---

## 11. Live Bus

### 11.1 Transport

- Server-Sent Events (default; works through firewalls, no special client setup)
- WebSocket (alternative for bidirectional)
- Internal: tokio MPSC channels for inter-worker

### 11.2 Channel topics

```
project.<hash>.file_changed
project.<hash>.test_status
project.<hash>.drift_finding
project.<hash>.subagent_event
session.<id>.compaction_detected
session.<id>.step_advanced
system.health
system.degraded_mode
```

### 11.3 Subscriber API

```typescript
const sub = await datatree.live.subscribe({
  topics: ["project.*.file_changed", "session.current.compaction_detected"],
  callback: (event) => { /* ... */ }
});
```

---

## 12. Plugin Manifest & Install

### 12.1 plugin.json

```jsonc
{
  "name": "datatree",
  "version": "0.1.0",
  "displayName": "Datatree — The AI Superbrain",
  "description": "Persistent per-project SQLite memory + 14-view live graph + drift detector + 30+ MCP tools. Claude never starts cold, never loses its place.",
  "author": "Anish Trivedi",
  "license": "MIT",
  "homepage": "https://github.com/anishtrivedi/datatree",
  "scopes": ["global", "user", "project"],
  "platforms": ["win32", "darwin", "linux"],
  "components": {
    "mcpServers": [
      { "name": "datatree", "config": ".mcp/datatree.json" }
    ],
    "hooks": [
      { "event": "SessionStart", "command": "datatree session-prime" },
      { "event": "UserPromptSubmit", "command": "datatree inject" },
      { "event": "PreToolUse", "command": "datatree pre-tool" },
      { "event": "PostToolUse", "command": "datatree post-tool" },
      { "event": "Stop", "command": "datatree turn-end" },
      { "event": "SessionEnd", "command": "datatree session-end" }
    ],
    "skills": [
      "skills/datatree-query.md",
      "skills/datatree-audit.md",
      "skills/datatree-resume.md"
    ],
    "agents": ["agents/datatree-archivist.md"],
    "commands": [
      { "name": "/dt-view", "command": "datatree view" },
      { "name": "/dt-audit", "command": "datatree audit" },
      { "name": "/dt-recall", "command": "datatree recall" },
      { "name": "/dt-blast", "command": "datatree blast" },
      { "name": "/dt-graphify", "command": "datatree graphify" },
      { "name": "/dt-godnodes", "command": "datatree godnodes" },
      { "name": "/dt-drift", "command": "datatree drift" },
      { "name": "/dt-history", "command": "datatree history" },
      { "name": "/dt-snap", "command": "datatree snap" },
      { "name": "/dt-doctor", "command": "datatree doctor" },
      { "name": "/dt-rebuild", "command": "datatree rebuild" },
      { "name": "/dt-step", "command": "datatree step" }
    ]
  },
  "install": {
    "preInstall": "scripts/install-supervisor.js",
    "postInstall": "scripts/start-daemon.js",
    "preUninstall": "scripts/stop-daemon.js"
  }
}
```

### 12.2 Install scopes

| Scope | Config location | Effect |
|---|---|---|
| Global | `~/.claude/plugins/datatree/` | All projects on this machine |
| User | `~/.claude.json` `datatree` key | This user across sync'd machines |
| Project | `<repo>/.claude/datatree.json` | Committed to git; team-shared |

### 12.3 Project-scope config example

```jsonc
// <repo>/.claude/datatree.json
{
  "rules": {
    "noHardcodedColors": true,
    "themeFile": "src/theme/tokens.css",
    "ipcContractsPath": "electron/ipc-types.ts",
    "minTestCoverage": 80
  },
  "scanners": ["theme", "security", "ipc-contracts", "react-perf"],
  "customTools": [
    { "name": "auditOrionSync", "path": ".claude/datatree-tools/orion-sync.ts" }
  ],
  "excludePaths": ["dist/", "out/", "release/"],
  "alertChannels": {
    "drift": "live-bus",
    "security": "blocking-prompt"
  }
}
```

### 12.4 Marketplace publishing

**Path**: self-hosted GitHub marketplace first, official Anthropic marketplace second.

```jsonc
// marketplace.json (in anishtrivedi/claude-plugins repo root)
{
  "plugins": [
    {
      "name": "datatree",
      "git": "https://github.com/anishtrivedi/datatree",
      "version": "0.1.0"
    }
  ]
}
```

User installs:
```
/plugin marketplace add github:anishtrivedi/claude-plugins
/plugin install datatree
```

---

## 13. Never-Fail Engineering Matrix

| Failure mode | Mechanism |
|---|---|
| Process crash | Supervisor watchdog; restart <100ms; backoff on crash-loop |
| Memory leak | Per-worker RSS cap; OOM-kill + restart |
| Infinite loop / freeze | 30s default tool timeout; watchdog kills hung worker |
| Disk full | Refuses writes when disk <5% free; auto-prunes old snapshots |
| DB corruption | WAL log replay → restore to last snapshot; RPO 1h, RTO 5s |
| Race conditions | Single-writer per shard; multi-reader via WAL |
| Slow LLM | Async job queue with backpressure; "queued" returned immediately |
| Hot-reload bugs | Shadow mode 60s before promotion; auto-rollback on >1% error rate |
| Schema migration | Versioned, append-only; never drops columns; downgrade-safe |
| Resource starvation | Per-worker cgroup/job-object limits |
| Network failure | Zero network deps in hot path; circuit breaker on remote calls |
| Power loss mid-write | WAL atomicity; partial writes rolled back on restart |
| Cross-platform bugs | Statically-linked Rust binaries per OS; CI runs chaos suite |
| Self-degradation | Brain overloaded → fall back to deterministic; never fully offline |
| Schema drift | `datatree doctor` self-test every 60s; failure → alert + restart |
| Crash dump loss | Every panic writes minidump to `~/.datatree/crashes/` |

### 13.1 SLA dashboard at `localhost:7777/health`

- Uptime % (24h / 7d / 30d)
- p50 / p95 / p99 query latency per tool
- Worker restart count
- Cache hit rate
- Disk usage trend
- LLM job queue depth

---

## 13.5 Database Operations Layer

This section is the single source of truth for how every part of datatree touches SQLite. No module outside this layer constructs file paths, issues raw SQL, or holds a database connection directly. All access flows through these seven sub-layers.

---

### 13.5.1 DB Builder

**Responsibility**: Given a project path, produce the full 21-shard directory tree under `~/.datatree/projects/<sha256(canonical_path)>/`, apply all schema DDL, set PRAGMAs, and record the schema version. Idempotent: skip files already at the current version, run migration scripts if version is behind.

**Public API - Rust trait**

```rust
// store/src/builder.rs
pub trait DbBuilder: Send + Sync {
    async fn build_project(&self, project_path: &Path) -> Result<ProjectShard, BuildError>;
    async fn rebuild_shard(&self, shard: &ProjectShard, name: ShardName) -> Result<(), BuildError>;
    fn schema_version(&self) -> u32;
}
```

**TypeScript wrapper**

```typescript
// mcp/src/db/builder.ts
interface DbBuilder {
  buildProject(projectPath: string): Promise<ProjectShard>;
  rebuildShard(shard: ProjectShard, name: ShardName): Promise<void>;
  schemaVersion(): number;
}
```

**Implementation strategy**

1. Canonicalize `project_path` before hashing. The hash is `sha256(canonical_utf8_path)` encoded as lowercase hex. Canonicalization is delegated to `AccessPathManager` (13.5.3).
2. For each of the 21 shard names, open with `rusqlite::Connection::open_with_flags(path, SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE)`.
3. Apply bootstrap PRAGMAs on every connection before DDL: `PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000; PRAGMA synchronous = NORMAL; PRAGMA mmap_size = 268435456`.
4. Check `_schema_version` table. Apply pending migrations in a transaction. Update version on commit. Forward-only, append-only, the SQLx/Diesel migration model.
5. Set file permissions to `0o600` (owner read/write only). On Windows use `SetFileAttributes` via `windows-rs`.
6. Write `shard_manifest.json` at the project root: hash, original_path, created_at, schema_version, shard_names. Recovery index for `meta.db` rebuild.

**Key invariants**

- Same project path always produces the same hash (canonical absolute path, not display or relative form).
- Builder never touches a shard already at the current schema version.
- Migrations execute inside a transaction. Mid-flight failure rolls back; shard stays at prior version.
- `build_project` is safe to call concurrently for different projects. Same-project concurrent calls: second caller blocks on SQLite WAL writer lock, finds schema current, returns immediately.

**Failure modes**

| Failure | Recovery |
|---|---|
| Disk full during DDL | Roll back; emit `DiskFullError`; supervisor triggers snapshot prune |
| Permission denied | `BuildError::PermissionDenied`; surfaced via `doctor()` |
| Migration SQL error | `BuildError::MigrationFailed`; shard stays at prior version |
| `_schema_version` missing | Treat as version 0; run all migrations |

**Performance targets**: Current-version project under 2ms. Cold build of all 21 shards under 150ms.

---

### 13.5.2 DB Finder

**Responsibility**: Resolve any caller-supplied input to a `ProjectShard` struct. Multi-strategy lookup with deterministic priority ordering. Supports cross-project queries.

**Public API - Rust trait**

```rust
// store/src/finder.rs
pub trait DbFinder: Send + Sync {
    async fn find(&self, input: FinderInput) -> Result<ProjectShard, FinderError>;
    async fn find_all(&self, predicate: CrossProjectPredicate) -> Result<Vec<ProjectShard>, FinderError>;
    async fn find_current(&self) -> Result<ProjectShard, FinderError>;
}

pub enum FinderInput {
    ExactPath(PathBuf),
    PartialName(String),
    Hash(String),
    FileInsideProject(PathBuf),
    RecentlyEdited,
}

pub enum CrossProjectPredicate {
    HasDependency { name: String, version_range: Option<String> },
    ContainsSymbol(String),
    ModifiedAfter(DateTime<Utc>),
    HasError { pattern: String },
}
```

**TypeScript wrapper**

```typescript
// mcp/src/db/finder.ts
interface DbFinder {
  find(input: FinderInput): Promise<ProjectShard>;
  findAll(predicate: CrossProjectPredicate): Promise<ProjectShard[]>;
  findCurrent(): Promise<ProjectShard>;
}
```

**Lookup strategy chain (tried in order; first hit wins)**

1. **Hash exact match**: 64-char hex input, check `~/.datatree/projects/<hash>/shard_manifest.json`. O(1), no DB query.
2. **Path hash**: canonicalize input path, compute sha256, check if shard directory exists. O(1).
3. **CWD ancestor traversal**: walk parent directories from `current_dir()` until canonical hash matches a row in `meta.db`. Stops at filesystem root or 32 levels.
4. **Partial name match**: `SELECT hash FROM projects WHERE display_name LIKE ?` in `meta.db`. Single match returns it; multiple matches return `FinderError::Ambiguous`.
5. **Recently edited**: `SELECT hash FROM projects ORDER BY last_accessed_at DESC LIMIT 1`.

**Cross-project search**: Iterates over `meta.db` project rows, opens each project's relevant shard, runs the predicate query, streams results (see 13.5.5).

**Key invariants**

- `find` is read-only. It never writes to any database.
- `find_current` runs on every MCP tool call without an explicit project. Strategy 3 is the hot path, under 1ms for typical projects.
- `meta.db` is written by the builder on shard creation; `last_accessed_at` updated on each `find_current` call.

**Failure modes**

| Failure | Behavior |
|---|---|
| No shard found | `FinderError::NotFound`; caller can invoke `build_project` |
| `meta.db` missing | Rebuild by scanning `~/.datatree/projects/*/shard_manifest.json` |
| `shard_manifest.json` missing | `FinderError::Corrupted`; trigger `rebuild_shard` |
| Ancestor traversal exceeds 32 levels | Return `FinderError::NotFound` |

**Performance targets**: Strategies 1/2 under 0.5ms. Strategy 3 under 1ms. `find_all` across 50 projects under 200ms.

---

### 13.5.3 Access Path Manager

**Responsibility**: Single source of truth for every file path in the datatree directory tree. No other module constructs paths with string concatenation.

**Public API - Rust**

```rust
// store/src/paths.rs
pub struct PathManager {
    root: PathBuf,  // ~/.datatree, overridable via DATATREE_HOME env var
}

impl PathManager {
    pub fn new() -> Self;
    pub fn meta_db(&self) -> PathBuf;
    pub fn cache_docs(&self) -> PathBuf;
    pub fn crashes(&self) -> PathBuf;
    pub fn supervisor_log(&self) -> PathBuf;
    pub fn project_root(&self, hash: &str) -> PathBuf;
    pub fn shard(&self, hash: &str, name: ShardName) -> PathBuf;
    pub fn shard_wal(&self, hash: &str, name: ShardName) -> PathBuf;
    pub fn shard_shm(&self, hash: &str, name: ShardName) -> PathBuf;
    pub fn snapshots_dir(&self, hash: &str) -> PathBuf;
    pub fn snapshot(&self, hash: &str, timestamp: &str) -> PathBuf;
    pub fn manifest(&self, hash: &str) -> PathBuf;
}

pub enum ShardName {
    Graph, History, ToolCache, Tasks, Semantic, Git, Memory, Errors,
    Multimodal, Deps, Tests, Perf, Findings, Agents, Refactors,
    Contracts, Insights, Livestate, Telemetry, Corpus,
}

impl ShardName {
    pub fn filename(&self) -> &'static str;  // "graph.db", "history.db", etc.
}
```

**TypeScript wrapper**

```typescript
// mcp/src/db/paths.ts
interface PathManager {
  metaDb(): string;
  shard(hash: string, name: ShardName): string;
  snapshotsDir(hash: string): string;
  snapshot(hash: string, timestamp: string): string;
}
type ShardName =
  | 'graph' | 'history' | 'tool_cache' | 'tasks' | 'semantic' | 'git'
  | 'memory' | 'errors' | 'multimodal' | 'deps' | 'tests' | 'perf'
  | 'findings' | 'agents' | 'refactors' | 'contracts' | 'insights'
  | 'livestate' | 'telemetry' | 'corpus';
```

**Key invariants**

- `PathManager::new()` called exactly once at process start, stored as `Arc<PathManager>`. No module calls `dirs::home_dir()` independently.
- `ShardName` is a compile-time guarantee against typos. Adding a shard requires updating the enum; the compiler enforces completeness via exhaustive match in `ShardName::filename()`.
- WAL and SHM companion files always co-located with parent `.db`; `shard_wal` and `shard_shm` derive from `shard` mechanically.
- `DATATREE_HOME` overrides the default root for testing and CI.

---

### 13.5.4 Query Layer

**Responsibility**: Typed, prepared, pooled query execution across all 21 shards. Single-writer per shard enforced via MPSC channel to the store-worker. Read queries run through a per-shard connection pool (up to 4 concurrent readers). Prepared statements cached per connection.

**Public API - Rust trait**

```rust
// store/src/query.rs
pub trait QueryExecutor: Send + Sync {
    async fn read<T: DeserializeOwned>(
        &self, shard: ShardName, project_hash: &str, query: TypedQuery<T>,
    ) -> Result<DbResponse<Vec<T>>, QueryError>;

    async fn write(
        &self, shard: ShardName, project_hash: &str, mutation: TypedMutation,
    ) -> Result<DbResponse<WriteResult>, QueryError>;

    fn read_stream<T: DeserializeOwned>(
        &self, shard: ShardName, project_hash: &str, query: TypedQuery<T>,
    ) -> impl Stream<Item = Result<T, QueryError>>;

    async fn explain(&self, shard: ShardName, project_hash: &str, sql: &str)
        -> Result<String, QueryError>;
}
```

**TypeScript wrapper**

```typescript
// mcp/src/db/query.ts
interface QueryExecutor {
  read<T>(shard: ShardName, projectHash: string, query: TypedQuery<T>): Promise<DbResponse<T[]>>;
  write(shard: ShardName, projectHash: string, mutation: TypedMutation): Promise<DbResponse<WriteResult>>;
  readStream<T>(shard: ShardName, projectHash: string, query: TypedQuery<T>): AsyncIterable<T>;
}
```

**Internal implementation**

The store-worker owns one `WriterTask` per shard holding a single `rusqlite::Connection` opened read-write. Write requests arrive via `tokio::sync::mpsc::Sender<WriteRequest>` (capacity 1024). The receiver loop processes one write at a time inside a transaction; the result returns through a oneshot channel.

Reads bypass the writer. A `ReaderPool` per shard holds up to 4 `rusqlite::Connection` objects opened `SQLITE_OPEN_READONLY`. WAL mode permits simultaneous readers alongside the single writer. Connections are leased via `tokio::sync::Semaphore` (4 permits).

Prepared statements: each connection maintains a `HashMap<u64, CachedStatement>` keyed by SQL hash. Cache is bounded to 256 entries per connection via LRU eviction. `TypedQuery<T>` carries the SQL string, a bind-params closure, and a row-mapper closure.

**Key invariants**

- No two tasks ever hold the writer connection for the same shard simultaneously. The MPSC channel is the mutex, no exceptions.
- Read connections are opened `SQLITE_OPEN_READONLY` and can never issue writes.
- Prepared statement cache is LRU-bounded; no unbounded memory growth.

**Failure modes**

| Failure | Behavior |
|---|---|
| `SQLITE_BUSY` on reader | Retry within `busy_timeout`; then `QueryError::Timeout` |
| Writer MPSC full | `QueryError::Backpressure` after 100ms wait |
| `SQLITE_CORRUPT` | `QueryError::Corrupted`; triggers integrity check + restore (13.5.7) |

**Performance targets**: Cached read under 0.5ms p99. Write through MPSC under 2ms p99. Pool lease acquisition under 0.1ms when a connection is available.

---

### 13.5.5 Response Layer

**Responsibility**: Every result from any query, injection, or lifecycle call is wrapped in a uniform envelope before crossing any API boundary.

**Envelope definition - Rust**

```rust
// store/src/response.rs
pub struct DbResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<DbError>,
    pub latency_ms: f64,
    pub cache_hit: bool,
    pub source_db: ShardName,
    pub source_project: String,
    pub rows_scanned: Option<u64>,
    pub rows_returned: Option<u64>,
}

pub enum DbError {
    NotFound    { table: String, id: String },
    Corrupted   { shard: ShardName, detail: String },
    Locked      { shard: ShardName, waited_ms: u64 },
    Timeout     { after_ms: u64 },
    SchemaMismatch { expected: u32, actual: u32 },
    Backpressure { queue_depth: usize },
    DiskFull    { available_bytes: u64 },
    Validation  { field: String, reason: String },
    Internal    { code: String, message: String },
}
```

**TypeScript envelope**

```typescript
// mcp/src/db/response.ts
interface DbResponse<T> {
  success: boolean;
  data: T | null;
  error: DbError | null;
  latency_ms: number;
  cache_hit: boolean;
  source_db: ShardName;
  source_project: string;
  rows_scanned: number | null;
  rows_returned: number | null;
}

type DbErrorCode =
  | 'NOT_FOUND' | 'CORRUPTED' | 'LOCKED' | 'TIMEOUT'
  | 'SCHEMA_MISMATCH' | 'BACKPRESSURE' | 'DISK_FULL' | 'VALIDATION' | 'INTERNAL';

interface DbError { code: DbErrorCode; message: string; detail: Record<string, unknown>; }
```

**Streaming responses**: For large row sets, the response layer returns `Stream<Item = Result<DbResponse<Row>, DbError>>`. The MCP server converts this to newline-delimited JSON over SSE. The cursor buffers at most 64 rows at a time, no full materialization.

**Latency accounting**: `latency_ms` measures from `read()` or `write()` invocation to last row ready. Includes lock acquisition, statement preparation if uncached, execution, and deserialization.

**Key invariants**

- `success: false` always has non-null `error`. `success: true` always has non-null `data` (may be empty `Vec`).
- `DbError` variants are exhaustive, no string-only errors cross the API boundary.
- `cache_hit: true` means the result came from `tool_cache.db` (layer E) or the prepared-statement cache.

---

### 13.5.6 Injection Layer

**Responsibility**: All insert, update, and soft-delete operations. Enforces idempotency, wraps in transactions, validates before write, emits post-write events to livebus, writes audit trail. No raw DML runs outside this layer.

**Public API - Rust trait**

```rust
// store/src/injection.rs
pub trait InjectionLayer: Send + Sync {
    async fn upsert<T: Serialize + HasSchema>(
        &self, shard: ShardName, project_hash: &str, record: T, idempotency_key: &str,
    ) -> Result<DbResponse<WriteResult>, InjectionError>;

    async fn soft_delete(
        &self, shard: ShardName, project_hash: &str, table: &str, id: &str,
    ) -> Result<DbResponse<WriteResult>, InjectionError>;

    async fn bulk_insert<T: Serialize + HasSchema>(
        &self, shard: ShardName, project_hash: &str, records: Vec<T>,
    ) -> Result<DbResponse<BulkWriteResult>, InjectionError>;
}
```

**TypeScript wrapper**

```typescript
// mcp/src/db/injection.ts
interface InjectionLayer {
  upsert<T>(shard: ShardName, projectHash: string, record: T, idempotencyKey: string): Promise<DbResponse<WriteResult>>;
  softDelete(shard: ShardName, projectHash: string, table: string, id: string): Promise<DbResponse<WriteResult>>;
  bulkInsert<T>(shard: ShardName, projectHash: string, records: T[]): Promise<DbResponse<BulkWriteResult>>;
}
```

**Internal pipeline for every write**

```
1. T::validate() -- field constraints + business rules
   Failure: InjectionError::Validation; no DB touched

2. SELECT result_id FROM _idempotency_log WHERE key = ?
   Hit: return prior WriteResult; cache_hit: true; stop

3. Send WriteRequest to store-worker MPSC channel

4. BEGIN IMMEDIATE TRANSACTION:
   a. DML (INSERT OR REPLACE / UPDATE WHERE id = ?)
   b. INSERT INTO audit_log (table, record_id, action, old_values, new_values)
   c. INSERT INTO _idempotency_log (key, executed_at, result_id, shard)
   d. COMMIT

5. Emit: project.<hash>.<table>_changed { id, action, changed_at }

6. Return DbResponse<WriteResult>
```

**Idempotency log schema** (present in every shard):

```sql
CREATE TABLE IF NOT EXISTS _idempotency_log (
    key          TEXT PRIMARY KEY,
    executed_at  TEXT NOT NULL DEFAULT (datetime('now')),
    result_id    TEXT NOT NULL,
    shard        TEXT NOT NULL
);
CREATE INDEX idx_idempotency_age ON _idempotency_log(executed_at);
```

Keys older than 7 days are pruned by the weekly vacuum lifecycle operation.

**Key invariants**

- Physical `DELETE` is never issued. All removal is soft-delete via `deleted_at`.
- Every write is atomic: DML + audit entry + idempotency entry commit together or not at all.
- Idempotency keys are caller-supplied. Auto-generation would make retries non-idempotent.
- Livebus emit failure is non-fatal. Write commits regardless; failure logged to `telemetry.db`.

**Failure modes**

| Failure | Behavior |
|---|---|
| Validation failure | `InjectionError::Validation`; no write attempted |
| Idempotency key collision | Return prior result; `cache_hit: true` |
| Deadlock (pathological) | Retry once with 50ms backoff; then `InjectionError::Locked` |
| Livebus emit failure | Write commits; failure logged; non-fatal |

**Performance targets**: Single upsert end-to-end under 5ms p99. Bulk insert of 1000 records in one transaction under 50ms.

---

### 13.5.7 Lifecycle Operations Layer

**Responsibility**: Backup, restore, snapshot, migration, vacuum, integrity check, repair, archive, and purge. Long-running or destructive operations run on a dedicated task pool, report progress on livebus, and are never invoked on the hot path.

**Public API - Rust trait**

```rust
// store/src/lifecycle.rs
pub trait LifecycleManager: Send + Sync {
    async fn snapshot(&self, project_hash: &str) -> Result<SnapshotRef, LifecycleError>;
    async fn restore(&self, project_hash: &str, snapshot: &SnapshotRef) -> Result<(), LifecycleError>;
    async fn backup_to(&self, project_hash: &str, dest: &Path) -> Result<(), LifecycleError>;
    async fn vacuum(&self, project_hash: &str, shard: Option<ShardName>) -> Result<VacuumStats, LifecycleError>;
    async fn integrity_check(&self, project_hash: &str) -> Result<IntegrityReport, LifecycleError>;
    async fn wal_checkpoint(&self, project_hash: &str, shard: ShardName) -> Result<(), LifecycleError>;
    async fn migrate(&self, project_hash: &str) -> Result<MigrationReport, LifecycleError>;
    async fn archive(&self, project_hash: &str) -> Result<ArchiveRef, LifecycleError>;
    async fn purge(&self, project_hash: &str, confirmed: bool) -> Result<(), LifecycleError>;
    async fn repair(&self, project_hash: &str, shard: ShardName) -> Result<RepairReport, LifecycleError>;
}
```

**TypeScript wrapper**

```typescript
// mcp/src/db/lifecycle.ts
interface LifecycleManager {
  snapshot(projectHash: string): Promise<SnapshotRef>;
  restore(projectHash: string, snapshot: SnapshotRef): Promise<void>;
  vacuum(projectHash: string, shard?: ShardName): Promise<VacuumStats>;
  integrityCheck(projectHash: string): Promise<IntegrityReport>;
  migrate(projectHash: string): Promise<MigrationReport>;
  archive(projectHash: string): Promise<ArchiveRef>;
  purge(projectHash: string, confirmed: boolean): Promise<void>;
  repair(projectHash: string, shard: ShardName): Promise<RepairReport>;
}
```

**Operation details**

`snapshot`: Uses SQLite online backup API (`sqlite3_backup_init / step / finish`) -- writer blocked less than 1ms per step; readers never blocked. Output written to `snapshots/YYYY-MM-DD-HH/<shard>.db`. After all 21 shards complete, runs `PRAGMA integrity_check` on each. Keeps last 7 snapshots. Emits `project.<hash>.snapshot_complete` on livebus.

`restore`: Drains writer MPSC channel (rejects new writes with `Backpressure`), replaces shard files by copy from snapshot directory, recycles all connections, resumes writer. Completes under 5s.

`vacuum`: `PRAGMA wal_checkpoint(TRUNCATE)` first, then `VACUUM`. Reclaims space from soft-deleted rows. Run weekly by supervisor scheduler.

`integrity_check`: `PRAGMA integrity_check` and `PRAGMA foreign_key_check` on every shard. Returns `IntegrityReport` with per-shard pass/fail. Called by `doctor()` MCP tool every 60s.

`repair`: On failure: (1) WAL replay from `-wal` companion file, (2) restore from most recent snapshot if still corrupt, (3) `DbBuilder::rebuild_shard` if no snapshot -- data for that shard is lost; other shards untouched. Emits `system.degraded_mode` during repair.

`archive`: Compresses project shard directory to `.tar.zst`, moves to `~/.datatree/archive/`, removes live directory. Updates `meta.db` row with `status = 'archived'`. Used for projects inactive 90+ days.

`purge`: Removes shard directory and `meta.db` row. Irreversible. Requires `confirmed: true` -- default `false` returns `LifecycleError::ConfirmationRequired`. Never exposed through MCP tools; CLI only: `datatree purge --project <hash> --confirm`.

**Key invariants**

- `snapshot` never holds an exclusive writer lock more than 1ms at a time.
- `restore` is the only operation that replaces shard files on disk.
- `purge` is the only operation that issues a physical filesystem delete. All other removal is soft-delete.
- All lifecycle operations emit progress events on livebus.

**Performance targets**: `snapshot` for 21 shards (typical 50MB) under 2s. `integrity_check` per shard under 500ms. `vacuum` per 100MB shard under 10s.

---

### 13.5.8 Unified Module Layout

```
datatree/
+-- store/
    +-- src/
        +-- lib.rs             -- builds the DaLayer struct composing all 7 sub-layers
        +-- builder.rs         -- DbBuilder trait + DefaultDbBuilder impl
        +-- finder.rs          -- DbFinder trait + MultiStrategyFinder impl
        +-- paths.rs           -- PathManager + ShardName enum
        +-- query.rs           -- QueryExecutor trait + PooledQueryExecutor + TypedQuery
        +-- response.rs        -- DbResponse<T> + DbError enum
        +-- injection.rs       -- InjectionLayer trait + TransactionalInjectionLayer impl
        +-- lifecycle.rs       -- LifecycleManager trait + DefaultLifecycleManager impl
        +-- pool.rs            -- ReaderPool + WriterTask + MPSC plumbing
        +-- migrations/
        |   +-- mod.rs
        |   +-- v001_initial_schema.sql
        |   +-- v002_add_idempotency_log.sql
        |   +-- ...
        +-- schemas/
        |   +-- graph.sql
        |   +-- history.sql
        |   +-- ...
        +-- tests/
            +-- builder_test.rs
            +-- finder_test.rs
            +-- query_test.rs
            +-- injection_test.rs
            +-- lifecycle_test.rs

datatree/
+-- mcp/
    +-- src/
        +-- db/
            +-- index.ts       -- re-exports all sub-layers as DaLayer object
            +-- builder.ts
            +-- finder.ts
            +-- paths.ts
            +-- query.ts
            +-- response.ts
            +-- injection.ts
            +-- lifecycle.ts
```

The `DaLayer` struct (Rust) and `DaLayer` object (TS) are the single entrypoint all callers use:

```rust
pub struct DaLayer {
    pub paths:     Arc<PathManager>,
    pub builder:   Arc<dyn DbBuilder>,
    pub finder:    Arc<dyn DbFinder>,
    pub query:     Arc<dyn QueryExecutor>,
    pub injection: Arc<dyn InjectionLayer>,
    pub lifecycle: Arc<dyn LifecycleManager>,
}
```

---

### 13.5.9 End-to-End Example -- Store a Decision and Emit a Live Event

All 7 sub-layers in action for one representative operation.

```typescript
// mcp/src/tools/store_decision.ts
import { da } from '../db/index.ts';  // DaLayer singleton

export async function storeDecision(params: StoreDecisionParams): Promise<DbResponse<WriteResult>> {
  // 13.5.2 Finder: resolve project from cwd via ancestor traversal, under 1ms
  const shard = await da.finder.findCurrent();

  // 13.5.1 Builder: idempotent -- skips if schema current, under 2ms
  await da.builder.buildProject(shard.projectPath);

  // 13.5.3 Paths: PathManager derives path -- no manual string construction
  const _dbPath = da.paths.shard(shard.hash, 'history');

  // 13.5.6 Injection: validate -> idempotency check -> transaction -> audit -> livebus emit
  const result = await da.injection.upsert(
    'history', shard.hash,
    { id: crypto.randomUUID(), problem: params.problem, solution: params.solution,
      root_cause: params.rootCause, session_id: params.sessionId,
      created_at: new Date().toISOString() },
    `decision:${params.sessionId}:${params.idempotencyIndex}`,
  );

  // 13.5.5 Response: result is already DbResponse<WriteResult> -- return directly
  // livebus emitted project.<hash>.history_changed inside the injection layer
  // 13.5.4 Query layer: used internally by injection for the idempotency SELECT
  // 13.5.7 Lifecycle: snapshot runs hourly via supervisor scheduler -- not called here
  return result;
}
```

When this returns, the vision layer WebSocket subscriber receives `project.<hash>.history_changed` within 50ms and pulses the `history.db` node in the live graph. A retry call with the same idempotency key returns `cache_hit: true` without touching the writer.

---

**Design references**: Single-writer channel mirrors rqlite leader-only writes with WAL-based read fan-out. Forward-only migration versioning follows the SQLx migration runner. Typed query structs with bind-param closures borrow from Diesel DSL applied to rusqlite without ORM overhead. Online backup API for non-blocking snapshots follows the litestream technique adapted for hourly point-in-time snapshots. Idempotency key pattern adapted from Stripe idempotency infrastructure for embedded single-node use.


## 14. Performance Budgets

| Operation | Target |
|---|---|
| Cold start (full daemon) | <100ms |
| MCP tool call (cached) | <1ms |
| MCP tool call (graph traversal) | <5ms |
| MCP tool call (semantic search) | <50ms |
| Live push latency | <50ms (file change → Claude notified) |
| Markdown ingest (per file) | <10ms |
| Tree-sitter parse (per file) | <20ms (avg) |
| Background full-project parse (10k files) | <10s |
| Resumption bundle generation | <100ms |
| Disk per project (10k files indexed) | <50MB |
| RAM per running daemon | <250MB |
| Vision app cold start | <500ms |
| Vision app frame budget | 16.67ms (60fps) on 100k-node graph |

---

## 15. Build Phases (5 shippable phases)

Each phase produces a working release. Datatree is usable from end of Phase 1 onwards.

### Phase 1 — Foundation (week 1)
**Ships v0.1**
- Rust supervisor binary
- Rust storage worker (3 of 8 DB files: graph.db, history.db, tasks.db)
- Bun MCP server with 5 core tools (`recall_decision`, `recall_conversation`, `step_status`, `step_resume`, `health`)
- Markdown ingest for CLAUDE.md / .claude/rules / README.md
- Plugin manifest + install script (global scope only)
- SessionStart + UserPromptSubmit hooks
- Step Ledger schema + basic CLI (`/dt-step`)

**Acceptance**: `claude plugin install datatree` works on Windows; Claude session shows `<datatree-context>` block; Step Ledger captures + resumes one numbered task.

### Phase 2 — Code Graph + Drift (week 2)
**Ships v0.2**
- Tree-sitter parsers for TS, JS, Python, Rust (4 most-used)
- CRG-mode tools: `blast_radius`, `call_graph`, `find_references`, `dependency_chain`
- Theme + types + security scanners
- Drift detector + redirect injection
- Constraint registry
- File watcher (X layer)
- 5 more MCP tools

**Acceptance**: Indexes Orion in <10s; blast_radius correct against hand-traced sample; drift findings appear within 1s of CLAUDE.md violation introduced.

### Phase 3 — Vision Layer (week 3)
**Ships v0.3**
- Tauri desktop app + web server
- 5 of 14 views (Force-Galaxy, Hierarchy Tree, Sunburst, Heatmap, Command Center)
- WebSocket live updates
- Search + filter UI
- `/dt-view` command opens it

**Acceptance**: Renders Orion full graph at 60fps; live update of edited file within 50ms.

### Phase 4 — Multimodal + Brain (week 4)
**Ships v0.4**
- Python multimodal sidecar (PDF, image OCR, Whisper)
- Embedding generation (local model)
- Leiden community detection
- Concept extraction (graphify-mode)
- `god_nodes`, `surprising_connections`, `audit_corpus` tools
- Remaining 9 view modes
- Risk dashboard

**Acceptance**: `graphify_corpus` on Orion produces report comparable to graphify v4 reference output.

### Phase 5 — Polish + Marketplace (week 5)
**Ships v1.0**
- All 30+ MCP tools
- All 5 injection modes
- All 27 storage layers
- User & project install scopes
- Marketplace listing (self-hosted)
- Voice navigation (vision)
- Time machine (vision)
- Cross-platform builds (Windows/Mac/Linux)
- Chaos engineering test suite
- Documentation site

**Acceptance**: Self-test suite green; chaos suite (random worker kills, disk-full simulation, DB corruption injection) recovers in <5s every time; one external user installs from marketplace and uses successfully.

---

## 16. Testing Strategy

### 16.1 Test pyramid

- **Unit tests** (Rust + Vitest for TS): every storage operation, every parser, every scanner
- **Integration tests**: MCP server round-trips, hook outputs, step ledger flows
- **End-to-end tests**: real Claude Code session with mocked LLM responses; verify context bundles, resumption after compaction
- **Chaos tests**: kill random workers, fill disk, corrupt DB, induce network failure, verify recovery
- **Performance tests**: budgets in §14 enforced via CI benchmarks

### 16.2 Property-based testing

- Step ledger never advances past unverified steps (QuickCheck-style)
- Single-writer invariant: no concurrent writes to same shard ever
- WAL replay always produces identical final state (deterministic)

### 16.3 Coverage target

- Rust: 85%+ (core invariants 100%)
- TypeScript: 80%+ (MCP tools 95%)

---

## 17. Open Questions / Deferred Decisions

| Question | Default for v1 | Revisit |
|---|---|---|
| Local LLM model choice (Phi-3 vs Qwen2.5 vs Llama-3) | Phi-3-mini-4k-instruct (Q4_K_M, 2.4GB) | After user feedback on Phase 4 |
| Embedding model | `bge-small-en-v1.5` (33MB, 384-dim) | Phase 4 |
| Voice nav engine | Whisper-tiny (local) + simple intent classifier | Phase 5 |
| Marketplace UI / discovery | None in v1 (CLI install only) | v1.1 |
| Multi-machine sync (datatree across laptops) | Out of scope for v1 | v2 |
| Mobile vision viewer | Out of scope | v2 |

---

## 18. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Bun MCP SDK immaturity | Medium | Medium | Maintain Python fallback option; abstract MCP layer behind an interface |
| Tree-sitter grammars incomplete for niche languages | Low | Low | Graceful degradation: file indexed structurally without function-level detail |
| Local LLM too slow on user's CPU | Medium | Medium | Optional: brain layer disabled by default; opt-in via config |
| Plugin manifest spec changes | Medium | High | Pin plugin API version; emit warnings on detected drift |
| Windows service registration requires admin | High | Medium | Fall back to user-mode autostart via Task Scheduler if admin denied |
| SQLite WAL file growth on long-running daemons | Medium | Low | Auto-VACUUM weekly; monitor in SLA dashboard |
| User mistypes constraint and datatree blocks legitimate work | Low | High | Every drift block has 1-key override (`--force`) with reason captured for review |
| Hot-reload introduces inconsistent state | Medium | Medium | Shadow mode; transactional swap; rollback on error rate |
| Scope creep during build | High | High | This document is the single source of truth; new features require explicit doc revision |

---

## 19. Success Criteria for v1.0 Release

Datatree v1.0 ships when ALL of the following are true:

1. ✅ All 30+ MCP tools implemented and tested
2. ✅ All 5 injection modes functional
3. ✅ All 27 storage layers schema-complete and exercised by integration tests
4. ✅ All 14 vision views render correctly on a 50k-node sample graph
5. ✅ Compaction-resilience demonstrated: 100-step task survives forced context compression with correct resumption
6. ✅ Chaos test suite passes 100 consecutive runs (random worker kill, disk fill, DB corruption)
7. ✅ Performance budgets in §14 met or exceeded
8. ✅ Cross-platform builds (Win/Mac/Linux) install + run identically
9. ✅ Marketplace listing accepts `/plugin install datatree`
10. ✅ One external user (not Anish) successfully uses datatree on a real project for one full work session
11. ✅ Documentation site live with: install guide, MCP tool reference, architecture overview, troubleshooting
12. ✅ Zero `TODO` / `FIXME` / `XXX` markers in source

---

## 16.5 Subagent Roster (Datatree's own agents)

Datatree ships its own subagents for parallel work, addressing the user requirement: "you will need to make agents and sub agents."

Each subagent is a markdown file with frontmatter, registered in the plugin manifest, callable from any conversation via the Agent tool.

| Subagent | Purpose | Tools |
|---|---|---|
| `datatree-archivist` | Captures conversation/tool/decision/constraint history; writes to history.db, decisions.db, constraints.db. Runs after every Stop event. | Read, Bash (datatree CLI) |
| `datatree-drift-hunter` | Scans changed files for rule violations; writes to findings.db; emits live alerts. Runs after every PostToolUse on Edit/Write. | Read, Grep, Glob, Bash |
| `datatree-blast-tracer` | Computes blast radius for a proposed change before it's made; injects warning if affects critical paths. Runs on PreToolUse for Edit/Write. | Read, Grep |
| `datatree-concept-extractor` | Multimodal pass: extracts concepts from PDFs, images, audio/video; writes to corpus.db. Run on demand or scheduled. | Read, Bash |
| `datatree-multimodal-ingester` | Watches for new media files; dispatches to whisper/OCR/PDF extractors. Writes to multimodal.db. | Bash |
| `datatree-step-verifier` | Runs acceptance check for current step; updates Step Ledger; refuses to advance if check fails. Called by `step_complete`. | Bash, Read |
| `datatree-doctor` | Health check: runs self-test suite, validates all shards, computes SLA snapshot. Runs every 60s by health-watchdog. | Bash |
| `datatree-resumer` | Composes resumption bundle after compaction detected; injects via UserPromptSubmit hook. | Read |
| `datatree-graph-builder` | Initial + incremental Tree-sitter ingestion; writes to graph.db. Runs on file change. | Bash |
| `datatree-curator` | Periodic insight summarizer (W layer): generates weekly "what was learned" rollups. Runs on Sundays via cron-like scheduler. | Read, Bash |
| `datatree-cluster-runner` | Periodic Leiden clustering; writes communities to corpus.db. Runs daily. | Bash |
| `datatree-snapshot-keeper` | Hourly snapshot rotation + retention policy enforcement. Runs hourly. | Bash |

### 16.5.1 Subagent invocation patterns

**Sequential within a session:**
```
PreToolUse hook → datatree-blast-tracer → returns "blast affects 12 files; here are 3 critical"
   → injection bundle augmented with this warning before Edit happens
```

**Parallel for indexing:**
```
SessionStart hook → spawns in parallel:
   datatree-graph-builder (incremental Tree-sitter)
   datatree-multimodal-ingester (new media files)
   datatree-drift-hunter (re-scan changed files)
   All three write to their respective DBs concurrently (single-writer per shard, no lock contention)
```

**Background continuous:**
```
health-watchdog → datatree-doctor every 60s
snapshot-scheduler → datatree-snapshot-keeper every 60min
weekly-scheduler → datatree-curator every Sunday 03:00 local
```

### 16.5.2 Subagent definition format

```markdown
---
name: datatree-drift-hunter
description: Scans changed files for rule violations against active constraints. Writes findings to findings.db and emits live alerts. Use proactively after any Edit/Write tool call.
tools: Read, Grep, Glob, Bash
model: haiku  # cheap; runs many times per session
---

# Datatree Drift Hunter

You are a focused drift detection agent...

## Procedure
1. Read current constraints from `~/.datatree/projects/<hash>/findings.db`
2. For each changed file, scan for rule violations using Grep patterns
3. Write findings via `datatree inject --layer findings ...`
4. Emit live alerts via `datatree livebus emit drift_finding ...`
5. Return JSON summary

## Output format
{ "findings_count": N, "critical": M, "files_scanned": [...] }
```

All subagent definition files live in `plugin/agents/` and ship with the plugin.

---

## 22. Local-Only Constraint (NO INTERNET)

This section is **load-bearing for the entire design** and addresses: "it has to be local not internet."

### 22.1 What is forbidden

Datatree must NEVER, in any code path, make outbound network calls during normal operation. This includes:

- ❌ No remote LLM calls (no Anthropic API, no OpenAI, no Google, no remote inference)
- ❌ No remote embeddings (no OpenAI embeddings, no Cohere)
- ❌ No remote vector DBs (no Pinecone, no Weaviate cloud)
- ❌ No live Context7 / WebFetch (cached results only; cache populated by user, not background fetches)
- ❌ No telemetry "phone home" (no usage analytics, no error reports leaving the machine, no auto-update checks)
- ❌ No remote MCP servers
- ❌ No cloud sync (no Dropbox/iCloud/Google Drive integration)
- ❌ No remote logging (Sentry, Datadog, etc.)

### 22.2 What replaces each

| Capability | Local replacement |
|---|---|
| LLM (concept extraction, summaries) | `llama.cpp` + Phi-3-mini-4k Q4_K_M (2.4GB on disk; runs on CPU at ~30 tok/s) |
| Embeddings | `bge-small-en-v1.5` ONNX (33MB; runs on CPU via `ort` Rust crate) |
| Vector DB | SQLite VSS extension OR `usearch` Rust crate (both fully local, mmap'd) |
| Speech-to-text | `faster-whisper` (Python sidecar) OR `whisper.cpp` (C, no Python needed) |
| OCR | `tesseract-rs` (Rust binding to Tesseract) |
| Image element detection | `onnxruntime` + a small CLIP model |
| External docs | `cache/docs/` populated only when user explicitly fetches; offline thereafter |
| Updates | User runs `datatree update` manually; checks a local file or user-provided URL only on explicit command |
| Crash reports | Written to `~/.datatree/crashes/` for user inspection; never uploaded |

### 22.3 Network egress test

Phase 5 must pass:
```
$ datatree daemon start
$ tcpdump -i any port 80 or port 443 -w datatree.pcap &
$ # ... full datatree workflow exercised: ingest, query, audit, view, graphify ...
$ kill %1
$ tcpdump -r datatree.pcap | wc -l
0
```

### 22.4 Allowed exceptions (with explicit user opt-in only)

These are off by default; require config flag to enable:

- `update.check_url` — if user sets a URL, datatree polls it once per launch for new versions
- `marketplace.url` — for the install command only; never polled in steady state
- `docs.fetch_on_demand` — when user runs `/dt-docs fetch <library>`, datatree calls Context7 / WebFetch ONCE for that library, caches forever

All three are off by default; setting any of them logs a one-time warning: "datatree will make outbound network calls because <feature> is enabled."

### 22.5 Risk register update

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Local LLM too slow on user's CPU | Medium | Medium | Layer can be disabled; deterministic extraction always available |
| Local model size (2-3GB) inflates install | High | Low | Optional download prompted at install; deferred until first use |
| No telemetry → can't see field bugs | Medium | Low | User-facing health dashboard surfaces issues; bug reports manual |

---

## 23. Cargo Workspace Layout

This section addresses: "you might need cargo and cargo lock too."

### 23.1 Workspace root `Cargo.toml`

```toml
[workspace]
resolver = "2"
members = [
    "common",
    "supervisor",
    "store",
    "parsers",
    "scanners",
    "brain",
    "livebus",
    "multimodal-bridge",
    "cli",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.78"
license = "MIT"
authors = ["Anish Trivedi"]
repository = "https://github.com/anishtrivedi/datatree"

[workspace.dependencies]
# Async runtime
tokio = { version = "1.40", features = ["full"] }
tokio-util = "0.7"
futures = "0.3"
async-trait = "0.1"

# Storage
rusqlite = { version = "0.32", features = ["bundled", "blob", "functions"] }
r2d2 = "0.8"
r2d2_sqlite = "0.25"

# Tree-sitter (per-language grammars added in parsers crate)
tree-sitter = "0.23"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Errors
thiserror = "1.0"
anyhow = "1.0"

# Logging / tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Hashing
sha2 = "0.10"
blake3 = "1.5"

# Time
chrono = { version = "0.4", features = ["serde"] }

# IPC
interprocess = "2.2"  # Unix sockets / Windows named pipes

# Process supervision
sysinfo = "0.32"

# Live bus
axum = "0.7"
tokio-stream = "0.1"

# Filesystem watching
notify = "6.1"

# Embeddings (ONNX)
ort = { version = "2.0.0-rc.4", default-features = false, features = ["ndarray"] }
tokenizers = { version = "0.20", default-features = false, features = ["onig"] }

# Tesseract / Whisper bindings (multimodal worker is mostly Python sidecar; minimal Rust)
# Plus optional whisper.cpp via whisper-rs for full Rust path

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = true
panic = "abort"

[profile.dev]
opt-level = 1
debug = true
```

### 23.2 Per-crate purpose

| Crate | Purpose |
|---|---|
| `common` | Shared types: `ProjectId`, `ShardHandle`, `RowId`, `DbError`, `DbLayer`, `Timestamp` |
| `supervisor` | Binary: process supervisor, child management, restart logic, Windows service wrapper |
| `store` | Library + binary: Builder/Finder/Path/Query/Response/Inject/Lifecycle (the 7 DB Ops sub-layers) |
| `parsers` | Binary: Tree-sitter worker pool; one binary, dispatches to N OS processes |
| `scanners` | Binary: scanner pool (theme, security, perf, a11y, etc.) |
| `brain` | Binary: embeddings + concept extraction + Leiden clustering |
| `livebus` | Binary: SSE/WS server on local socket; multi-agent pubsub |
| `multimodal-bridge` | Binary: minimal Rust shim that spawns the Python multimodal sidecar and proxies messages |
| `cli` | Binary: `datatree` CLI itself (subcommands: install, build, view, audit, recall, etc.) |

### 23.3 Cargo.lock

Generated on first `cargo build` and committed to the repo. Pinned versions ensure reproducible builds across team / CI / users' machines.

---

## 21. Reference Mining Synthesis (CRG, graphify, tree-sitter)

This section captures findings from background research agents. Datatree must match or exceed every capability listed.

### 21.1 CRG capability matrix (must match or beat)

**MCP tools to provide datatree equivalents for** (CRG has 24; datatree targets 30+):

| CRG tool | Datatree equivalent | Notes |
|---|---|---|
| `build_or_update_graph_tool` | `step_status` + automatic via file watcher | Continuous, not on-demand |
| `get_impact_radius_tool` | `blast_radius` | Same; configurable depth, 500-node cap |
| `query_graph_tool` | `call_graph`, `find_references`, `dependency_chain` | Split into focused tools |
| `get_review_context_tool` | `recall_file` with `detail_level` | Token-efficient |
| `get_minimal_context_tool` | `recall_file` with `detail_level=minimal` | ~100 token output |
| `list_graph_stats_tool` | `health` | Includes graph stats |
| `semantic_search_nodes_tool` | `recall_concept` | Vector + keyword hybrid |
| `embed_graph_tool` | Automatic via brain-worker | No explicit call needed |
| `find_large_functions_tool` | `audit(scope=complexity)` | Part of audit umbrella |
| `list_flows_tool` | `god_nodes` + flow analysis | Cross-references multimodal corpus |
| `get_flow_tool` | `recall_flow` (new tool) | |
| `get_affected_flows_tool` | Part of `blast_radius` extended output | |
| `list_communities_tool` | Part of `god_nodes` | Leiden-clustered |
| `get_community_tool` | New: `recall_community` | |
| `get_architecture_overview_tool` | Vision layer + `audit(scope=architecture)` | |
| `detect_changes_tool` | Combination: `blast_radius` + `audit` + git.db | |
| `refactor_tool` | New: `refactor_preview` | Includes T layer (refactor history) |
| `apply_refactor_tool` | New: `refactor_apply` | |
| `generate_wiki_tool` | `audit_corpus` (graphify-style report) | |
| `get_wiki_page_tool` | `recall_concept` | |
| `list_repos_tool` | New: `list_projects` | Reads meta.db |
| `cross_repo_search_tool` | New: `search_all_projects` | Y layer |
| `get_docs_section_tool` | `recall_docs` | Q layer |
| `run_postprocess_tool` | Automatic via worker pipeline | No explicit call |

**Hooks to support**:
- PostToolUse (auto-update) ✅ already in design
- SessionStart ✅
- Plus 4 more datatree-specific hooks: UserPromptSubmit, PreToolUse, Stop, SessionEnd

**Platforms to install on** (CRG supports 10; datatree targets all):
Claude Code, Cursor, Windsurf, Codex, Zed, Continue, OpenCode, Antigravity, Qwen Code, Qoder, plus Aider, Trae, Kiro, Hermes, Factory Droid, OpenClaw, Gemini CLI, GitHub Copilot CLI, VS Code Copilot Chat — **18 total**.

**Languages to parse** (CRG supports 23; datatree targets 25+):
Python, TypeScript/TSX, JavaScript, Vue SFC, Go, Rust, Java, Scala, C#, Ruby, Kotlin, Swift, PHP, Solidity, C/C++, Dart, R, Perl, Lua, Luau, Elixir, Objective-C, Bash/Shell, GDScript, Zig, PowerShell, Julia, Svelte SFC, plus SystemVerilog, Verilog, Jupyter (.ipynb), Databricks notebooks.

**Performance targets to beat**:
| Metric | CRG | Datatree target |
|---|---|---|
| Token reduction (review) | 6.8x avg | **10x+ avg** |
| Token reduction (live coding) | 14.1x avg | **20x+ avg** |
| First build (500 files) | ~10s | **<5s** |
| Incremental update | <2s | **<500ms** |
| Large repo first build (>10k files) | 30-60s | **<15s** |

**Killer features to match or beat**:
1. Token efficiency by default ✅ (datatree's context fusion bundles)
2. Risk-scored change analysis ✅ (audit + blast_radius combined)
3. Execution flow detection with criticality ✅ (god_nodes + flow analysis)
4. Zero-config auto-install ✅ (plugin manifest detects all platforms)
5. Multi-repo daemon ✅ (Y layer + meta.db)

**Datatree's additional killer features (beyond CRG)**:
6. Compaction-resilient Step Ledger
7. 14 visualization views vs 1
8. Multimodal corpus engine (from graphify)
9. Live push channel + bidirectional subscriptions
10. Drift detector with constraint enforcement
11. Self-shipped subagent roster
12. 100% offline operation

### 21.2 Graphify mining (in progress, will append)
*Background agent still running; findings will be folded in as v2 of this document.*

### 21.3 Tree-sitter mining (in progress, will append)
*Background agent still running.*

### 21.4 Universal AI platform mining (in progress, will append)
*Background agent still running.*

### 21.5 DB Operations Layer best practices (in progress, will append)
*Background agent still running.*

---

## 24. Security (Pragmatic Baseline)

Matches CRG's mitigation set with datatree-specific adjustments. Local-only operation (§22) eliminates most network-level attack surface; this section covers the remainder.

### 24.1 Mitigation matrix

| Vector | Mitigation |
|---|---|
| SQL Injection | All queries use parameterized `?` placeholders (rusqlite + sqlx; no string concatenation ever) |
| Path Traversal | `validate_project_root()` requires `.git`, `.claude`, or `package.json` in target; absolute paths canonicalized via `dunce::canonicalize` |
| Prompt Injection | `sanitize_name()` strips control chars, caps at 256; user content rendered as data, never as instructions |
| XSS (vision app) | `escapeHtml()` on all node labels; JSON embedded with `</script>` escaped; CSP header `default-src 'self'` on the Tauri/web shell |
| Subprocess Injection | Never `shell=True`; all process spawns use list arguments; user input never interpolated into commands |
| Supply Chain | Dependencies pinned with upper bounds (`Cargo.lock`, `bun.lockb`); `cargo deny check` in CI for advisories |
| CDN Tampering | No CDN. All vision-app assets bundled locally. Sigma.js, deck.gl, D3 vendored at build time |
| Credential Leakage | Datatree has no API keys (local-only); user-supplied config secrets read from env vars only, never logged |
| Symlink Attacks | Symlinked files explicitly skipped in scanners (CRG pattern); symlink resolution gated by config flag |
| Disk Permissions | `~/.datatree/` created with `0700` (Unix) / restricted ACL (Windows); refuses to start if perms looser |

### 24.2 CI scanning pipeline

Mirrors CRG's pragmatic approach (no SOC2 theater):

| Tool | Purpose | Gate |
|---|---|---|
| `cargo clippy --deny warnings` | Rust lint | Fail PR on warning |
| `cargo deny check` | License + vulnerability + duplicate-dep audit | Fail PR on advisory |
| `cargo audit` | RustSec advisory DB scan | Fail PR on known CVE |
| `cargo test --workspace` | Unit + integration tests | Required pass |
| `bun test` (TS workspace) | TS unit tests | Required pass |
| `biome check` | TS lint + format | Fail PR on warning |
| `tsc --noEmit` | Type-check (matches user's hard rule) | Required pass |
| `gitleaks` | Pre-commit hook, secret detection | Block commit on hit |

### 24.3 What datatree explicitly does NOT do

- ❌ No mandatory encryption-at-rest (optional via OS-level FDE — BitLocker/FileVault recommended, not enforced)
- ❌ No mandatory key rotation infra
- ❌ No SBOM generation in v1 (added in v1.1 if community asks)
- ❌ No signed releases in v1 (added when binary distribution scales)
- ❌ No enterprise audit logging beyond per-write `audit.db` rows

Rationale: datatree runs on a single user's local machine, never sees the network, processes only files the user already has access to. Excessive security ceremony costs build velocity without reducing realistic risk for the threat model.

### 24.4 Threat model boundary

Datatree assumes:
- The local user is trusted (it's their machine, their files, their context)
- The OS provides process isolation and filesystem permissions
- Other processes on the machine running as the same user can read `~/.datatree/` (acceptable; this matches every other dotfile)
- Malicious files in the indexed corpus could attempt prompt injection — mitigated by sanitization (24.1) but users should not paste hostile code unreviewed

---

## 25. Power Multipliers (where the saved security headroom goes)

These are capabilities beyond the baseline that exploit local-only + multi-process design to outperform CRG and graphify on raw analytical depth.

### 25.1 Deeper code analysis (beyond Tree-sitter AST)

Every parsed file gets THREE analysis passes, not one:

1. **Syntactic pass** (Tree-sitter) — what CRG/graphify do
2. **Semantic pass** — type inference where possible:
   - TypeScript: spawn `tsserver` per project, query type info via Language Service Protocol; cached per file hash
   - Python: bundled `jedi` + `pyright` for type inference
   - Rust: query `rust-analyzer` over IPC for full HIR awareness
   - Go: invoke `gopls` via LSP
3. **Effect pass** — classify every function: `pure | reads | writes | network | spawn | mutate_global` based on observed I/O syscalls (heuristic; static analysis fallback)

Result: blast-radius queries return not just "callers" but "callers WHO ARE PURE vs IMPURE", letting Claude reason about safe-to-cache vs side-effecting code.

### 25.2 Temporal analysis (beyond snapshot)

Every git commit indexed into `git.db`. Datatree can answer:
- "Show me the call graph as it existed 3 weeks ago"
- "Which functions were edited together in the last 90 days?" (co-change frequency = hidden coupling)
- "Which file is changed most often immediately after this one?" (sequential coupling = workflow detection)
- "Diff this function's blast-radius between v1.0 and HEAD"

### 25.3 Dynamic profile fusion (optional opt-in)

If user provides a profile JSON (e.g., from Chrome DevTools, perf, or Vitest's `--coverage`), datatree:
- Overlays runtime hot spots onto the static graph
- Reveals which "unused" code is actually called via reflection/dynamic dispatch
- Flags high-complexity + high-traffic functions as critical-path

### 25.4 Cross-modal concept fusion (beyond graphify)

Graphify extracts concepts from each modality independently. Datatree links them:
- A README mention of "the auth flow" → `auth_flow` concept node
- A code function `loginUser()` in `src/auth/login.ts` → `loginUser` code node
- A meeting recording transcript mentioning "we decided to switch from sessions to JWTs" → `auth_decision_jwt` decision node
- A screenshot of the login page → `login_ui` UI node

Datatree's brain links all four into one cluster, weighted by EXTRACTED/INFERRED confidence, so "show me everything about auth" returns code + docs + decisions + UI + meetings as one ranked result.

### 25.5 Marker-based idempotent injection (from graphify)

When datatree writes to user's `CLAUDE.md`, `AGENTS.md`, etc., it wraps its section:
```markdown
<!-- datatree-start v1.0 -->
... content datatree controls ...
<!-- datatree-end -->
```

Re-runs replace, never duplicate. User edits between markers are preserved by hash-comparison; a warning is emitted if the user has edited datatree's section, asking confirmation before overwrite.

### 25.6 SHA256-keyed extraction cache (from graphify)

Every extraction (Tree-sitter parse, semantic concept extraction, embedding) keyed by file content SHA256. Re-runs on unchanged files cost 0ms. Cache persisted across machine reboots.

### 25.7 Parallel subagent dispatch (graphify pattern, datatree-improved)

Graphify uses Claude subagents for concept extraction, dispatching 20-25 files per chunk. Datatree:
- Uses **local LLM (Phi-3) by default** so dispatch is free of API cost
- Falls back to Claude subagents only when user explicitly opts in for higher-quality extraction
- Larger chunks (50 files) since local LLM has no rate-limit
- Parallel dispatch via tokio JoinSet

### 25.8 Hyperedge support (from graphify)

Datatree's graph supports `n-ary edges` not just pairs:
- "These 4 files implement the IRepository protocol" — one hyperedge, not 4 pairwise edges
- "These 7 functions all participate in the auth flow" — one hyperedge
- Vision layer renders hyperedges as shaded regions (graphify pattern)

### 25.9 Rationale extraction (from graphify, expanded)

`rationale_for` edges captured from:
- Code comments explaining "why" (regex + LLM-pass)
- Decision records (ADRs, RFCs)
- Conversation history mentioning a decision
- Commit messages with `BREAKING:` or `WHY:` prefixes

Datatree can answer "why did we choose Rust for the supervisor?" by pulling rationale edges across all sources.

### 25.10 Tree-sitter best practices (from research)

Per tree-sitter mining (§21.3):
- One `Parser` per worker thread, NEVER shared
- All `Query` patterns compiled at startup, cached in static
- Queries always pass `old_tree` to `parse()` — incremental reuse mandatory
- Edits batched: 10 sequential keystrokes → 1 `TSInputEdit` covering the span
- Error recovery via explicit `(ERROR)` and `(MISSING)` queries → flagged but graph is still built

### 25.11 Query budget enforcement

Every MCP tool declares its expected token output budget. Datatree's response layer:
- Truncates results to budget if exceeded
- Returns `{truncated: true, full_size_estimate: N, continuation_token: "..."}` so Claude can ask for more if needed
- Default budget: 2K tokens for `recall_*` tools, 5K for `audit_*`, 500 for `step_status`

### 25.12 Continuous benchmark suite

Every release runs against a fixed corpus (Orion + httpx + FastAPI + Next.js mini) and emits:
- Token reduction ratio per tool (must beat CRG's 6.8x review / 14.1x live coding by ≥30%)
- p99 latency per tool
- Memory peak
- Disk delta

Regression in any metric blocks release.

### 25.13 Live-corrective drift mode (datatree-exclusive)

When drift detected (e.g., hardcoded color appears in a `.tsx`), datatree's drift hunter doesn't just flag — it can OPTIONALLY auto-suggest a fix in the Command Center, ready to apply with one keystroke. Off by default; opt-in via `drift.auto_suggest: true`.

### 25.14 Multi-graph union (cross-project datatree-exclusive)

Across the user's `meta.db`-registered projects, datatree can:
- Find duplicate code across projects ("this `parseDate` function exists in 4 of your repos")
- Surface common patterns ("you write Zustand stores this way in 6 projects — codify as your personal pattern")
- Propagate constraints ("you rejected `default exports` in Orion; apply to all projects?")

---

## 21.2 Graphify Capability Matrix (must absorb)

From background research agent.

### 21.2.1 Multimodal extraction (datatree adopts)

| Capability | Adopt as |
|---|---|
| 25 code language extensions via Tree-sitter | Already in §21.1 |
| Markdown / RST / PDF / DOCX / XLSX | Multimodal worker pipeline (§10.2) |
| Image OCR via Claude vision (when remote) | Local Tesseract + tiny CLIP (per local-only §22) |
| Video transcription (faster-whisper, base model) | `whisper.cpp` for full-Rust path; faster-whisper Python fallback |
| YouTube via yt-dlp | Off-by-default; opt-in via `multimodal.youtube_enabled: true` (network egress) |

### 21.2.2 Graphify killer techniques (datatree adopts)

1. **Honest confidence tagging** (EXTRACTED 1.0 / INFERRED 0.4-0.9 / AMBIGUOUS 0.1-0.3) — adopted in §10.3
2. **Deterministic AST + LLM fusion** — adopted: AST always runs; LLM optional via local Phi-3
3. **Hyperedges + semantic similarity edges** — adopted in §25.8
4. **Leiden clustering on graph topology (no embeddings required)** — adopted; embeddings ADDITIONAL not REPLACEMENT
5. **Domain-aware Whisper prompting from corpus god-nodes** — adopted in `whisper-worker`

### 21.2.3 Graphify limitations (datatree fixes)

| Graphify limit | Datatree fix |
|---|---|
| Static snapshot, no temporal | §25.2 temporal analysis |
| 5000-node HTML render limit | WebGL renderer handles 100k+ |
| No type inference | §25.1 semantic pass |
| No runtime/dynamic data | §25.3 profile fusion (opt-in) |
| External imports dropped | §25.14 cross-project linking via meta.db |
| No feedback loops | Memory layer (I) records which findings user acts on |
| No customizable extraction | Per-project config can declare custom extractors (§12.3) |

---

## 21.3 Tree-sitter Mastery (from research)

Adopted into PARSE worker design. Key wins:

- **`tree-sitter` Rust crate (v0.24+)** — chosen for performance + thread safety
- **Parser pool**: 3-8 parsers per CPU core (datatree default: `cpu_count * 4`)
- **Cached compiled queries**: all 10 core query patterns (functions, classes, imports, calls, decorators, etc.) compiled at startup, stored in `OnceCell`
- **Incremental re-parse mandatory**: every parse passes `old_tree` from cache
- **Edit batching**: file watcher debounces to 50ms, batches keystrokes into single `TSInputEdit`
- **Error recovery**: `(ERROR)` and `(MISSING)` queried separately; build graph anyway with confidence tags
- **Performance targets** (10k-line file):
  - First parse: <50ms
  - Incremental re-parse: <1ms
  - Query (find all functions): <5ms
  - Tree copy (COW): <0.1ms

### 21.3.1 Languages locked for v1.0

25 languages from CRG + graphify intersection:
TypeScript, TSX, JavaScript, JSX, Vue SFC, Svelte SFC, Python, Rust, Go, Java, Scala, C, C++, C#, Ruby, Kotlin, Swift, PHP, Solidity, Dart, R, Perl, Lua, Luau, Elixir, Objective-C, Bash, GDScript, Zig, PowerShell, Julia, SystemVerilog, Verilog, Markdown, JSON, TOML, YAML, Jupyter (.ipynb), Databricks notebooks.

That's 38 effectively (some are sub-grammars). All Tier 1 (≥4-star quality per matrix in research output).

### 21.3.2 Anti-patterns datatree avoids

- ❌ Reuse stale `TSNode` after edit → always fetch fresh from tree
- ❌ Share `Parser` across threads → one per worker
- ❌ Compile query in hot loop → compile once, store static
- ❌ Parse without `old_tree` → always pass cached tree
- ❌ Ignore error nodes → query and capture them with `confidence: AMBIGUOUS`

---

## 21.4 Universal AI Platform Integration Matrix (from research)

Datatree v1.0 supports 18 platforms. Per-platform manifest + MCP config + hook strategy:

| # | Platform | Manifest | MCP path | Hook style |
|---|---|---|---|---|
| 1 | Claude Code | `CLAUDE.md` + `.claude/settings.json` | `.mcp.json` or `~/.claude.json` | Full hooks (PreToolUse, PostToolUse, SessionStart, UserPromptSubmit, Stop, SubagentStop, PreCompact) |
| 2 | Codex | `AGENTS.md` | `~/.codex/config.toml` | Subagent dispatch via `multi_agent=true` |
| 3 | Cursor | `.cursor/rules/*.mdc` + `AGENTS.md` | `.cursor/mcp.json` | `~/.cursor/hooks.json` (afterFileEdit, sessionStart, beforeShellExecution) |
| 4 | Windsurf | `.windsurfrules` + global rules | `~/.codeium/windsurf/mcp_config.json` | Workflows in `.windsurf/workflows/*.md` |
| 5 | Zed | `AGENTS.md` | `~/.config/zed/settings.json` `context_servers` | Zed extension API |
| 6 | Continue | `.continuerc.json` | `~/.continue/config.json` `mcpServers` (array) | `.continue/hooks/` limited |
| 7 | OpenCode | `AGENTS.md` | `.opencode.json` `mcpServers` | `.opencode/plugins/*.ts` (file.edited, session.created, tool.execute.before/after) |
| 8 | Antigravity | `AGENTS.md` + `GEMINI.md` | `~/.gemini/antigravity/mcp_config.json` | Built-in agent runtime |
| 9 | Gemini CLI | `GEMINI.md` | `~/.gemini/settings.json` | Custom commands TOML in `~/.gemini/commands/` |
| 10 | Aider | `.aider.conf.yml` + `CONVENTIONS.md` | `.aider.conf.yml mcp_servers:` | Git hooks only |
| 11 | GitHub Copilot CLI / VS Code | `.github/copilot-instructions.md` | `.vscode/mcp.json` or `~/.config/github-copilot/mcp.json` | VS Code task hooks |
| 12 | Factory Droid | `AGENTS.md` | `~/.factory/mcp.json` | `Task` tool subagents |
| 13 | Trae / Trae-CN | `AGENTS.md` | `~/.trae/mcp.json` | No PreToolUse — AGENTS.md is always-on |
| 14 | Kiro | `.kiro/steering/*.md` | `.kiro/settings/mcp.json` | `.kiro/hooks/*.kiro.hook` |
| 15 | Qoder | `QODER.md` | `.qoder/mcp.json` | `.qoder/settings.json` hooks |
| 16 | OpenClaw | `CLAUDE.md` / `AGENTS.md` | `.mcp.json` | None (sequential) |
| 17 | Hermes | `AGENTS.md` | `.mcp.json` or `~/.hermes/mcp.json` | Claude-compatible |
| 18 | Qwen Code | `QWEN.md` | `~/.qwen/settings.json` | None |

### 21.4.1 Manifest writing rules (from §21.4 research)

1. **AGENTS.md is the universal base** — all 17+ platforms read it
2. **Marker-based idempotent injection** — wrap with `<!-- datatree-start v1.0 -->` / `<!-- datatree-end -->`
3. **MCP config family-aware**:
   - JSON object (most platforms): `{"mcpServers": {"datatree": {...}}}`
   - JSON array (Continue): `{"mcpServers": [{"name": "datatree", ...}]}`
   - TOML (Codex): `[mcp_servers.datatree]`
4. **Hook capability map** — datatree gracefully degrades on platforms without hooks (uses AGENTS.md only)
5. **Backup before merge** — every config write makes `<file>.bak` first
6. **Dry-run mode** — `datatree install --dry-run` shows the diff without applying
7. **POSIX + PowerShell variants** of all installer scripts
8. **Honor `core.hooksPath`** when installing git hooks (Husky compatibility)

### 21.4.2 Auto-detection logic

```rust
fn detect_installed_platforms() -> Vec<Platform> {
    let mut found = vec![];
    if Path::new("~/.codex/").exists() { found.push(Platform::Codex); }
    if Path::new("~/.cursor/").exists() { found.push(Platform::Cursor); }
    if Path::new("~/.codeium/windsurf/").exists() { found.push(Platform::Windsurf); }
    // ... per matrix above
    if always_true() { found.push(Platform::ClaudeCode); }  // ClaudeCode + Qoder always tried
    found
}
```

`datatree install` (no args) auto-detects all installed platforms and configures each with appropriate manifest + MCP entry + hooks.

---

## 21.5 DB Operations Layer Best Practices

*Background agent still running. Findings will be folded in as v2.*

---

## 21.6 OUT-COMPETE Matrix — every CRG feature, datatree's stronger counterpart

This is the hard scorecard. Datatree v1.0 must beat CRG on every row. No exceptions.

### 21.6.1 Token reduction targets (must beat 8.2x average)

CRG benchmark (from user-supplied data):

| Repo | CRG reduction |
|---|---|
| express | 0.7x |
| fastapi | 8.1x |
| flask | 9.1x |
| gin | 16.4x |
| httpx | 6.9x |
| nextjs | 8.0x |
| **CRG average** | **8.2x** |

**Datatree targets** (must hit ALL):

| Repo | Datatree target | Multiplier vs CRG |
|---|---|---|
| express | ≥3x | 4.3x improvement |
| fastapi | ≥20x | 2.5x improvement |
| flask | ≥25x | 2.7x improvement |
| gin | ≥40x | 2.4x improvement |
| httpx | ≥18x | 2.6x improvement |
| nextjs | ≥22x | 2.7x improvement |
| **Datatree average** | **≥25x** | **3.0x improvement** |

**How datatree achieves 3x improvement over CRG:**

1. **Same blast-radius compression** as CRG (matches baseline)
2. **+ Context fusion bundles** (§4.2) — recalls decisions/constraints from history.db instead of Claude re-deriving them: ~30% additional token saving
3. **+ Tool-call cache** (E layer) — identical Bash/Grep/Read returns cached result in <1ms with 0 re-derivation tokens
4. **+ Markdown-as-context layer** (§8) — CLAUDE.md drinks once, summarized into bundle; not re-read each turn
5. **+ Compaction-resilient resumption** (§7.3) — no re-reading docs after compaction; replay 5K ledger tokens vs 500K of source files
6. **+ Pre-tool enrichment** (§4.3) — `Read` calls short-circuited to cached summaries when file unchanged
7. **+ Concept-cluster routing** (§25.4) — single semantic query returns cross-modal slice instead of multiple raw scans

### 21.6.2 Feature-by-feature beat matrix

| # | CRG feature | CRG capability | Datatree counterpart | Why MORE powerful |
|---|---|---|---|---|
| 1 | Incremental updates | <2s after change | <500ms after change | 4x faster: file-watcher debounce + parser pool with cached queries (§25.10) |
| 2 | 23 languages + notebooks | 23 langs, .ipynb | 38 effective grammars (§21.3.1) including Markdown / JSON / TOML / YAML / SystemVerilog / Verilog | +15 grammars + notebook support |
| 3 | Blast-radius analysis | Function/class/file affected | + WHO IS PURE vs IMPURE per call site (§25.1) + temporal blast (was-affected 3-weeks-ago) (§25.2) | Effect-aware blast radius |
| 4 | Auto-update hooks | PostToolUse + git pre-commit | 6 hook types: SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Stop, SessionEnd, plus PreCompact (§4) | 7 hook surfaces vs CRG's 2 |
| 5 | Semantic search | Optional vector embeddings (cloud-capable) | Local-only embeddings (§22) via ONNX bge-small + SQLite VSS, always-on, never-network | Always-on, fully offline |
| 6 | Interactive visualisation | D3 force graph (1 view, ~5K node limit) | 14 view modes (§9.2), WebGL @ 60fps, 100K+ nodes | 14x more views, 20x scale |
| 7 | Hub & bridge detection | Betweenness centrality | + cross-community + cross-language + peripheral-to-hub + temporal hub (was-hub-3-weeks-ago) | Multi-dimensional centrality |
| 8 | Surprise scoring | Cross-community / cross-language coupling | + cross-modal (code↔docs↔meetings↔decisions) (§25.4) | Adds 3 modal dimensions |
| 9 | Knowledge gap analysis | Isolated, untested, thin | + bus-factor (no human touched in 6mo, still has callers), + drift-rotted (CLAUDE.md says exists, code doesn't) | Adds 2 risk dimensions |
| 10 | Suggested questions | Auto-generated from graph | + voice navigation ("what's broken now?"), + LLM-routed query suggestions per session context | Voice + context-aware |
| 11 | Edge confidence | EXTRACTED / INFERRED / AMBIGUOUS | Same scheme + per-source provenance (which extractor, when, against which rule version) | Provenance audit trail |
| 12 | Graph traversal | BFS/DFS, configurable budget | Same + temporal traversal (graph as it existed at commit X) + cross-project traversal (Y layer) | + 2 traversal modes |
| 13 | Export formats | GraphML, Cypher, Obsidian, SVG | + JSON, Mermaid, DOT, PNG, PDF, standalone HTML, native Tauri view, embeddable web view | 4 → 8 formats |
| 14 | Graph diff | Snapshot comparison | + animated time-lapse (vision §9.5), + future-state projection (LLM what-if) | Visual + predictive |
| 15 | Token benchmarking | Naive vs graph ratios per question | Continuous benchmark suite (§25.12) on every release; regression blocks release | Enforced via CI |
| 16 | Memory loop | Q&A as markdown re-ingestion | Full conversation/decision/error/constraint history (C/D/J layers) auto-captured; survives compaction (§7) | Auto + compaction-resilient |
| 17 | Community auto-split | Recursive Leiden for >25% communities | Same + manual override + per-community embedding refinement | Same + tunable |
| 18 | Execution flows | Entry-point criticality | + cross-modal flows (decision → code → test → docs → UI screenshot all in one flow) (§25.4) | Cross-modal flows |
| 19 | Community detection | Leiden + resolution scaling | Same algorithm + topology + optional embedding-augmented edges (§25.4) | + embedding signal |
| 20 | Architecture overview | Coupling warnings | + 14 visual perspectives (§9.2) + Command Center + drift overlay | Multi-view + drift-aware |
| 21 | Risk-scored reviews | `detect_changes` maps diffs | Same + Step Ledger (§7) verifies acceptance per step + drift detector pre-empts violations | Verified + pre-emptive |
| 22 | Refactoring tools | Rename preview + dead-code | + refactor history (T layer) with one-keystroke undo + framework-aware suggestions per project | Undoable + framework-aware |
| 23 | Wiki generation | Markdown from communities | Same + auto-generated weekly insight rollups (W layer) + cross-modal corpus report (graphify-style) | + temporal + multimodal |
| 24 | Multi-repo registry | Register & cross-repo search | Same + multi-graph union (§25.14): "find this fn across all your projects", "propagate constraint to all projects" | Cross-project propagation |
| 25 | Multi-repo daemon | Child processes + health checks | Single supervisor (§3.1), single binary, fault domains, watchdog self-test, auto-restart in <100ms | Architecturally stronger |
| 26 | MCP prompts | 5 workflow templates (review, architecture, debug, onboard, pre-merge) | + 12 datatree-specific prompts: drift-fix, blast-preview, decision-archeology, theme-audit, security-sweep, refactor-safety, test-gap-fill, perf-baseline, dep-upgrade-safety, architecture-review, code-onboarding, debug-history-walk | 5 → 17 prompts |
| 27 | Full-text search | FTS5 hybrid (keyword + vector) | Same + cross-shard FTS (Y layer) + decision-history FTS (D layer) + conversation FTS (C layer) | Multi-source FTS |
| 28 | Local storage | SQLite in .code-review-graph/ | 20 sharded SQLite files (§2.1) per project + WAL + hourly snapshots + per-project isolation | 20x more granular, fault-isolated |
| 29 | Watch mode | Continuous updates | Same + live push channel (§11) + multi-agent pubsub + drift alerts | + bidirectional + alerts |

**Score**: 29 out of 29 features beaten or matched-with-extension. No CRG feature is uncountered.

### 21.6.3 New datatree-exclusive features (no CRG counterpart)

These exist in datatree, do not exist in CRG:

1. **Compaction-resilient Step Ledger** (§7) — the killer feature
2. **Context fusion bundles** (§4.2) — composes Claude's context per turn
3. **Markdown-drinking** (§8) — drinks every .md like CLAUDE.md
4. **Multimodal corpus** (graphify-style, §10) — PDF/image/audio/video
5. **Subagent roster** (§16.5) — 12 datatree-shipped subagents
6. **Native Command Center UI** (§9.7) — goal stack, drift indicator, search
7. **Time machine** (§9.5) — git scrub + future projection
8. **Voice navigation** (§9.6 mention)
9. **Cross-modal concept fusion** (§25.4) — code+docs+meetings+UI in one cluster
10. **Multi-graph union** (§25.14) — cross-project propagation
11. **Temporal analysis** (§25.2) — co-change frequency, sequential coupling
12. **Effect-aware blast** (§25.1) — pure vs impure callers
13. **Drift detector + constraint enforcement** (§4.3, §7.5) — pre-empts violations
14. **Three-pass code analysis** (§25.1) — Tree-sitter + LSP + effect classification
15. **Hyperedges** (§25.8, from graphify, not in CRG) — n-ary relationships
16. **Live bus** (§11) — bidirectional SSE/WS to all subscribers
17. **Marketplace plugin** (§12) — Claude Code plugin distribution
18. **18-platform install** (§21.4) — vs CRG's 10
19. **Continuous benchmark gate** (§25.12) — release-blocking regression checks

### 21.6.4 Performance OUT-COMPETE matrix

| Operation | CRG | Datatree target | Improvement |
|---|---|---|---|
| First build (500 files) | ~10s | <3s | 3.3x faster |
| Incremental update | <2s | <500ms | 4x faster |
| Large repo first build (10k files) | 30-60s | <12s | 2.5-5x faster |
| Token reduction (review) avg | 8.2x | ≥25x | 3.0x better |
| Visualization node ceiling | ~5,000 nodes | 100,000+ nodes | 20x scale |
| Cold start (daemon) | n/a (CRG has no daemon model) | <100ms | New capability |
| MCP query (cached) | unknown | <1ms | New SLA |
| Live update latency (file → notification) | n/a | <50ms | New capability |
| Recovery time after corruption | manual rebuild | <5s automatic | New capability |
| Compaction recovery (Claude resumes correct step) | n/a (CRG can't) | <1 user prompt | New capability |

### 21.6.5 Acceptance criterion for v1.0

A run of `datatree benchmark --against crg --corpus orion+httpx+fastapi+nextjs+flask+gin+express` must produce a report showing datatree wins on ≥27 of 29 features and matches the token-reduction targets in §21.6.1. CI gates the release on this report.

---

## 20. Sign-Off

This design is approved by:

- [ ] Anish Trivedi (product owner) — signature: ____________
- [x] Claude (architect) — captured 2026-04-23

Once signed: invoke `superpowers:writing-plans` skill to produce `docs/plans/2026-04-23-datatree-implementation.md` (numbered Step Ledger entries, mirrored to `tasks.db` once Phase 1 ships).

---

**END OF DESIGN DOCUMENT**
