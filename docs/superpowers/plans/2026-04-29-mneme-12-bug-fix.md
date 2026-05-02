# Mneme v0.3.2 — 12-Bug Fix Cycle (2026-04-29 home cycle)

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. **IRON RULE:** every task uses tree-sitter MCP for structural code queries (project name `mneme-final`) AND superpowers skills for process discipline. No work without both.

**Goal:** Close every bug surfaced by the 2026-04-29 install postmortem on our high-end AWS test instance, so the next install passes REAL-1 (interactive Claude Code on EC2) without manual workarounds, hook auto-spawn loops, or visible console-window storms.

**Architecture:** 12 bug-fix branches (one per bug A–L), each TDD'd with a failing test before implementation, then merged into `main` of the local git repo at `C:\Users\<USER>\Desktop\MNEME HOME\mneme final 2026-04-29\source`. After all 12 land: cargo test workspace gate, release build, package final.zip (no version bump per `feedback_mneme_dont_bump_version_unless_shipping.md` — stays 0.3.2), REAL-1 + REAL-2 on EC2.

**Tech Stack:** Rust 1.95 workspace (12 crates) + Bun 1.3 TS MCP + Tauri vision + PowerShell install scripts + tree-sitter MCP + superpowers skills.

**Hard rules (non-negotiable):**
- Tree-sitter MCP project `mneme-final` is the structural query path. Grep only for plain-text searches.
- Every fix follows RED-GREEN-REFACTOR. Failing test FIRST.
- Every test agent uses `superpowers:test-driven-development` + `superpowers:systematic-debugging` + `superpowers:verification-before-completion` skills.
- `feedback_no_severity_downgrade.md`: every bug is critical. No tier ordering, no "cheap win" framing.
- All fixes happen LOCAL at `mneme final 2026-04-29\source`. EC2 is test-only (`feedback_mneme_fix_local_test_ec2.md`).
- No version bump until REAL-1 + REAL-2 pass (`feedback_mneme_dont_bump_version_unless_shipping.md`).
- Every dispatched implementer agent: `model: opus`, `subagent_type: general-purpose`, `run_in_background: true`, full power per `feedback_agent_dispatch_full_power.md`.
- Hooks STDIN-JSON contract preserved: every hook binary exits 0 on internal error.
- 100% local invariant: no outbound network at runtime.

---

## File Structure

| Cluster | Files Touched | Bugs |
|---|---|---|
| **install.ps1** cluster | `scripts/install.ps1`, `INSTALL.md`, `VERSION.txt`, `START-HERE.md` | A, B, F, G |
| **supervisor/manager.rs** cluster | `supervisor/src/manager.rs`, `supervisor/src/child.rs` | D, J, L |
| **cli/ipc.rs** cluster | `cli/src/ipc.rs`, `common/src/worker_ipc.rs`, `mcp/src/db.ts` | E, K |
| **models** isolated | `cli/src/commands/models.rs`, `models/README.md` | C |
| **install.rs** isolated | `cli/src/commands/install.rs`, `scripts/uninstall.ps1` | H |
| **investigation** | `docs/dev/SESSION-2026-04-29-FIX-LOG.md` (new) + supervisor recovery log | I |

5 cluster branches → merge sequence install.ps1 → manager.rs → ipc.rs → models → install.rs → I. Conflicts resolved by branch order.

---

## Task A — INSTALL.md + VERSION.txt + START-HERE.md filename + LocalZip canonicalization

**Bug:** `INSTALL.md:13` references `mneme-v0.3.0-windows-x64.zip`. Following the docs verbatim makes `install.ps1` go to GitHub Releases for "latest" (returns v0.3.0 because v0.3.2 is unreleased), downloads 58.4 MB, overwrites freshly-extracted v0.3.2 with the buggy v0.3.0 — all 8 Wave 2 bugs return.

**Files:**
- Modify: `INSTALL.md:13` (the `Expand-Archive -Path mneme-v0.3.0-windows-x64.zip ...` line)
- Modify: `VERSION.txt` — add explicit `-LocalZip` step
- Modify: `START-HERE.md` — already correct, audit for consistency
- Modify: `scripts/install.ps1` — make `-LocalZip` discoverable via `--help` and the default banner; surface a hard error if the canonical zip filename is not found in CWD

