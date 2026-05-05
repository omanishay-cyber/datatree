<div class="mneme-hero">
<div class="mneme-hero-grid">
<div class="mneme-hero-left">

<span class="eyebrow">v0.4.0 — Keystone shipped</span>

<h1 class="hero-title">Stop grepping.<br />Start recalling.</h1>

<p class="lede">Mneme is a local-only memory layer for AI coding tools. Index your project once, then ask for any symbol, dependency, or call graph in seconds — across sessions, restarts, and compaction.</p>

<div class="cta-row">
  <a class="mneme-cta mneme-cta-primary" href="./install/index.html">Install in one line<span class="arrow">&nbsp;→</span></a>
  <a class="mneme-cta mneme-cta-text" href="./concepts/architecture.html">See how it works<span class="arrow">&nbsp;→</span></a>
</div>

<div class="mneme-stats">
  <div class="mneme-stat"><span class="num">50</span><span class="lbl">MCP tools</span></div>
  <div class="mneme-stat"><span class="num">14</span><span class="lbl">Graph views</span></div>
  <div class="mneme-stat"><span class="num">22</span><span class="lbl">Storage layers</span></div>
  <div class="mneme-stat"><span class="num">3</span><span class="lbl">Resolvers</span></div>
  <div class="mneme-stat"><span class="num">100%</span><span class="lbl">Local-only</span></div>
</div>

</div>
<div class="mneme-hero-right">

<div class="mneme-terminal">
  <div class="mneme-terminal-bar">
    <span class="dot"></span><span class="dot"></span><span class="dot"></span>
    <span class="title">~/code/your-project — mneme</span>
  </div>
  <div class="mneme-terminal-body">
<pre><code><span class="prompt">$</span> <span class="cmd">mneme recall_concept "spawn"</span>
  <span class="arrow">→</span>  <span class="out-strong">crate::manager::WorkerPool::spawn</span>
     <span class="path">supervisor/src/manager.rs:1100</span>
     <span class="out">callers:</span> <span class="num">12</span>  <span class="out">deps:</span> <span class="num">5</span>  <span class="out">tests:</span> <span class="num">3</span>

<span class="prompt">$</span> <span class="cmd">mneme blast manager.rs --depth=2</span>
  <span class="arrow">→</span>  <span class="out-strong">risk:</span> <span class="ok">moderate</span>  <span class="out">edges:</span> <span class="num">17</span>
     <span class="out">— supervisor/src/health.rs (4×)</span>
     <span class="out">— cli/src/commands/build.rs (2×)</span>
     <span class="out">— supervisor/src/api_graph.rs (1×)</span>

<span class="prompt">$</span> <span class="cmd">mneme why "Why does v0.4.0 exist?"</span>
  <span class="arrow">→</span>  <span class="out">audit measured recall</span> <span class="num">2/10</span><span class="out">.</span>
     <span class="out">v0.4.0 ships the keystone — three real</span>
     <span class="out">resolvers + symbol-anchored embeddings.</span>
     <span class="out-strong">→  ledger</span><span class="out">: keystone-2026-05-05</span><span class="mneme-terminal-cursor"></span></code></pre>
  </div>
</div>

</div>
</div>
</div>

<div class="mneme-marquee">
  <div class="mneme-marquee-track">
    <span>Local-only</span><span>50 MCP tools</span><span>3 resolvers</span><span>14 graph views</span><span>22 storage layers</span><span>Apache-2.0</span><span>No telemetry</span><span>Sub-second rebuild</span><span>Symbol-anchored embeddings</span>
    <span>Local-only</span><span>50 MCP tools</span><span>3 resolvers</span><span>14 graph views</span><span>22 storage layers</span><span>Apache-2.0</span><span>No telemetry</span><span>Sub-second rebuild</span><span>Symbol-anchored embeddings</span>
  </div>
</div>

<div class="mneme-trust-strip">
  <div class="mneme-trust-item"><span class="trust-num">3 resolvers</span><span class="trust-lbl">Rust · TypeScript · Python (v0.4.0 audit)</span></div>
  <div class="mneme-trust-item"><span class="trust-num">&lt; 500 ms</span><span class="trust-lbl">Server-rendered first paint on a 50k-node graph</span></div>
  <div class="mneme-trust-item"><span class="trust-num">0 bytes</span><span class="trust-lbl">Outbound on the default install</span></div>
