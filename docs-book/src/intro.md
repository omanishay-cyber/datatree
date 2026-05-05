<div class="mneme-hero">

<span class="eyebrow">v0.4.0 — Keystone shipped</span>

<h1 class="hero-title">The AI superbrain<br />for your codebase.</h1>

<p class="lede">Mneme is a local-only memory layer for AI coding tools. It indexes your project once, then keeps the graph fresh as you edit. Your AI sees the code through Mneme's lens — not through ad-hoc grep — and gets the same answers across sessions, restarts, and compaction.</p>

<div class="cta-row">
  <a class="mneme-cta mneme-cta-primary" href="./install/index.html">Install in one line<span class="arrow">&nbsp;→</span></a>
  <a class="mneme-cta mneme-cta-secondary" href="./concepts/architecture.html">Read the architecture<span class="arrow">&nbsp;→</span></a>
</div>

<div class="mneme-stats">
  <div class="mneme-stat"><span class="num">50</span><span class="lbl">MCP tools</span></div>
  <div class="mneme-stat"><span class="num">14</span><span class="lbl">Graph views</span></div>
  <div class="mneme-stat"><span class="num">22</span><span class="lbl">Storage layers</span></div>
  <div class="mneme-stat"><span class="num">3</span><span class="lbl">Languages indexed</span></div>
  <div class="mneme-stat"><span class="num">100%</span><span class="lbl">Local-only</span></div>
</div>

<div class="mneme-terminal">
  <div class="mneme-terminal-bar">
    <span class="dot"></span><span class="dot"></span><span class="dot"></span>
    <span class="title">~/code/your-project — mneme</span>
  </div>
  <div class="mneme-terminal-body">
<pre><code><span class="prompt">$</span> <span class="cmd">mneme recall_concept "spawn"</span>
  <span class="arrow">→</span>  <span class="out-strong">crate::manager::WorkerPool::spawn</span>  <span class="path">supervisor/src/manager.rs:1100</span>
     <span class="out">pub async fn spawn(&amp;self, job: Job) -&gt; Result&lt;JobId&gt;</span>
     <span class="out">callers:</span> <span class="num">12</span>  <span class="out">dependents:</span> <span class="num">5</span>  <span class="out">tests:</span> <span class="num">3</span>

<span class="prompt">$</span> <span class="cmd">mneme blast supervisor/src/manager.rs --depth=2</span>
  <span class="arrow">→</span>  <span class="out-strong">risk:</span> <span class="ok">moderate</span>  <span class="out">  edges to refactor:</span> <span class="num">17</span>
     <span class="out">— supervisor/src/health.rs    (calls spawn 4×)</span>
     <span class="out">— cli/src/commands/build.rs   (calls spawn 2× in run_dispatched)</span>
     <span class="out">— supervisor/src/api_graph.rs (1 indirect through worker pool)</span>

<span class="prompt">$</span> <span class="cmd">mneme why "Why does v0.4.0 exist?"</span>
  <span class="arrow">→</span>  <span class="out">2026-05-05 audit measured recall</span> <span class="num">2/10</span> <span class="out">vs CRG</span> <span class="num">6/10</span><span class="out">.</span>
     <span class="out">Root cause: no symbol resolver. v0.4.0 ships the keystone —</span>
     <span class="out">three real per-language resolvers plus symbol-anchored embeddings.</span>
     <span class="out-strong">→  ledger</span><span class="out">: keystone-2026-05-05</span><span class="mneme-terminal-cursor"></span></code></pre>
  </div>
</div>

</div>

<h2 class="mneme-section-heading">What's inside</h2>

<p class="mneme-section-sub">A daemon, a graph, a resolver, and the MCP tools your AI host can call. Everything runs on your machine.</p>

<div class="mneme-features">

<div class="mneme-feature">
  <div class="icon">SR</div>
  <h3>Symbol resolver</h3>
  <p>Three real resolvers — Rust, TypeScript, Python — turn syntactic names like <code>spawn</code>, <code>super::spawn</code>, and <code>crate::manager::spawn</code> into one canonical string per logical symbol. The keystone of v0.4.0.</p>
</div>

<div class="mneme-feature">
  <div class="icon">PT</div>
  <h3>Soft-redirect hooks</h3>
  <p>When the AI calls <code>Grep</code> on something resolver-shaped, the PreToolUse hook injects a hint pointing at <code>find_references</code>. Never blocks. Always fail-open.</p>
</div>

<div class="mneme-feature">
  <div class="icon">VG</div>
  <h3>Vision SPA</h3>
  <p>14 graph views — call graph, dependency mesh, force-directed galaxy, time-travel, treemap, sunburst, sankey — paint locally in under 500&nbsp;ms.</p>
</div>

<div class="mneme-feature">
  <div class="icon">MC</div>
  <h3>50 MCP tools</h3>
  <p>Every query the AI needs: <code>recall_concept</code>, <code>find_references</code>, <code>blast_radius</code>, <code>call_graph</code>, <code>audit_*</code>. Local. Deterministic. Audited.</p>
</div>

<div class="mneme-feature">
  <div class="icon">LO</div>
  <h3>Local-only by design</h3>
  <p>Daemon binds to <code>127.0.0.1</code>. Embeddings via ONNX Runtime, the optional LLM via llama.cpp, the graph in plain SQLite under <code>~/.mneme/</code>. No telemetry. No cloud sync.</p>