**Steps:**

- [ ] **A.1 RED test (PowerShell Pester):** new `scripts/test/install-localzip-canonical.tests.ps1` — assert that `INSTALL.md` and `VERSION.txt` and `START-HERE.md` all reference exactly the **same** zip filename (`mneme-v0.3.2-windows-x64.zip`). Failure mode: `INSTALL.md` mismatches.
- [ ] **A.2 Run test, see RED.**
- [ ] **A.3 GREEN — fix `INSTALL.md:13`.** Use tree-sitter `find_text` to confirm exact line, then `Edit` to change `mneme-v0.3.0-windows-x64.zip` → `mneme-v0.3.2-windows-x64.zip`. Also wrap in canonical `-LocalZip` invocation:

```markdown
Expand-Archive -Path mneme-v0.3.2-windows-x64.zip -DestinationPath "$env:USERPROFILE\.mneme" -Force
cd "$env:USERPROFILE\.mneme"
.\scripts\install.ps1 -LocalZip "$env:USERPROFILE\.mneme\..\mneme-v0.3.2-windows-x64.zip"
```

- [ ] **A.4 Update `VERSION.txt`** to document `-LocalZip` as the canonical step 3.
- [ ] **A.5 Run test, see GREEN.**
- [ ] **A.6 Commit.** `git commit -m "fix(A): canonicalize v0.3.2 zip filename + LocalZip in install docs"`

---

## Task B — Drop --skip-hooks from install.ps1's `mneme install` invocation

**Bug:** `install.ps1` still passes `--skip-hooks` to the child `mneme install` invocation (postmortem §3.B; vm-test-results phase6_smoke confirms `settings_hook_count=0`). K1 made hooks default-on but the install.ps1 caller wasn't updated.

**Files:**
- Modify: `scripts/install.ps1` — find the `& $MnemeBin install ... --skip-hooks` invocation in the platform-registration step (~step 7/8) and remove the `--skip-hooks` flag.
- Test: `scripts/test/install-hooks-default-on.tests.ps1` (new)

**Steps:**

- [ ] **B.1 Use tree-sitter** `find_text(project="mneme-final", pattern="--skip-hooks", file_pattern="scripts/**/*.ps1")` — already located at lines 54 and 1247 (comments only). Use `find_text(pattern="install.*skip", use_regex=true)` to find the actual invocation site.
- [ ] **B.2 RED test:** assert install.ps1 does NOT pass `--skip-hooks` to `mneme install`. Pester regex check on the file body excluding comment lines.
- [ ] **B.3 Run, see RED.**
- [ ] **B.4 GREEN.** Remove the flag from the invocation line. Update the post-install banner to say "Hooks registered (8/8) — persistent-memory pipeline live" instead of the warning block.
- [ ] **B.5 Run test, see GREEN.** Plus `mneme doctor` smoke after a fresh install must show `hooks_registered: 8/8`.
- [ ] **B.6 Commit.** `git commit -m "fix(B): install.ps1 no longer passes --skip-hooks (K1 default-on respected)"`

---

## Task C — `mneme models install --from-path` registers ALL bundled models

**Bug:** Bundle ships 5 model files (3.5 GB total). `mneme models install --from-path C:\mneme-final\models\` registers only the BGE ONNX. The 4 GGUFs (phi-3-mini-4k 2.28 GB, qwen-coder-0.5b 469 MB, qwen-embed-0.5b 609 MB) silently skipped. Even BGE inert without `--features real-embeddings` rebuild + `onnxruntime.dll` on PATH.

**Files:**
- Modify: `cli/src/commands/models.rs::install_from_path` — walk dir, recognise `.onnx`, `.gguf`, `.ggml`, `.bin`, `tokenizer.json`. Write per-model manifest entries to `~/.mneme/models/manifest.json`.
- Create: `models/README.md` (in bundle source) — document the install behavior + `onnxruntime.dll` requirement
- Test: `cli/src/commands/models.rs` `#[cfg(test)] mod tests` — fixture dir with all 5 files, assert all 5 registered.

**Steps:**

