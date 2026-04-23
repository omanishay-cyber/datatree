import { useEffect, useRef } from "react";
import * as d3 from "d3";
import { fetchGraph } from "../api";

interface SunburstNode {
  name: string;
  value?: number;
  children?: SunburstNode[];
}

function toTree(nodes: { id: string; label?: string; size?: number }[]): SunburstNode {
  // Group nodes by the slash-segmented label to get a folder-ish hierarchy.
  const root: SunburstNode = { name: "root", children: [] };
  for (const n of nodes) {
    const segments = (n.label ?? n.id).split(/[/\\]/).filter(Boolean);
    let cursor = root;
    for (let i = 0; i < segments.length; i += 1) {
      const seg = segments[i] ?? "";
      cursor.children = cursor.children ?? [];
      let child = cursor.children.find((c) => c.name === seg);
      if (!child) {
        child = { name: seg, children: [] };
        cursor.children.push(child);
      }
      if (i === segments.length - 1) {
        child.value = (child.value ?? 0) + (n.size ?? 1);
      }
      cursor = child;
    }
  }
  return root;
}

export function Sunburst(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;
    fetchGraph("sunburst", { signal: ac.signal }).then((payload) => {
      if (cancelled || !ref.current) return;
      const data = toTree(payload.nodes);
      const width = 720;
      const radius = width / 2;

      const root = d3
        .hierarchy<SunburstNode>(data)
        .sum((d) => d.value ?? 0)
        .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));

      const partition = d3.partition<SunburstNode>().size([2 * Math.PI, radius]);
      partition(root);

      const arc = d3
        .arc<d3.HierarchyRectangularNode<SunburstNode>>()
        .startAngle((d) => d.x0)
        .endAngle((d) => d.x1)
        .innerRadius((d) => d.y0)
        .outerRadius((d) => d.y1 - 1);

      const color = d3.scaleOrdinal(d3.quantize(d3.interpolateRainbow, root.children?.length ?? 8));

      const svg = d3.select(ref.current).attr("viewBox", `${-radius} ${-radius} ${width} ${width}`);
      svg.selectAll("*").remove();

      svg
        .selectAll("path")
        .data(root.descendants().filter((d) => d.depth > 0) as d3.HierarchyRectangularNode<SunburstNode>[])
        .join("path")
        .attr("d", arc)
        .attr("fill", (d) => {
          let p: d3.HierarchyNode<SunburstNode> = d;
          while (p.depth > 1 && p.parent) p = p.parent;
          return color(p.data.name);
        })
        .attr("opacity", 0.85)
        .append("title")
        .text((d) => `${d.ancestors().map((a) => a.data.name).reverse().join(" / ")} — ${d.value ?? 0}`);
    });
    return () => {
      cancelled = true;
      ac.abort();
    };
  }, []);

  return (
    <div className="vz-view vz-view--sunburst">
      <svg ref={ref} className="vz-view-canvas" />
    </div>
  );
}
