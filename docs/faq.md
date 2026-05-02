# FAQ

Current version: **v0.3.2** (hotfix, 2026-05-02). See
[`CHANGELOG.md`](../CHANGELOG.md) for the full history.

## The big-picture questions

### What is mneme trying to fix?

Every AI coding assistant has the same three flaws:

1. **Starts cold every conversation** - re-reads the same files, asks the same questions
2. **Loses its place when context compacts** - you give Claude a 100-step plan and at step 50 the conversation compresses; Claude forgets and starts over
3. **Drifts from your rules** - your `CLAUDE.md` says "no hardcoded colors", but five prompts later Claude hardcodes one

mneme fixes all three at the architecture level (not the prompt level) by externalising the memory into a local SQLite graph that gets silently fed back into Claude's context each turn.

### Is this just another RAG system?

No. RAG chunks your documents and does embedding lookup when you ask a question. mneme is doing the opposite: it tracks **structured state** (numbered steps, decisions, constraints, verbatim conversation turns, file-by-file graph edges) and **proactively injects** the correct slice into every turn - before Claude has even thought to ask.

mneme does have an embeddings store and supports semantic recall, but that's one tool among 48. The headline feature is the Step Ledger, which is not RAG at all - it's a verified state machine.

### Do you send my code to a server?

No. mneme runs **100% locally**. No cloud, no telemetry, no API keys, no "phone home" on startup, no embedded analytics. Models are CPU-only and either bundled with the binaries or downloaded once from a path you specify. You can block mneme at your firewall and it will keep working.

The bootstrap installer is the only one-time network event - it pulls binaries
from `github.com/omanishay-cyber/mneme/releases` and models from
`huggingface.co/aaditya4u/mneme-models` (with GitHub Releases as a fallback
mirror for the model weights). After that, nothing leaves your machine.

### How is this different from code-review-graph or graphify?

- **code-review-graph** (CRG) is the state-of-the-art deterministic code graph. mneme's structural graph builds on the same idea (Tree-sitter AST -> SQLite) but adds 21 more storage layers (22 total + meta.db), compaction resilience, and the Step Ledger. Measured p95 token reduction is 3.5x (see [BENCHMARKS.md](../benchmarks/BENCHMARKS.md)); CRG comparison pending a Linux CI run.
- **graphify** is a multimodal knowledge-graph builder that uses LLM subagents to extract concepts from PDFs/audio/video. mneme absorbs graphify's multimodal pipeline as one of its workers - they're complementary, not competing.

See the README's benchmark table for a feature-by-feature comparison.

---

## Installation & setup

### Why do I need Rust, Bun, and Python all three?

You **don't** if you install via the bootstrap one-liner - the released
binaries are pre-built and don't need any toolchain on your machine. You only
need Rust + Bun + Python if you build mneme from source. See
[`docs/dev-setup.md`](dev-setup.md) for the dev install.

When the codebase itself is built, each language is used for what it's best at:

- **Rust** - supervisor, storage, parsers, scanners, brain. Must be fast, fault-tolerant, and statically linkable.
- **Bun + TypeScript** - MCP server and vision app. Hot-reloadable tool definitions; `bun:sqlite` is the fastest SQLite binding in any runtime.
- **Python** - multimodal sidecar. PDF/OCR/Whisper ecosystems are irreplaceable here.

### Install failed. What do I check?

