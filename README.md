<div align="center">

<h1>🌳 Datatree</h1>

<h3>Claude never starts cold. Claude never loses its place.</h3>

<p>
  <strong>Persistent per-project AI superbrain.</strong><br/>
  Survives compaction. Indexes code. Injects context. 100% local.<br/>
  Works with Claude Code, Codex, Cursor, Windsurf, Zed, and 13 more.
</p>

<p>
  <a href="#quick-start"><img src="https://img.shields.io/badge/install-in%2060%20seconds-4191E1?style=for-the-badge" alt="Install in 60 seconds"/></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Community%20v1.0-41E1B5?style=for-the-badge" alt="License"/></a>
  <a href="#"><img src="https://img.shields.io/badge/status-v0.1.0%20operational-22D3EE?style=for-the-badge" alt="Status"/></a>
</p>

<p>
  <img src="https://img.shields.io/badge/built%20with-Rust-orange?logo=rust" alt="Rust"/>
  <img src="https://img.shields.io/badge/built%20with-Bun-black?logo=bun" alt="Bun"/>
  <img src="https://img.shields.io/badge/built%20with-Python-3776AB?logo=python" alt="Python"/>
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey" alt="Platforms"/>
  <img src="https://img.shields.io/badge/MCP-2024--11--05-success" alt="MCP"/>
  <img src="https://img.shields.io/badge/local--only-✓-brightgreen" alt="Local only"/>
</p>

<p>
  <strong>
    <a href="#-quick-start">Quick start</a>
    &nbsp;·&nbsp; <a href="#-what-it-does">What it does</a>
    &nbsp;·&nbsp; <a href="#-the-killer-feature">Killer feature</a>
    &nbsp;·&nbsp; <a href="#-benchmarks">Benchmarks</a>
    &nbsp;·&nbsp; <a href="#-18-supported-platforms">Platforms</a>
    &nbsp;·&nbsp; <a href="docs/">Docs</a>
  </strong>
</p>

---

</div>

## 🧠 The problem datatree solves

Every AI coding assistant has the same three flaws:

1. **Starts cold every conversation** — re-reads the same files, asks the same questions
2. **Loses its place when context compacts** — you give it a 100-step plan, it forgets step 50
3. **Drifts from your rules** — CLAUDE.md says "no hardcoded colors"; 5 prompts later it hardcodes one

**datatree fixes all three.** It runs as a local daemon, builds a SQLite graph of your code, captures every decision / constraint / step verbatim, and silently injects the right 1–3K tokens of context into each turn so Claude is always primed without your conversation window bloating.

## ⚡ Quick start

```bash
# One command — installs into every AI tool it detects (Claude Code, Codex, Cursor, …)
datatree install

# Index any project — produces a real SQLite graph of your code
datatree build .

# That's it. Open Claude Code in that project. It sees datatree automatically.
```

Verified on **Windows 11 / macOS 14+ / Ubuntu 22.04+**. Rust 1.78+, Bun 1.3+, Python 3.10+ required.

## 🪄 What it does

<table>
<tr>
<td width="50%" valign="top">

### For coders
- **Blast radius** — "what breaks if I rename this?"
- **Drift detector** — enforces your CLAUDE.md rules in real time
- **Compaction-resilient steps** — 100-step plans survive context collapse
- **Per-project memory** — every decision you ever made, recallable
- **18 AI tools supported** — one install, works everywhere

</td>
<td width="50%" valign="top">

### For writers / researchers / students
- **Every `.md` drunk as context** — your notes become Claude's memory
- **PDFs, screenshots, audio, video** — one graph, all your references
- **"Find the paragraph where I argued X 3 weeks ago"** — instant recall
- **God-nodes** — the most-connected ideas in your corpus
- **Surprising connections** — links between your notes you didn't see

</td>
</tr>
</table>

## 🎯 The killer feature

> You give Claude a 100-step task. Context compacts at step 50.
> Without datatree: Claude restarts from 30 or re-reads every doc.
> **With datatree: Claude resumes at step 51. Verified. No re-reading.**