- [x] **C.1 Tree-sitter probe:** `get_symbols(file_path="cli/src/commands/models.rs")` to map current functions. Then `get_file` for the install_from_path body.
- [x] **C.2 RED test:** new `models_install_from_path_registers_all_bundled_files` — fixture tempdir with 5 fake files (BGE.onnx 100 KB, tokenizer.json 1 KB, phi-3-mini-4k.gguf 100 KB, qwen-coder-0.5b.gguf 100 KB, qwen-embed-0.5b.gguf 100 KB). Call install_from_path. Assert manifest.json has 5 entries with detected `kind` (embedding, tokenizer, llm, llm, embedding).
- [x] **C.3 Run test, see RED** — current implementation only catches BGE.
- [x] **C.4 GREEN.** Implement: read_dir, for each `*.onnx|*.gguf|*.ggml|*.bin|tokenizer.json`, append manifest row. Detect `kind`: `*.onnx` → embedding-model, `tokenizer.json` → embedding-tokenizer, `*.gguf|*.ggml` with `embed` in name → embedding-llm, `*.gguf|*.ggml` else → llm. Update `mneme doctor` to show all registered models per kind.
- [x] **C.5 Run, see GREEN.**
- [x] **C.6 Document `onnxruntime.dll` requirement.** Added `mneme models install-onnx-runtime` subcommand stub (v0.3.3 will auto-fetch + sha256-verify); `models/README.md` documents the manual install procedure for Windows/Linux/macOS. Auto-fetch deferred to v0.3.3 to land alongside the explicit-opt-in `--from-url` network path.
- [x] **C.7 Commit.** `60d79ab fix(C): models install --from-path registers all bundled GGUFs + ONNX` (single commit; models/README.md + onnx subcommand stub bundled in).

---

## Task D — `CREATE_NO_WINDOW` on worker spawns

**Bug:** 22 worker processes spawn visible cmd.exe windows on every supervisor boot (postmortem §3.D + §12.5 — the "hydra heads"). Daemon spawn has the right flags via `cli/src/commands/uninstall.rs:448-449`. Worker spawn at `supervisor/src/manager.rs:231 (spawn_os_process)` does NOT.

**Files:**
- Modify: `supervisor/src/manager.rs::spawn_os_process` (line 231) — add `#[cfg(windows)] command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB | CREATE_NO_WINDOW)`
- Test: `supervisor/src/tests.rs` (new test) — spawn a worker, assert no console window via Win32 `GetConsoleWindow()` returns 0 in child, OR check `STARTUPINFO::dwFlags` has `STARTF_USESHOWWINDOW` and `wShowWindow == SW_HIDE`.

**Steps:**

- [ ] **D.1 Tree-sitter:** `get_file(path="supervisor/src/manager.rs", start_line=170, max_lines=80)` to read the spawn_child + spawn_os_process bodies.
- [ ] **D.2 RED test:** `spawn_os_process_uses_create_no_window_on_windows` — spawn a small test exe (a stub binary that writes its `GetConsoleWindow()` return to stdout), assert output is `0` (no console).
- [ ] **D.3 Run test, see RED.**
- [ ] **D.4 GREEN.** Add `#[cfg(windows)] use std::os::windows::process::CommandExt;` and `command.creation_flags(0x00000008 | 0x00000200 | 0x01000000 | 0x08000000);` (DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB | CREATE_NO_WINDOW).
- [ ] **D.5 Run, see GREEN.**
- [ ] **D.6 Manual smoke (host):** start daemon, `Start-Process` watch — count visible console windows = 0.
- [ ] **D.7 Commit.** `git commit -m "fix(D): worker spawns use CREATE_NO_WINDOW (no more 22-window storm)"`

---

## Task E — Hooks must NOT auto-spawn the daemon

**Bug:** Hook commands (`mneme pre-tool`, `post-tool`, `inject`, `turn-end`, `session-prime`, `session-end`, `session-start`, `pre-compact`, `subagent`) call `IpcClient::request` via the default path which auto-spawns `mneme-daemon` on connect-failure (`cli/src/ipc.rs:484-502`, `spawn_daemon_detached` at line 612). The current code at `cli/src/commands/build.rs:4358` says "hooks WANT auto-spawn" — that decision is **wrong** and is the root of the resurrection loop. With bug D this produced the "hydra heads."

