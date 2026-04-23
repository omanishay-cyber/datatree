import { useEffect, useRef, useState } from "react";
import Graph from "graphology";
import Sigma from "sigma";
import forceAtlas2 from "graphology-layout-forceatlas2";
import { fetchGraph, type GraphPayload } from "../api";
import { useVisionStore } from "../store";

// View 1 — Sigma.js v3 WebGL force-directed graph.
// Targets 60fps on 100K nodes via WebGL renderer + ForceAtlas2 pre-layout.

export function ForceGalaxy(): JSX.Element {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const sigmaRef = useRef<Sigma | null>(null);
  const [error, setError] = useState<string | null>(null);
  const selectNodes = useVisionStore((s) => s.selectNodes);
  const liveEvents = useVisionStore((s) => s.liveEvents);

  useEffect(() => {
    if (!containerRef.current) return;
    const ac = new AbortController();
    let cancelled = false;

    const start = performance.now();
    fetchGraph("force-galaxy", { signal: ac.signal })
      .then((payload: GraphPayload) => {
        if (cancelled || !containerRef.current) return;
        const g = new Graph({ multi: false, type: "mixed" });
        for (const n of payload.nodes) {
          g.addNode(n.id, {
            label: n.label ?? n.id,
            x: n.x ?? Math.random(),
            y: n.y ?? Math.random(),
            size: n.size ?? 4,
            color: n.color ?? "#7aa7ff",
          });
        }
        for (const e of payload.edges) {
          if (!g.hasNode(e.source) || !g.hasNode(e.target)) continue;
          g.addEdge(e.source, e.target, { weight: e.weight ?? 1, color: "#3a4a66" });
        }

        // Pre-layout: a few iterations of ForceAtlas2 to settle positions.
        if (g.order > 0) {
          forceAtlas2.assign(g, {
            iterations: payload.nodes.length > 5000 ? 30 : 60,
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

        const elapsed = performance.now() - start;
        if (elapsed > 500) {
          // First-paint budget exceeded; surface for telemetry callers.
          // eslint-disable-next-line no-console
          console.warn(`force-galaxy first-paint ${elapsed.toFixed(0)}ms (>500 budget)`);
        }
      })
      .catch((err: Error) => {
        if (err.name !== "AbortError") setError(err.message);
      });

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
      {error && <div className="vz-view-error">graph error: {error}</div>}
    </div>
  );
}
