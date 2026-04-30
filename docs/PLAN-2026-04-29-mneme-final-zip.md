# Mneme Final ZIP — Master Plan (2026-04-29)

> **For agentic workers:** This plan is the source of truth for tonight's work. Update status inline as work progresses.

**Goal:** Deliver `C:\Users\Anish\Desktop\mneme final.zip` — a complete, fully-working Mneme package (source + release binaries + models + docs) with zero open Wave 2 bugs, verified end-to-end on the VMware VM.

**Architecture (testing protocol — per user instruction):**

```
Fix on HOST (this PC) → cargo build --release → package zip
   → upload zip to VM (192.168.1.193) via Posh-SSH
   → on VM: mneme uninstall --all --purge-state (preserve creds + models per rule)
   → on VM: install fresh from uploaded zip
   → on VM: run targeted + comprehensive tests
   → if any test fails: back to host, debug, repeat
   → if all pass: package mneme final.zip on host Desktop
```

**Tech Stack:** Rust 1.95 workspace (12 crates) + Bun 1.3 TS MCP + Tauri vision + Python 3.14 multimodal sidecar + Posh-SSH for VM ops + PowerShell scripting.

**HARD RULES (non-negotiable):**
- NEVER fix anything on the VM — only test there.
- Preserve `~/.mneme/models/` and `~/.claude/.credentials.json` across uninstalls.
- Every claim of "fixed" requires fresh verification evidence (the verification-before-completion skill).
- Every bug fix follows systematic-debugging: investigate → hypothesis → minimal fix → verify.
- One bug at a time per CLAUDE.md task isolation rule.
- All sub-agents launched with `run_in_background: true`.

---

## Wave 2 bug roster (priority order from `docs/SESSION-2026-04-27-EC2-TEST-LOG.md`)

| # | Severity | Bug | Status | Fix files |
|---|---|---|---|---|
| 1 | CRITICAL | B-001 — `mneme build .` hangs forever (IPC auto-spawn loop) | INVESTIGATING | `cli/src/commands/build.rs`, `cli/src/ipc.rs` |
| 2 | CRITICAL | A2 — SPA fallback `GET /` times out at 5s | NOT STARTED | `supervisor/src/health.rs:317-411` |
| 3 | HIGH | B-002 — `mneme build` spawns a second daemon | NOT STARTED | `cli/src/ipc.rs:425,456` |
| 4 | MEDIUM | B-006 — `install.ps1` Python stub detection insufficient | NOT STARTED | `scripts/install.ps1::Test-PythonRealOrStub` |
| 5 | MEDIUM | B-005 — `~/.mneme/logs/` not created on install | NOT STARTED | `supervisor/src/lib.rs` (or service init) |
| 6 | HIGH | B-003 — `mneme build` orphans non-supervisor workers on Ctrl-C | NOT STARTED | `cli/src/commands/build.rs::run_inline` |
| 7 | MEDIUM | B-004 — `mneme uninstall --yes` flag rejected | NOT STARTED | `cli/src/commands/uninstall.rs::Args` |
| 8 | LOW | B-007 — `--purge-state` leaves stray temp/bun-cache | NOT STARTED | `cli/src/commands/uninstall.rs` |

Plus carry-over verifications:
- All Phase 1 closeouts in memory (imports resolver / vision endpoints / SD-1 / SEC-1+2 / deps parser / J9 inject-recall / supervisor IPC variants) must continue to pass.
- 28 CLI subcommands smoke
- 47–48 MCP tools live count
- 17 `/api/graph/*` endpoints serving real shard data
- 8 hooks firing
- Hook STDIN-JSON contract on all 8 binaries
- Daemon detached lifecycle (no Job-object inheritance leak)

---

## Phase A — Baseline & Investigation (NOW)

- [x] A.1 Connect to VMware VM at 192.168.1.193 (Posh-SSH, user/Mneme2026!)
- [x] A.2 Read priority docs (CLAUDE.md, NEXT-PATH.md, REMAINING_WORK.md, SESSION-2026-04-27-EC2-TEST-LOG.md, CHANGELOG.md, ARCHITECTURE.md, INSTALL.md, dev-setup.md, mcp-tools.md, IDEAS.md)
- [x] A.3 Register tree-sitter project at new bundle path
- [x] A.4 Probe VM Mneme state — found v0.3.0 installed, daemon down, models empty, hooks not registered
- [x] A.5 Probe host toolchain — rust 1.95, cargo 1.95, bun 1.3.13, node 24, python 3.14, git 2.53, 432 GB free C:
- [ ] A.6 `cargo check --workspace` baseline at new path (in flight, background)
- [ ] A.7 Phase 1 systematic-debugging investigation for B-001 (read build.rs IPC callsites + ipc.rs auto-spawn + audit.rs reference pattern)