**Files:**
- Modify: `cli/src/commands/{pre_tool,post_tool,inject,turn_end,session_prime,session_end,session_start,pre_compact}.rs` — every hook constructs `IpcClient::default_path().with_no_autospawn()` and silent-fails on `Err(CliError::Ipc)`.
- Modify: `cli/src/hook_payload.rs` — helper `try_hook_ipc<F>(f: F)` wraps any IPC call, swallows connection errors, exits 0.
- Test: `cli/tests/hook_no_autospawn.rs` (new) — for each hook binary, set socket path to bogus, invoke via `Command`, assert exit code 0 + no `mneme-daemon` spawned in the process tree (Windows: `Get-Process mneme-daemon` count unchanged).

**Steps:**

- [x] **E.1 Tree-sitter:** `find_usage(symbol="make_client")` in cli/src/commands/, `find_text(pattern="IpcClient::default", file_pattern="cli/src/commands/*.rs")` to enumerate hook IPC call sites.
- [x] **E.2 RED test:** hook_no_autospawn_when_pipe_missing — spawn `mneme pre-tool` with `MNEME_IPC=\\.\pipe\mneme-bogus-7777`, assert exit 0, assert no mneme-daemon process exists post-call.
- [x] **E.3 Run, see RED** — current behaviour spawns daemon.
- [x] **E.4 GREEN.** Build a `try_hook_ipc()` helper that constructs `IpcClient::default_path().with_no_autospawn().with_timeout(Duration::from_millis(500))` and on Err(Ipc) returns Ok(()) silently. Refactor every hook command to use it.
- [x] **E.5 Update build.rs:4358 comment** — the rationale comment must flip to: "Hooks NEVER auto-spawn. The supervisor not being up means mneme is intentionally inactive; the user runs `mneme daemon start` to activate it."
- [x] **E.6 Run test, see GREEN.**
- [x] **E.7 Manual smoke (host):** kill daemon, run `claude --print "hi"` — `Get-Process mneme*` returns nothing during AND after. No console windows flash.
- [x] **E.8 Commit.** `git commit -m "fix(E): hooks no-op silently when daemon down (kills the resurrection loop)"`

---

## Task F — Reorder install.ps1 — link.exe pre-check before `cargo install tauri-cli`

**Bug:** `install.ps1:693-711` runs `cargo install tauri-cli --locked --version "^2.0"` (3-5 min, 560 crates, ~53 MB download) BEFORE checking link.exe / cl.exe. On a machine without MSVC Build Tools, the install spends 5 min downloading then fails at link stage with `linker 'link.exe' not found`.

**Files:**
- Modify: `scripts/install.ps1` — add `Test-MsvcLinker` function near the top, call it before the G4 cargo install block. If missing, write a clear "MSVC missing, skipping Tauri (install via `winget install Microsoft.VisualStudio.2022.BuildTools` then re-run install.ps1)" warning and skip.
- Test: `scripts/test/install-msvc-precheck.tests.ps1` (new) — Pester test mocks `Get-Command link.exe -ErrorAction SilentlyContinue` to return null, asserts the cargo install block is NOT entered.

**Steps:**

- [ ] **F.1 Tree-sitter:** `find_text(pattern="cargo.*install.*tauri", file_pattern="scripts/install.ps1")` — confirmed line 695. Read surrounding context with `get_file`.
- [ ] **F.2 RED test:** Pester mock `Get-Command link.exe` → $null, run install.ps1 -DryRun -NoToolchain:$false, assert tauri-cli install was NOT attempted.
- [ ] **F.3 Run, see RED.**
- [ ] **F.4 GREEN.** Add helper:
```powershell
function Test-MsvcLinker {
    $linkExe = Get-Command link.exe -ErrorAction SilentlyContinue
    $clExe   = Get-Command cl.exe -ErrorAction SilentlyContinue
    return ($linkExe -ne $null -and $clExe -ne $null)
}
```
Wrap line 693+ in `if (Test-MsvcLinker) { ... } else { Write-Warn "[G4] MSVC link.exe missing — skipping cargo install tauri-cli. Install: winget install Microsoft.VisualStudio.2022.BuildTools" }`.
- [ ] **F.5 Run, see GREEN.**
- [ ] **F.6 Commit.** `git commit -m "fix(F): install.ps1 pre-checks MSVC link.exe before cargo install tauri-cli"`

