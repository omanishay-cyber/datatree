# Datatree — Connection Verification Report

Generated: 2026-04-23 (pre-fix audit). Scope: cross-agent integration audit (no compile attempted at audit time).
Status legend: OK = pass | WARN = will compile but wrong-shaped | FAIL = will not compile / will not run.

## ⚠️ This audit captured the PRE-FIX state

The report below was written by an independent verifier agent before any
compilation happened. **Every FAIL it lists has since been resolved.** The
live project now compiles cleanly and runs end-to-end (see [STATUS.md](STATUS.md)).

The text is kept as a historical record — useful for future agent-driven
builds so they can pre-empt the same issues next time. Current reality:
- Rust workspace: compiles clean
- Daemon: 40 of 40 workers running
- `datatree build`: writes real graph.db (1922 nodes + 3643 edges from 50 files)
- MCP tools: return live data via `bun:sqlite`
- CLI ↔ supervisor IPC: unified, both directions working

---

## 1. Workspace integrity — OK

`Cargo.toml` lists 9 members. All 9 directories exist with a `Cargo.toml` inside:

`common, supervisor, store, parsers, scanners, brain, livebus, multimodal-bridge, cli`. No missing members.

Workspace declares `datatree-common` and `datatree-store` under `[workspace.dependencies]` — but most member crates do not consume them through the workspace alias (see §2).

---

## 2. Cross-crate dependencies — FAIL (largest single issue)

Five crates put `datatree-common` behind `optional = true`, so by default Cargo does NOT pull it in:

| Crate | How `common` is declared | Default-on? | Same for store? |
|---|---|---|---|
| `supervisor` | `common = { path="../common", package="datatree-common", optional=true }` behind `with-common` feature | NO | n/a |
| `parsers` | `common = { path="../common", optional=true }` behind `shared-types` feature | NO | n/a |
| `scanners` | `common = { path="../common", package="datatree-common", optional=true }` behind `with-common` | NO | n/a |
| `brain` | `common = { path="../common", optional=true }` (no feature gate referenced) | NO | n/a |
| `livebus` | `common = { path="../common", package="datatree-common", optional=true }` behind `with-common` | NO | n/a |
| `cli` | not even listed as a dependency; `with-common` feature exists but feature has empty body | NO | NO |
| `store` | `datatree-common.workspace = true` (mandatory) | YES | n/a |
| `multimodal-bridge` | `datatree-common.workspace = true` (mandatory) | YES | n/a |

Implication: today `cargo build --workspace` will compile, but each crate will be using the agent's *fallback* shadow types (see §3). The shared-type contract is silently broken.

Action: in all 6 marked crates, switch the dep from `optional = true` to a plain workspace dep (`datatree-common.workspace = true`) and delete the local fallback definitions.

---

## 3. Common types referenced correctly — FAIL

`common/` exposes 30+ canonical types. Only `store/` actually `use`s them. Shadow re-definitions found:

| Crate | File | Type re-defined locally |
|---|---|---|
| parsers | `src/job.rs` | `Confidence`, `NodeKind`, `Node`, `EdgeKind`, `Edge` (lines 24/52/73/99/118) |
| scanners | `src/scanner.rs` | `Severity`, `Finding` (lines 37/64) |
| brain | `src/lib.rs` | `NodeId(u128)` — incompatible with common's `NodeId(i64)` (line 51) |
| livebus | `src/event.rs` | `Event` struct (line 18) |
| supervisor | — | none found yet (mostly own types) |
| cli | — | none |
| multimodal-bridge | — | only `main.rs`, no shadow types |

The brain crate's `NodeId(u128)` collides with the canonical `NodeId(i64)` from `common`. Once §2 is fixed this is a hard type error every time brain emits a node id to the store.

Action: delete the shadow types in `parsers/src/job.rs`, `scanners/src/scanner.rs`, `brain/src/lib.rs`, `livebus/src/event.rs`. Re-import from `datatree_common`. Convert `brain` u128 nodes to i64.

---

## 4. MCP plugin integrity — OK with one missing command file

`plugin/plugin.json` declares 12 commands, 3 skills, 6 agents.

- 3 skills (`datatree-query.md`, `datatree-audit.md`, `datatree-resume.md`) — all present.
- 6 agents (`archivist`, `drift-hunter`, `blast-tracer`, `step-verifier`, `doctor`, `resumer`) — all present.
- Hooks reference 6 *commands*, not files (e.g. `datatree session-prime`) — handled by the CLI binary, not loose files. Verified in §8.
- 12 slash commands declared. Only 5 command markdown stub files exist in `plugin/commands/` (`dt-audit, dt-doctor, dt-recall, dt-step, dt-view`). 7 are missing: `dt-blast, dt-graphify, dt-godnodes, dt-drift, dt-history, dt-snap, dt-rebuild`.

