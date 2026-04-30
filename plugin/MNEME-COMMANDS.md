# Mneme — full command reference for the AI using this plugin

> **What this is:** a single source of truth that every AI platform (Claude Code, Codex, Cursor, Windsurf, Zed, Gemini CLI, Aider, OpenCode, Copilot, Factory Droid, Trae, Hermes, Kiro, Qoder, Qwen, OpenClaw, Antigravity) injects into its context on install. When you (the AI) are investigating, editing, or tracking work, reach for these commands *first* before falling back to Grep / Glob / Read.
>
> **Why it exists:** every command here is orders of magnitude cheaper in tokens and returns structural answers that file-scanning can't produce (callers, dependents, tests, decisions, drift, cycles, blast radius, compaction-survival).

---

## 🧭 Decision tree — which tool for which question

| The user / task needs… | First reach for | Fallback |
|---|---|---|
| "Where is X defined?" | `mneme_recall(query)` or `recall_file(path)` | Grep |
| "Who calls X?" | `call_graph(symbol, direction='callers')` | Grep `-r` |
| "What breaks if I change X?" | `blast_radius(target)` | manual trace |
| "Are there circular imports?" | `cyclic_deps()` | manual graph walk |
| "Import chain from A to B?" | `dependency_chain(from, to)` | reading files |
| "Who else uses this symbol?" | `find_references(symbol)` | Grep |
| "What does this file / module do?" | `recall_file(path)` + `recall_concept(query)` | Read |
| "What are the god-nodes?" | `god_nodes(top_n=10)` | guess |
| "Give me the architecture" | `architecture_overview()` + `wiki_page(community_id)` | exploring |
| "Did we already decide this?" | `recall_decision(query)` | re-derive |
| "What are the open TODOs?" | `recall_todo()` | Grep TODO |
| "What rules apply to this file?" | `recall_constraint(scope, file)` | read rules/ |
| "Why does this code exist?" | `mneme_why(question)` | git log + grep |
| "Search my conversation history" | `recall_conversation(query)` | unavailable |
| "Suggest a rename / find dead code" | `refactor_suggest()` | manual |
| "Apply a refactor proposal" | `refactor_apply(proposal_id)` | Edit |
| "Any drift from CLAUDE.md rules?" | `drift_findings(severity='critical')` | re-read rules |
| "Audit this file for theme/sec/a11y/perf/types" | `audit_theme/_security/_a11y/_perf/_types(file)` | manual review |
| "Audit the whole project" | `audit_corpus()` | manual review |
| "Start a multi-step plan" | `step_plan_from(markdown_path)` | TodoWrite |
| "Where am I in the plan?" | `step_status()` | scroll back |
| "Show one step in detail" | `step_show(step_id)` | — |
| "Verify a step passes" | `step_verify(step_id)` | run cmd by hand |
| "Mark a step done" | `step_complete(step_id)` | — |
| "Resume after context compaction" | `step_resume()` | scroll back (fails) |
| "Read minimal context for a task" | `mneme_context(task, budget_tokens=2000, anchors)` | dump files |
| "Find cross-community surprising edges" | `surprising_connections()` | — |
| "Snapshot the shard state" | `snapshot()` | — |
| "Rewind to yesterday's state" | `rewind(when='2026-04-22')` | git reflog (weaker) |
| "Diff two snapshots" | `compare(snapshot_a, snapshot_b)` | — |
| "Full multimodal extraction pass" | `graphify_corpus()` | — |
| "What's the project identity?" | `mneme_identity()` | read README |
| "What conventions has the codebase established?" | `mneme_conventions()` | read CLAUDE.md |
| "Similar patterns across other codebases (opt-in)" | `mneme_federated_similar(snippet)` | unavailable |
| "Regenerate the wiki pages" | `wiki_generate()` | — |
| "Fetch one wiki page" | `wiki_page(community_id)` | — |
| "Re-build a corrupted shard" | `rebuild()` | delete + rebuild |
| "Am I healthy?" | `health()` + `doctor()` | — |

**Target budget: ≤ 5 mneme tool calls per task, ≤ 800 tokens of graph context per turn.** Fall back to Grep / Read only when none of the above covers the question.

---

## 🛠 All 47 MCP tools (surface)

