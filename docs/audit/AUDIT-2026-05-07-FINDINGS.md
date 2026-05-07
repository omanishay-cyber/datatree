# Mneme v0.4.0 Audit — 2026-05-07 (173 findings)

> **Source:** 8-agent parallel deep audit run 2026-05-07.
> **Total:** 173 findings = 11🔴 + 62🟠 + 74🟡 + 22🟢 + 4 already-fixed pre-audit + 4 false-positives.
> **Recovered:** 173 of 173 findings recovered from agent .output files.
> **Tracker rule:** every fix that ships green flips `[ ]` → `[x] (commit SHA)`.

## Summary

| Agent | Findings | Crit | High | Med | Low |
|-------|----------|------|------|-----|-----|
| UX | 30 | 3 | 12 | 12 | 3 |
| Security | 15 | 0 | 7 | 6 | 2 |
| Error-handler | 43 | 8 | 16 | 15 | 4 |
| Pragmatic | 5 | 0 | 0 | 2 | 3 |
| Super-tester (CLI) | 28 | 0 | 6 | 14 | 4 |
| Performance | 16 | 0 | 6 | 9 | 1 |
| Testing-gaps | 24 | 0 | 11 | 9 | 4 |
| Reliability | 12 | 0 | 4 | 7 | 1 |
| **TOTAL** | **173** | **11** | **62** | **74** | **22** |

Note: UX critical count 3 + Error-handler critical count 8 = 11. The 4 already-fixed pre-audit and 4 false-positives below are *included* in the per-agent severity counts (they were originally reported critical/high before the cross-check).

---

## 🔴 Critical (11)