---

## Task G — Replace stale SQLite portable URL

**Bug:** `install.ps1:720` hardcodes `https://www.sqlite.org/2025/sqlite-tools-win-x64-3470100.zip` — postmortem §3.G captured live `(404) Not Found`.

**Files:**
- Modify: `scripts/install.ps1:720` — switch primary path to `winget install SQLite.SQLite`, fall back to a HEAD-probed canonical sqlite.org URL.
- Test: `scripts/test/install-sqlite-fallback.tests.ps1` (new) — when winget returns success, assert no portable download attempted; when winget fails, assert HEAD probe is run before download.

**Steps:**

- [ ] **G.1 Tree-sitter probe:** confirmed `$sqliteUrl` at install.ps1:720.
- [ ] **G.2 RED test:** Pester run with mocked winget returning 0 — assert sqliteUrl block NOT entered.
- [ ] **G.3 Run, see RED.**
- [ ] **G.4 GREEN.** Restructure G7 block:
```powershell
if (-not (Get-Command sqlite3 -ErrorAction SilentlyContinue)) {
    # Primary path: winget
    $wingetResult = & winget install --id SQLite.SQLite --silent --accept-source-agreements --accept-package-agreements 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-OK "[G7] SQLite installed via winget"
    } else {
        # Fallback: portable zip with HEAD probe
        $sqliteUrls = @(
            'https://www.sqlite.org/2026/sqlite-tools-win-x64-3490000.zip',
            'https://www.sqlite.org/2025/sqlite-tools-win-x64-3470100.zip'
        )
        foreach ($url in $sqliteUrls) {
            try {
                $head = Invoke-WebRequest -Uri $url -Method Head -ErrorAction Stop
                if ($head.StatusCode -eq 200) { $sqliteUrl = $url; break }
            } catch { continue }
        }
        if ($sqliteUrl) { ... } else { Write-Warn "[G7] SQLite missing — install manually" }
    }
}
```
- [ ] **G.5 Run, see GREEN.**
- [ ] **G.6 Commit.** `git commit -m "fix(G): SQLite install via winget primary, HEAD-probed portable fallback"`

---

## Task H — `drop_standalone_uninstaller` reliability + integration test

**Bug:** Postmortem §6 — file did not exist after install on the AWS test fleet. Code at `cli/src/commands/install.rs:574` is wired but the function may have failed silently (line 244 `warn!(error = %e, ...)` swallows).

**Files:**
- Modify: `cli/src/commands/install.rs::drop_standalone_uninstaller` — fail loud (return error to caller), confirm `include_str!("../../../scripts/uninstall.ps1")` resolves at compile time, write to `~/.mneme/uninstall.ps1` with verified bytes.
- Modify: `cli/src/commands/install.rs:243` — propagate the error instead of warn-and-continue.
- Test: `cli/tests/install_writes_standalone_uninstaller.rs` (new integration) — run a stub install in a tempdir, assert `<tempdir>/.mneme/uninstall.ps1` exists with size > 5000 bytes.

**Steps:**

- [x] **H.1 Tree-sitter:** `get_file(path="cli/src/commands/install.rs", start_line=570, max_lines=100)` to read the full function body.
- [x] **H.2 RED test:** integration test creates fake home, runs install, asserts file exists. Currently this passes IF code path is reached, fails IF the function silently errored.
- [x] **H.3 Run, see baseline.** Diagnose silent failure by changing the warn to return Err.
- [x] **H.4 GREEN.** Make function fail loud + add a second post-install verification: read back the written file, sha256-compare against the include_str! constant.
- [x] **H.5 Run, see GREEN.**
- [x] **H.6 Commit.** `git commit -m "fix(H): drop_standalone_uninstaller fails loud + content-verifies the write"` — landed as `b11a8d9` on `fix/install-drop-standalone-uninstaller`. Diagnosis: silent `warn!` swallow + `dirs::home_dir()` ignoring `USERPROFILE` on Windows (uses Win32 `SHGetKnownFolderPath`). Fix: `MNEME_HOME` is now consulted first, then dir creation, then post-write byte-equality check, then `?`-propagation through new `CliError::DropUninstaller`.