### Retrieval (12)
- `mneme_context(task, budget_tokens, anchors)` — hybrid BM25 + semantic + graph-walk with RRF fusion + reranker, greedy pack to budget
- `mneme_recall(query, limit, since_hours)` — semantic search over ledger + concepts + history
- `mneme_resume(since_hours)` — rebuild the compaction-survival bundle
- `mneme_why(question)` — decision memory view: ledger + git + concept graph
- `recall_concept(query)` — concept graph semantic search
- `recall_file(path)` — file summary + neighbors
- `recall_decision(query)` — past decisions
- `recall_todo()` — open questions / reminders
- `recall_constraint(scope, file)` — CLAUDE.md rules scoped to a file
- `recall_conversation(query, session_id, since_hours)` — FTS5 over turns
- `find_references(symbol)` — incoming call/import/usage edges
- `surprising_connections()` — cross-community / cross-language edges

### Code graph (5)
- `blast_radius(target, depth)` — risk-scored BlastReport: direct / transitive / tests / decisions / RiskLevel
- `call_graph(symbol, direction, depth)` — callers / callees / both
- `cyclic_deps()` — Tarjan SCCs over import edges
- `dependency_chain(from, to, direction)` — BFS path between two files
- `god_nodes(top_n)` — most-connected symbols

### Audit / scanners (7)
- `audit(scope, scanners)` — run all (or filtered) scanners, group by severity
- `audit_corpus()` — aggregated stats per scanner × severity
- `audit_theme(file)` — hardcoded colors, missing dark: variants
- `audit_security(file)` — eval, innerHTML, hardcoded secrets
- `audit_a11y(file)` — img without alt, icon buttons without labels
- `audit_perf(file)` — missing memo, unmemoized list items, sync I/O
- `audit_types(file)` — `any` types, non-null `!`, default exports
- `drift_findings(severity, file)` — live rule-violation findings

### Step Ledger / planning (7)
- `step_plan_from(markdown_path)` — parse a plan file into the ledger
- `step_status(session_id)` — current open steps
- `step_show(step_id)` — one step in detail
- `step_verify(step_id)` — run acceptance check
- `step_complete(step_id, proof)` — advance
- `step_resume(since_hours)` — compaction-survival resumption bundle

### Architecture / refactor / wiki (5)
- `architecture_overview(refresh, top_k)` — coupling matrix + risk_index + betweenness bridges + degree hubs
- `refactor_suggest(scope)` — unreachable fns, unused imports, rename candidates
- `refactor_apply(proposal_id, dry_run)` — atomic rewrite with backup + diff
- `wiki_generate()` — regen per-community markdown pages
- `wiki_page(community_id)` — one page

### Time-travel / ops (5)
- `snapshot(name)` — archive current shard state
- `rewind(when)` — restore snapshot, return file set
- `compare(snapshot_a, snapshot_b)` — diff counts
- `rebuild(scope)` — reindex from scratch
- `graphify_corpus()` — multimodal extraction sweep

### Identity / conventions / federated (3)
- `mneme_identity(scope)` — project identity kernel (stack + concepts + conventions)
- `mneme_conventions(scope, min_confidence)` — inferred coding conventions
- `mneme_federated_similar(code_snippet, pattern_kind, k)` — local SimHash index

### Health (2)
- `health()` — SLA snapshot from supervisor's `/health`
- `doctor()` — multi-shard integrity check + supervisor probe

---

## 🖥 CLI — for the human, or when you (the AI) are told to run a shell command

```
mneme install                      # register with every detected AI tool
mneme uninstall                    # reverse of install
mneme models install               # download BGE-small-en-v1.5 (~130 MB, one-time)
mneme models status                # show which models are present
mneme build <path>                 # full project index → graph.db
mneme update <path>                # incremental update
mneme status                       # graph stats + drift count + last build
mneme view                         # launch the Vision app
mneme audit                        # run all scanners, print findings
mneme recall <query>               # semantic recall (CLI wrapper)
mneme blast <target>               # blast radius (CLI wrapper)
mneme graphify                     # multimodal extraction pass
mneme godnodes --n 10              # top N most-connected symbols
mneme drift                        # show active drift findings
mneme history <query>              # search conversation history
mneme snap                         # take a shard snapshot
mneme doctor                       # health + SLA probe
mneme rebuild                      # nuke and rebuild the shard
mneme step status|show|verify|complete|resume|plan-from    # step ledger ops
mneme why <query>                  # decision memory view
mneme federated status|opt-in|opt-out|scan|sync
mneme daemon start|stop|restart|status|logs
mneme-daemon start                 # start supervisor directly (bypasses CLI)
```