Plugin loaders that read command markdown for help text will show empty for those 7. CLI subcommands themselves exist (§8).

---

## 5. Install template integrity — WARN (15 of 18 platforms present, but see §6)

`plugin/templates/` contains 18 folders. Each has at least one `.template`/`.partial`/`.json` file; none are empty.

Platform list: `aider, antigravity, claude-code, codex, continue, copilot, cursor, factory-droid, gemini-cli, hermes, kiro, openclaw, opencode, qoder, qwen, trae, windsurf, zed` — 18 total, matches design §21.4.

Note: design §21.4 also names `trae-cn` as a 19th variant (mentioned in README). Not present as its own folder; assume folded into `trae`.

---

## 6. CLI <-> install templates — FAIL

`cli/src/commands/install.rs` and every adapter under `cli/src/platforms/*.rs` generate manifest content **inline via `markers.rs`**. There is zero reference to `plugin/templates/` from any Rust file (`grep` returned no matches for `templates`, `include_str!`, or `include_bytes!`).

Implication: the 18 hand-crafted template files in `plugin/templates/` are unused by the actual installer. They are reference-only documentation today.

Action: either (a) wire each platform adapter to `include_str!` its template file, or (b) delete `plugin/templates/` and rely solely on inline generation. Recommend (a) — single source of truth, easier to tune per-platform copy without recompile.

---

## 7. Vision <-> Livebus — OK

