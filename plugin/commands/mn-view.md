---
name: /mn-view
description: Open the mneme live graph viewer (14 view modes). Tauri desktop app by default; falls back to localhost:7777 web view.
command: mneme view
---

# /mn-view

Open the live mneme graph viewer. Renders the per-project knowledge
graph in any of 14 view modes (Force-Galaxy, Hierarchy Tree, Sunburst,
Treemap, Sankey type/domain, Arc/Chord, Timeline, Heatmap Grid, Layered
Architecture, 3D Galaxy, Theme Palette, Test Coverage, Risk Dashboard).

## Usage

```
/mn-view                          # opens the desktop viewer at default view
/mn-view --view force-galaxy      # specific view
/mn-view --filter "src/auth/**"   # filter the graph
/mn-view --web                    # serve at localhost:7777 instead
/mn-view --export svg --out g.svg # export instead of opening
```

## What this does

1. Resolves the project shard via `mneme finder find-by-cwd`.
2. Spawns the Tauri viewer (or `serve --web` if `--web`).
3. Connects the viewer to the live-bus over WebSocket so node states
   update in real time as files change.

## Interactions inside the viewer

- Hover → tooltip with file/lines/last-commit/blast-radius
- Click → side panel (file content + summary + tests + history)
- Right-click → context menu (open in editor, find references, run audit)
- Cmd+click → multi-select for combined blast radius
- Drag → physics ripple through dependents
- Lasso → "audit this region against my rules"
- Bottom slider → time-machine scrub
- Toolbar → toggle AI overlays (concept clusters, drift heatmap, risk)
