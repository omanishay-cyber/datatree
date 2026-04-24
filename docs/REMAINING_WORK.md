# Remaining Work — Mneme

Parked items from the 2026-04-23 revival sessions. Each entry: **what**, **why deferred**, **acceptance criteria**, **effort estimate**. Pick these up when revisiting.

Last updated: 2026-04-23 (after v0.2.2 ship).

---

## Tier 1 — Engineering work (safe to delegate to an agent)

### 1. Wire remaining MCP tools to real data
- **What:** ~24 of 47 MCP tools are still stubs. Wired as of v0.2.2: `blast_radius`, `recall_concept`, `health`, `doctor`, `god_nodes`, `step_status`, `step_resume`, `drift_findings` + whatever Phase C8 lands. Remaining stubs: the audit_* family, recall_file/decision/constraint/todo, call_graph, cyclic_deps, dependency_chain, architecture_overview, surprising_connections, refactor_suggest/apply, rebuild, snapshot, rewind, compare, graphify_corpus, wiki_generate/page, audit_corpus, audit_types.
- **Why deferred:** mechanical batch work; each tool is 45–90 min using the `blast_radius.ts` pattern. Best done in agent fanouts of 5 tools each.
- **Acceptance:** every `mcp/src/tools/*.ts` returns real data (not the stub placeholder). Wired ratio ≥ 95%.
- **Effort:** 3–5 days of agent dispatches.

### 2. Real BGE-small ONNX embeddings
- **What:** `ort` crate dep is already uncommented in `Cargo.toml`, but `brain/src/embeddings.rs` code path is still the hashing-trick fallback. Need to (a) ship the BGE-small ONNX model file + tokenizer.json in the installer OR auto-download on first use, (b) replace the hash-trick branch in `embed_text` with real ort inference, (c) measure recall@10 improvement in benchmarks.
- **Why deferred:** `ort` on Windows has known compat issues with native DLL loading; needs the ONNX Runtime `onnxruntime.dll` + `onnxruntime_providers_shared.dll` shipped alongside or picked up via `ORT_DYLIB_PATH` env var. This is 1–2 days of install pipeline work, not pure code.
- **Acceptance:** `mneme build` and `recall_concept` both use real BGE embeddings. Benchmark `bench-recall` precision@10 jumps from 10% baseline to at least 50%.
- **Effort:** 1–2 days.

### 3. Supervisor-mediated worker dispatch
- **What:** Currently `mneme build` runs the parse/scan/brain pipeline inline in the CLI process. Workers spawn but idle. The supervisor's IPC has a channel but no routing logic that pulls work items off a queue and hands them to workers.
- **Why deferred:** 2 days of architectural work. Requires: a job queue (supervisor owns it), a worker protocol (request job → process → return result), proper backpressure + per-worker health tracking. Must not regress auto-restart.
- **Acceptance:** `mneme build .` on a 1000-file repo shows all 40 workers active (CPU busy) rather than idle. Parse/scan throughput at least 2× the inline path.
- **Effort:** 2 days.

### 4. Multimodal Rust↔Python bridge + PDF ingestion
- **What:** Python sidecar in `workers/multimodal/` is installed but no live IPC to Rust. Pick PDF as first pipeline — extract text + layout via Python (pypdfium2 or pymupdf), stream nodes back to Rust, inject into graph via `multimodal-bridge`.
- **Why deferred:** 2 days of cross-language IPC work. Recommended transport: shared SQLite write queue, OR gRPC via tonic+grpcio-tools, OR simple JSON-lines stdio.
- **Acceptance:** `mneme build ./path-with-pdfs/` indexes PDF content as real graph nodes. `recall_concept("specific phrase from PDF")` returns the PDF page as a hit.
- **Effort:** 2 days.

### 5. Julia + Zig Tree-sitter grammar ABI mismatch
- **Status (as of 2026-04-23):** a grammar-fix agent was dispatched in the v0.2.2 session and is expected to either upgrade the crate versions, switch to maintained forks, or rewrite the test expectations. See the commit log for resolution.
- **Why deferred if not resolved:** upstream `tree-sitter-julia` / `tree-sitter-zig` crates may still be pinned to tree-sitter 0.20-0.23 runtimes; our workspace pins 0.25. Fork pattern (like `tree-sitter-kotlin-sg` / `tree-sitter-svelte-ng`) is the fallback.
- **Acceptance:** `cargo test -p mneme-parsers julia_grammar_smoke zig_grammar_smoke` passes. No runtime ABI mismatch warnings when indexing a .jl or .zig file.

