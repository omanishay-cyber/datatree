# Vision SPA (14 views)

Mneme's web UI lives at `http://127.0.0.1:7777/`. 14 views over your code-graph, served by the daemon's HTTP layer (no separate frontend server). Built with React 18 + Sigma.js v3 (WebGL) + d3 + deck.gl.

## The 14 views

| View | What it shows |
|---|---|
| **Force Galaxy** | Force-directed graph of every node (16K+ on big repos). Genesis first-paint <500 ms via server-pre-computed sunflower-spiral seed. |
| **3D Galaxy** | Three.js orbital scene; pan + rotate + zoom for spatial exploration. |
| **Treemap** | Hierarchical area chart sized by file/directory weight. |
| **Sunburst** | Radial hierarchy (alternative treemap). |
| **Hierarchy Tree** | Expandable file tree with kind-coloured leaves. |
| **Arc Chord** | Cross-module call edges as a circular chord diagram. |
| **Layered Architecture** | Layers (UI → app → domain → infra) auto-detected from edge directions. |
| **Kind Flow** | Sankey: how nodes of one kind feed into nodes of another. |
| **Domain Flow** | Sankey: cross-domain edges (the surprising-connections view). |
| **Community Matrix** | Adjacency heatmap of detected communities. |
| **Heatmap** | File-level activity heatmap (commits × time). |
| **Test Coverage** | Per-file test density + assertion count. |
| **Theme Palette** | Audit findings tagged "theme" / UI consistency dashboard. |
| **Timeline Scrubber** | Git history scrubber — drag to see the graph at any past commit. |

## Empty state

When you open `http://127.0.0.1:7777/` with **no built project**, the SPA loads but the views are empty (zero nodes / edges). This is expected — Mneme needs at least one indexed project to populate.

```bash
cd ~/your-project
mneme build .
```

Then refresh. Or use `mneme view` from inside a project — it computes the project hash and opens the URL with `?project=<hash>` already attached.

## Project picker

If multiple projects are indexed, the SPA shows a dropdown to switch between them. The URL parameter `?project=<hash>` selects which `~/.mneme/projects/<hash>/graph.db` the views read from.

## Server-pre-computed layout (Genesis keystone)

Force Galaxy used to render with random initial positions + a 1-2 iteration FA2 warm-up before paint. On a 17K-node graph that was a 3-second white screen. Item #124 added `/api/graph/layout` — the server runs a deterministic community-aware sunflower spiral and returns `(qualified_name, x, y)` triples in the same window the SPA fetches. Sigma seeds positions from the snapshot before WebGL paints — first-paint drops to <500 ms.

The FA2 worker still runs for refinement after first paint, so the layout converges further over the first ~5 s. Fallback to random init when the layout endpoint is unavailable; the layout is a speed-up, never a correctness gate.

## API surface

The daemon exposes 17 endpoints under `/api/graph/*`. All return JSON, no auth (localhost-only), bound to `127.0.0.1:7777`.

| Endpoint | Purpose |
|---|---|
| `/api/health` | Liveness ping |
| `/api/graph/status` | Node/edge/file counts + last build timestamp |
| `/api/graph/nodes?limit=N` | Top-N nodes for the force view |
| `/api/graph/edges?limit=N` | Top-N edges scoped to the same node window |
| `/api/graph/layout?limit=N` | Pre-computed positions (Item #124) |
| `/api/graph/files?limit=N` | File metadata for the treemap |
| `/api/graph/findings` | Audit findings table |
| `/api/graph/file-tree` | Hierarchical file tree |
| `/api/graph/kind-flow` | Sankey edges per kind |
| `/api/graph/domain-flow` | Sankey edges per domain |
| `/api/graph/community-matrix` | Adjacency matrix per community |
| `/api/graph/commits` | Git commit timeline |
| `/api/graph/heatmap` | Activity heatmap data |
| `/api/graph/layers` | Layered architecture |
| `/api/graph/galaxy-3d` | 3D positions for the orbital view |
| `/api/graph/test-coverage` | Per-file test stats |
| `/api/graph/theme-palette` | Theme audit findings |
| `/api/graph/hierarchy` | Hierarchy tree data |

## Tauri shell

The `mneme-vision` binary in `~/.mneme/bin/` is a Tauri shell wrapping the same SPA. Use it on Windows to avoid browser tab clutter, or on macOS for a native window. It just proxies to `http://127.0.0.1:7777/` internally.

## Building from source

```bash
cd source/vision
bun install --frozen-lockfile
bun run build               # produces dist/ — copied into ~/.mneme/static/vision/ on install
```

## See also

- [Architecture](./architecture.md) — where the daemon's HTTP layer fits in
- [Genesis release](../releases/v0.4.0.md) — the layout endpoint + first-paint perf win
- [Troubleshooting](../troubleshooting.md) — when views go empty / port conflicts