---

## Task I — Worker startup crash (-1073741510) — root cause + diagnosis

**Bug:** Postmortem §17 captured worker crash loop with exit `-1073741510` (`STATUS_CONTROL_C_EXIT`) on first daemon start. Did not reproduce after clean reinstall — most likely a v0.3.0/v0.3.2 mixed-binary scenario from bug A. Need (1) defensive version check, (2) clearer recovery log.

**Files:**
- Modify: `supervisor/src/lib.rs::run` — at boot, every worker exe path gets `--version` invoked, mismatch fails with `BinaryVersionSkew { worker: name, expected: env!("CARGO_PKG_VERSION"), actual }`.
- Modify: `supervisor/src/manager.rs` — when a worker that previously had crashes hits `restart_count == 0` for 60s, emit a tracing INFO line: "child={} recovered from crash loop after N restarts" (the missing recovery log identified in postmortem §17).
- Create: `docs/dev/SESSION-2026-04-29-FIX-LOG.md` — document the crash + the bug A linkage.

**Steps:**

- [x] **I.1 Tree-sitter:** `find_text(pattern="--version", file_pattern="supervisor/src/**/*.rs")` and `get_symbols("supervisor/src/lib.rs")` to find boot path.
- [x] **I.2 RED test:** `boot_refuses_when_worker_version_skews` (stub script prints `0.0.1`, expect `BinaryVersionSkew`) + `manager_logs_recovery_after_stable_uptime` (synth crash-loop-then-stable handle, assert one-shot recovery emit + re-emit-after-restart contract). Live in `supervisor/src/tests.rs`.
- [x] **I.3 Run, see RED.** Initial implementation lacked the boot probe + recovery flag; both tests failed.
- [x] **I.4 GREEN.** Implemented `manager::probe_worker_versions` (free fn; called from `lib::run` before `spawn_all`), `manager::ChildManager::check_recovery_logs` (per-handle one-shot via new `crash_loop_recovery_logged` field on `ChildHandle`), `lib::run_recovery_logger` (5s periodic task), and `SupervisorError::BinaryVersionSkew { worker, expected, actual }` in `supervisor/src/error.rs`.
- [x] **I.5 Document in fix log.** New `docs/dev/SESSION-2026-04-29-FIX-LOG.md` Bug-I section captures the symptom, full Five Whys (down to "no version probe at boot + no recovery log"), bug-A linkage, and the defence-in-depth design rationale.
- [x] **I.6 Commit.** Branch `fix/worker-startup-crash-defensive`, two commits: `add55cc docs(I): bug-I investigation linked to bug A` + `402adc7 fix(I): supervisor boot-time worker version check + recovery log`. `cargo check --workspace` exit 0; the three new tests (`boot_refuses_when_worker_version_skews`, `manager_logs_recovery_after_stable_uptime`, `parse_semver_via_boot_probe_consistency`) all pass.

---

## Task J — Unbounded restart channel (no silent dropped requests)

**Bug:** `supervisor/src/manager.rs:407-417` uses bounded `mpsc::channel(restart_channel_cap())` for restart requests. When workers crash faster than the restart loop drains, `TrySendError::Full` fires and request is dropped. Postmortem §12.1 captured 11 dropped restarts in 5s. The CHANGELOG v0.2.0 (line 709) promised "mpsc::UnboundedChannel<RestartRequest>" — that promise wasn't kept.

**Files:**
- Modify: `supervisor/src/manager.rs:29 (restart_channel_cap)` — delete (no longer needed).
- Modify: `supervisor/src/manager.rs::ChildManager::new` — switch to `mpsc::unbounded_channel::<RestartRequest>()`.
- Modify: `supervisor/src/manager.rs:407-417` — replace `try_send` with `send` (unbounded, never fails on full).
- Modify: `supervisor/src/manager.rs:130 (take_restart_rx)` — type updates to `UnboundedReceiver`.
- Modify: `supervisor/src/manager.rs::run_restart_loop` — receive from unbounded.
- Test: `supervisor/src/tests.rs` — new test `unbounded_restart_channel_never_drops_under_load` — spawn 50 fail-fast workers (sleep 10ms then exit code 1), assert all 50 reach restart_count >= 5 within 30 s.