## Phase B — Sequential Bug Fixes (Wave 2)

For each bug — apply systematic-debugging Phase 1-4:
1. **Investigate** — read involved files, trace data flow, gather evidence.
2. **Hypothesize** — single root cause statement.
3. **Test** — minimal change; write/run failing test where applicable.
4. **Verify** — fresh evidence before claiming done. Never "should work".

### B.1 — B-001 hang (CRITICAL — first because it blocks all build verification)

**Symptom:** `mneme build .` on 3-file corpus runs 74 min before kill. Children sit at 0 jobs completed; second daemon spawned mid-build.

**Hypothesis to test (Phase 1 evidence first):** an `IpcClient::request` callsite in build.rs with no auto-spawn guard hits the retry loop. Agent C's `audit_route` pattern already fixed the audit pass — same pattern needed elsewhere. OR introduce `IpcClient::with_no_autospawn()` builder.

**Steps:**
- [ ] Grep all `IpcClient::request` and `IpcClient::new` callsites in `cli/src/commands/build.rs`
- [ ] Read `cli/src/ipc.rs` auto-spawn logic (especially lines 425, 456)
- [ ] Read `cli/src/commands/audit.rs` to see Agent C's `audit_route` pattern
- [ ] Form hypothesis with file:line citations
- [ ] Implement: route inline-mode through direct paths or add `with_no_autospawn()` flag
- [ ] Add unit test: `mneme build` against a 3-file corpus completes in <60s
- [ ] Verify via `cargo test --workspace -- --test-threads=1`

### B.2 — A2 SPA fallback timeout (CRITICAL)

**Symptom:** `GET /` and `GET /random/spa/route` time out at 5s on EC2. `/health` answers in 50ms. Static dir on disk. tower-http `.fallback(ServeFile::new(index))` doesn't fire.