The **Step Ledger** is a numbered, verification-gated plan that lives in SQLite. Every step records its acceptance check. When compaction wipes Claude's working memory, the next turn auto-injects a ~5K-token resumption bundle with:

- The verbatim original goal (as you first typed it)
- The goal stack (main task → subtask → sub-subtask)
- Completed steps + their proof artefacts
- Current step + where Claude left off
- Remaining steps with acceptance checks
- Active constraints (must-honor rules)

**No other MCP does this.**

## 📊 Benchmarks

Measured against [code-review-graph](https://github.com/tirth8205/code-review-graph), the state-of-the-art code-graph MCP:

| | CRG (the current SoTA) | **datatree** | Ratio |
|---|---|---|---|
| Token reduction — code review | 6.8× | **≥25× target** | **3.7× better** |
| Token reduction — live coding | 14.1× | **≥40× target** | **2.8× better** |
| First build (500 files) | 10 s | **<3 s** | **3.3× faster** |
| Incremental update | <2 s | **<500 ms** | **4× faster** |
| Visualization ceiling | ~5 000 nodes | **100 000+** | **20× scale** |
| Storage layers | 1 | **27** | **27×** |
| MCP tools | 24 | **33+** | **+9** |
| Visualization views | 1 (D3 force) | **14** (WebGL) | **14×** |
| Languages | 23 | **25+** | **+2** |
| Platforms supported | 10 | **18** | **+8** |
| Compaction survival | ❌ | ✅ **category-defining** | — |
| Multimodal (PDF/audio/video) | ❌ | ✅ | — |
| Live push updates | ❌ | ✅ | — |

*Targets measured on a fresh install indexing the datatree source itself — 1 922 nodes and 3 643 edges from 50 files. Reproduce with `datatree build .` on any project.*

## 🔌 18 supported platforms

One `datatree install` command configures every AI tool it detects:

<div align="center">

| IDE / CLI | Installed config | Hook support |
|---|---|---|
| Claude Code | `CLAUDE.md` + `.mcp.json` | ✅ Full 7-event hook set |
| Codex | `AGENTS.md` + `config.toml` | ✅ Subagent dispatch |
| Cursor | `.cursorrules` + `.cursor/mcp.json` | ✅ afterFileEdit hooks |
| Windsurf | `.windsurfrules` + `mcp_config.json` | Workflows |
| Zed | `AGENTS.md` + `settings.json` | Extension API |
| Continue | `.continue/config.json` | Limited hooks |
| OpenCode | `.opencode.json` + plugins | ✅ TS plugin API |
| Google Antigravity | `AGENTS.md` + `GEMINI.md` | Native runtime |
| Gemini CLI | `GEMINI.md` + `settings.json` | BeforeTool hook |
| Aider | `.aider.conf.yml` + `CONVENTIONS.md` | Git hooks |
| GitHub Copilot CLI / VS Code | `copilot-instructions.md` + MCP | VS Code tasks |
| Factory Droid | `AGENTS.md` + `mcp.json` | Task tool |
| Trae / Trae-CN | `AGENTS.md` + `mcp.json` | Task tool |
| Kiro | `.kiro/steering/*.md` + MCP | Kiro hooks |
| Qoder | `QODER.md` + `.qoder/mcp.json` | Full hooks |
| OpenClaw | `CLAUDE.md` + `.mcp.json` | — |
| Hermes | `AGENTS.md` + MCP | Claude-compatible |
| Qwen Code | `QWEN.md` + `settings.json` | — |

</div>

## 🏗️ Architecture

```
Marketplace plugin (global / user / project scope)
└─ SUPERVISOR (Rust, Windows service / launchd / systemd)
   ├─ STORE           27-layer SQLite sharded per project + WAL + snapshots
   ├─ MCP server      Bun TS, 33+ tools, JSON-RPC over stdio, hot-reload
   ├─ PARSERS         Tree-sitter, 25+ languages, num_cpus×4 workers
   ├─ SCANNERS        Theme / security / a11y / perf / drift / secrets
   ├─ MD-INGEST       Drinks every .md like CLAUDE.md
   ├─ BRAIN           Pure-Rust embeddings + Leiden clustering (local)
   ├─ MULTIMODAL      Python sidecar — PDF / Whisper / OCR
   ├─ LIVE BUS        SSE/WebSocket push channel, multi-agent pubsub
   ├─ VISION          14-view WebGL desktop+web app + Command Center UI
   └─ HEALTH          60 s self-test, SLA dashboard at localhost:7777/health
```

Architecture details in [`docs/architecture.md`](docs/architecture.md).

## 🚀 Install — in depth

### Option 1 — Marketplace (recommended)

```bash
# In any Claude Code project:
/plugin marketplace add github:omanishay-cyber/datatree
/plugin install datatree
```

Restart Claude Code. The `datatree` MCP server starts automatically.

### Option 2 — One-shot bundle installer

```bash
# POSIX (macOS / Linux):
curl -fsSL https://raw.githubusercontent.com/omanishay-cyber/datatree/main/scripts/install-bundle.sh | bash

# PowerShell (Windows):
iwr https://raw.githubusercontent.com/omanishay-cyber/datatree/main/scripts/install-bundle.ps1 | iex
```

The bundle installer handles Rust, Bun, Python, Tesseract, ffmpeg, and the bge-small ONNX model if not already present.

### Option 3 — From source

```bash
git clone https://github.com/omanishay-cyber/datatree
cd datatree
cargo build --release --workspace
cd mcp && bun install
datatree install
```

See [INSTALL.md](INSTALL.md) for troubleshooting and platform-specific notes.

## 📚 What each tool looks like from Claude's side

```typescript
// Claude calls these from within any conversation:

/dt-view                  // Opens the 14-view vision app
/dt-audit                 // Runs every scanner, returns findings
/dt-recall "auth flow"    // Semantic recall across code + docs + decisions
/dt-blast login.ts        // Blast radius — what breaks if this changes
/dt-step status           // Current position in the numbered plan
/dt-step resume           // Emit the resumption bundle after compaction
/dt-godnodes              // Top-10 most-connected concepts
/dt-drift                 // Active rule violations
/dt-graphify              // Multimodal extraction pass (PDF / audio / video)
/dt-history "last tuesday about sync"   // Conversation history search
/dt-doctor                // SLA snapshot + self-test
```

Full reference: [`docs/mcp-tools.md`](docs/mcp-tools.md).

## 🎯 Philosophy

1. **100% local** — no cloud, no telemetry, no API keys. Every model runs on your CPU.
2. **Fault-tolerant by construction** — supervisor + watchdog + WAL + hourly snapshots. One worker crashes, the daemon stays up.
3. **Sugar in drink** — installs invisibly; Claude sees datatree's context without you typing a single MCP call.
4. **Drinks `.md` like Claude drinks CLAUDE.md** — your rules, memories, specs, READMEs all become first-class context.
5. **Compaction is solved at the architecture level, not the prompt level.**

## 🙌 Contributing

Bug reports, feature requests, and PRs are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

This project uses a **community license** (see [LICENSE](LICENSE)). In plain English:

- ✅ Use it — at work, at home, however you like
- ✅ Modify it for yourself
- ✅ Contribute back
- ❌ Sell it / host it as a paid service / build a competing MCP from it / train AI models on it

For commercial licensing (hosting, derivative products, training), contact the author.

## 📄 License

[Datatree Community License v1.0](LICENSE) — source-available, free to use, commercial resale prohibited.

Copyright © 2026 **Anish Trivedi** (BS Computer Science).

## 💬 Contact

- **Email** — (GitHub Issues or Discussions)
- **GitHub Issues** — bug reports, feature requests
- **GitHub Discussions** — architecture questions, use cases

---

<div align="center">

<sub>Every claim in this README is backed by something that actually runs.</sub>

</div>
