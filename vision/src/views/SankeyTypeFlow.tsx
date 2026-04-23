import { useEffect, useRef } from "react";
import * as d3 from "d3";
import { sankey, sankeyLinkHorizontal, type SankeyGraph } from "d3-sankey";
import { fetchGraph } from "../api";

interface SNode {
  name: string;
}

interface SLink {
  source: number | SNode;
  target: number | SNode;
  value: number;
}

export function SankeyTypeFlow(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;
    fetchGraph("sankey-type", { signal: ac.signal }).then((payload) => {
      if (cancelled || !ref.current) return;

      const types = Array.from(new Set(payload.nodes.map((n) => n.type ?? "unknown")));
      const nodeIndex = new Map(types.map((t, i) => [t, i] as const));
      const nodes: SNode[] = types.map((name) => ({ name }));
      const aggregate = new Map<string, number>();
      for (const e of payload.edges) {
        const sType = payload.nodes.find((n) => n.id === e.source)?.type ?? "unknown";
        const tType = payload.nodes.find((n) => n.id === e.target)?.type ?? "unknown";
        if (sType === tType) continue;
        const key = `${sType}|${tType}`;
        aggregate.set(key, (aggregate.get(key) ?? 0) + (e.weight ?? 1));
      }
      const links: SLink[] = [];
      for (const [key, value] of aggregate) {
        const [s, t] = key.split("|");
        if (!s || !t) continue;
        links.push({ source: nodeIndex.get(s)!, target: nodeIndex.get(t)!, value });
      }

      const width = 1100;
      const height = 700;
      const sk = sankey<SNode, SLink>().nodeWidth(14).nodePadding(12).extent([
        [10, 10],
        [width - 10, height - 10],
      ]);
      const graph: SankeyGraph<SNode, SLink> = sk({ nodes: nodes.map((d) => ({ ...d })), links: links.map((d) => ({ ...d })) });

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
        .attr("stroke-width", (d) => Math.max(1, d.width ?? 1));

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
    });
    return () => {
      cancelled = true;
      ac.abort();
    };
  }, []);

  return (
    <div className="vz-view vz-view--sankey">
      <svg ref={ref} className="vz-view-canvas" />
    </div>
  );
}
