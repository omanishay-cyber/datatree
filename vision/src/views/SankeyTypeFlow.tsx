import { useEffect, useRef, useState } from "react";
import * as d3 from "d3";
import { sankey, sankeyLinkHorizontal, type SankeyGraph } from "d3-sankey";
import { fetchKindFlow } from "../api/graph";

interface SNode {
  name: string;
  kind: string;
}

interface SLink {
  source: number | SNode;
  target: number | SNode;
  value: number;
}

type Status = "loading" | "empty" | "ready" | "error";

export function SankeyTypeFlow(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [counts, setCounts] = useState<{ nodes: number; links: number }>({
    nodes: 0,
    links: 0,
  });

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;

    (async (): Promise<void> => {
      try {
        const res = await fetchKindFlow(ac.signal, 20000);
        if (cancelled || !ref.current) return;
        if (res.error) {
          setError(res.error);
          setStatus("error");
          return;
        }
        if (res.nodes.length === 0 || res.links.length === 0) {
          setStatus("empty");
          return;
        }
        setCounts({ nodes: res.nodes.length, links: res.links.length });

        const indexById = new Map(res.nodes.map((n, i) => [n.id, i] as const));
        const nodes: SNode[] = res.nodes.map((n) => ({ name: n.kind, kind: n.kind }));
        const links: SLink[] = [];
        for (const l of res.links) {
          const s = indexById.get(l.source);
          const t = indexById.get(l.target);
          if (s == null || t == null) continue;
          links.push({ source: s, target: t, value: l.value });
        }

        const width = 1100;
        const height = 700;
        const sk = sankey<SNode, SLink>().nodeWidth(14).nodePadding(12).extent([
          [10, 10],
          [width - 10, height - 10],
        ]);
        const graph: SankeyGraph<SNode, SLink> = sk({
          nodes: nodes.map((d) => ({ ...d })),
          links: links.map((d) => ({ ...d })),
        });

        const svg = d3.select(ref.current).attr("viewBox", `0 0 ${width} ${height}`);
        svg.selectAll("*").remove();

        svg
          .append("g")
          .attr("fill", "none")
          .attr("stroke-opacity", 0.5)
          .selectAll("path")
          .data(graph.links)
          .join("path")
          .attr("d", sankeyLinkHorizontal())
          .attr("stroke", "#7aa7ff")
          .attr("stroke-width", (d) => Math.max(1, d.width ?? 1))
          .append("title")
          .text((d) => `${d.value} edges`);

        const node = svg.append("g").selectAll("g").data(graph.nodes).join("g");
        node
          .append("rect")
          .attr("x", (d) => d.x0 ?? 0)
          .attr("y", (d) => d.y0 ?? 0)
          .attr("height", (d) => (d.y1 ?? 0) - (d.y0 ?? 0))
          .attr("width", (d) => (d.x1 ?? 0) - (d.x0 ?? 0))
          .attr("fill", "#41E1B5");
        node
          .append("text")
          .attr("x", (d) => (d.x0 ?? 0) - 6)
          .attr("y", (d) => ((d.y0 ?? 0) + (d.y1 ?? 0)) / 2)
          .attr("dy", "0.35em")
          .attr("text-anchor", "end")
          .attr("fill", "#cdd6e4")
          .attr("font-size", 11)
          .text((d) => d.name);

        setStatus("ready");
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
    };
  }, []);

  return (
    <div className="vz-view vz-view--sankey">
      <svg ref={ref} className="vz-view-canvas" />
      {status === "loading" && (
        <div className="vz-view-hint" role="status">
          loading graph.db edges grouped by kind...
        </div>
      )}
      {status === "empty" && (
        <div className="vz-view-error" role="status">
          no edges in shard yet -- run <code>mneme build .</code> to index the project
        </div>
      )}
      {status === "error" && error && (
        <div className="vz-view-error" role="alert">
          sankey error: {error}
        </div>
      )}
      {status === "ready" && (
        <p className="vz-view-hint">
          {counts.nodes.toLocaleString()} kinds - {counts.links.toLocaleString()} flows
        </p>
      )}
    </div>
  );
}
