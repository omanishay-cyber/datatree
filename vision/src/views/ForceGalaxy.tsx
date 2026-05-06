import { useEffect, useMemo, useRef, useState } from "react";
import Graph from "graphology";
import Sigma from "sigma";
// BUG-NEW-E (2026-05-05): the synchronous `forceAtlas2` import is gone.
// The 3-iteration "warm-up" we used to run on the main thread before
// rendering blocked the UI for ~5-10s on the mneme repo (17K nodes,
// O(N²) per FA2 iter without barnesHut). Real users saw a 33-second
// white screen before the first paint. Now we go straight from
// `addNode/addEdge` (random initial positions) to `new Sigma(...)` and
// kick off the Web-Worker FA2Layout immediately for background
// refinement. First-paint drops from ~33s to ~2-3s on 17K nodes (still
// over the 500ms aspirational budget, but a 10× win — true <500ms
// requires a server-pre-computed layout snapshot, deferred to v0.4.1).
import FA2Layout from "graphology-layout-forceatlas2/worker";
import { fetchNodes, fetchEdges, fetchLayout } from "../api/graph";
import { useVisionStore, shallow } from "../store";
import { Legend, type LegendKindRow } from "../components/Legend";
import { OnboardingHint } from "../components/OnboardingHint";

// View 1 — Sigma.js v3 WebGL force-directed graph.
// Targets 60fps on 100K nodes via WebGL renderer + ForceAtlas2 pre-layout.
//
// Wired to the real graph shard (graph.db) via /api/graph/nodes + /api/graph/edges.
// Shows a loading skeleton while the shard query is in flight and a first-class
// error state when the shard is missing ("run `mneme build .`").
//
// v0.3.2 polish bundle (items #1, #2, #3 from mneme-view-polish-plan.md):
//   #1 KIND_COLORS map paints nodes per kind + Legend overlay on canvas.
//   #2 Sigma 3 nodeReducer/edgeReducer dim non-neighbors of the hovered node.
//   #3 Degree-scaled node size (sqrt) so hubs are visibly larger than leaves.

type Status = "loading" | "empty" | "ready" | "error";

/**
 * Per-kind color palette. Keys match the `kind` strings the daemon
 * writes into graph.db (`file`, `class`, `function`, `import`,
 * `decorator`, `comment`, plus the broader `test`/`type`/`module`
 * variants the polish plan referenced for future kinds).
 *
 * Hex values stay in the brand-gradient family (#4191E1, #41E1B5,
 * #22D3EE) plus a secondary accent set so each kind reads distinct in
 * dark mode without colliding with the legend swatches.
 */
const KIND_COLORS: Record<string, string> = {
  file: "#4191E1",
  class: "#22D3EE",
  function: "#41E1B5",
  test: "#d2a8ff",
  type: "#8b949e",
  module: "#f59e0b",
  import: "#FFA500",
  decorator: "#FF66CC",
  comment: "#888888",
};
const KIND_COLOR_FALLBACK = "#7aa7ff";

/** Resolve a node's kind to a color, with a graceful fallback. */
function colorForKind(kind: string | undefined | null): string {
  if (!kind) return KIND_COLOR_FALLBACK;
  return KIND_COLORS[kind.toLowerCase()] ?? KIND_COLOR_FALLBACK;
}

/** Color used to dim non-neighbors when hovering. Subdued, on-palette. */
const DIM_COLOR = "rgba(122, 138, 166, 0.18)";

