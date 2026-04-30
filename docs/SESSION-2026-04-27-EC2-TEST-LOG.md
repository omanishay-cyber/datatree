# EC2 Test Log — 2026-04-27 v0.3.2 home cycle Wave 1

Captured from EC2 `52.15.134.175` (i-0b96b31b31d9c73f0) during post-install
verification of the v0.3.2 home-cycle wave-1 zip
(`mneme-v0.3.2-windows-x64.zip` 55.22 MB).

## Test method

1. Cleanup phase: backup `~/.claude/.credentials.json`, kill all mneme procs,
   `mneme uninstall --all --purge-state`, force `Remove-Item ~/.mneme`.
2. Ship `mneme-v0.3.2-windows-x64.zip` via pscp.
3. `Expand-Archive zip -DestinationPath ~/.mneme -Force`.
4. Run `~/.mneme/scripts/install.ps1` (failed at G3 Python detection but
   binaries deployed cleanly — see B-006 below).
5. Verify via probe script: `Get-Process`, `/health`, `/api/graph/*`,
   `claude mcp list`, hook timing, A2 SPA serving.

## Wave 1 verification results

### Verified WORKING

- **A. version bump 0.3.0 → 0.3.2** (Agent A): `mneme --version` returns
  `mneme 0.3.2` from the freshly-installed binary at
  `~/.mneme/bin/mneme.exe` (50.7 MB). Embedded version string matches
  workspace `Cargo.toml`. PASS.

- **B. A5 macos-private-api cfg gate** (Agent B): N/A on EC2 (Windows);
  `cargo check` clean locally. Already-fixed-in-tree noted at home-cycle
  baseline. PASS.

- **C. 4.3 audit-inline daemon spawn fix** (Agent C): not directly tested
  on EC2 because `mneme build` hangs (B-001 below). The fix is in the
  binary (verified locally via `mcp__tree-sitter__find_text "audit_route"`
  and 5 unit tests passing). Will verify end-to-end after B-001 lands.

- **C. K3+H4 build summary status** (Agent C): not directly tested on EC2
  because build hangs (B-001).

- **D. SessionEnd hook bounded** (Agent D): `claude mcp list` runs without
  emitting `Hook cancelled`. PASS. Total `claude mcp list` time was
  11.64s on cold cache (cached MCP init for 4 servers + 4 probe handshakes
  + the bounded hook at the end). The 9.36s pre-fix red number was the
  isolated SessionEnd hook itself (Agent D's regression test); the
  11.64s here is the entire end-to-end CLI flow. Not a regression.

- **E. C2 daemon /health metrics** (Agent E): `/health` returns 200 with
  populated fields:
  - `disk_usage_mb: 28482` ✓ (was hardcoded `0.0` pre-fix)
  - `queue_depth: 0` ✓ (new field)
  - `p50_us: 0, p95_us: 0, p99_us: 0` ✓ (new fields, zero because no
    jobs dispatched yet)
  - `cache_hit_rate: 0.0` ✓ (forward-progress proxy default)
  - `watcher: { p50_ms: 0, p95_ms: 0, p99_ms: 0, ... }` ✓ (new fields)
  PASS.

- **E. livebus wiring** (Agent E): cannot test fully without a WS client,
  but daemon process tree shows `livebus-worker` supervised, the new bus
  bridge task is in the binary (verified locally), and 7 named children
  appear in `/health`. INFERRED PASS pending WS round-trip test.

- **F. A12 install ships SPA** (Agent F): `~/.mneme/static/vision/`
  contains `index.html` (655 bytes) + 37 assets/. Matches the daemon's
  expected layout per `resolve_static_dir()`. PASS.

- **`/api/graph/*` routes** (Wave 1 F1 D2/D3/D4): all 16 endpoints
  return 200, mostly empty arrays because no project is indexed yet
  (build hang prevents indexing). HTTP plumbing PASS.

### Verified NOT WORKING

- **E. A2 SPA fallback** (Agent E): both `GET /` and `GET /random/spa/route`
  **TIME OUT at 5s**. The daemon is alive (`/health` responds 200 in
  ~50ms on the same connection), the static dir is on disk
  (`~/.mneme/static/vision/index.html` 655 bytes confirmed), but
  `tower-http`'s `.fallback(ServeFile::new(index))` is not serving any
  request. Agent E's wiring landed in the binary but is not functional.

  **Hypothesis (for Wave 2 agent to confirm)**: `ServeDir::new(&dir)
  .not_found_service(serve_index)` was replaced with `.fallback(...)`,
  but the `.fallback` API in tower-http 0.5 may behave differently than
  expected — it might require the inner `ServeDir` to match before the
  fallback fires, and on cold start the dir traversal may block.
  Alternative: the route mount happens BEFORE the daemon's tokio runtime
  has fully spun up the file system handle.

  **Reproducer**: `Invoke-WebRequest -Uri "http://127.0.0.1:7777/" -UseBasicParsing -TimeoutSec 30`.

## NEW bugs found during EC2 test (Wave 2 queue)

### B-001 (CRITICAL): `mneme build .` hangs indefinitely

- **Symptom**: `mneme build .` on a 3-file test corpus
  (`app.ts` + `package.json` + `README.md` totaling ~200 bytes) ran for
  74 minutes before being killed.
- **Telemetry during hang**:
  - 4 mneme-parsers processes alive at low CPU (~2.0 each, cumulative)
  - 1 mneme-store, 1 mneme-md-ingest, 1 mneme-scanners, 1 mneme-brain,
    1 mneme-livebus, 1 mneme-daemon (supervised)
  - **Plus**: 1 SECOND mneme-daemon (PID 1916) spawned at 4:38 PM
    alongside the original daemon (PID 1216, started 3:59 PM)
  - **Plus**: 4 ORPHAN parser/scanner/brain/md-ingest processes
    spawned at 4:38 PM that are NOT supervisor children
  - `/health.children[*].total_jobs_completed = 0` for ALL 7 named
    children — meaning the inline build pipeline never dispatched a
    single job to the supervisor. It tried to use IPC (auto-spawned a
    second daemon) but neither path completed.
- **Likely root cause**: an inline-mode IPC call somewhere in the build
  pipeline (suspected: per-file post-parse hook into supervisor for
  embeddings/semantic.db writes, or the file-event push into livebus)
  hits the auto-spawn-and-retry loop in `IpcClient::request`. With no
  bound on retries, the build never returns. This is the same family of
  bug as Agent C's 4.3 fix (`run_audit_pass` was the obvious offender,
  but other IPC sites in build.rs likely have the same shape).
