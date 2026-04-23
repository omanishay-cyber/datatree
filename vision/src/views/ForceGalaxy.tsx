import { useEffect, useRef, useState } from "react";
import Graph from "graphology";
import Sigma from "sigma";
import forceAtlas2 from "graphology-layout-forceatlas2";
import { fetchNodes, fetchEdges } from "../api/graph";
import { useVisionStore } from "../store";

// View 1 — Sigma.js v3 WebGL force-directed graph.
// Targets 60fps on 100K nodes via WebGL renderer + ForceAtlas2 pre-layout.
//
// Wired to the real graph shard (graph.db) via /api/graph/nodes + /api/graph/edges.
// Shows a loading skeleton while the shard query is in flight and a first-class
// error state when the shard is missing ("run `mneme build .`").

type Status = "loading" | "empty" | "ready" | "error";

export function ForceGalaxy(): JSX.Element {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const sigmaRef = useRef<Sigma | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [counts, setCounts] = useState<{ nodes: number; edges: number }>({ nodes: 0, edges: 0 });
  const selectNodes = useVisionStore((s) => s.selectNodes);
  const liveEvents = useVisionStore((s) => s.liveEvents);

  useEffect(() => {
    if (!containerRef.current) return;
    const ac = new AbortController();
    let cancelled = false;

    (async (): Promise<void> => {
      const start = performance.now();
      try {
        const [nodesRes, edgesRes] = await Promise.all([
          fetchNodes(ac.signal, 4000),
          fetchEdges(ac.signal, 16000),
        ]);
        if (cancelled || !containerRef.current) return;

        const nodes = nodesRes.nodes;
        const edges = edgesRes.edges;

        if (nodesRes.error) {
          setError(nodesRes.error);
          setStatus("error");
          return;
        }
        if (nodes.length === 0) {
          setStatus("empty");
          return;
        }

        const g = new Graph({ multi: false, type: "mixed" });
        for (const n of nodes) {
          g.addNode(n.id, {
            label: n.label ?? n.id,
            x: n.x ?? Math.random(),
            y: n.y ?? Math.random(),
            size: n.size ?? 4,
            color: n.color ?? "#7aa7ff",
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

        if (g.order > 0) {
          forceAtlas2.assign(g, {
            iterations: nodes.length > 5000 ? 30 : 60,
            settings: { gravity: 1, scalingRatio: 8 },
          });
        }

        sigmaRef.current = new Sigma(g, containerRef.current, {
          renderEdgeLabels: false,
          enableEdgeEvents: false,
          allowInvalidContainer: true,
        });

        sigmaRef.current.on("clickNode", ({ node }) => {
          const attrs = g.getNodeAttributes(node);
          selectNodes([{ id: node, label: String(attrs["label"] ?? node) }]);
        });

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
      sigmaRef.current?.kill();
      sigmaRef.current = null;
    };
  }, [selectNodes]);

  // Pulse nodes when livebus reports edits.
  useEffect(() => {
    const sigma = sigmaRef.current;
    if (!sigma) return;
    const last = liveEvents[liveEvents.length - 1];
    if (!last?.nodeId) return;
    const graph = sigma.getGraph();
    if (!graph.hasNode(last.nodeId)) return;
    const original = graph.getNodeAttribute(last.nodeId, "color");
    graph.setNodeAttribute(last.nodeId, "color", "#41E1B5");
    const t = setTimeout(() => {
      if (graph.hasNode(last.nodeId!)) graph.setNodeAttribute(last.nodeId!, "color", original);
    }, 400);
    return () => clearTimeout(t);
  }, [liveEvents]);

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
        <p className="vz-view-hint">
          {counts.nodes.toLocaleString()} nodes · {counts.edges.toLocaleString()} edges · from graph.db
        </p>
      )}
    </div>
  );
}
