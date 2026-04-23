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

function domainOf(label: string): string {
  const seg = label.split(/[/\\]/).filter(Boolean);
  return seg[0] ?? "root";
}

export function SankeyDomainFlow(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;
    fetchGraph("sankey-domain", { signal: ac.signal }).then((payload) => {
      if (cancelled || !ref.current) return;

      const labelById = new Map(payload.nodes.map((n) => [n.id, n.label ?? n.id] as const));
      const domains = Array.from(
        new Set(Array.from(labelById.values()).map((l) => domainOf(l))),
      );
      const idx = new Map(domains.map((d, i) => [d, i] as const));
      const aggregate = new Map<string, number>();
      for (const e of payload.edges) {
        const s = domainOf(labelById.get(e.source) ?? e.source);
        const t = domainOf(labelById.get(e.target) ?? e.target);
        if (s === t) continue;
        aggregate.set(`${s}|${t}`, (aggregate.get(`${s}|${t}`) ?? 0) + (e.weight ?? 1));
      }
      const nodes: SNode[] = domains.map((name) => ({ name }));
      const links: SLink[] = [];
      for (const [key, value] of aggregate) {
        const [s, t] = key.split("|");
        if (!s || !t) continue;
        links.push({ source: idx.get(s)!, target: idx.get(t)!, value });
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
        .attr("stroke-opacity", 0.45)
        .selectAll("path")
        .data(graph.links)
        .join("path")
        .attr("d", sankeyLinkHorizontal())
        .attr("stroke", "#22D3EE")
        .attr("stroke-width", (d) => Math.max(1, d.width ?? 1));

      const node = svg.append("g").selectAll("g").data(graph.nodes).join("g");
      node
        .append("rect")
        .attr("x", (d) => d.x0 ?? 0)
        .attr("y", (d) => d.y0 ?? 0)
        .attr("height", (d) => (d.y1 ?? 0) - (d.y0 ?? 0))
        .attr("width", (d) => (d.x1 ?? 0) - (d.x0 ?? 0))
        .attr("fill", "#4191E1");
      node
        .append("text")
        .attr("x", (d) => (d.x1 ?? 0) + 6)
        .attr("y", (d) => ((d.y0 ?? 0) + (d.y1 ?? 0)) / 2)
        .attr("dy", "0.35em")
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
