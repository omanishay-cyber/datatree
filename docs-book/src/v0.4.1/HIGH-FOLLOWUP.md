# HIGH follow-ups carried into v0.4.1

**Date opened:** 2026-05-06
**Source audit:** `docs/debug/AUDIT-2026-05-05-deep.md`

This file tracks the remaining HIGH items from the 2026-05-05 deep audit
that are NOT closed in v0.4.0 master. Every item here has a concrete
next step — none are "deferred indefinitely" or "design decision".
Anish's standing rule: no cockroach stays alive.

---

## HIGH-2 — Unify CLI in-proc + daemon build paths

**File:** `cli/Cargo.toml:75-115`, `cli/src/commands/build.rs`,
`supervisor/src/build_pipeline.rs` (new in v0.4.1).

**State today:** `mneme build` runs the indexing pipeline in-process
when the daemon is unreachable, and the daemon's supervisor runs the
same logic when it dispatches to its store-worker. The two
implementations have drifted slightly (different progress events,
different chunking, different rebuild semantics on stale .wal).

**Why deferred from v0.4.0:** Unifying these requires:
1. A new `BuildPipeline` trait that both consumers implement.
2. Refactoring `build.rs::run` to consume the trait.
3. Wiring the supervisor's worker dispatch through the same trait.
4. Updating ~12 chaos tests that distinguish the two paths.
5. Re-running the v0.3.2-to-v0.4.0 migration chaos suite end-to-end
   on the Linux + Windows VMs to confirm nothing regresses.

That's a 3-5 day refactor with a non-trivial risk surface. Doing it
right inside the v0.4.0 ship window would push the release another
week. Doing it wrong (rushing the unification) breaks the
single-most-load-bearing user-facing command.

**Plan for v0.4.1 (week of 2026-05-12):**
- Day 1: design `BuildPipeline` trait + sketch ownership / progress
  channel shape. Land as a non-functional commit so downstream agents
  can review.
- Day 2: port `cli/build.rs` to the trait.
- Day 3: port supervisor's worker dispatch.
- Day 4: full chaos-suite rerun on Anish PC + Linux VM + Windows VM.
- Day 5: monitoring window for regression reports before close.

---

## HIGH-3 — Unify dual IPC surfaces (stdin jobs + socket control)

**File:** `supervisor/src/ipc.rs:1-44`, `common/src/worker_ipc.rs`.

**State today:** Worker children read job dispatch from stdin
(line-delimited JSON). The CLI talks to the supervisor over the unix
socket / named pipe (length-framed binary). Both are "IPC" but the
framing, error handling, and reconnection logic are duplicated.

**Why deferred from v0.4.0:** HIGH-49 just landed the store IPC
12-variant dispatch macro. Unifying the OUTER IPC surfaces (worker
stdin vs. CLI socket) on top of that needs:
1. A unified message envelope (length-framed JSON over either
   transport).
2. A small `IpcTransport` trait that abstracts stdin pipe and socket.
3. Migration of every worker spawn site (10+ in supervisor/manager.rs).
4. Backward-compat shim for in-flight workers during a self-update
   swap so a v0.4.0 daemon can still talk to a v0.4.1 worker.

**Plan for v0.4.1:**
- Day 1: trait + envelope design, ratify with the secrets_redact +
  HIGH-19 dispatch invariants (no unvalidated pool prefix etc.).
- Day 2: port worker spawn sites in manager.rs.
- Day 3: backward-compat shim + soak test on the stdin-wedge
  reproducer (Bug #233).
- Day 4: regression pass on the supervisor chaos tests.

---

## HIGH-45 — Continue api_graph god-file extraction

**File:** `supervisor/src/api_graph/mod.rs` (currently 3,316 LOC).

**State today (post v0.4.0):** mod.rs is down from 3,977 LOC to
~3,316 LOC after extracting:
- `health.rs` (api_health, api_daemon_health, voice_stub, stub_handler)
- `projects.rs` (api_projects + DiscoveredProject + ProjectsResponse +
   load_meta_projects + newest_db_mtime_iso + count_table)
- `layout.rs` (api_graph_layout + cache + compute_layout + 4 cache tests)

**Remaining handler families to extract (each is ~50-300 LOC):**
1. `nodes_edges.rs` — api_graph_nodes + api_graph_edges + the
   GraphNode / GraphEdge types
2. `tree.rs` — api_graph_file_tree
3. `flow.rs` — api_graph_kind_flow + api_graph_domain_flow
4. `community.rs` — api_graph_community_matrix
5. `commits.rs` — api_graph_commits
6. `heatmap.rs` — api_graph_heatmap
7. `architecture.rs` — api_graph_layers + api_graph_galaxy_3d
8. `coverage.rs` — api_graph_test_coverage
9. `theme.rs` — api_graph_theme_palette
10. `hierarchy.rs` — api_graph_hierarchy
11. `status.rs` — api_graph_status (status bar handler, not the
    `mneme status` CLI which is unrelated)
12. `files_findings.rs` — api_graph_files + api_graph_findings

After all 12 land, mod.rs should be ~500 LOC: just ApiGraphState +
ProjectQuery + the build_router function + middleware + with_layer_db
helpers + the integration tests that exercise multiple handlers.

**Pattern proven by health/projects/layout extractions:**
1. Copy block to new sibling submodule with `use super::{...};` for
   shared state.
2. Mark the handler `pub(super) async fn` so build_router can call it
   via `<submodule>::handler_name`.
3. Delete the block from mod.rs.
4. Add `mod foo; use foo::handler_name;` near the existing imports.
5. Move related tests into the submodule's own `#[cfg(test)] mod tests`.
6. Re-import any types the parent test mod still uses.

Each extraction is a single commit. They can land in any order — the
submodules don't depend on each other.

---

## HIGH-46 — Continue doctor god-file extraction

**File:** `cli/src/commands/doctor/mod.rs` (currently ~3,150 LOC).

**State today (post v0.4.0):** Renamed from `doctor.rs` into a module
dir. Color tinting added (`colorize_status_value`). NO handler
extraction yet — the file is still one big mod.rs.

**Remaining probe families to extract:**
1. `render.rs` — `line` + box-drawing helpers + `print_banner` +
   `colorize_status_value` (already present in mod.rs but as a
   sibling module would make the contract explicit)
2. `hooks_probe.rs` — settings.json hook validation
3. `mcp_probe.rs` — claude.json + register-mcp checks
4. `daemon_probe.rs` — PID + socket + /health probe
5. `models_probe.rs` — BGE / tokenizer / ORT DLL discovery
6. `toolchain_probe.rs` — G1-G10 dev-tool probes
7. `sandbox_probe.rs` — Defender / SELinux / sandbox checks

**Bonus (mechanical):** the audit also flagged 109 unwrap()/expect()
calls in doctor.rs. While extracting, audit each — replace with `?`
propagation or explicit fallbacks. Target: drop the count from 109
to under 30.

---

## How to close items here

When closing, edit this file: change the section header from "##" to
"## ✅ CLOSED " and add a one-line "Closed in commit `<sha>`" footnote.
Don't delete entries — the audit trail matters more than file size.