</div>

<h2 class="mneme-section-heading">What's inside</h2>

<p class="mneme-section-sub">A daemon, a graph, three resolvers, and 50 MCP tools your AI host can call. Everything runs on your machine.</p>

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

<h2 class="mneme-section-heading">Frequently asked</h2>

<div class="mneme-faq">

<details class="mneme-faq-item">
<summary><span>Does Mneme send any data off my machine?</span><span class="chev">+</span></summary>
<div class="answer">

The default install never opens an outbound connection. The daemon binds to <code>127.0.0.1</code>. Embeddings run locally via ONNX Runtime, the optional LLM via llama.cpp, and the graph lives in plain SQLite under <code>~/.mneme/</code>. The only opt-in network feature is <code>federated_similar</code>, which exchanges blake3-hashed signatures only between machines you control — and only if you turn it on.

</div>
</details>

<details class="mneme-faq-item">
<summary><span>How is this different from grep, ripgrep, or my IDE's "Find References"?</span><span class="chev">+</span></summary>
<div class="answer">

Grep is text matching. Mneme is a real symbol resolver — it knows that <code>super::spawn</code>, <code>crate::manager::spawn</code>, and a re-exported <code>spawn()</code> are the same logical symbol, and it indexes their callers, dependencies, and tests in a graph database. Your IDE's "Find References" only sees the file you have open; Mneme sees the whole project at once and exposes it through 50 MCP tools your AI host can call.

</div>
</details>

<details class="mneme-faq-item">
<summary><span>Which AI hosts work with Mneme?</span><span class="chev">+</span></summary>
<div class="answer">

Anything that speaks the Model Context Protocol (MCP) — Claude Code, Claude Desktop, Continue, Cursor, and the growing list of MCP-aware IDEs. Mneme registers an MCP server that exposes 50 tools and stays running in the background. See the <a href="./mcp/tools.html">MCP tools page</a> for the full registry.

</div>
</details>

<details class="mneme-faq-item">
<summary><span>How do I update? Will my graph survive?</span><span class="chev">+</span></summary>
<div class="answer">

Run <code>mneme update</code>. v0.4.0 ships an apply-with-rollback updater: it downloads the new binary, runs a post-swap health check, and restores the old binary automatically if the check fails. Your graph survives the update — the schema is forward-only, never drops columns, never renames. On v0.4.0 specifically, run <code>mneme rebuild</code> once after upgrade to pick up the symbol-anchored embeddings.

</div>
</details>

</div>

<h2 class="mneme-section-heading">Why Mneme</h2>

<p>When you ask an AI "where does <code>WorkerPool::spawn</code> get called?", the cheap answer is regex over text. The slightly less-cheap answer is grep. Both miss <code>super::spawn</code>, <code>crate::manager::spawn</code>, <code>use crate::manager; spawn()</code>, and aliased re-exports. Mneme answers with structural certainty — parser-built call graphs, symbol resolver, BGE embeddings anchored on canonical names — all in a daemon the AI talks to via MCP.</p>

<p>The 2026-05-05 audit comparing Mneme to CRG and graphify identified one root cause behind both the recall gap (Mneme 2/10 vs CRG 6/10) and the token gap (Mneme 1.34× vs CRG's claimed 6.8×): no symbol resolver. v0.4.0 ships the keystone.</p>

<div class="mneme-callout">
<div class="mneme-callout-title"><span class="dot"></span>Local-only by design</div>

<p>Nothing leaves your machine. The HTTP daemon binds to <code>127.0.0.1</code>. The embedding model runs locally via ONNX Runtime. The optional LLM runs locally via llama.cpp. The graph database is plain SQLite under <code>~/.mneme/</code>. <strong>No telemetry, no analytics, no cloud sync.</strong></p>

</div>

<div class="mneme-final-cta">
  <h2>Ready to give your AI a memory?</h2>
  <p>Install once, run forever. The graph stays fresh as you edit. Apache-2.0, no signup, no telemetry.</p>
  <a class="mneme-cta mneme-cta-primary mneme-cta-large" href="./install/index.html">Install in one line<span class="arrow">&nbsp;→</span></a>
  <p class="final-cta-meta">or <a href="./releases/v0.4.0.html">read the v0.4.0 release notes</a></p>
</div>

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