- `vision/src/livebus.ts` opens WS to `${proto}://${window.location.host}/ws` (i.e. through Vision's own `server.ts`).
- `vision/server.ts` upgrades `/ws` and proxies to `LIVEBUS_WS = ws://127.0.0.1:7778/ws`.
- `livebus/src/lib.rs` declares `DEFAULT_PORT: u16 = 7778`. Confirmed.
- `vision/server.ts` proxies `/api/graph` to `DATATREE_IPC = http://127.0.0.1:7780` (POST to `/graph`) — i.e. the **supervisor**, not SQLite. Correct.

Caveat: `supervisor/` source contains no binding on port 7780 (`grep 7780` empty). The supervisor exposes IPC via a Unix socket / named pipe today; Vision expects HTTP. Either supervisor needs an HTTP shim on 7780, or vision/server.ts needs to switch to the IPC socket path. Today both endpoints just fall back to the placeholder data path.

---

## 8. Hooks <-> MCP server — OK

CLI subcommands (`cli/src/commands/`) match the 6 hook events 1:1:
`session_prime.rs, inject.rs, pre_tool.rs, post_tool.rs, turn_end.rs, session_end.rs`.

MCP TS handlers (`mcp/src/hooks/`) mirror the same 6 names:
`session_prime.ts, inject.ts, pre_tool.ts, post_tool.ts, turn_end.ts, session_end.ts`.

---

## 9. Schema integrity — OK

`store/src/schema.rs::schema_sql()` has a match arm for every one of the 22 `DbLayer` variants in `common/src/layer.rs` (Graph through Audit + Meta). Zero mismatches.

---

## 10. Documentation pointers — WARN

- `docs/design/` contains all 4 expected files (main + resource-policy, ux-mandate, knowledge-worker addenda).
- `README.md` links the main design doc and *generally* mentions "addenda", but does not link the 3 addenda by name. Consider adding explicit links.
- `LICENSE` exists and starts with `DATATREE PROPRIETARY LICENSE` — OK. (Cargo metadata still claims `license = "MIT"` in workspace package — see §12.)

---

## 11. File count tally

| Folder | Files |
|---|---|
| brain | 14 |
| cli | 51 |
| common | 16 |
| docs | 4 |
| livebus | 12 |
| mcp | 48 |
| multimodal-bridge | 2 |
| parsers | 12 |
| plugin | 65 |
| scanners | 20 |
| scripts | 25 |
| store | 10 |
| supervisor | 13 |
| tests | 0 (empty) |
| vision | 39 |
| workers | 41 |
| **Total** | **378** |

LOC by extension (raw `wc -l`):

| Ext | LOC |
|---|---|
| .rs | 20,845 |
| .ts | 4,623 |
| .tsx | 1,953 |
| .py | 3,160 |
| .sh | 1,594 |
| .ps1 | 1,195 |
| .md | 4,554 |
| .json | 326 |
| .toml | 806 |
| **Sum** | **~39,056** |

---

## 12. Inconsistency hunt

**TODO/FIXME/XXX/STUB markers** (source only, excluding .md): 6 occurrences in 4 files —
`brain/src/tests.rs (1)`, `scanners/src/scanners/a11y.rs (1)`, `scanners/src/scanners/theme.rs (2)`, `parsers/src/incremental.rs (2)`. None block compile.

**`unimplemented!()` / `todo!()` / `panic!()`**: only 2 — a `todo!()` inside a test fixture string (brain/src/tests.rs) and `panic!("variant mismatch")` in a CLI test (cli/src/ipc.rs). Both inside tests; safe.

**Hardcoded internet URLs** (source only):
- `scripts/install-runtime.sh` and `scripts/install-runtime.ps1` curl `https://bun.sh/install` and Homebrew install URL — these are part of bootstrap/runtime install. Acceptable per design, but flag: the local-only invariant in CLAUDE.md says "datatree must NEVER make outbound network calls during normal operation" — install-runtime is arguably normal operation. Document or gate behind `--allow-network` flag.
- `scanners/src/scanners/secrets.rs` line 107 — `https://hooks.slack.com/...` is a regex *pattern* used to detect leaked Slack webhooks. Legitimate.
- `livebus/src/tests.rs` line 179 — `http://{local}` test URL. Local loopback. Fine.
- `cli/src/platforms/mod.rs` line 317 — references `github.com/anishtrivedi/datatree` in a help string. Fine.
- All other URLs are loopback (`http://127.0.0.1`, `localhost`).

**License inconsistency**: workspace `Cargo.toml` declares `license = "MIT"`, README + LICENSE declare proprietary. Mismatch will mislead `cargo` consumers.

---

## 13. Cross-platform path safety

Rust source: 4 files contain string literals with leading `./` or `../` or `src/` —
`scanners/src/tests.rs`, `scanners/src/scanners/markdown_drift.rs`, `livebus/src/tests.rs`, `livebus/src/event.rs`.

Spot-checked: most are test fixtures (file paths inside synthetic markdown) or relative example paths. No production-code path joins via string concatenation found. PathManager (in common) is the canonical builder but is only directly used by `store/` today.

Scripts: 9 shell scripts use `#!/bin/bash` shebangs. PowerShell mirrors exist for each. No POSIX-only Python found — all `.py` files in workers/ use forward slashes which are cross-platform in Python.

---

## PRIORITY FIX LIST (top 10, in order)

1. **Un-flag `datatree-common`** in `supervisor/`, `parsers/`, `scanners/`, `brain/`, `livebus/` Cargo.tomls — change `optional = true` to `.workspace = true`. (§2)
2. **Add `datatree-common` and `datatree-store` to `cli/Cargo.toml`** (currently absent — relies on a no-op `with-common` feature). (§2)
3. **Delete shadow types** in `parsers/src/job.rs`, `scanners/src/scanner.rs`, `brain/src/lib.rs`, `livebus/src/event.rs`; re-import from `datatree_common`. (§3)
4. **Reconcile `brain` `NodeId(u128)`** with canonical `NodeId(i64)` — pick one width and migrate. (§3)
5. **Wire `cli/src/platforms/*.rs` to the actual templates** in `plugin/templates/` via `include_str!`, OR delete `plugin/templates/`. (§6)
6. **Add an HTTP IPC server on port 7780** in `supervisor/` (or change Vision's `DATATREE_IPC` default to the supervisor's actual transport). Otherwise `/api/graph` always falls back to placeholder. (§7)
7. **Create the 7 missing slash-command markdown files** under `plugin/commands/` (`dt-blast, dt-graphify, dt-godnodes, dt-drift, dt-history, dt-snap, dt-rebuild`). (§4)
8. **Fix the workspace license string** in root `Cargo.toml` — `"MIT"` contradicts the proprietary `LICENSE` and README. Set to `"LicenseRef-Datatree-Proprietary"` or similar. (§12)
9. **Resolve the 6 `TODO`/`FIXME` markers** in `scanners/`, `brain/`, `parsers/` before first release. (§12)
10. **Add explicit links to the 3 addenda** (`resource-policy`, `ux-mandate`, `knowledge-worker`) from `README.md` so users see them. (§10)

End of report. ~1,490 words.
