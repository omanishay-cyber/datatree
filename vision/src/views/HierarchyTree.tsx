import { useEffect, useRef, useState } from "react";
import * as d3 from "d3";
import { fetchHierarchy, type HierarchyNode } from "../api/graph";

type Status = "loading" | "empty" | "ready" | "error";

function countLeaves(n: HierarchyNode): number {
  if (!n.children || n.children.length === 0) return 1;
  return n.children.reduce((s, c) => s + countLeaves(c), 0);
}

export function HierarchyTree(): JSX.Element {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [leafCount, setLeafCount] = useState(0);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;

    (async (): Promise<void> => {
      try {
        const res = await fetchHierarchy(ac.signal, 4000);
        if (cancelled || !svgRef.current) return;
        if (res.error) {
          setError(res.error);
          setStatus("error");
          return;
        }
        const tree = res.tree;
        const leaves = countLeaves(tree);
        if (!tree.children || tree.children.length === 0 || leaves <= 1) {
          setStatus("empty");
          return;
        }
        setLeafCount(leaves);

        const root = d3.hierarchy<HierarchyNode>(tree);
        const nodeCount = root.descendants().length;
        const width = 1200;
        const height = Math.max(600, Math.min(8000, nodeCount * 12));
        const layout = d3.tree<HierarchyNode>().size([height - 40, width - 260]);
        layout(root);

        const svg = d3
          .select(svgRef.current)
          .attr("viewBox", `0 0 ${width} ${height}`);
        svg.selectAll("*").remove();
        const g = svg.append("g").attr("transform", "translate(120, 20)");

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
              .linkHorizontal<
                d3.HierarchyPointLink<HierarchyNode>,
                d3.HierarchyPointNode<HierarchyNode>
              >()
              .x((d) => (d as d3.HierarchyPointNode<HierarchyNode>).y)
              .y((d) => (d as d3.HierarchyPointNode<HierarchyNode>).x),
          );

        const node = g
          .append("g")
          .selectAll("g")
          .data(root.descendants())
          .join("g")
          .attr(
            "transform",
            (d) =>
              `translate(${(d as d3.HierarchyPointNode<HierarchyNode>).y},${
                (d as d3.HierarchyPointNode<HierarchyNode>).x
              })`,
          );

        node
          .append("circle")
          .attr("r", (d) => (d.children ? 4 : 3))
          .attr("fill", (d) => {
            if (!d.data.kind) return d.children ? "#7aa7ff" : "#41E1B5";
            switch (d.data.kind) {
              case "module":
                return "#f59e0b";
              case "class":
                return "#22D3EE";
              case "file":
                return "#4191E1";
              default:
                return "#41E1B5";
            }
          });

        node
          .append("text")
          .attr("dy", "0.32em")
          .attr("x", (d) => (d.children ? -8 : 8))
          .attr("text-anchor", (d) => (d.children ? "end" : "start"))
          .attr("fill", "#cdd6e4")
          .attr("font-size", 10)
          .text((d) => d.data.name)
          .append("title")
          .text(
            (d) =>
              `${d
                .ancestors()
                .map((a) => a.data.name)
                .reverse()
                .join(" / ")}${d.data.kind ? ` (${d.data.kind})` : ""}`,
          );

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
    <div className="vz-view vz-view--tree">
      <svg ref={svgRef} className="vz-view-canvas" />
      {status === "loading" && (
        <div className="vz-view-hint" role="status">
          loading graph.db qualified-name hierarchy...
        </div>
      )}
      {status === "empty" && (
        <div className="vz-view-error" role="status">
          no module/class/file nodes yet -- run <code>mneme build .</code>
        </div>
      )}
      {status === "error" && error && (
        <div className="vz-view-error" role="alert">
          tree error: {error}
        </div>
      )}
      {status === "ready" && (
        <p className="vz-view-hint">{leafCount.toLocaleString()} leaf nodes from graph.db</p>
      )}
    </div>
  );
}
