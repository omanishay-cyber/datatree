import { useEffect, useRef, useState } from "react";
import * as d3 from "d3";
import { fetchGraph, type GraphPayload } from "../api";

interface TreeDatum {
  id: string;
  parent: string | null;
  label: string;
}

function toHierarchy(payload: GraphPayload): TreeDatum[] {
  // Convert flat node/edge to parent-pointer using first incoming edge.
  const parentByChild = new Map<string, string>();
  for (const e of payload.edges) {
    if (!parentByChild.has(e.target)) parentByChild.set(e.target, e.source);
  }
  return payload.nodes.map((n) => ({
    id: n.id,
    parent: parentByChild.get(n.id) ?? null,
    label: n.label ?? n.id,
  }));
}

export function HierarchyTree(): JSX.Element {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;

    fetchGraph("hierarchy-tree", { signal: ac.signal })
      .then((payload) => {
        if (cancelled || !svgRef.current) return;
        const data = toHierarchy(payload);
        if (data.length === 0) return;

        const stratify = d3
          .stratify<TreeDatum>()
          .id((d) => d.id)
          .parentId((d) => d.parent);

        let root: d3.HierarchyNode<TreeDatum>;
        try {
          root = stratify(data);
        } catch {
          // Synthesise a single root if the graph isn't a forest.
          const synthetic: TreeDatum[] = [
            { id: "__root__", parent: null, label: "root" },
            ...data.map((d) => ({ ...d, parent: d.parent ?? "__root__" })),
          ];
          root = stratify(synthetic);
        }

        const width = 1200;
        const height = 800;
        const layout = d3.tree<TreeDatum>().size([height - 40, width - 220]);
        layout(root);

        const svg = d3.select(svgRef.current).attr("viewBox", `0 0 ${width} ${height}`);
        svg.selectAll("*").remove();
        const g = svg.append("g").attr("transform", "translate(80, 20)");

        g.append("g")
          .attr("fill", "none")
          .attr("stroke", "#3a4a66")
          .attr("stroke-width", 1.2)
          .selectAll("path")
          .data(root.links())
          .join("path")
          .attr(
            "d",
            d3
              .linkHorizontal<d3.HierarchyPointLink<TreeDatum>, d3.HierarchyPointNode<TreeDatum>>()
              .x((d) => (d as d3.HierarchyPointNode<TreeDatum>).y)
              .y((d) => (d as d3.HierarchyPointNode<TreeDatum>).x),
          );

        const node = g
          .append("g")
          .selectAll("g")
          .data(root.descendants())
          .join("g")
          .attr(
            "transform",
            (d) =>
              `translate(${(d as d3.HierarchyPointNode<TreeDatum>).y},${(d as d3.HierarchyPointNode<TreeDatum>).x})`,
          );

        node
          .append("circle")
          .attr("r", 4)
          .attr("fill", (d) => (d.children ? "#7aa7ff" : "#41E1B5"));

        node
          .append("text")
          .attr("dy", "0.32em")
          .attr("x", (d) => (d.children ? -8 : 8))
          .attr("text-anchor", (d) => (d.children ? "end" : "start"))
          .attr("fill", "#cdd6e4")
          .attr("font-size", 11)
          .text((d) => d.data.label);
      })
      .catch((err: Error) => {
        if (err.name !== "AbortError") setError(err.message);
      });

    return () => {
      cancelled = true;
      ac.abort();
    };
  }, []);

  return (
    <div className="vz-view vz-view--tree">
      <svg ref={svgRef} className="vz-view-canvas" />
      {error && <div className="vz-view-error">tree error: {error}</div>}
    </div>
  );
}
