<div class="mneme-hero">

<span class="eyebrow">v0.4.0 — keystone shipped</span>

<h1 class="hero-title">Mneme. <span class="grad">Persistent memory</span> for AI coding.</h1>

<p class="lede">Survives context wipes. Runs entirely local. 50 MCP tools, 14 graph views, 22 storage layers, three real symbol resolvers, and a 3-layer self-ping system that nudges your AI back to structural answers instead of regex grep.</p>

<div class="cta-row">
  <a class="mneme-cta mneme-cta-primary" href="./install/index.html">Install →</a>
  <a class="mneme-cta mneme-cta-secondary" href="./releases/v0.4.0.html">What's new in v0.4.0</a>
</div>

</div>

## Why Mneme

When you ask an AI "where does `WorkerPool::spawn` get called?", the cheap answer is regex over text. The slightly less-cheap answer is grep. Both miss `super::spawn`, `crate::manager::spawn`, `use crate::manager; spawn()`, and aliased re-exports. Mneme answers with structural certainty: parser-built call graphs, a per-language symbol resolver, BGE embeddings anchored on canonical names, all in a daemon your AI talks to via MCP.

```text
mneme recall_concept "spawn"
  →  crate::manager::WorkerPool::spawn  (supervisor/src/manager.rs:1100)
     pub async fn spawn(&self, job: Job) -> Result<JobId>
     [callers: 12, dependents: 5, tests: 3]
```

## Built for the work

<div class="mneme-features">

<div class="mneme-feature">
  <div class="icon-wrap">
    <svg fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M9.75 3.104v5.714a2.25 2.25 0 01-.659 1.591L5.27 14.21A2.25 2.25 0 005.25 17.4l2.16 2.32a1.125 1.125 0 001.71-.144l5.34-7.115a2.25 2.25 0 00.452-1.351V3.104"/></svg>
  </div>
  <h3>Symbol resolver</h3>
  <p>Three real resolvers — Rust, TypeScript / JavaScript, Python — turn syntactic names into one canonical string per logical symbol. The keystone of v0.4.0.</p>
</div>

<div class="mneme-feature">
  <div class="icon-wrap">
    <svg fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M9.813 15.904 9 18.75l-.813-2.846a4.5 4.5 0 0 0-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 0 0 3.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 0 0 3.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 0 0-3.09 3.09Z"/></svg>
  </div>
  <h3>Symbol-anchored embeddings</h3>
  <p><code>recall_concept "spawn"</code> now matches the canonical-anchored function row, not the README. v0.3.x measured 2/10; v0.4.0 reaches CRG parity at ~6/10.</p>
</div>

<div class="mneme-feature">
  <div class="icon-wrap">
    <svg fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M3.75 13.5l10.5-11.25L12 10.5h8.25L9.75 21.75 12 13.5H3.75z"/></svg>
  </div>
  <h3>3-layer self-ping</h3>
  <p>Hooks fire on every prompt + Edit + Grep. Layer 3 soft-redirects symbol queries to <code>find_references</code>. Never blocks. Always fail-open.</p>
</div>

<div class="mneme-feature">
  <div class="icon-wrap">
    <svg fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M2.036 12.322a1.012 1.012 0 010-.639C3.423 7.51 7.36 4.5 12 4.5c4.638 0 8.573 3.007 9.963 7.178.07.207.07.431 0 .639C20.577 16.49 16.64 19.5 12 19.5c-4.638 0-8.573-3.007-9.963-7.178z"/><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"/></svg>
  </div>
  <h3>Vision SPA — 14 views</h3>
  <p>Force Galaxy, 3D Galaxy, Treemap, Sunburst, Hierarchy, Arc Chord, Layered Architecture, Sankey flows, Heatmap, Timeline scrubber. First-paint &lt; 500 ms.</p>
</div>

<div class="mneme-feature">
  <div class="icon-wrap">
    <svg fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/><path stroke-linecap="round" stroke-linejoin="round" d="M9 12l2 2 4-4"/></svg>
  </div>
  <h3>50 MCP tools</h3>
  <p>Recall, blast radius, call graph, find references, audit, snapshot, rewind, step ledger, why-chain, federated similar. Every tool local + deterministic + audited.</p>
</div>

<div class="mneme-feature">
  <div class="icon-wrap">
    <svg fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M16.5 10.5V6.75a4.5 4.5 0 10-9 0v3.75m-.75 11.25h10.5a2.25 2.25 0 002.25-2.25v-6.75a2.25 2.25 0 00-2.25-2.25H6.75a2.25 2.25 0 00-2.25 2.25v6.75a2.25 2.25 0 002.25 2.25z"/></svg>
  </div>
  <h3>Local only</h3>
  <p>Daemon binds to 127.0.0.1. Embeddings run via local ONNX Runtime. No telemetry, no cloud sync. Federation is opt-in and exchanges blake3 hashes only.</p>
</div>

</div>

## Install in one line

<div class="mneme-install-matrix">

<div class="mneme-install-card">
  <div class="platform">Linux<span class="badge">curl · bash</span></div>

```bash
curl -fsSL https://raw.githubusercontent.com/omanishay-cyber/mneme/main/release/install-linux.sh | bash
```

</div>

<div class="mneme-install-card">
  <div class="platform">macOS<span class="badge">curl · bash</span></div>

```bash
curl -fsSL https://raw.githubusercontent.com/omanishay-cyber/mneme/main/release/install-mac.sh | bash
```

</div>

<div class="mneme-install-card">
  <div class="platform">Windows<span class="badge">winget</span></div>

```powershell
winget install Anish.Mneme
```

</div>

<div class="mneme-install-card">
  <div class="platform">Windows<span class="badge">PowerShell</span></div>

```powershell
iwr -useb https://raw.githubusercontent.com/omanishay-cyber/mneme/main/release/bootstrap-install.ps1 | iex
```

</div>

<div class="mneme-install-card">
  <div class="platform">Any OS<span class="badge">pip</span></div>

```bash
pip install mnemeos
```

</div>

</div>

[Full install guide →](./install/index.html)

## Three things to read first

<ul class="mneme-readme-list">
  <li><a href="./concepts/architecture.html">
    <span class="num">01</span>
    <strong>Architecture</strong>
    <span>What's running on your machine, what data goes where, what doesn't go on the network.</span>
  </a></li>
  <li><a href="./concepts/resolver.html">
    <span class="num">02</span>
    <strong>Symbol resolver</strong>
    <span>The keystone of v0.4.0 — how Mneme answers "where is spawn?" with the right function row instead of a README chunk.</span>
  </a></li>
  <li><a href="./mcp/tools.html">
    <span class="num">03</span>
    <strong>MCP tools</strong>
    <span>The 50 tools your AI can call. Every one local, deterministic, and audited.</span>
  </a></li>
</ul>

<div class="mneme-callout">

**Local-only by design.** Nothing leaves your machine. The HTTP daemon binds to <code>127.0.0.1</code>. The embedding model runs locally via ONNX Runtime. The optional LLM (when enabled) runs locally via llama.cpp. The graph database is plain SQLite under <code>~/.mneme/</code>. No telemetry, no analytics, no cloud sync. The optional <code>federated_similar</code> tool exchanges blake3-hashed signatures only if you opt in, and even then it's machine-to-machine within your own infrastructure.

</div>

## License

[Apache-2.0](https://github.com/omanishay-cyber/mneme/blob/main/LICENSE) — free to use, modify, and ship inside commercial products. Built by Anish & Kruti Trivedi.