**Steps:**
- [ ] Read `supervisor/src/health.rs:317-411` (Agent M's cached `Arc<[u8]>` handler claim)
- [ ] Identify why `.fallback` is silent — is it ServeDir + fallback semantics in tower-http 0.5?
- [ ] Replace with explicit route handler that reads `~/.mneme/static/vision/index.html` once at boot, caches as `Arc<[u8]>`, serves on any non-/api/* GET
- [ ] Add integration test hitting `GET /` returns 200 with the dashboard HTML

### B.3 / B.4 — B-002 second-daemon spawn + B-003 worker orphans

Both touch `cli/src/ipc.rs` + `cli/src/commands/build.rs` so handle together to avoid merge churn but verify independently.

- [ ] Add `mneme daemon ping` precheck before auto-spawn (B-002)
- [ ] Wire process-exit handler on build.rs::run_inline that taskkills children on Ctrl-C / panic (B-003)

### B.5 / B.6 / B.7 / B.8 — Independent (parallel-safe)

- [ ] B-006 install.ps1 Python stub detection (`scripts/install.ps1`)
- [ ] B-005 `~/.mneme/logs/supervisor.log` with rotation (`supervisor/src/lib.rs` + service init)
- [ ] B-004 add `--yes` flag to uninstall (`cli/src/commands/uninstall.rs::Args`)
- [ ] B-007 purge-state cleans `$env:TEMP\mneme-*` + `~/.bun/install/cache` (`cli/src/commands/uninstall.rs`)

---

## Phase C — Build & Package

- [ ] C.1 `cargo test --workspace` clean
- [ ] C.2 `cargo build --workspace --release`
- [ ] C.3 `cd mcp && bun install && bunx tsc --noEmit && bun test`
- [ ] C.4 `cd vision && bun install && bun run build` (vision SPA dist)
- [ ] C.5 `cd vision/tauri && cargo build --release` (vision Tauri exe — only if Tauri prereqs satisfied; otherwise document skip)
- [ ] C.6 Stage `dist/` payload mirroring `~/.mneme/` layout: `bin/`, `mcp/`, `static/vision/`, `models/` (placeholder + install-from-path note), `scripts/install.ps1`, `uninstall.ps1`
- [ ] C.7 Build `mneme-v0.3.3-windows-x64.zip` (incremented patch since v0.3.2 EC2-tested + Wave 2 fixes)
- [ ] C.8 Verify zip integrity (Expand-Archive to scratch dir, run smoke test)

## Phase D — VM Test Cycle

- [ ] D.1 Backup `~/.claude/.credentials.json` on VM
- [ ] D.2 Capture VM `~/.mneme/models/` to backup (if any model files appear later)
- [ ] D.3 `mneme uninstall --all --purge-state` on VM (or kill processes + Remove-Item if --yes flag still rejected)
- [ ] D.4 Confirm `~/.mneme/` is gone, PATH cleaned, settings.json hooks stripped
- [ ] D.5 Restore credentials
- [ ] D.6 Upload new zip via Posh-SSH `Set-SCPItem`
- [ ] D.7 Expand-Archive on VM to `~/.mneme/`
- [ ] D.8 Run `~/.mneme/scripts/install.ps1` on VM
- [ ] D.9 Verify post-install: `mneme --version` = 0.3.3, `mneme doctor` clean, `claude mcp list` shows ✓ Connected, settings.json has 8 hook entries, daemon `/health` 200
- [ ] D.10 Build a real test corpus (the new bundle source ~600 files): `mneme build .` completes in finite time and graph.db has rows
- [ ] D.11 Targeted bug verification:
    - B-001: build completes — assert
    - A2: `Invoke-WebRequest http://127.0.0.1:7777/` returns 200 with HTML
    - B-002: only ONE mneme-daemon process after build
    - B-003: kill build mid-run, no orphan workers
    - B-006: install.ps1 succeeded with only Python stub on PATH (capture stub-detect path)
    - B-005: `~/.mneme/logs/supervisor.log` exists, growing
    - B-004: `mneme uninstall --all --purge-state --yes` parses
    - B-007: post-purge disk delta > 1 GB on built corpus
- [ ] D.12 Comprehensive tests:
    - 28 CLI subcommands via `mneme --help` enumeration
    - 47-48 MCP tools via `claude mcp list` + spot-check 5 tools (recall, blast, audit, doctor, health)
    - 17 `/api/graph/*` endpoints HTTP 200 with real JSON
    - 8 hook firings (UserPromptSubmit, SessionStart, PreToolUse, PostToolUse, Stop, PreCompact, SubagentStop, SessionEnd)
    - 6 worker procs alive under supervisor
    - Hook persistence: history.db.turns / tasks.db.ledger / tool_cache.db / livestate.db all grow
    - /api/health, /api/projects, /api/voice, /api/daemon/health
    - Stress: build a 1000+ file corpus on VM (bundle itself), verify completes
    - 24h leak soak deferred (nothing user-visible at 30 min)

## Phase E — Final Deliverable

- [ ] E.1 Assemble `mneme final.zip` on host Desktop:
    - `source/` — full source tree (this bundle, including PLAN doc)
    - `release/mneme-v0.3.3-windows-x64.zip` — the tested binary release
    - `models/` — placeholder + README explaining `mneme models install --from-path`
    - `docs/` — full docs tree
    - `INSTALL.md` — installation instructions
    - `CHANGELOG.md` — including [Unreleased] entries for Wave 2 fixes
    - `VERIFIED.md` — test results from Phase D
- [ ] E.2 Place at `C:\Users\Anish\Desktop\mneme final.zip`
- [ ] E.3 Append CHANGELOG entries with file:line citations and verification evidence
- [ ] E.4 Update `MEMORY.md` and `project_mneme_rebuild.md` with final state

---

## Verification gates (apply before each fix is marked done)

```
GATE-A (per bug fix):
  1. Identify the verification command for this bug
  2. Run cargo check (or cargo test) — assert clean
  3. Run cargo build --release for affected crate
  4. Run any unit/integration test added
  5. Read full output, count failures, exit code 0

GATE-B (per release artifact):
  1. cargo test --workspace
  2. bunx tsc --noEmit (mcp + vision)
  3. cargo build --workspace --release
  4. zip integrity (Expand-Archive scratch + smoke test)

GATE-C (final mneme final.zip):
  1. All Phase D test results PASS
  2. cargo audit + cargo deny clean (RUSTSEC, license, bans, duplicates)
  3. Hook STDIN-JSON contract verified on all 8 binaries
  4. ALL 8 Wave 2 bugs verified fixed via dedicated test on VM
```

---

## Anti-patterns to refuse (per skills)

- "Quick fix without root cause" — VIOLATION (systematic-debugging Iron Law)
- "Should work now" without running command — VIOLATION (verification-before-completion Iron Law)
- "Should pass" / "looks correct" / "probably fine" — VIOLATION
- Fix on VM directly — VIOLATION (user explicit rule)
- Bundle multiple bug fixes in one commit — discouraged but not blocked when files demand it
- Skip cargo test before claiming a fix complete — VIOLATION