### 6. All remaining vision views wired
- **What:** As of v0.2.2, all 15 views render real data end-to-end (verified runtime with 1922 nodes / 3643 edges). But some views (HierarchyTree d3-sankey overload, ProjectGalaxy3D deck.gl API drift) have pre-existing TypeScript errors that were out of scope for the quick-wire pass.
- **Why deferred:** each view has its own library quirk; individual fixes.
- **Acceptance:** `cd vision && bunx tsc --noEmit` is fully clean.
- **Effort:** 2–4 hours per view × ~3 broken views = 1 day.

### 7. Benchmark CSV numbers published against external baseline
- **What:** `BENCHMARKS.md` (committed in v0.2.2) has mneme-vs-cold numbers. Next: run `just bench-compare-csv` against code-review-graph (CRG) on the same fixture and publish the comparison.
- **Why deferred:** needs CRG checked out + indexed + cross-indexed fixture. Time-gated but straightforward.
- **Acceptance:** `BENCHMARKS.md` has a "Mneme vs CRG" table with token-reduction and recall@10 deltas.
- **Effort:** 3 hours.

### 8. CI benchmark seed baseline
- **What:** `.github/workflows/bench-baseline.yml` exists but nobody has clicked "Run workflow" to seed the first baseline artifact. Until someone does, PR comments will show "No baseline artifact" instead of a comparison.
- **Why deferred:** requires one-time manual trigger by a maintainer with Actions write access.
- **Acceptance:** go to Actions → `Mneme Benchmark Baseline` → Run workflow on main branch. Then next PR shows the regression comment.
- **Effort:** 2 minutes + ~15 min CI runtime.

---

## Tier 2 — Needs human involvement (NOT agent-delegable)

### 9. 60-second demo video
- **What:** a short screen recording showing: `mneme install` → `mneme build .` → `/mn-recall_concept` returning real hits → `/mn-blast_radius` on a file → `/mn-step_resume` after a simulated compaction. Embed the recorded asciinema (or mp4) in README hero.
- **Why parked:** requires a human behind the keyboard. Cannot be delegated to an agent because it needs screen-recording of a real terminal session + voiceover or caption.
- **How to tackle:**
  - Install asciinema on Windows via WSL (`apt install asciinema`) OR use OBS Studio for a windowed recording.
  - Script the 60-sec flow: 10 sec install, 10 sec build, 15 sec recall, 15 sec blast, 10 sec resume.
  - Record, trim in asciicast2gif or ffmpeg, embed.
  - Add as `docs/demo.cast` or `docs/demo.gif`; link from README hero.
- **Effort:** 1–2 hours for a polished single take; half a day with retakes.

### 10. Domain registration + landing page
- **What:** Register `mneme.dev` (or similar). Build a short Astro / Next static site. Deploy to Cloudflare Pages / Vercel / Netlify.
- **Why parked:** requires a domain-owner account (your credit card), DNS access (your Cloudflare account or whatever registrar), and content decisions that should be yours (copywriting, tagline, hero image).
- **How to tackle:**
  - Registrar: Cloudflare Registrar (≈$10/yr for .dev, no markup, WHOIS privacy built-in).
  - Stack: Astro 4.x + Tailwind + the existing `og.png` and `og.svg` from `docs/`. Or Next 15 app router + Tailwind.
  - Sections: hero + demo embed + feature grid (re-use the README stats) + install tabs + footer with GitHub link.
  - Host: Cloudflare Pages (free, connects directly to GitHub; auto-deploy on push to `main` or a dedicated `site` branch).
  - DNS: apex + www → Cloudflare Pages. Add CAA record for Let's Encrypt.
- **Effort:** 1 day end-to-end (domain + scaffold + content + deploy). Add 1 day polish for hero visuals.

---

## Parking rules
1. Anything in this file is **not lost** — it's scheduled. Just not blocking v0.2.x or v0.3.0.
2. When resuming work on any item: move its block from here into `CHANGELOG.md [Unreleased]` "Planned for vX.Y" when it's being actively targeted.
3. When an item ships: delete the block from this file AND add a concrete entry in CHANGELOG.md for the release that contained it.
4. Never silently delete an item — if something is no longer relevant, add a short strikethrough + reason line.

---

## History
- **2026-04-23** — File created during v0.2.2 ship. Parked items #9 (demo video) and #10 (domain + landing page) per user instruction ("you can park and we can do later on"). Tier-1 items #1–#8 listed for agent pickup in future sessions.