</div>

<div class="mneme-feature">
  <div class="icon">HR</div>
  <h3>Hot rebuild on edit</h3>
  <p>The daemon watches your tree. Save a file and the affected slice of the graph re-builds in under a second. Symbol-anchored embeddings keep the same canonical name across edits.</p>
</div>

</div>

<h2 class="mneme-section-heading">Install in one line</h2>

<p class="mneme-section-sub">Pick your platform. The script downloads a signed binary, drops it on PATH, and runs <code>mneme doctor</code> to verify the install.</p>

<div class="mneme-install-matrix">

<div class="mneme-install-card">
  <div class="platform"><span>Linux</span><span class="badge">curl</span></div>

```bash
curl -fsSL https://raw.githubusercontent.com/omanishay-cyber/mneme/main/release/install-linux.sh | bash
```

</div>

<div class="mneme-install-card">
  <div class="platform"><span>macOS</span><span class="badge">curl</span></div>

```bash
curl -fsSL https://raw.githubusercontent.com/omanishay-cyber/mneme/main/release/install-mac.sh | bash
```

</div>

<div class="mneme-install-card">
  <div class="platform"><span>Windows</span><span class="badge">PowerShell</span></div>

```powershell
iwr -useb https://raw.githubusercontent.com/omanishay-cyber/mneme/main/release/bootstrap-install.ps1 | iex
```

</div>

<div class="mneme-install-card">
  <div class="platform"><span>Python</span><span class="badge">pip</span></div>

```bash
pip install mnemeos
```

</div>

<div class="mneme-install-card">
  <div class="platform"><span>Windows</span><span class="badge">winget</span></div>

```powershell
winget install Anish.Mneme
```

</div>

</div>

<p style="margin-top:14px"><a href="./install/index.html">Full install guide, including air-gapped setups →</a></p>

<h2 class="mneme-section-heading">Three things to read first</h2>

<div class="mneme-read-first">

<a class="mneme-read-card" href="./concepts/architecture.html">
  <span class="step">01</span><span class="step-title">Architecture</span><span class="arrow">→</span>
  <span class="step-desc">What's running on your machine, what data goes where, what doesn't go on the network.</span>
</a>

<a class="mneme-read-card" href="./concepts/resolver.html">
  <span class="step">02</span><span class="step-title">Symbol resolver</span><span class="arrow">→</span>
  <span class="step-desc">The keystone of v0.4.0. The reason your AI gives a sharper answer than yesterday.</span>
</a>

<a class="mneme-read-card" href="./mcp/tools.html">
  <span class="step">03</span><span class="step-title">MCP tools</span><span class="arrow">→</span>
  <span class="step-desc">The 50 tools your AI can call. Every one local, deterministic, and audited.</span>
</a>

</div>

<h2 class="mneme-section-heading">Why Mneme</h2>

When you ask an AI "where does `WorkerPool::spawn` get called?", the cheap answer is regex over text. The slightly less-cheap answer is grep. Both miss `super::spawn`, `crate::manager::spawn`, `use crate::manager; spawn()`, and aliased re-exports. Mneme answers with structural certainty — parser-built call graphs, symbol resolver, BGE embeddings anchored on canonical names — all in a daemon the AI talks to via MCP.

```text
mneme recall_concept "spawn"
  →  WorkerPool::spawn  (supervisor/src/manager.rs:1100)
     pub async fn spawn(&self, job: Job) -> Result<JobId>
     [callers: 5, dependents: 12, tests: 3]
```

The 2026-05-05 audit comparing Mneme to CRG and graphify identified one root cause behind both the recall gap (Mneme 2/10 vs CRG 6/10) and the token gap (Mneme 1.34× vs CRG's claimed 6.8×): no symbol resolver. v0.4.0 ships the keystone.

<div class="mneme-callout">
<div class="mneme-callout-title"><span class="dot"></span>Local-only by design</div>

<p>Nothing leaves your machine. The HTTP daemon binds to <code>127.0.0.1</code>. The embedding model runs locally via ONNX Runtime. The optional LLM runs locally via llama.cpp. The graph database is plain SQLite under <code>~/.mneme/</code>. <strong>No telemetry, no analytics, no cloud sync.</strong></p>

<p>The optional <code>federated_similar</code> tool exchanges blake3-hashed signatures only if you opt in, and only between machines you control. The default install never opens an outbound connection.</p>
</div>

[Read the v0.4.0 release notes &rarr;](./releases/v0.4.0.html)

<div class="mneme-footer">
  <div class="footer-meta">
    <strong>Mneme</strong> &middot; built by <strong>Anish Trivedi & Kruti Trivedi</strong> &middot; <span class="version-pill">v0.4.0</span>
  </div>
  <div class="footer-links">
    <a href="https://github.com/omanishay-cyber/mneme">GitHub</a>
    <a href="https://github.com/omanishay-cyber/mneme/blob/main/LICENSE">Apache-2.0</a>
    <a href="./releases/changelog.html">Changelog</a>
    <a href="./contributing.html">Contributing</a>
  </div>
</div>