**Steps:**

- [ ] **J.1 Tree-sitter:** `find_usage(symbol="restart_channel_cap")` and `find_usage(symbol="RestartRequest")` to enumerate all type references.
- [ ] **J.2 RED test:** unbounded_restart_channel_never_drops_under_load — currently fails because some workers see drops.
- [ ] **J.3 Run, see RED.**
- [ ] **J.4 GREEN.** Convert types as listed.
- [ ] **J.5 Run, see GREEN.** Plus existing `watchdog_respawns_crashed_worker` still passes.
- [ ] **J.6 Commit.** `git commit -m "fix(J): supervisor restart channel is unbounded — no silent drops under load"`

---

## Task K — Re-resolve supervisor pipe name on connect failure

**Bug:** PID-scoped pipe written to `~/.mneme/supervisor.pipe`. CLI/MCP/worker-IPC clients cache the pipe path (read once at startup). When daemon respawns with new PID, clients dial the dead pipe (postmortem §12.2 captured live evidence).

**Files:**
- Modify: `cli/src/ipc.rs::IpcClient::request` — on connect failure, call `IpcClient::default_path()` again to re-read `~/.mneme/supervisor.pipe`, retry once with the fresh path before giving up.
- Modify: `common/src/worker_ipc.rs::resolve_socket_path` — same pattern.
- Modify: `mcp/src/db.ts` — same pattern in TS.
- Test: `cli/src/ipc.rs` `#[cfg(test)] mod tests` — write fake supervisor.pipe with name X, attempt connect (fails, X dead), update file to name Y while client is mid-retry, assert second attempt uses Y.

**Steps:**

- [x] **K.1 Tree-sitter:** confirmed cite at `cli/src/ipc.rs:410`, `common/src/worker_ipc.rs:136`, `mcp/src/db.ts:42`.
- [x] **K.2 RED test:** ipc_re_resolves_pipe_name_on_connect_failure — currently fails because IpcClient caches socket_path field.
- [x] **K.3 Run, see RED.**
- [x] **K.4 GREEN.** Replace `socket_path: PathBuf` field with `socket_path_resolver: Box<dyn Fn() -> PathBuf>` OR add `refresh_socket_path()` method called on connect failure.
- [x] **K.5 Run, see GREEN.**
- [x] **K.6 Apply same pattern to `common/src/worker_ipc.rs` and `mcp/src/db.ts`** (TS version writes a sibling test).
- [x] **K.7 Commit.** `git commit -m "fix(K): IPC clients re-read ~/.mneme/supervisor.pipe on connect failure"`

---

## Task L — `restart_dropped_count` observability

**Bug:** Even after bug J fix (unbounded channel), an observability gauge is valuable. `restart_count` only counts successful respawns; dropped requests + still-pending requests are invisible.

**Files:**
- Modify: `supervisor/src/child.rs::ChildState` (line 104 area) — add `pub restart_dropped_count: u64`.
- Modify: `supervisor/src/child.rs::ChildState::new` — initialise to 0.
- Modify: `supervisor/src/manager.rs:413-415` (TrySendError::Full and ::Closed branches) — increment via `state.restart_dropped_count += 1` (these branches stay even with unbounded — Closed can still fire).
- Modify: `supervisor/src/manager.rs::ChildSnapshot` — add `restart_dropped_count: u64` field.
- Modify: `supervisor/src/health.rs::format_prometheus` — emit `mneme_child_restart_dropped_count{child="X"} N`.
- Modify: `cli/src/commands/doctor.rs::render_per_worker_box` — show `dropped=X` next to `restarts=N`.
- Test: `supervisor/src/tests.rs` — assert dropped count exposed in snapshot after channel-closed simulation.

**Steps:**