Walk down [`docs/INSTALL.md`'s troubleshooting section](INSTALL.md#troubleshooting). The most common causes:

1. **CPU too old** - mneme requires AVX2/BMI2/FMA (Intel Haswell 2013+ / AMD Excavator 2015+). On older hardware, build from source.
2. **PATH not refreshed after install** - open a new terminal so `~/.mneme/bin` is on PATH.
3. **Windows Defender quarantine** - the installer adds `~/.mneme/` to exclusions when run with admin; without admin you have to add it yourself.
4. **Phi-3 download failed** - the bootstrap tries Hugging Face Hub first, then GitHub Releases. If both fail, install models manually with `mneme models install --from-path /path/to/local/mirror`.

### Where is my data stored?

Everything lives under `~/.mneme/`:

- `~/.mneme/projects/<sha>/` - per-project shards (one folder per project)
- `~/.mneme/snapshots/` - hourly rolling snapshots of each shard
- `~/.mneme/cache/` - embedding cache, docs cache, multimodal cache
- `~/.mneme/bin/` - the worker binaries (~250 MB)
- `~/.mneme/models/` - bge-small + Qwen 2.5 + Phi-3-mini-4k (~3 GB total)
- `~/.mneme/logs/` - supervisor + worker logs
- `~/.mneme/run/` - PID file + IPC discovery (named pipe / unix socket)

Remove the folder and mneme is gone. Nothing lives anywhere else.

### Does mneme slow down my machine?

The supervisor uses ~30-80 MB RAM idle. During active indexing it'll push one
CPU core for a few seconds. The 22 daemon workers stay resident but idle
between jobs (a few MB each). The daemon is designed to be invisible when
nothing's happening.

### Why do I need a 64-bit modern CPU?

The release binaries are compiled with `-C target-cpu=x86-64-v3` so they
require AVX2, BMI2, FMA instructions (Intel Haswell 2013+ or AMD Excavator
2015+). Almost every PC sold since 2013 qualifies. Older hardware needs to
build from source with `RUSTFLAGS="-C target-cpu=x86-64"` to drop the
baseline. 32-bit Windows is not supported because Bun has no 32-bit Windows
build.

---

## Claude Code integration

### How does Claude know mneme is there?

When you run the bootstrap installer (or `mneme install`), a block gets injected into your global `~/CLAUDE.md` and an MCP server entry gets added to `~/.claude.json`. Every future Claude Code session reads the CLAUDE.md block as context and launches `mneme mcp stdio` as its MCP server. The 8 mneme hook entries also get registered under `~/.claude/settings.json::hooks` so context is auto-injected at every tool boundary. Restart Claude Code once after install for the MCP connection to come up.

### Can I turn it off for one conversation?

Yes. In that Claude Code project, edit `.claude/settings.local.json`:

```json
{
  "mcpServers": {
    "mneme": { "enabled": false }
  }
}
```

Or delete the `<!-- mneme-start v1.0 -->` block from your CLAUDE.md temporarily.

### Does this work with Codex / Cursor / Windsurf?

Yes. `mneme install` auto-configures Claude Code on first run, and 18 more AI tools via `mneme register-mcp --platform <name>`. See the table in [`docs/INSTALL.md`](INSTALL.md#register-mcp-with-any-of-the-19-supported-ai-tools).

### What if I use multiple AI tools on the same project?

mneme's state is per-project, not per-tool. All 19 supported tools will see the same graph, the same decisions, the same Step Ledger. You can be in Claude Code one hour and Cursor the next and everything continues.

---

## The Step Ledger

### What does "compaction-resilient" actually mean?

Claude Code's context window has a hard limit. When you fill it, the system automatically compresses older turns into a summary to free room. This is called compaction. The problem: compression loses detail. If you were on step 50 of a 100-step plan, Claude often restarts from step 30 or rereads every doc to figure out where it was.

With mneme, the Step Ledger lives in SQLite. Each step has an explicit status, a verification command, and recorded proof artifacts. When compaction happens, Claude's next turn calls `step_resume` which emits a ~5K-token bundle with the exact state: what's done, what's next, what's blocked, what constraints are active. Claude picks up at step 51.

### How do I create a Step Ledger?

Tell Claude something like *"Create a step ledger for this work"* and then write a numbered plan. Claude's tool call `step_plan_from` ingests a markdown roadmap. Or: each TaskCreate item you make becomes a step automatically if you're using mneme's wrapper.

### Can the Step Ledger span multiple conversations?

Yes. The Step Ledger is per-project, not per-conversation. You can close Claude Code, reopen it tomorrow, and the ledger state is exactly as you left it.

---

## Auditing / scanning

### What's in `mneme audit`?

11 built-in scanners run in parallel across the scanner-worker pool (~5x
faster on multi-core machines as of v0.3.2 / B12) and stream their findings
into `findings.db` incrementally. See
[`docs/architecture.md`](architecture.md#the-11-built-in-scanners) for the
full list.

### My audit hangs. What do I do?

v0.3.2 replaced the wall-clock `MNEME_AUDIT_TIMEOUT_SEC` (now removed) with a
per-line stall detector (`MNEME_AUDIT_LINE_TIMEOUT_SEC`, default 30 s). On a
multi-hour audit of a giant project the per-line guard alone keeps the audit
unstuck without binning legitimate long runs. Findings stream to disk
incrementally so you can `mneme audit` again on a subset and pick up where
it left off. See [`docs/env-vars.md`](env-vars.md) for the full env reference.

---

## License & commercial use

### Can I use mneme at my job?

**Yes.** Production Use includes coding / debugging / writing / research / notes at your day job. You don't need to pay anyone to run mneme as part of commercial employment.

### Can I sell mneme?

**No.** No selling copies, no selling access, no selling installations, no charging for mneme itself.

### Can I sell a product built on top of mneme?

It depends. Building *a tool whose primary value proposition is mneme* (another persistent-memory MCP, another AI superbrain, etc.) is not allowed. Building *your own product that happens to integrate mneme internally* is fine if mneme isn't the main thing being sold. When in doubt, open a GitHub Issue to discuss.

### Can I host mneme as a paid service?

**No** - that's specifically prohibited. Commercial hosting requires a separate license.

### Can I modify mneme locally?

**Yes.** Modify for your own use, write custom MCP tools, add scanners, tweak prompts - all allowed. You can't redistribute the modified version.

### What if I find a bug and want to send a fix?

PRs welcome. See [CONTRIBUTING.md](../CONTRIBUTING.md). By submitting a PR you're agreeing your contribution is licensed under the [Apache-2.0](../LICENSE).

---

## Performance & scale

### Will it work on my 100k-file monorepo?

Yes in theory. The architecture is designed for monorepo scale (WebGL visualisation handles 100k+ nodes, WAL SQLite scales to GBs of graph data, parser workers parallelise across CPU cores). In practice v0.3.2 self-indexes the Mneme source tree (11,417 nodes / 26,708 edges / 359 files, measured 2026-04-23) and the benchmark CI indexes Django (~300k LOC) and TypeScript (~2M LOC); larger-repo performance tuning is ongoing. A v0.3.2 benchmark re-run on the audit-cycle corpus is pending - tracked in [`docs/REMAINING_WORK.md`](REMAINING_WORK.md).

### How much disk does it use?

About 50 MB per 10k-file project for the graph + history + findings shards.
Plus ~3 GB one-time for the model lineup (bge-small + Qwen 2.5 Coder/Embed +
Phi-3-mini-4k). Snapshots are rotated; worst-case disk usage is bounded.

### How much RAM?

Idle daemon: 30-80 MB. Peak during active indexing of a 10k-file project: ~500 MB across all 22 workers. No single worker holds more than ~200 MB under normal load.

---

## Bugs & support

### Where do I report a bug?

[GitHub Issues](https://github.com/omanishay-cyber/mneme/issues) - please include OS, CPU model, and the output of `mneme doctor --json`.

### Where do I ask a question?

[GitHub Discussions](https://github.com/omanishay-cyber/mneme/discussions) - any "how would I" or architectural-design question is welcome.

### Security vulnerability?

Please **do not** file a public issue. Open an Issue with `[SECURITY]` in the title and say "please contact me privately" - a maintainer will reach out via GitHub DM to continue in confidence.

---

## See also

- [`docs/INSTALL.md`](INSTALL.md) - install paths + troubleshooting
- [`docs/architecture.md`](architecture.md) - how mneme is built
- [`docs/dev-setup.md`](dev-setup.md) - build from source
- [`docs/mcp-tools.md`](mcp-tools.md) - reference for every MCP tool
- [`docs/env-vars.md`](env-vars.md) - all `MNEME_*` env vars
- [`docs/REMAINING_WORK.md`](REMAINING_WORK.md) - parked items + v0.4.0 backlog

---

[← back to README](../README.md)