- [x] **ERR-1** — `cli/src/build_lock.rs:112` — silent-swallow `let _ = writer.write_all(stamp.as_bytes())` — racing build sees blank stamp (`609f23f`)
- [x] **ERR-2** — `cli/src/commands/build.rs:8443` — FALSE POSITIVE: `expect("open shard ro")` is in `#[cfg(test)] mod empty_shard_tests` (`609f23f`)
- [x] **ERR-3** — `cli/src/commands/build.rs:8454` — FALSE POSITIVE: `let _ = store.inject.insert(...).await` is in `#[cfg(test)] mod empty_shard_tests` (`609f23f`)
- [x] **ERR-4** — `cli/src/commands/self_update.rs:513` — `unreachable!()` in `compare_pre_release_identifiers` → return `Ordering::Equal` (`609f23f`)
- [x] **ERR-5** — `cli/src/commands/self_update.rs:1718` — silent `let _ = fs::rename(&backup, current)` rollback → loud error + return Err (`609f23f`)
- [x] **ERR-6** — `cli/src/commands/status.rs:120` — silent `let _ = render_supervisor_detail(...).await` → eprintln warn (`609f23f`)
- [x] **ERR-7** — `supervisor/src/update_check.rs:473` — silent `let _ = std::fs::write(&tmp, json.as_bytes())` → tracing warn (`609f23f`)
- [x] **ERR-8** — `supervisor/src/health.rs:1575` — FALSE POSITIVE: `let _ = axum::serve(listener, app).await` is in test helper at line 1575; production health server at line 314 already error-handled (`609f23f`)
- [x] **UX-1** — `cli/src/commands/update.rs:1` — `mneme update` vs `mneme self-update` verb collision; rename `update` → `reindex` with deprecated alias (`609f23f`)
- [x] **UX-2** — `cli/src/commands/install.rs:1` — first-run has no guided onboarding sequence; print 4-step post-install Next-Steps checklist gated on receipt-count=0 (`b71ee52`)
- [x] **UX-3** — `vision/src/App.tsx:222` — daemon-down banner says `mneme-daemon start` (binary doesn't exist as user command); change to `mneme daemon start` (`609f23f`)

---

## 🟠 High (62)

### Already shipped — Batch 2 (commit `b71ee52`)

- [x] **SEC-1** — `supervisor/src/ipc.rs:1105` — WriteTurn / WriteLedgerEntry / WriteToolCall / WriteFileEvent accept arbitrarily-sized content fields with no size cap; per-field byte caps added
- [x] **SEC-2** — `supervisor/src/ipc.rs:1105` — Hook write commands accept arbitrary `project: PathBuf` and create shards anywhere; project registry validation added on hook writes
- [x] **SEC-3** — `supervisor/src/ipc.rs:850` — Blast `depth` and GodNodes `n` unbounded; clamp depth to 10, n to 50_000
- [x] **SEC-5** — `cli/src/commands/self_update.rs:928` — `browser_download_url` from GitHub release JSON used without origin validation; canonical release URL prefix gate added
- [x] **SEC-7** — `supervisor/src/api_graph/graph.rs:112` — `?limit=` interpolated via `format!` into SQL bypassing parameter binding; switched to `LIMIT ?1`/`?2` parameterization

### Pre-audit already fixed (commit `d73af4d`)

- [x] **REVIEW-1** — `cli/src/commands/godnodes.rs:233` — `print_gods` missing `display_path` — `\\?\` prefix reaches Windows terminal; 5th display_path site fixed
- [x] **REVIEW-2** — `cli/src/commands/export.rs:890` — `mneme export --format=jsonld` still emits Apache-2.0 license URL after Personal-Use migration; URL updated
- [x] **CLI-14** — `cli/src/commands/find_references.rs:71` — em-dash `—` in error string mojibakes on cp1252 Windows terminals; replaced with `--`

### Unfixed — Error handling

- [ ] **ERR-9** — `cli/src/commands/build.rs:3676` — `Err(_) => return` on git.db open silently produces zero churn stats
- [ ] **ERR-10** — `cli/src/commands/build.rs:3883` — `Err(_) => return` on intent.toml read silently disables intent-indexing pass
- [ ] **ERR-11** — `cli/src/commands/log.rs:134` — `let _ = mirror_to_disk(&log_dir, sample)` silently drops disk-mirror write errors
- [ ] **ERR-12** — `supervisor/src/ipc.rs:700` — `let _ = manager.kill_child(&n).await` in RestartAll silently ignores individual kill failures
- [ ] **ERR-13** — `supervisor/src/watcher.rs:257` — `let _ = tx_for_notify.send(res)` drops fs events when channel full/closed; index goes stale silently
- [ ] **ERR-14** — `cli/src/commands/build.rs:2960` — `Err(_) => return stats` in git stats harvesting discards error type entirely
- [ ] **ERR-15** — `cli/src/commands/build.rs:4638` — `Err(_) => stats.status = "no-git"` swallows git rev-parse OS error; EACCES indistinguishable from no-git
- [ ] **ERR-16** — `cli/src/commands/uninstall.rs:62` — `let _ = std::fs::write(marker_path, text)` silently swallows uninstall progress marker write in 4 sites
- [ ] **ERR-17** — `supervisor/src/api_graph/projects.rs:99` — `let _ = conn.busy_timeout(...)` discards SQLite busy_timeout pragma failure
- [ ] **ERR-18** — `store/src/query.rs:551` — `let _ = conn.busy_timeout(...)` plus 3 WAL pragmas silently dropped on writer connection
- [ ] **ERR-19** — `brain/src/embeddings.rs:805` — `.flat_map(|h| h.join().expect("hashing_embed panicked"))` re-panics caller thread, kills brain
- [ ] **ERR-20** — `brain/src/llm.rs:228` — `BACKEND_SINGLETON.get().expect(...)` panics on TOCTOU race between concurrent inits
- [ ] **ERR-21** — `cli/src/commands/doctor/mcp_probe.rs:145` — `let _ = s.read_to_end(&mut all)` silently drops stderr drain IO errors in MCP probe
- [ ] **ERR-22** — `scanners/src/main.rs:199` — `shard_writer_order.remove(pos).unwrap()` panics if VecDeque shrinks unexpectedly
- [ ] **ERR-23** — `supervisor/src/ws.rs:348` — `let _ = sink.send(Message::Text(payload)).await` leaks subscription per stale WS client
- [ ] **ERR-24** — `cli/src/commands/build.rs:5832` — `let _ = crate::windowless_command("taskkill")...` silently ignores subprocess kill failure → zombie locks

### Unfixed — UX

- [ ] **UX-4** — `cli/src/commands/uninstall.rs:1` — `mneme uninstall --purge-state` has no interactive confirmation prompt
- [ ] **UX-5** — `cli/src/commands/recall.rs:38` — `WARN: NO EMBEDDING MODEL CONFIGURED` is all-caps stderr; lower tone, route to stdout, sentence case
- [ ] **UX-6** — `cli/src/commands/audit.rs:835` — Audit summary prints absolute findings.db path including SHA-shard hash; replace with `mneme audit --show` hint
- [ ] **UX-7** — `cli/src/commands/doctor/mod.rs:266` — `mneme daemon start` remediation buried at bottom; promote blocking issues to top banner
- [ ] **UX-8** — `cli/src/commands/history.rs:188` — Timestamps in UTC ISO never match user's local timezone; use `chrono::Local`, keep `--utc` for scripting
- [ ] **UX-9** — `cli/src/commands/status.rs:156` — `term.clear_screen()` destroys scrollback unconditionally; gate on `term.is_term()`
- [ ] **UX-10** — `cli/src/commands/doctor/mod.rs:387` — Per-worker row 140+ chars wraps on 80-col terminals; measure terminal width, truncate or continuation-line
- [ ] **UX-11** — `vision/src/components/SidePanel.tsx:46` — Side panel always shows placeholder; implement `/api/graph/files/:id` or replace 4 dead tabs
- [ ] **UX-12** — `vision/src/components/FilterBar.tsx:4` — Hardcoded TYPE/DOMAIN facets may not match any real project; populate dynamically from API
- [ ] **UX-13** — `cli/src/commands/blast.rs:39` — `--depth` defaults to 1 with no visible truncation indicator; add `Showing depth=1. Use --deep for transitive impact.` footer
- [ ] **UX-14** — `vision/src/views/index.ts:31` — 14 sidebar views with no data-availability indicator; status dot or dim labels for empty views
- [ ] **UX-15** — `cli/src/commands/models.rs:1` — `install-onnx-runtime` is a stub that may exit silently; print explicit "coming in future release" message

### Unfixed — Super-tester (CLI)

- [ ] **CLI-1** — `cli/src/commands/update.rs:23` — `--yes` flag has `default_value_t = true`, cannot be disabled; flag is silently mandatory
- [ ] **CLI-3** — `cli/src/commands/status.rs:157` — `mneme status` clears terminal scrollback unconditionally (related to UX-9)
- [ ] **CLI-4** — `cli/src/commands/log.rs:54` — `--interval` doc says min=1/max=3600 but parser accepts any u64; clamp silent
- [ ] **CLI-5** — `cli/src/commands/recall.rs:207` — daemon-up `recall` ignores `--type` filter when supervisor returns 0 hits and falls back to semantic
- [ ] **CLI-10** — `cli/src/commands/audit.rs:901` — `resolve_project` silently falls back to `"."` when both arg and CWD unavailable
- [ ] **CLI-12** — `cli/src/commands/self_update.rs:1054` — Default `mneme self-update` always fails on current releases (no .minisig, no `--allow-unsigned`)
- [ ] **CLI-23** — `cli/src/commands/blast.rs:86` — Supervisor blast path doesn't check empty results; prints `0 dependent(s)` instead of `no node matches target`

### Unfixed — Security

- [ ] **SEC-4** — `cli/src/commands/self_update.rs:123` — `MNEME_RELEASE_PUBKEY: Option<&str> = None`; supply-chain protection is inert
- [ ] **SEC-6** — `cli/src/commands/self_update.rs:1733` — `clear_macos_quarantine` invokes `xattr` via PATH lookup; vulnerable to PATH-prepend attack

### Unfixed — Performance

- [ ] **PERF-001** — `cli/src/commands/call_graph.rs:175` — BFS issues one SQL query per visited node — N+1 on edges table
- [ ] **PERF-002** — `cli/src/commands/call_graph.rs:232` — Enrichment loop fires one `query_row` per visited node
- [ ] **PERF-003** — `cli/src/commands/find_references.rs:111` — Leading-wildcard LIKE `'%::' || ?1` defeats `idx_nodes_qualified`
- [ ] **PERF-004** — `cli/src/commands/call_graph.rs:126` — Same leading-wildcard LIKE on seed resolve in `bfs_call_graph`
- [ ] **PERF-005** — `supervisor/src/api_graph/projects.rs:197` — `/api/projects` async handler does N synchronous SQLite opens + read_dir on tokio runtime
- [ ] **PERF-006** — `brain/src/embed_store.rs:148` — `upsert_many` is O(N×M) — linear scan per item against existing ids

### Unfixed — Testing gaps

- [ ] **TEST-001** — `cli/src/commands/pretool_edit_write.rs` — PreToolUse Edit/Write hook ships with zero tests; JSON shape, drain_stdin, run() return Ok
- [ ] **TEST-002** — `cli/src/commands/doctor/mcp_probe.rs` — Entire MCP doctor probe module (475 LOC, 9 functions) has no tests
- [ ] **TEST-003** — `cli/src/commands/doctor/daemon_probe.rs` — All four doctor daemon-probe pub fns have zero tests
- [ ] **TEST-004** — `cli/src/commands/doctor/hooks_probe.rs` — `compose_hooks_message` has 6-row truth table but zero tests
- [ ] **TEST-005** — `cli/src/commands/doctor/update_probe.rs` — `render_update_channel_box` has 5 untested match arms
- [ ] **TEST-006** — `cli/src/commands/install.rs` — 905-LOC install orchestrator has no integration test; only 4 leaf helpers covered
- [ ] **TEST-007** — `cli/src/commands/uninstall.rs:1373` — Test calls `let _ = run(args).await` discarding result; doesn't verify dry-run announcement
- [ ] **TEST-008** — `cli/src/commands/recall.rs` — Core SQL paths (recall_fts, recall_like, recall_semantic, NUL rejection, missing-index) have no tests
- [ ] **TEST-009** — `cli/src/commands/pretool_grep_read.rs` — SEC-001/006/007 stdin/config caps + homoglyph fix have no regression tests
- [ ] **TEST-010** — `store/src/lifecycle.rs` — Entire LIFECYCLE trait implementation (10 async methods, 417 LOC) has zero tests
- [ ] **TEST-011** — `supervisor/src/watchdog.rs` — Watchdog (force-kill on heartbeat miss, restart loop) has no tests

### Unfixed — Reliability

- [ ] **REL-NEW-A** — `supervisor/src/watchdog.rs:212` — `self_test_pass` never probes worker `/health` endpoint despite doc comment; wedged-but-PID-alive workers invisible
- [ ] **REL-NEW-B** — `supervisor/src/lib.rs:819` — Router task does sync `std::fs::read_to_string` on tokio runtime thread without timeout
- [ ] **REL-NEW-C** — `supervisor/src/job_queue.rs:295` — Job has no per-job retry counter; poison-pill file requeues forever, takes pool to Dead in ~5 min
- [ ] **REL-NEW-D** — `supervisor/src/lib.rs:543` — `shutdown_all` uses `?` — propagating early aborts background-task join + cleanup; orphan IPC pipe

---

## 🟡 Medium (74)

### Error handling

- [ ] **ERR-25** — `cli/src/commands/build.rs:4434` — `Err(_) => Vec::new()` on file metadata reads; SQLITE_BUSY masquerades as zero files
- [ ] **ERR-26** — `brain/src/main.rs:231` — `let _ = tracing_subscriber_init()` swallowed; brain runs without logging
- [ ] **ERR-27** — `brain/src/main.rs:234` — `Box<dyn Error>` erases concrete error type for tracing init
- [ ] **ERR-28** — `supervisor/src/main.rs:218` — Same as ERR-26 for supervisor; child crashes invisible
- [ ] **ERR-29** — `scanners/src/main.rs:934` — Same as ERR-26 for scanner; audit-scan failures opaque
- [ ] **ERR-30** — `cli/src/commands/self_update.rs:1733` — macOS quarantine clear via `xattr` silently dropped on failure
- [ ] **ERR-31** — `cli/src/commands/build.rs:3527` — `Err(_) => continue` on path canonicalize silently skips files in git stats
- [ ] **ERR-32** — `cli/src/skill_matcher.rs:134` — Bare `Err(_) => return out` on skill file read masks TOML parse / UTF-8 / permission errors
- [ ] **ERR-33** — `cli/src/commands/doctor/models_probe.rs:130` — `let _ = conn.execute_batch(DDL)` silently swallowed; downstream COUNT lies
- [ ] **ERR-34** — `supervisor/src/update_check.rs:469` — `Err(_) => return` on serde_json serialization; banner re-fires every launch
- [ ] **ERR-35** — `cli/src/commands/pretool_edit_write.rs:44` — `let _ = drain_stdin()` swallowed in PreToolUse hook
- [ ] **ERR-36** — `cli/src/commands/build.rs:5768` — `Err(_) => return` on file read in secrets-scanning pass; permission-denied = unscanned
- [ ] **ERR-37** — `supervisor/src/job_queue.rs:348` — `let _ = waker.send(outcome)` silently dropped when receiver gone
- [ ] **ERR-38** — `parsers/src/main.rs:175` — `let _ = stdout.write_all(b"\n").await` parser stdout drops; queue drain hang
- [ ] **ERR-39** — `scanners/src/main.rs:705` — `let _ = buf_out.write_all(&bytes).await` scanner stdout drops; CPU waste

### UX

- [ ] **UX-16** — `cli/src/main.rs:484` — Version-available notice prints to stderr on every command; pollutes piped output
- [ ] **UX-17** — `cli/src/commands/doctor/mod.rs:301` — 57-char-wide ASCII box overflows narrow TTYs; no max-width constraint
- [ ] **UX-18** — `vision/src/App.tsx:341` — 150ms view-switch debounce applied to keyboard nav; arrow-key feels laggy
- [ ] **UX-19** — `vision/src/App.tsx:363` — Sidebar nav has no group-aware keyboard navigation; Tab moves through 14 items linearly
- [ ] **UX-20** — `cli/src/commands/recall.rs:532` — Output uses `[kind]` and `qualified_name` — most users don't know what qualified_name is
- [ ] **UX-21** — `vision/src/views/HeatmapGrid.tsx:182` — Error state uses `role=status` instead of `role=alert`; screen readers don't announce immediately
- [ ] **UX-22** — `cli/src/commands/build.rs:0` — `mneme build` has no Ctrl+C progress summary; partial completion not surfaced
- [ ] **UX-23** — `vision/src/command-center/CommandCenter.tsx:91` — Files panel silently truncates to 50; add `+N more` disclosure
- [ ] **UX-24** — `cli/src/commands/log.rs:1` — `mneme log` has no startup hint about disk log location

### Super-tester (CLI)

- [ ] **CLI-2** — `cli/src/commands/why.rs:28` — `mneme why` accepts empty / NUL / whitespace queries; other commands reject
- [ ] **CLI-6** — `cli/src/commands/blast.rs:67` — `--deep` silently dropped when `--depth=1` passed explicitly; comment lies
- [ ] **CLI-7** — `cli/src/commands/blast.rs:200` — Supervisor-served blast loses Windows long-path prefix matching; daemon-state-dependent zero results
- [ ] **CLI-8** — `cli/src/commands/audit.rs:415-423` — Cross-shard orphan banner threshold ≥25 hides 1-24 orphans behind table-only signal
- [ ] **CLI-9** — `cli/src/commands/audit.rs:81` — `FINDINGS_FLUSH_BUFFER=100` no env override; can't tune fleet-wide
- [ ] **CLI-11** — `cli/src/commands/daemon.rs:81` — `daemon op` is `String` not Subcommand; typos return bare error, no help enumeration
- [ ] **CLI-13** — `cli/src/commands/self_update.rs:980` — Download progress uses `eprintln!` newlines instead of in-place updates; floods logs or stays silent
- [ ] **CLI-16** — `cli/src/commands/history.rs:130-133` — Header printed inside row loop instead of before; fragile under refactor
- [ ] **CLI-17** — `cli/src/commands/history.rs:51` — Comment says supervisor-IPC removed but no SQLITE_BUSY retry on tasks.db
- [ ] **CLI-19** — `cli/src/commands/view.rs:99-105` — Path-3 fallthrough message tells user to install Tauri without explaining how
- [ ] **CLI-20** — `cli/src/commands/audit.rs:134` — `--wait` doc-comment doesn't mention exit-code asymmetry; CI gates without `--wait` are no-ops
- [ ] **CLI-22** — `cli/src/commands/recall.rs:208` — When supervisor returns 0 hits AND embedding fallback fails, error dropped silently
- [ ] **CLI-25** — `cli/src/commands/audit.rs:116` — `--scope` and `--severity` accept any string; defer validation to runtime, no clap enumeration
- [ ] **CLI-26** — `cli/src/commands/find_references.rs:137` — IN clause `vec!["?"; n].join(",")` — no upper bound on `targets.len()` (defensive)
- [ ] **CLI-27** — `cli/src/commands/models.rs:246-253` — `models install` no-op when marker exists but model files missing; gas-lights user

### Security

- [ ] **SEC-8** — `store/src/ipc.rs:176` — Windows named pipe created with default ACL; same-user processes can connect
- [ ] **SEC-9** — `supervisor/src/ipc.rs:1232` — `is_existing_project_dir` validates only path existence, not registry; timing oracle for path hash
- [ ] **SEC-10** — `supervisor/src/ipc.rs:1105` — No rate limit on hook write IPC commands; 256 concurrent + 30s timeout = saturation vector
- [ ] **SEC-11** — `cli/src/commands/post_tool.rs:115` — `result_file` PathBuf accepted over IPC unvalidated; future readers may follow attacker-controlled path
- [ ] **SEC-12** — `supervisor/src/update_check.rs:279` — Background `update_check` uses default webpki-roots; no CA pinning vs self_update's pinned 3-CA
- [ ] **SEC-13** — `supervisor/src/update_check.rs:200` — `tag_name` from GitHub API stored + displayed without ANSI escape stripping; terminal injection vector

### Performance

- [ ] **PERF-007** — `brain/src/embed_store.rs:161` — `remove` uses `Vec::drain(off..off+DIM)` shifting every later vector; 76 MB memmove per delete
- [ ] **PERF-008** — `brain/src/embeddings.rs:388` — LRU `cache_put` linear-scans order deque on every cache hit
- [ ] **PERF-009** — `cli/src/commands/recall.rs:336` — Semantic recall full-scans embeddings table — unbounded RAM + linear cosine
- [ ] **PERF-010** — `cli/src/commands/recall.rs:388` — Per-hit `query_row` against graph.db for display enrichment
- [ ] **PERF-011** — `cli/src/commands/recall.rs:486` — `recall_like` double `LIKE '%query%'` on two columns — full table scan
- [ ] **PERF-012** — `supervisor/src/api_graph/graph.rs:482` — `insert_into_tree` linear `iter().position()` per path segment; quadratic in fan-out
- [ ] **PERF-013** — `brain/src/retrieve.rs:285` — `GraphIndex::two_hop` clones every neighbour label twice per anchor
- [ ] **PERF-014** — `scanners/src/scanners/architecture.rs:266` — `degree_top_k` clones every edge endpoint string into degree map
- [ ] **PERF-015** — `store/src/query.rs:246` — Reader connection acquisition holds RwLock writer for entire `pool::build()`
- [ ] **PERF-016** — `supervisor/src/api_graph/graph.rs:568` — kind-flow / domain-flow GROUP BY scan unbounded by node count

### Testing gaps

- [ ] **TEST-012** — `supervisor/src/manager.rs` — ChildManager has 21 pub fns and 1 inline test (`windows_kill_pid_flags`)
- [ ] **TEST-013** — `brain/src/llm.rs` — LLM helpers (`extract_concepts`, `route_query`, `summarize_function`) have 0 tests
- [ ] **TEST-014** — `brain/src/concept_store.rs` — ConceptStore has 8 pub methods/fns and 0 tests
- [ ] **TEST-015** — `brain/src/worker.rs` — `spawn_worker` (BUG-A2-043 flush, BUG-A2-044 timeout) has no tests
- [ ] **TEST-016** — `store/src/integrity.rs` — 9 of 10 cross-shard relationships in audit matrix uncovered
- [ ] **TEST-017** — `store/src/secrets_redact.rs` — 5 of 8 secret patterns (Anthropic, OpenAI, GitHub, Slack, capture-group) lack direct tests
- [ ] **TEST-018** — `cli/src/commands/userprompt_submit.rs` — `classify_prompt_intent` 40-char boundary + malformed-JSON branch untested
- [ ] **TEST-019** — `cli/src/commands/self_update.rs` — `health_check_new_binary` only `missing binary` tested; timeout + non-zero-exit branches uncovered
- [ ] **TEST-020** — `supervisor/src/api_graph/projects.rs` — Project-discovery HTTP route has zero tests across 5 functions

### Reliability

- [ ] **REL-NEW-E** — `supervisor/src/manager.rs:449` — `monitor_child` swallows `wait()` error with no restart trigger; worker stranded forever
- [ ] **REL-NEW-F** — `supervisor/src/manager.rs:421` — stdout/stderr forwarder tasks have no error handling on `push_raw` or unbounded line size
- [ ] **REL-NEW-G** — `cli/src/commands/self_update.rs:1466` — Rollback after health-check failure swallows every `fs::rename` error; bricked install silent
- [ ] **REL-NEW-H** — `supervisor/src/manager.rs:1196` — `dispatch_job` holds per-worker stdin Mutex for full 10s timeout, serialising the pool
- [ ] **REL-NEW-I** — `supervisor/src/ipc.rs:2178` — `write_response` has no timeout — slow client stalls per-connection task; semaphore exhaustion
- [ ] **REL-NEW-J** — `supervisor/src/manager.rs:909` — Backoff `tokio::time::sleep` not interruptible by shutdown; graceful stop waits up to max_backoff
- [ ] **REL-NEW-K** — `cli/src/commands/self_update.rs:1614` — `health_check_new_binary` uses `std::thread::sleep` inside CLI async context
- [ ] **REL-NEW-L** — `supervisor/src/manager.rs:936` — `shutdown_all` aborts monitor tasks but doesn't await child exit; orphaned workers possible

### Pragmatic

- [ ] **REVIEW-3** — `cli/src/commands/history.rs:176` — `collect_rows` uses `filter_map(|r| r.ok())`; silently drops per-row rusqlite errors
- [ ] **REVIEW-4** — `cli/src/commands/status.rs:96` — `daemon_alive` reads `probes.last()` — brittle coupling to insertion order

---

## 🟢 Low (22)

### Error handling

- [ ] **ERR-40** — `cli/src/platforms/mod.rs:910` — `let _ = std::fs::copy(&timestamped, &legacy)` legacy manifest copy silent
- [ ] **ERR-41** — `cli/src/platforms/mod.rs:943` — `let _ = f.sync_all()` fsync after manifest write silent
- [ ] **ERR-42** — `cli/src/commands/uninstall.rs:1373` — `let _ = run(args).await` in test helper; pattern leaks to production
- [ ] **ERR-43** — `brain/src/worker.rs:169` — `let _ = std::fs::create_dir_all(parent)` pending-flush marker dir silent

### UX

- [ ] **UX-25** — `cli/src/commands/why.rs:23` — `mneme why` requires query in quotes for multi-word; help doesn't enforce
- [ ] **UX-26** — `vision/src/components/OnboardingHint.tsx:37` — Onboarding hint only on Force Galaxy; 13 other views have no first-visit guidance
- [ ] **UX-27** — `cli/src/commands/status.rs:93` — Probe labels use internal IDs (`http :7777`, `ipc roundtrip`) meaningless to non-developers
- [ ] **UX-28** — `vision/src/views/index.ts:68` — Description strings expose library jargon (Sigma.js, deck.gl, D3) via title tooltips
- [ ] **UX-29** — `cli/src/commands/audit.rs:807` — Audit summary column header `scanner` should be `domain`/`category`
- [ ] **UX-30** — `vision/src/views/ForceGalaxy.tsx:13` — 2-3s first-paint window has no in-component progress indicator

### Super-tester (CLI)

- [ ] **CLI-15** — `cli/src/commands/snap.rs:60-67` — VACUUM INTO embeds path via `format!`; lossy `display()` mismatches fs::metadata reads
- [ ] **CLI-18** — `cli/src/commands/view.rs:268` — URL scheme validator rejects uppercase `HTTP://`; misleading error message
- [ ] **CLI-21** — `cli/src/commands/why.rs:207` — Git SHA byte-slicing `&g.sha[..g.sha.len().min(10)]` assumes ASCII; defensive fragility
- [ ] **CLI-24** — `cli/src/commands/uninstall.rs:483` — `std::process::exit(0)` in `--purge-state` skips Drop / lock cleanup
- [ ] **CLI-28** — `cli/src/commands/inject.rs:705-708` — File-token extension heuristic accepts `.123` as extension; spurious queries

### Security

- [ ] **SEC-14** — `cli/src/commands/post_tool.rs:170` — `spool_to_temp` uses nanosecond timestamps as uniqueness; symlink race + cross-user readable
- [ ] **SEC-15** — `release/bootstrap-install.ps1:68` — `MNEME_VERSION` regex passes long values; URL-length abuse / DNS noise possible

### Performance

(none ranked low; PERF-007 through PERF-016 all ranked medium)

### Testing gaps

- [ ] **TEST-021** — `cli/tests/rebuild_integration.rs` — Rebuild round-trip integration test permanently `#[ignore]`'d; never runs in CI
- [ ] **TEST-022** — `cli/src/commands/install.rs` — `claude_code_likely_running` tested for "never panics" only; boolean not asserted
- [ ] **TEST-023** — `cli/src/commands/recall.rs` — `EMBED_WARNED` OnceLock idempotency has no test
- [ ] **TEST-024** — `cli/src/commands/uninstall.rs:1316-1335` — `dry_run_with_unknown_platform_returns_error` siblings only assert `is_err()`; variant uncovered

### Reliability

(REL-NEW-J/K/L counted as medium above; the residual_risks section in the agent report contained no separate ranked-low items)

### Pragmatic

- [ ] **REVIEW-5** — `cli/src/main.rs:64` — MCP tool count in `--help` (49) disagrees with README.md (50); 48 tool files actually present

---

## 📋 Test gaps note

The 24 Testing-gaps findings above are catalogued as gaps (not bugs) — they identify modules / functions that ship without test coverage, so a regression introduced anywhere in those surfaces would not be caught by CI. They are tracked in this report alongside the bug findings because closing each gap is an explicit follow-up unit of work, not because they represent runtime defects.

---

## False-positive log

Pre-fix cross-check confirmed three of the originally-flagged Critical Error-handler items are NOT production bugs. They have been closed (with `[x] (commit-sha)`) in the Critical section above for accounting clarity, and annotated in source with the FALSE-POSITIVE rationale rather than being mechanically rewritten:

- **ERR-2** — `cli/src/commands/build.rs:8443` — inside `#[cfg(test)] mod empty_shard_tests`; `.expect("open shard ro")` is acceptable in test failure
- **ERR-3** — `cli/src/commands/build.rs:8454` — inside `#[cfg(test)] mod empty_shard_tests`; `let _ = ...await;` swallows are intentional, downstream `count_rows` assertion fails if seed didn't land
- **ERR-8** — `supervisor/src/health.rs:1575` — line 1575 is a test helper; the production health server at line 314 is already error-handled

A fourth false-positive class was identified for the Pragmatic batch:

- **CLI-14** is also REVIEW-equivalent — the em-dash had already been replaced in the pre-audit batch (commit `d73af4d`), so this finding shipped already-green.

---

## Files audited (top 30 by finding density)

- `cli/src/commands/build.rs` — ERR-2, ERR-3, ERR-9, ERR-10, ERR-14, ERR-15, ERR-24, ERR-25, ERR-31, ERR-36, UX-22
- `cli/src/commands/self_update.rs` — ERR-4, ERR-5, ERR-30, SEC-4, SEC-5, SEC-6, CLI-12, CLI-13, REL-NEW-G, REL-NEW-K, TEST-019
- `supervisor/src/ipc.rs` — ERR-12, SEC-1, SEC-2, SEC-3, SEC-9, SEC-10, REL-NEW-I
- `supervisor/src/manager.rs` — TEST-012, REL-NEW-E, REL-NEW-F, REL-NEW-H, REL-NEW-J, REL-NEW-L
- `cli/src/commands/recall.rs` — UX-5, UX-20, CLI-5, CLI-22, PERF-009, PERF-010, PERF-011, TEST-008, TEST-023
- `cli/src/commands/audit.rs` — UX-6, UX-29, CLI-8, CLI-9, CLI-10, CLI-20, CLI-25
- `cli/src/commands/uninstall.rs` — ERR-16, ERR-42, UX-4, CLI-24, TEST-007, TEST-024
- `cli/src/commands/doctor/mod.rs` — UX-7, UX-10, UX-17
- `cli/src/commands/doctor/mcp_probe.rs` — ERR-21, TEST-002
- `cli/src/commands/doctor/daemon_probe.rs` — TEST-003
- `cli/src/commands/doctor/hooks_probe.rs` — TEST-004
- `cli/src/commands/doctor/update_probe.rs` — TEST-005
- `cli/src/commands/doctor/models_probe.rs` — ERR-33
- `cli/src/commands/install.rs` — UX-2, TEST-006, TEST-022
- `cli/src/commands/blast.rs` — UX-13, CLI-6, CLI-7, CLI-23
- `cli/src/commands/call_graph.rs` — PERF-001, PERF-002, PERF-004
- `cli/src/commands/find_references.rs` — CLI-14, CLI-26, PERF-003
- `cli/src/commands/status.rs` — ERR-6, UX-9, UX-27, CLI-3, REVIEW-4
- `cli/src/commands/why.rs` — UX-25, CLI-2, CLI-21
- `cli/src/commands/log.rs` — ERR-11, UX-24, CLI-4
- `cli/src/commands/history.rs` — UX-8, CLI-16, CLI-17, REVIEW-3
- `cli/src/commands/godnodes.rs` — REVIEW-1
- `cli/src/commands/export.rs` — REVIEW-2
- `cli/src/commands/update.rs` — UX-1, CLI-1
- `cli/src/commands/models.rs` — UX-15, CLI-27
- `cli/src/commands/snap.rs` — CLI-15
- `cli/src/commands/view.rs` — CLI-18, CLI-19
- `cli/src/commands/inject.rs` — CLI-28
- `cli/src/commands/daemon.rs` — CLI-11
- `cli/src/commands/post_tool.rs` — SEC-11, SEC-14
- `cli/src/commands/pretool_edit_write.rs` — ERR-35, TEST-001
- `cli/src/commands/pretool_grep_read.rs` — TEST-009
- `cli/src/commands/userprompt_submit.rs` — TEST-018
- `cli/src/skill_matcher.rs` — ERR-32
- `cli/src/build_lock.rs` — ERR-1
- `cli/src/main.rs` — UX-16, REVIEW-5
- `cli/tests/rebuild_integration.rs` — TEST-021
- `supervisor/src/health.rs` — ERR-8
- `supervisor/src/ws.rs` — ERR-23
- `supervisor/src/watcher.rs` — ERR-13
- `supervisor/src/watchdog.rs` — REL-NEW-A, TEST-011
- `supervisor/src/lib.rs` — REL-NEW-B, REL-NEW-D
- `supervisor/src/job_queue.rs` — ERR-37, REL-NEW-C
- `supervisor/src/update_check.rs` — ERR-7, ERR-34, SEC-12, SEC-13
- `supervisor/src/api_graph/graph.rs` — SEC-7, PERF-012, PERF-016
- `supervisor/src/api_graph/projects.rs` — ERR-17, PERF-005, TEST-020
- `brain/src/embeddings.rs` — ERR-19, PERF-008
- `brain/src/embed_store.rs` — PERF-006, PERF-007
- `brain/src/llm.rs` — ERR-20, TEST-013
- `brain/src/main.rs` — ERR-26, ERR-27
- `brain/src/worker.rs` — ERR-43, TEST-015
- `brain/src/concept_store.rs` — TEST-014
- `brain/src/retrieve.rs` — PERF-013
- `store/src/query.rs` — ERR-18, PERF-015
- `store/src/ipc.rs` — SEC-8
- `store/src/lifecycle.rs` — TEST-010
- `store/src/integrity.rs` — TEST-016
- `store/src/secrets_redact.rs` — TEST-017
- `scanners/src/main.rs` — ERR-22, ERR-29, ERR-39
- `scanners/src/scanners/architecture.rs` — PERF-014
- `parsers/src/main.rs` — ERR-38
- `cli/src/platforms/mod.rs` — ERR-40, ERR-41
- `vision/src/App.tsx` — UX-3, UX-18, UX-19
- `vision/src/components/SidePanel.tsx` — UX-11
- `vision/src/components/FilterBar.tsx` — UX-12
- `vision/src/components/OnboardingHint.tsx` — UX-26
- `vision/src/views/index.ts` — UX-14, UX-28
- `vision/src/views/HeatmapGrid.tsx` — UX-21
- `vision/src/views/ForceGalaxy.tsx` — UX-30
- `vision/src/command-center/CommandCenter.tsx` — UX-23
- `release/bootstrap-install.ps1` — SEC-15