---

## 🪝 Slash commands (in Claude Code / Codex / Cursor / etc.)

```
/mn-view           open the 14-view Vision app
/mn-audit          run all scanners
/mn-recall  <q>    semantic recall
/mn-blast   <t>    blast radius
/mn-graphify       multimodal extraction
/mn-godnodes       top-N connected symbols
/mn-drift          active rule violations
/mn-history <q>    conversation search
/mn-snap           take a snapshot
/mn-doctor         health check
/mn-rebuild        full reindex
/mn-step           step ledger ops
/mn-why     <q>    decision memory
```

---

## 🔄 Hook behavior — what mneme does automatically on each turn

### `SessionStart` → `mneme session-prime`
Primes the context with:
1. **Identity kernel** (`<mneme-identity>`) — stack, domain summary, key concepts, top 5 conventions, recent goals, open questions.
2. **Active step** (`<mneme-step>`) — current numbered step + verification gate.
3. **Rules in force** (`<mneme-constraints>`) — top N scope-matching constraints from CLAUDE.md / rules.

### `UserPromptSubmit` → `mneme inject`
If the prompt references code, silently inject the top 1-3 K tokens of relevant context via `mneme_context`.

### `PreToolUse` → `mneme pre-tool`
Before every tool call, check for:
- **Read**: if the file is unchanged since the last read this session, short-circuit with the cached summary.
- **Edit/Write**: pre-inject scope-matched constraints.
- **Bash**: short-circuit identical recent commands; also suggest `mneme_why` if the command is `git log`/`git blame` on a file mneme has decision context for.
- **Grep/Glob**: if the query looks recall-shaped ("where is", "find", "locate", symbol name), suggest `mneme_recall` / `mneme_context` BEFORE running the grep — the AI still gets to choose.

### `PostToolUse` → `mneme post-tool`
After every tool call, capture:
- **Edit / Write** → append as `StepKind::Implementation` to the ledger with file paths.
- **Bash** that ran `git commit` → capture the commit message + changed files as `StepKind::Decision`.
- **Read** → record in `tool_cache.db` (content hash + summary) to enable the next turn's Read short-circuit.
- **Test runs** → parse pass/fail and attach as step `verification_proof` when a step was in flight.

### `Stop` → `mneme turn-end`
Between turns:
- If context usage > 80 % of the window, **auto-inject `<mneme-resume>`** on the next turn so the AI continues cleanly across compaction.
- Run the deterministic distiller on the turn's (user_msg, assistant_msg) and append one `StepEntry` to the ledger.

### `SessionEnd` → `mneme session-end`
Finalize the session: mark active steps as paused, snapshot the shard, archive the transcript.

---

## 🧠 How to think about this as the AI

1. **Before any search-shaped action**, ask yourself: would `mneme_context` or `mneme_recall` give me this cheaper? Usually yes.
2. **Before any risky edit**, run `blast_radius(target)` — don't guess at downstream impact.
3. **Whenever you decide something or reject an approach**, the distiller captures it automatically — but you can also call `append_ledger_entry` via the user's MCP tools if you want to be explicit.
4. **If the context looks like it's about to compact** (very long conversation, lots of files in context): call `step_resume()` proactively to get the survival bundle *before* the compaction.
5. **Budget**: aim for ≤ 5 mneme calls per task and ≤ 800 tokens of graph-injected context per turn. More than that usually means you're using it wrong.

---

## 📍 Data locations

- **Per-project shards:** `~/.mneme/projects/<sha>/*.db` — graph, history, semantic, findings, tasks, memory, wiki, architecture, media, conventions, federated, contracts, insights, …
- **Global meta:** `~/.mneme/meta.db`
- **Models:** `~/.mneme/llm/` (empty by default; `mneme models install` populates BGE-small)
- **IPC socket:** Unix `~/.mneme/supervisor.sock` · Windows `\\.\pipe\mneme-daemon-<pid>`
- **Health endpoint:** `http://127.0.0.1:7777/health`
- **Logs ring:** in-memory via `mneme daemon logs`

---

**Source of truth for command names and arg shapes:** the zod schemas in `mcp/src/types.ts` and the handler source in `mcp/src/tools/*.ts`. This file is regenerated from those on every release; never hand-edit.
