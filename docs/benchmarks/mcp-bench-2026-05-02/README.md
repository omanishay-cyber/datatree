# MCP Bench - 2026-05-02

Comparison of four code-graph MCP servers, run through Claude Code 2.1.126 on
a Windows 11 AWS test instance. Same model, same prompt, same project, isolated
per MCP via `--strict-mcp-config`.

See the **Comparison: four code-graph MCPs** section in the [project README](../../../README.md)
for the headline numbers and verdicts.

## What's in here

- [`queries.json`](queries.json) - the five standardized prompts, each scored against a hand-curated ground-truth list
- [`ground-truth.md`](ground-truth.md) - hand-curated expected results per query
- [`mcp-mneme-only.json`](mcp-mneme-only.json), [`mcp-treesitter-only.json`](mcp-treesitter-only.json), [`mcp-crg-only.json`](mcp-crg-only.json), [`mcp-graphify-only.json`](mcp-graphify-only.json) - the four `--strict-mcp-config` JSON files (one MCP per query, no built-in `Read`/`Grep`/`Glob` allowed)
- [`*-mcp-wrapper.cmd`](.) - cwd-fixing CMD wrappers for the four MCP servers
- [`run-query.ps1`](run-query.ps1) - per-query runner, captures wall time + JSON envelope from `claude --print --output-format json`
- [`run-all-bench.ps1`](run-all-bench.ps1) - matrix runner (5 queries x 4 MCPs = 20 cells; supports `BENCH_MCPS` and `BENCH_QUERIES` env-var filtering)
- [`score-result.ps1`](score-result.ps1) - auto-scorer (counts ground-truth markers in each response, 0-10)
- [`final-table.ps1`](final-table.ps1) - reporting helper that emits the 4-column markdown table
- [`results/`](results/) - raw `*.json` envelopes (one per `(MCP, query)` cell) plus the exact prompt text fed to Claude

## Corpus

The original bench used an Electron + React + TypeScript codebase that lives
on a separate AWS test instance. For the 2026-05-02 re-run, the corpus was
the **mneme workspace itself** at
`D:\...\source` (Rust + TypeScript + Python, 50K+ LOC, 400+ files). The same
corpus was indexed by all four MCPs before the queries ran:

- mneme: `mneme build .` (4 380 files indexed, 13 graphs assembled)
- tree-sitter: per-query parse (no persistent index)
- code-review-graph: `code-review-graph build` (4 180 nodes, 37 171 edges)
- graphify: `graphify update .` (3 929 nodes, 7 196 edges)

The substitution is documented in [`ground-truth.md`](ground-truth.md) -
ground-truth markers were rewritten to match mneme-workspace symbols
(`PathManager`, `DbBuilder::build_or_migrate`, `Store::open`, `worker_ipc`,
`livebus`, etc.) instead of the Electron app's auth symbols.

## How to reproduce

```powershell
# On the host with Claude Code, mneme, tree-sitter MCP, code-review-graph,
# and mcp-graphify-autotrigger installed, with each MCP's index already built
# against the corpus directory:
pwsh ./run-all-bench.ps1 -BenchDir <bench-dir> -ProjectDir <corpus-dir> -TimeoutSec 600
pwsh ./final-table.ps1 -ResultsDir <bench-dir>/results
```

Per-query timeout was 600 s. Filter via `$env:BENCH_MCPS = 'tree-sitter'` or
`$env:BENCH_QUERIES = 'Q1,Q3'` to run a subset.

## Per-MCP notes

- **mneme MCP** finished every cell inside its 600 s budget at the lowest
  total cost ($4.86) and the lowest output token count (25,796) on the
  panel after the symbol- and path-resolution fixes landed on 2026-05-03.
  Full citations on Q1, Q2, Q5 (9/10) and Q4 (8/10). Q3 is partial (5/10):
  the on-disk graph holds 68,495 call edges between TypeScript nodes and
  the Rust parser emits structural `contains` edges only, so a Rust-to-Rust
  call traversal returns empty even when the file-level citations are
  correct. Tracked for v0.3.3.
- **tree-sitter** wins on raw recall (9.0 avg) by re-parsing on demand, but
  spends 1.4× the cost and 1.8× the tokens mneme uses to do it. Q5 takes
  246 s versus mneme's 108 s. With the new 600 s budget, the cell that
  previously timed out now returns a strong concurrency answer.
- **CRG** answered 3 of 5 with rich citations (9/10 on Q1, Q3, Q4). Q2
  scored 5 (the graph has no `IMPORTS_FROM` edges so blast-radius
  propagates only via call edges - a partial answer with valid citations).
  Q5 ran past the 600 s budget.
- **graphify** jumps from 0/5 to 4/5 9-scores on this run after switching
  the MCP wrapper from the autotrigger fork (`mcp-graphify-autotrigger
  0.3.0`, broken on `fastmcp 3.x`) to the official `graphifyy 0.6.7+`
  stdio server (`python -m graphify.serve <graph.json>`). Q5 is partial
  (5/10) because the graph indexes structural edges only.

Every cell in the published table is a measured number from a real Claude
process exit - no placeholders, no "(skipped)" cells, no em-dash gaps. The
auto-scorer caps any answer that explicitly admits "cannot answer" at 5/10
even when it cites real symbols, so a 5 here means a partial answer with
valid citations, not a wrong one. A 0 score with a 600 s wall is what the
auto-scorer counted in the response.
