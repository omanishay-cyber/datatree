# mneme-vision

The mneme **vision app** — 14-view live graph + Command Center. Two
processes run side by side:

| Process | Default port | Script | Purpose |
|---|---:|---|---|
| Vite SPA  | `5173` | `bun run dev`   | React + sigma + deck.gl UI |
| Bun API   | `7777` | `bun run serve` | Reads `~/.mneme/projects/<project-id>/*.db` and streams graph data |

The SPA fetches from the API at `http://localhost:7777`. Both must be
running for the graph to render.

## Quick start

```bash
# Install dependencies (Bun is required; see https://bun.sh)
bun install

# Run BOTH processes at once (recommended)
bun run dev:full
# This wraps `concurrently` and starts:
#   - vite on :5173 (the UI)
#   - bun server.ts on :7777 (the API)

# Or run them in two separate terminals:
#   Terminal 1 → bun run dev
#   Terminal 2 → bun run serve
```

Visit `http://localhost:5173` once both are up.

## Scripts

| Script | What it does |
|---|---|
| `bun run dev`        | Vite dev server only (UI without live data — useful for pure CSS work) |
| `bun run dev:full`   | UI **and** API together via `concurrently` |
| `bun run serve`      | API only (`server.ts` — reads the per-project SQLite shards) |
| `bun run build`      | Production build of the SPA into `dist/` |
| `bun run preview`    | Serve the production build locally |
| `bun run typecheck`  | `tsc --noEmit` against `src/`, `server.ts`, `vite.config.ts` |
| `bun run lint`       | `biome check src` |
| `bun run format`     | `biome format --write src` |
| `bun run tauri:dev`  | Tauri shell (delegates to `tauri/`) |
| `bun run tauri:build`| Tauri release build |

## Where data lives

The API server reads project shards from the user's local
`PathManager` root:

- `~/.mneme/projects/<project-id>/graph.db`     — nodes + edges
- `~/.mneme/projects/<project-id>/history.db`   — conversation log
- `~/.mneme/projects/<project-id>/findings.db`  — drift findings
- `~/.mneme/projects/<project-id>/wiki.db`      — generated wiki pages
- ... and ~22 more shards (see `common/src/layer.rs`)

Older installs that pre-date the rename may have data under
`~/.datatree/projects/`. The API server transparently reads either
location (see `server/shard.ts`), with `~/.mneme/` taking precedence.

## Architecture

The vision app is a **read-only window** onto the local mneme shards.
It never writes to the supervisor. All mutations flow through the CLI
or the MCP server. This keeps the WAL-protected single-writer
invariant intact.

| Layer | Tech |
|---|---|
| UI                  | React 18 + TypeScript + Vite 5 |
| Graph rendering     | sigma 3 (force-galaxy), deck.gl 9 (geo views), three 0.165 (3D) |
| Layout / data       | graphology + graphology-layout-forceatlas2, d3 / d3-sankey |
| State               | zustand |
| Animation           | framer-motion |
| API server          | Bun (`server.ts`) reading SQLite shards via `bun:sqlite` |
| Desktop shell       | Tauri 2 (in `tauri/`) |

## Development tips

- The **typecheck** script enforces strict TS and `noUncheckedIndexedAccess`.
  Run it before any PR — `bun run typecheck` should produce zero output.
- The Bun API server hot-reloads on `server.ts` save (Bun built-in).
- The Vite SPA hot-reloads on any `src/**/*.{ts,tsx,css}` save.
- For Tauri development, see `tauri/README.md` (or
  `bun run tauri:dev`).

## Related docs

- [`../README.md`](../README.md) — top-level Mneme overview
- [`../ARCHITECTURE.md`](../ARCHITECTURE.md) — system architecture
- [`../docs/dev-setup.md`](../docs/dev-setup.md) — full dev setup
- [`tauri/`](./tauri/) — desktop wrapper