- **Fix surface**: audit `cli/src/commands/build.rs` for every
  `IpcClient::request` callsite; route inline-mode through direct paths
  per Agent C's `audit_route` pattern, OR introduce
  `IpcClient::with_no_autospawn()` builder (Agent C's suggested
  alternative).

### B-002 (HIGH): `mneme build` spawns a second daemon

- **Symptom**: when daemon is already running, `mneme build` triggers
  `IpcClient::request` → which on connect failure (?) calls
  `spawn_daemon_detached()` → second `mneme-daemon.exe` process appears.
- **Repro**: with daemon already running, run `mneme build .` in a
  freshly-installed corpus.
- **Likely root cause**: `IpcClient` connection-failure detection is
  fragile (e.g., short read of body returns "supervisor unreachable"
  even when supervisor IS running). The auto-spawn fires anyway. See
  `cli/src/ipc.rs:425` and `cli/src/ipc.rs:456`.
- **Fix surface**: add a `mneme daemon ping` precheck OR exponential
  backoff with explicit "is the supervisor really down?" probe before
  auto-spawning a second one.

### B-003 (HIGH): Build orphans non-supervisor workers

- **Symptom**: when `mneme build` was killed, 4 of its inline-spawned
  workers (parser, scanner, brain, md-ingest) survived as orphans
  because they weren't supervised by the supervisor. They sit idle
  consuming RAM until manually `taskkill`'d.
- **Fix surface**: inline build mode should EITHER attach its workers
  to the supervisor on spawn (so cleanup is automatic) OR register a
  Ctrl-C / process-exit handler that taskkills its own children.

### B-004 (MEDIUM): `mneme uninstall --yes` flag is rejected

- **Symptom**: `mneme uninstall --all --purge-state --yes` returns
  `error: unexpected argument '--yes' found`. The clap usage line
  shows `mneme.exe uninstall --all --purge-state` (no `--yes`).
- **Where claimed**: `OFFICE-TODO.md` §1.2 step 4, `NEXT-PATH.md`
  Phase B6 step 4. Both call `mneme uninstall --all --purge-state --yes`.
  Either the docs lie or the flag was removed without doc update.
- **Fix surface**: `cli/src/commands/uninstall.rs::Args` — add `#[arg(long)] yes: bool` to the clap struct so non-interactive callers can confirm. Or document that `--all` already implies non-interactive.

### B-005 (MEDIUM): No `~/.mneme/logs/` dir on EC2 install

- **Symptom**: probe found no `~/.mneme/logs/` directory after install +
  daemon start. Supervisor logs are presumably going somewhere else
  (stdout? in-memory ring? `~/.mneme/run/` ?). Cannot tail
  `mneme daemon logs` if the path is wrong.