- [ ] **L.1 Tree-sitter:** `find_usage(symbol="restart_count")` — already enumerated. Mirror every site.
- [ ] **L.2 RED test:** `restart_dropped_count_visible_in_snapshot` — close the restart channel, attempt enqueue, assert `snapshot().restart_dropped_count >= 1`.
- [ ] **L.3 Run, see RED.**
- [ ] **L.4 GREEN.** Add field, increment, expose in snapshot + health + doctor.
- [ ] **L.5 Run, see GREEN.**
- [ ] **L.6 Commit.** `git commit -m "fix(L): supervisor exposes restart_dropped_count alongside restart_count"`

---

## Phase Test (after all 12 land)

- [ ] **T.1** `cargo check --workspace` (must be EXIT 0).
- [ ] **T.2** `cargo test --workspace --test-threads=1` (must be EXIT 0; expect ~300+ cli + 60+ daemon tests).
- [ ] **T.3** `cd mcp && bun install && bunx tsc --noEmit && bun test`.
- [ ] **T.4** `cd vision && bun install && bunx tsc --noEmit && bun run build`.
- [ ] **T.5** `cd vision/tauri && cargo build --release` (only if MSVC present; otherwise skip with documented gap).

## Phase Build

- [ ] **B.1** `cargo build --workspace --release` (~4 min).
- [ ] **B.2** Confirm 9 release binaries present in `target/release/`.

## Phase Package

- [ ] **P.1** `scripts/test/stage-release-zip.ps1` → `mneme-v0.3.2-windows-x64.zip` on host Desktop.
- [ ] **P.2** `scripts/test/stage-final-zip.ps1` → final.zip with everything.
- [ ] **P.3** SHA256 checksum.

## Phase REAL-1 (per `feedback_mneme_real_world_test.md`)

- [ ] **R1.1** Connect to EC2 via plink.exe -pw `<password>`.
- [ ] **R1.2** Backup `~/.claude/.credentials.json`.
- [ ] **R1.3** Wipe `~/.mneme` + `~/.bun/install/cache` + Defender exclusions + PATH entries.
- [ ] **R1.4** Restore credentials.
- [ ] **R1.5** Upload final.zip + install.ps1.
- [ ] **R1.6** Run install fresh per START-HERE.md verbatim — count visible console windows = 0.
- [ ] **R1.7** Drive `claude --print --model claude-haiku-4-5` with maintainer-proxy prompts.
- [ ] **R1.8** Assert: 0 visible windows, single mneme-daemon PID across 30 min, 22 workers steady, 48/48 MCP tools, 8/8 hooks, history.db.turns + tasks.db.ledger_entries + tool_cache.db.tool_calls all > 0.

## Phase REAL-2 (per `feedback_mneme_stress_test_protocol.md`)

- [ ] **R2.1** From zero: wipe + install + exercise 28 CLI / 47 MCP / 17 slash / 6 hooks / 26 skills / 6 agents.
- [ ] **R2.2** Concurrent + big-data + crash + lifecycle.
- [ ] **R2.3** 24h leak soak — Phase D idle-after-load. Memory must return to baseline (`feedback_leak_is_the_leak.md`).
- [ ] **R2.4** REAL-1 interactive Claude session as final gate.

---

## Self-Review

| Bug | Spec coverage | Test specified | File:line cited |
|---|---|---|---|
| A | INSTALL.md:13 + VERSION.txt + START-HERE.md | Pester | yes |
| B | install.ps1 invocation | Pester | needs find via tree-sitter |
| C | models.rs install_from_path | Rust unit | yes (need full read) |
| D | manager.rs:231 spawn_os_process | Rust integration | yes |
| E | hook commands cluster | Rust integration | yes (ipc.rs:484-502) |
| F | install.ps1:693-711 | Pester | yes |
| G | install.ps1:720 | Pester | yes |
| H | install.rs:574 + 243 | Rust integration | yes |
| I | supervisor/lib.rs run | Rust unit | needs read |
| J | manager.rs:407-417 | Rust integration | yes |
| K | ipc.rs:410, worker_ipc.rs:136, db.ts:42 | Rust unit + TS | yes |
| L | child.rs:104 + manager.rs:844 | Rust integration | yes |

No placeholders. No "TBD". Every task names exact file targets. Test names included. Commit messages drafted.

