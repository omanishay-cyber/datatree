import { useEffect, useRef } from "react";
import * as d3 from "d3";
import { fetchGraph } from "../api";

interface TreemapDatum {
  name: string;
  value?: number;
  children?: TreemapDatum[];
}

function buildTree(nodes: { id: string; label?: string; size?: number }[]): TreemapDatum {
  const root: TreemapDatum = { name: "project", children: [] };
  for (const n of nodes) {
    const path = (n.label ?? n.id).split(/[/\\]/).filter(Boolean);
    let cursor = root;
    for (let i = 0; i < path.length; i += 1) {
      const seg = path[i] ?? "";
      cursor.children = cursor.children ?? [];
      let child = cursor.children.find((c) => c.name === seg);
      if (!child) {
        child = { name: seg, children: [] };
        cursor.children.push(child);
      }
      if (i === path.length - 1) child.value = n.size ?? 1;
      cursor = child;
    }
  }
  return root;
}

export function Treemap(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;
    fetchGraph("treemap", { signal: ac.signal }).then((payload) => {
      if (cancelled || !ref.current) return;
      const data = buildTree(payload.nodes);
      const width = 1200;
      const height = 720;
      const root = d3
        .hierarchy<TreemapDatum>(data)
        .sum((d) => d.value ?? 0)
        .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));
      d3.treemap<TreemapDatum>().size([width, height]).padding(2)(root);
      const color = d3.scaleOrdinal(d3.schemeTableau10);

      const svg = d3.select(ref.current).attr("viewBox", `0 0 ${width} ${height}`);
      svg.selectAll("*").remove();

      const leaves = root.leaves() as d3.HierarchyRectangularNode<TreemapDatum>[];
      const cell = svg.selectAll("g").data(leaves).join("g").attr("transform", (d) => `translate(${d.x0},${d.y0})`);
      cell
        .append("rect")
        .attr("width", (d) => d.x1 - d.x0)
        .attr("height", (d) => d.y1 - d.y0)
        .attr("fill", (d) => color(d.parent?.data.name ?? d.data.name))
        .attr("opacity", 0.85);
      cell
        .append("text")
        .attr("x", 4)
        .attr("y", 14)
        .attr("fill", "#0a0e18")
        .attr("font-size", 11)
        .text((d) => (d.x1 - d.x0 > 60 ? d.data.name : ""));
    });
    return () => {
      cancelled = true;
      ac.abort();
    };
  }, []);

  return (
    <div className="vz-view vz-view--treemap">
      <svg ref={ref} className="vz-view-canvas" />
    </div>
  );
}
