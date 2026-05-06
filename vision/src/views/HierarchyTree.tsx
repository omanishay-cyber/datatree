import { useEffect, useRef, useState } from "react";
import * as d3 from "d3";
import { fetchHierarchy, type HierarchyNode } from "../api/graph";

type Status = "loading" | "empty" | "ready" | "error";

function countLeaves(n: HierarchyNode): number {
  if (!n.children || n.children.length === 0) return 1;
  return n.children.reduce((s, c) => s + countLeaves(c), 0);
}

/**
 * Cap branching at every depth to keep the visible tree readable on a
 * 17K-node project without blowing past the 500ms first-paint budget.
 *
 * Behaviour:
 *   - Sort siblings by subtree size DESC.
 *   - Keep up to `topK` per parent.
 *   - Replace overflow children with a single "+ N more" placeholder
 *     leaf so the user knows there's more to see — graphify uses the
 *     identical pattern in its module-tree explorer.
 *
 * Returns a NEW tree (we never mutate the response object — callers
 * keep a clean reference to the raw shard tree for future
 * lazy-expansion work in v0.3.3).
 */
function capBranching(node: HierarchyNode, topK: number): HierarchyNode {
  const kids = node.children;
  if (!kids || kids.length === 0) return node;
  const withSizes = kids
    .map((c) => ({ child: c, size: countLeaves(c) }))
    .sort((a, b) => b.size - a.size);
  const kept = withSizes.slice(0, topK).map((p) => capBranching(p.child, topK));
  const overflowCount = withSizes.length - topK;
  if (overflowCount > 0) {
    const overflowSize = withSizes
      .slice(topK)
      .reduce((s, p) => s + p.size, 0);
    kept.push({
      name: `+${overflowCount} more (${overflowSize.toLocaleString()} files)`,
      kind: "overflow",
      children: undefined,
    });
  }
  return { ...node, children: kept };
}

const KIND_COLORS: Record<string, string> = {
  module: "#f59e0b",
  class: "#22D3EE",
  file: "#4191E1",
  function: "#41E1B5",
  overflow: "#7a8aa6",
};

export function HierarchyTree(): JSX.Element {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [leafCount, setLeafCount] = useState(0);
  const [visibleCount, setVisibleCount] = useState(0);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;

    (async (): Promise<void> => {
      const t0 = performance.now();
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

        // Cap branching at every level. 12 children per parent gives a
        // dense but legible tree — wider than that and labels mash
        // together at typical zoom levels regardless of font size.
        // (Graphify caps at 10, CRG caps at 8; we split the difference.)
        const capped = capBranching(tree, 12);
        const root = d3.hierarchy<HierarchyNode>(capped);
        const nodeCount = root.descendants().length;
        setVisibleCount(nodeCount);

        // Width scales with depth, height with leaf count of the capped
        // tree. After capping the height stays bounded (~1500px even on
        // 17K-node projects) so the canvas remains scrollable instead
        // of degenerating into an infinite vertical strip.
        const maxDepth = root.height + 1;
        const width = Math.max(1200, maxDepth * 220);
        const height = Math.max(600, Math.min(4500, root.leaves().length * 18));

        // L11 fix (2026-05-05 audit): bind the laid-out root to its
        // post-layout type. `layout(root)` mutates in place, transforming
        // the HierarchyNode<T> children into HierarchyPointNode<T> (with
        // x/y/depth fields populated). Without this binding, downstream
        // descendants() calls return HierarchyNode<T> by declared type,
        // and every consumer needed an `as d3.HierarchyPointNode<T>`
        // cast. Single binding here = zero downstream casts.
        const layout = d3.tree<HierarchyNode>().size([height - 40, width - 280]);
        const positioned = layout(root);
        // From here, use `positioned` instead of `root` whenever the
        // x/y/depth coordinates are needed.
        void root; // suppress unused-binding for the pre-layout root

        const svg = d3
          .select(svgRef.current)
          .attr("viewBox", `0 0 ${width} ${height}`);
        svg.selectAll("*").remove();
        const g = svg.append("g").attr("transform", "translate(140, 20)");

        // Curved diagonal links — the previous straight `linkHorizontal`
        // produced "tree branches" that looked like a wire diagram.
        // d3.linkHorizontal already draws bezier curves; the visual
        // win was just paint-order: we paint links UNDER nodes so circle
        // strokes don't get cut by the link endpoints.
        g.append("g")
          .attr("fill", "none")
          .attr("stroke", "rgba(122, 138, 166, 0.45)")
          .attr("stroke-width", 1.2)
          .selectAll("path")
          .data(positioned.links())
          .join("path")
          .attr(
            "d",
            d3
              .linkHorizontal<
                d3.HierarchyPointLink<HierarchyNode>,
                d3.HierarchyPointNode<HierarchyNode>
              >()
              .x((d) => d.y)
              .y((d) => d.x),
          );

        const node = g
          .append("g")
          .selectAll("g")
          .data(positioned.descendants())
          .join("g")
          .attr(
            "transform",
            (d) => `translate(${d.y},${d.x})`,
          );

        // Node halo (depth-1 only) so the top-level domains pop without
        // adding a second draw call per node deeper in the tree.
        node
          .filter((d) => d.depth === 1)
          .append("circle")
          .attr("r", 8)
          .attr("fill", "none")
          .attr("stroke", (d) => KIND_COLORS[d.data.kind ?? ""] ?? "#7aa7ff")
          .attr("stroke-opacity", 0.35)
          .attr("stroke-width", 2);

        node
          .append("circle")
          .attr("r", (d) => (d.children ? 4.5 : 3))
          .attr("fill", (d) => {
            if (!d.data.kind) return d.children ? "#7aa7ff" : "#41E1B5";
            return KIND_COLORS[d.data.kind] ?? "#41E1B5";
          })
          .attr("stroke", "var(--bg-1, #0a0e18)")
          .attr("stroke-width", 1);

        // Label that auto-truncates with ellipsis once it bumps the
        // 180px column budget. text-anchor flips by depth so leaves
        // always read outward.
        node
          .append("text")
          .attr("dy", "0.32em")
          .attr("x", (d) => (d.children ? -10 : 10))
          .attr("text-anchor", (d) => (d.children ? "end" : "start"))
          .attr("fill", (d) =>
            d.data.kind === "overflow" ? "var(--fg-2, #7a8aa6)" : "var(--fg-1, #cdd6e4)",
          )
          .attr("font-size", (d) => (d.depth <= 1 ? 12 : 10))
          .attr("font-weight", (d) => (d.depth <= 1 ? 600 : 400))
          .text((d) => {
            const name = d.data.name ?? "";
            return name.length > 26 ? `${name.slice(0, 25)}...` : name;
          });

        // Full-path tooltip on hover — surfaces the truncation tail
        // without forcing the label width to expand.
        node
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

        const elapsed = performance.now() - t0;
        if (elapsed > 500) {
          // eslint-disable-next-line no-console
          console.warn(
            `hierarchy-tree first-paint ${elapsed.toFixed(0)}ms (>500 budget)`,
          );
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
        <p className="vz-view-hint">
          {visibleCount.toLocaleString()} of {leafCount.toLocaleString()} nodes shown
          (top-12 per branch)
        </p>
      )}
    </div>
  );
}