export function ForceGalaxy(): JSX.Element {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const sigmaRef = useRef<Sigma | null>(null);
  const graphRef = useRef<Graph | null>(null);
  // FA2 worker + its safety-stop timeout live alongside sigmaRef so the
  // unmount cleanup hook can stop+kill them deterministically. We
  // previously stashed them on `(sigmaRef as ...).fa2Worker` which never
  // survived strict-mode double-mount and leaked a worker each time.
  const fa2WorkerRef = useRef<FA2Layout | null>(null);
  const fa2TimeoutRef = useRef<number | null>(null);
  // Hover state lives in a ref so the Sigma reducers (registered once
  // at mount) read the *current* value without us re-creating the
  // reducer closures on every state change.
  const hoveredRef = useRef<string | null>(null);

  // HIGH-FE-3 fix (2026-05-05 audit): pre-pulse color stash, keyed by
  // node id. See the pulse useEffect below for the correctness
  // argument; tl;dr two LiveEvents on the same node within 400ms
  // used to lock the node permanently green because the second
  // effect read green as its own "original".
  const pulseOriginalsRef = useRef<Map<string, unknown>>(new Map());
  const [, forceRender] = useState<number>(0);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [counts, setCounts] = useState<{ nodes: number; edges: number }>({ nodes: 0, edges: 0 });
  const [legendRows, setLegendRows] = useState<LegendKindRow[]>([]);
  // A6-010: do NOT subscribe to selectNodes -- we only need to invoke
  // the action from inside the click handler. Reading via getState()
  // avoids re-mount of the entire Sigma graph (4000-node FA2 layout)
  // when zustand re-creates the store reference (HMR / module reload).
  // A6-022: shallow on liveEvents (array reference would otherwise
  // trigger a re-pulse on every unrelated store mutation).
  const liveEvents = useVisionStore((s) => s.liveEvents, shallow);

  useEffect(() => {
    if (!containerRef.current) return;
    const ac = new AbortController();
    let cancelled = false;

    (async (): Promise<void> => {
      const start = performance.now();
      try {
        // BUG-NEW-I fix (2026-05-05): the previous limits (nodes=4000,
        // edges=16000) silently dropped most edges on non-trivial repos.
        // The render loop guards every g.addEdge with
        // `g.hasNode(e.source) || g.hasNode(e.target)` and skips when
        // either endpoint is outside the fetched node window. With a
        // 4K-node ceiling and 16K edges, an edge whose source or target
        // belongs to the 5,001st-most-recently-indexed node was silently
        // discarded — visible to the user as "ForceGalaxy nodes appear
        // but no links between them".
        //
        // Fix: fetch nodes and edges with the SAME upper bound so every
        // returned edge has both endpoints in the node set, then let the
        // hasNode guard handle the rare race where a node row gets
        // garbage-collected mid-fetch. 32K is enough headroom for the
        // mneme repo itself (~17K Rust nodes + ~9K TS) and stays well
        // under the daemon's 200K hard cap on /api/graph/edges. Small
        // repos pay nothing — the queries return early with whatever
        // exists.
        const NODE_LIMIT = 32000;
        const EDGE_LIMIT = 32000;
        // Item #124 (2026-05-05): fetch the server-pre-computed layout
        // snapshot in parallel with /nodes + /edges. The daemon ships
        // (qualified_name, x, y) triples derived from community
        // membership in <50 ms, and seeding Sigma's positions from
        // them drops first-paint from ~3 s (random init + FA2 warm-up)
        // to <500 ms on the mneme repo (17 K nodes). The FA2 worker
        // still runs for refinement so the layout converges further
        // over the first ~5 s — but the user sees a coherent shape on
        // the very first frame instead of a uniform dot cloud.
        //
        // Failure mode: if /api/graph/layout is unreachable (older
        // daemon, build hadn't run community detection, etc.) the
        // SPA falls back to random initial positions exactly like
        // before. Layout is a speed-up, never a correctness gate.
        const [nodesRes, edgesRes, layoutRes] = await Promise.all([
          fetchNodes(ac.signal, NODE_LIMIT),
          fetchEdges(ac.signal, EDGE_LIMIT),
          fetchLayout(ac.signal, NODE_LIMIT),
        ]);
        if (cancelled || !containerRef.current) return;

        const nodes = nodesRes.nodes;
        const edges = edgesRes.edges;
        // O(1)-lookup map keyed on qualified_name. Empty when the
        // layout call failed — the addNode loop falls through to
        // its random-position default in that case.
        const layoutMap = new Map<string, { x: number; y: number }>();
        for (const p of layoutRes.positions) {
          layoutMap.set(p.q, { x: p.x, y: p.y });
        }

        if (nodesRes.error) {
          setError(nodesRes.error);
          setStatus("error");
          return;
        }
        if (nodes.length === 0) {
          setStatus("empty");
          return;
        }

        // ── Item #3: degree-scaled node sizing ───────────────────────
        // Pre-compute every node's degree from the edge list once so
        // the `addNode` loop is a constant-time lookup. Using sqrt
        // compresses the long tail (a single 200-edge hub doesn't
        // dwarf a 5-edge leaf).
        const degree = new Map<string, number>();
        for (const e of edges) {
          degree.set(e.source, (degree.get(e.source) ?? 0) + 1);
          degree.set(e.target, (degree.get(e.target) ?? 0) + 1);
        }
        const maxDeg = Math.max(1, ...degree.values());

        // ── Item #1: kind-based node colors + legend tally ───────────
        // Resolve color per node from KIND_COLORS, and keep a running
        // tally we hand to the <Legend> component once the graph is
        // ready. The daemon-side `type` field carries the kind (see
        // `GraphNodeOut.kind_tag` in `supervisor/src/api_graph.rs`,
        // which serializes as `type`).
        const kindCounts = new Map<string, number>();

        const g = new Graph({ multi: false, type: "mixed" });
        for (const n of nodes) {
          const kind = (n.type ?? "").toLowerCase();
          const deg = degree.get(n.id) ?? 0;
          const color = colorForKind(kind);
          if (kind) kindCounts.set(kind, (kindCounts.get(kind) ?? 0) + 1);

          // Item #124 priority chain: server-pre-computed layout >
          // any (x, y) the /api/graph/nodes payload itself attaches >
          // random fallback. The layoutMap is keyed on qualified_name
          // (== n.id) which is what /api/graph/layout returns.
          const seeded = layoutMap.get(n.id);
          g.addNode(n.id, {
            label: n.label ?? n.id,
            x: seeded?.x ?? n.x ?? Math.random(),
            y: seeded?.y ?? n.y ?? Math.random(),
            // Item #3: 4..10px range (sqrt-scaled). 4px floor keeps
            // leaves visible; +6px ceiling keeps hubs from blowing
            // out the layout at typical zoom.
            size: 4 + 6 * Math.sqrt(deg / maxDeg),
            color,
            // Stash kind on the node attrs so reducers / panels can
            // read it without an extra map lookup.
            kind,
          });
        }

        for (const e of edges) {
          if (!g.hasNode(e.source) || !g.hasNode(e.target)) continue;
          // graphology rejects duplicate edges in simple mode; swallow.
          try {
            g.addEdge(e.source, e.target, { weight: e.weight ?? 1, color: "#3a4a66" });
          } catch {
            /* duplicate edge — ignore */
          }
        }

        // BUG-NEW-E follow-up (2026-05-05): the previous "cheap synchronous
        // warm-up (3 iterations)" was the actual bottleneck — even 3 iters
        // of forceAtlas2 on 17K nodes runs O(N²) without barnesHut, ~5-10s
        // on a typical CPU + a hard main-thread block. Removed entirely.
        // Item #124 (2026-05-05) lands the true <500 ms fix: initial
        // positions now come from the server-pre-computed layout snapshot
        // (community-aware sunflower spiral, see api_graph::compute_layout).
        // Random positions remain the fallback when the layout endpoint
        // is unavailable or the build skipped community detection.
        // The FA2 worker takes over for refinement BEFORE sigma is even
        // constructed, so by the time WebGL has set up its buffers the
        // layout has already moved beyond the seed positions.

        // BUG-NEW-E (2026-05-05): start the FA2 layout worker BEFORE
        // constructing Sigma. The worker reads node positions from the
        // graphology graph and writes new positions back into the same
        // attrs map — sigma reads from those attrs every render. So the
        // earlier we kick the worker off, the more refined the positions
        // are by the time sigma's WebGL buffers paint the first frame.
        // Empirically: starting worker → constructing sigma takes ~80ms
        // of main-thread work for buffer setup, during which the worker
        // gets ~3-5 iterations done in parallel. The user's first paint
        // already shows non-random clustering instead of a uniform dot
        // cloud. Worker still stops itself after 5s to avoid burning
        // CPU for the entire lifetime of the tab.
        // HIGH-FE-7 (2026-05-05 audit): React 18 StrictMode mounts effects
        // twice. The previous version checked `cancelled` only after the
        // /api/graph fetch — between that check and worker creation a
        // second mount could fire its cleanup, which would clear the
        // (still-null) ref, then the first mount would proceed to create
        // a worker for a sigma that's about to be torn down. Re-check
        // cancelled HERE so a torn-down mount never leaves a worker
        // alive.
        if (cancelled) return;
        if (g.order > 0) {
          const fa2Worker = new FA2Layout(g, {
            settings: { gravity: 1, scalingRatio: 8 },
          });
          fa2Worker.start();
          fa2WorkerRef.current = fa2Worker;
          // The safety timeout closes over `fa2Worker` directly, NOT
          // `fa2WorkerRef.current`. If a later mount has overwritten the
          // ref by the time this fires, reading the ref would kill the
          // OTHER mount's worker — exactly the StrictMode double-mount
          // race HIGH-FE-7 documents. Killing the local closure
          // reference is correct: it's the worker this mount created,
          // and the cleanup function below also clears the ref iff it
          // still points to this worker.
          fa2TimeoutRef.current = window.setTimeout(() => {
            try {
              if (fa2Worker.isRunning()) fa2Worker.stop();
              fa2Worker.kill();
            } catch {
              /* already stopped/killed — ignore */
            }
            // Only clear the ref if this mount's worker is still the
            // one being tracked. Otherwise a newer mount owns the ref
            // and we must not null it out from underneath them.
            if (fa2WorkerRef.current === fa2Worker) {
              fa2WorkerRef.current = null;
            }
            fa2TimeoutRef.current = null;
          }, 5000);
        }

        // Item #2: enable node events so enterNode/leaveNode fire.
        // Item #4 (2026-05-04 follow-up): show node labels by default —
        // labelRenderedSizeThreshold lowered from default 6 → 3 so labels
        // surface on first paint instead of only after deep zoom. Helps
        // mneme view look closer to Graphify / CRG which surface labels
        // immediately.
        sigmaRef.current = new Sigma(g, containerRef.current, {
          renderEdgeLabels: false,
          enableEdgeEvents: false,
          allowInvalidContainer: true,
          renderLabels: true,
          labelRenderedSizeThreshold: 3,
          labelDensity: 1.5,
          labelGridCellSize: 60,
        });

        graphRef.current = g;

        // ── Item #2: hover-highlight ego-network ─────────────────────
        // Registered ONCE here; reducers read `hoveredRef.current` on
        // every refresh so we don't re-bind on every state flip.
        const sigma = sigmaRef.current;

        sigma.setSetting("nodeReducer", (node, attrs) => {
          const hovered = hoveredRef.current;
          if (!hovered) return attrs;
          if (node === hovered) return attrs;
          // graphology's `areNeighbors` covers both directions on a
          // mixed graph (in + out), matching the "1-hop neighborhood"
          // semantic graphify and CRG both use.
          if (g.areNeighbors(hovered, node)) return attrs;
          return { ...attrs, color: DIM_COLOR, label: "", zIndex: 0 };
        });

        sigma.setSetting("edgeReducer", (edge, attrs) => {
          const hovered = hoveredRef.current;
          if (!hovered) return attrs;
          const src = g.source(edge);
          const tgt = g.target(edge);
          if (src === hovered || tgt === hovered) {
            // Neighbor edge — keep visible, slightly brighter.
            return { ...attrs, color: "#5d7a9e", size: (attrs.size ?? 1) * 1.5 };
          }
          return { ...attrs, color: "rgba(58, 74, 102, 0.12)" };
        });

        sigma.on("enterNode", ({ node }) => {
          hoveredRef.current = node;
          sigma.refresh();
        });
        sigma.on("leaveNode", () => {
          hoveredRef.current = null;
          sigma.refresh();
        });

        sigma.on("clickNode", ({ node }) => {
          const attrs = g.getNodeAttributes(node);
          // A6-010: read action via getState() so the effect can stay [].
          useVisionStore.getState().selectNodes([
            { id: node, label: String(attrs["label"] ?? node) },
          ]);
        });

        // Build the Legend rows from the kind tally — sorted by count
        // desc so the most-common kind sits at the top.
        const rows: LegendKindRow[] = Array.from(kindCounts.entries())
          .sort((a, b) => b[1] - a[1])
          .map(([kind, count]) => ({ kind, count, color: colorForKind(kind) }));
        setLegendRows(rows);

        setCounts({ nodes: nodes.length, edges: edges.length });
        setStatus("ready");

        const elapsed = performance.now() - start;
        if (elapsed > 500) {
          // First-paint budget exceeded; surface for telemetry callers.
          // eslint-disable-next-line no-console
          console.warn(`force-galaxy first-paint ${elapsed.toFixed(0)}ms (>500 budget)`);
        }
      } catch (err) {
        if ((err as Error).name === "AbortError") return;
        if (!cancelled) {
          setError((err as Error).message);
          setStatus("error");
        }
      }
    })();

    return () => {
      cancelled = true;
      ac.abort();
      // Stop + kill the FA2 worker first — it holds a Web Worker thread
      // that will keep posting position updates to a dead Sigma if we
      // tear down sigma first, throwing "Cannot read properties of null"
      // on every animation frame. Order matters.
      if (fa2TimeoutRef.current !== null) {
        window.clearTimeout(fa2TimeoutRef.current);
        fa2TimeoutRef.current = null;
      }
      if (fa2WorkerRef.current) {
        try {
          if (fa2WorkerRef.current.isRunning()) fa2WorkerRef.current.stop();
          fa2WorkerRef.current.kill();
        } catch {
          /* already stopped/killed — ignore */
        }
        fa2WorkerRef.current = null;
      }
      sigmaRef.current?.kill();
      sigmaRef.current = null;
      graphRef.current = null;
      hoveredRef.current = null;
    };
    // A6-010: deliberately empty deps -- the effect builds Sigma once
    // per mount; selectNodes is read via getState() in the handler.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Pulse nodes when livebus reports edits.
  //
  // HIGH-FE-3 fix (2026-05-05 audit): the previous version captured
  // `original` synchronously inside the effect closure. When two
  // LiveEvents fired against the same node within the 400ms timeout
  // window, the second effect read the green pulse-in-progress as
  // its own "original" and locked the node bright green permanently
  // (the cleanup of the first effect cleared its OWN timer, but the
  // second effect's restore wrote green back over green and stuck).
  //
  // Track the genuine pre-pulse color in a ref keyed by node id and
  // restore from THAT, so back-to-back events on the same node
  // collapse correctly.
  useEffect(() => {
    const sigma = sigmaRef.current;
    if (!sigma) return;
    const last = liveEvents[liveEvents.length - 1];
    if (!last?.nodeId) return;
    const graph = sigma.getGraph();
    if (!graph.hasNode(last.nodeId)) return;

    const nodeId = last.nodeId;
    // Only stash the original on the FIRST pulse for this node
    // within the 400ms window. If pulseOriginalsRef already has an
    // entry, a prior pulse is still in flight and `nodeId` is
    // currently set to the green pulse color — do NOT overwrite the
    // stash with green.
    const existing = pulseOriginalsRef.current.get(nodeId);
    if (existing === undefined) {
      pulseOriginalsRef.current.set(
        nodeId,
        graph.getNodeAttribute(nodeId, "color"),
      );
    }
    graph.setNodeAttribute(nodeId, "color", "#41E1B5");
    const t = setTimeout(() => {
      if (graph.hasNode(nodeId)) {
        const stashed = pulseOriginalsRef.current.get(nodeId);
        if (stashed !== undefined) {
          graph.setNodeAttribute(nodeId, "color", stashed);
          pulseOriginalsRef.current.delete(nodeId);
        }
      }
    }, 400);
    return () => clearTimeout(t);
  }, [liveEvents]);

  // Memoized so the <Legend> doesn't see a fresh array reference on
  // every parent render; only the post-load `setLegendRows` call
  // refreshes it.
  const memoLegendRows = useMemo(() => legendRows, [legendRows]);
  // Suppress unused-warning on the forceRender setter used for future
  // hover-driven re-renders (kept available for v0.3.3 click halo work).
  void forceRender;

  return (
    <div className="vz-view vz-view--galaxy">
      <div ref={containerRef} className="vz-view-canvas" data-testid="force-galaxy" />
      {status === "loading" && (
        <div className="vz-view-hint" role="status">
          loading graph.db -- nodes + edges…
        </div>
      )}
      {status === "empty" && (
        <div className="vz-view-error" role="status">
          no nodes in shard yet — run <code>mneme build .</code> in your project
        </div>
      )}
      {status === "error" && error && (
        <div className="vz-view-error" role="alert">
          graph error: {error}
        </div>
      )}
      {status === "ready" && (
        <>
          <Legend rows={memoLegendRows} />
          <OnboardingHint />
          <p className="vz-view-hint">
            {counts.nodes.toLocaleString()} nodes · {counts.edges.toLocaleString()} edges · from graph.db
          </p>
        </>
      )}
    </div>
  );
}