- **Fix surface**: standardize on `~/.mneme/logs/supervisor.log` with
  rotation. Make sure it's created at daemon start.

### B-006 (MEDIUM): install.ps1 G3 Python detection insufficient

- **Symptom**: `install.ps1` step 1d `[G3] Python` ran
  `& $pythonExe.Source --version 2>&1` against the Microsoft-Store-stub
  python at `C:\Users\Administrator\AppData\Local\Microsoft\WindowsApps\python.exe`.
  The stub printed `Python was not found; run without arguments to install
  from the Microsoft Store...` to stderr and the script aborted with
  `NativeCommandError`.
- **CHANGELOG note** (per `CHANGELOG.md` v0.3.2): "Microsoft-Store-Python
  stub detection" was added — but appears insufficient for the case
  where the stub IS on PATH and resolved by `Get-Command python`.
- **Fix surface**: `install.ps1::Test-PythonRealOrStub` — detect by
  parsing `--version` output AND by checking whether the binary path is
  under `WindowsApps\` (the stub's location).

### B-007 (LOW): Disk only barely freed by `mneme uninstall --purge-state`

- **Symptom**: pre-uninstall disk = 3.0 GB free; post-uninstall = 3.4 GB
  free. Only 0.4 GB freed despite `Remove-Item ~/.mneme -Recurse -Force`.
  Then after the hung build: 1.7 GB free (consumed 1.6 GB).
- **Hypothesis**: build's intermediate state lives outside `~/.mneme/`
  (likely in `$env:TEMP\` or in `~/.bun/` cache). The uninstall doesn't
  touch those paths.
- **Fix surface**: `mneme uninstall --purge-state` should also clean
  `$env:TEMP\mneme-*`, `~/.bun/install/cache` (per CHANGELOG K1
  recommendation), and any auxiliary state.

## Files modified this Wave 1 cycle (already in master)

- 17 files in commit `8722403` (C+D+E+F changes)
- 11 files in commit `8deb1fb` (Agent A version bump) merged via `2a1ec23`

## Wave 2 priority order (recommended)

1. **B-001** `mneme build` hang — blocks REAL-1 acceptance
2. **A2 SPA fallback timeout** — blocks vision UI from working at all
3. **B-002** `mneme build` second-daemon spawn — confuses the process
   tree and disk metrics
4. **B-006** install.ps1 Python stub detection — install.ps1 fails on
   any clean Windows machine where Python isn't pre-installed
5. **B-005** logs dir — operational visibility
6. **B-003** build worker orphans — cleanup hygiene
7. **B-004** uninstall --yes — docs vs code mismatch
8. **B-007** purge-state scope — cleanup completeness

## Pre-existing carryover from OFFICE-TODO.md (still open)

- 4 K10 chaos tests with `#[ignore]` (need test-only hooks in prod code)
- K4 local LLM install (manual ~2.4 GB GGUF download)
- 4.1 schema column-additive migration (Phase B work)
- I1 batch 3 — 8 more empty shards (perf, errors, agents, refactors,
  contracts, insights, livestate, telemetry)
- QA-4 view.rs test non-determinism
- QA-6 hook_writer e2e fixture

## Recommended Wave 2 dispatch

6 parallel agents per the iron rules (`feedback_agent_dispatch_full_power.md`):

- **Agent G** (CRITICAL): B-001 + B-002 — audit `cli/src/commands/build.rs`
  for every IPC callsite, apply Agent C's `audit_route`-style inline
  pattern, plus add `IpcClient::with_no_autospawn()` builder.
- **Agent H** (CRITICAL): A2 SPA fallback timeout — diagnose Agent E's
  `.fallback(ServeFile::new(index))` route in
  `supervisor/src/health.rs::compose_app_router`. Likely needs
  `ServeDir::new(&dir).fallback(serve_index_handler)` or equivalent.
- **Agent I**: B-003 build worker cleanup — register process-exit
  handler in `cli/src/commands/build.rs::run_inline` that kills its
  spawned children on Ctrl-C / panic.
- **Agent J**: B-004 + B-005 — add `--yes` flag to uninstall, ensure
  `~/.mneme/logs/supervisor.log` exists with rotation.
- **Agent K**: B-006 — robust Python stub detection in `install.ps1`.
- **Agent L** (parallel-safe): stale-doc cleanup — INSTALL.md,
  NEXT-PATH.md, ROADMAP.md mark A5/A12 as done; consolidate.

Each agent gets the iron-rule preamble (tree-sitter MCP + superpowers
skills + LOCAL only + don't commit centrally).
