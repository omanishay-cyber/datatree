import { useEffect, useRef, useState } from "react";
import * as d3 from "d3";
import { fetchFileTree, type FileTreeNode } from "../api/graph";

type Status = "loading" | "empty" | "ready" | "error";

function leafCount(node: FileTreeNode): number {
  if (!node.children || node.children.length === 0) return 1;
  return node.children.reduce((sum, c) => sum + leafCount(c), 0);
}

export function Sunburst(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [fileCount, setFileCount] = useState(0);
  const [breadcrumb, setBreadcrumb] = useState<string>("");

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;

    (async (): Promise<void> => {
      const t0 = performance.now();
      try {
        const res = await fetchFileTree(ac.signal, 4000);
        if (cancelled || !ref.current) return;
        if (res.error) {
          setError(res.error);
          setStatus("error");
          return;
        }
        const tree = res.tree;
        const count = leafCount(tree);
        if (!tree.children || tree.children.length === 0 || count <= 1) {
          setStatus("empty");
          return;
        }
        setFileCount(count);

        // Sunburst geometry. Inner cutout (radius * 0.18) keeps the very
        // smallest wedges from collapsing into an unreadable knot at the
        // origin and gives us a place to surface the breadcrumb on
        // hover. Mirrors the "donut hole" pattern graphify uses for its
        // module-mass view.
        const width = 760;
        const radius = width / 2;
        const innerCutout = radius * 0.18;

        const root = d3
          .hierarchy<FileTreeNode>(tree)
          .sum((d) => d.value ?? 1)
          // Stable sort: by value DESC so big leaves anchor each ring,
          // tie-broken by name ASC so reloads paint the same wedges in
          // the same slots (the previous `b.value? - a.value?` was a
          // no-op when both values were undefined and let d3's internal
          // ordering thrash on every frame).
          .sort((a, b) => {
            const dv = (b.value ?? 0) - (a.value ?? 0);
            if (dv !== 0) return dv;
            return d3.ascending(a.data.name, b.data.name);
          });

        // partition().size([2π, radius - inner]) lays the wedges on a
        // ring outside the cutout. Earlier code used the full radius
        // which slammed depth-1 wedges to the origin and produced the
        // "not circling — looks like a flat blob" symptom the user
        // reported.
        d3
          .partition<FileTreeNode>()
          .size([2 * Math.PI, radius - innerCutout])(root);

        const arc = d3
          .arc<d3.HierarchyRectangularNode<FileTreeNode>>()
          .startAngle((d) => d.x0)
          .endAngle((d) => d.x1)
          // Pad each wedge by 1px on the inside, 1px gap between rings.
          .padAngle(0.003)
          .padRadius(radius / 2)
          .innerRadius((d) => innerCutout + d.y0)
          .outerRadius((d) => Math.max(innerCutout + d.y0, innerCutout + d.y1 - 1));

        // Item #10: swap d3.interpolateRainbow (perceptually awful for
        // ordinal data) for d3.schemeTableau10 — same categorical
        // palette graphify uses. Top-level children become the domain
        // rows for the ordinal scale; descendants inherit from their
        // depth-1 ancestor so each "wedge" reads as one color family
        // with depth-driven opacity to differentiate sub-rings.
        const topNames = root.children?.map((c) => c.data.name) ?? [];
        const color = d3
          .scaleOrdinal<string>()
          .domain(topNames)
          .range(d3.schemeTableau10);

        const svg = d3
          .select(ref.current)
          .attr("viewBox", `${-radius} ${-radius} ${width} ${width}`);
        svg.selectAll("*").remove();

        // Outer ring guide so the user can SEE the circle even when
        // top-level wedges are tiny. Subtle, on-palette stroke.
        svg
          .append("circle")
          .attr("r", radius - 4)
          .attr("fill", "none")
          .attr("stroke", "rgba(122, 138, 166, 0.12)")
          .attr("stroke-width", 1);

        const descendants = root
          .descendants()
          .filter((d) => d.depth > 0) as d3.HierarchyRectangularNode<FileTreeNode>[];

        const wedge = svg
          .append("g")
          .attr("class", "vz-sunburst-wedges")
          .selectAll("path")
          .data(descendants)
          .join("path")
          .attr("d", arc)
          .attr("fill", (d) => {
            let p: d3.HierarchyNode<FileTreeNode> = d;
            while (p.depth > 1 && p.parent) p = p.parent;
            return color(p.data.name);
          })
          // Depth-driven opacity: top ring 0.95, next 0.78, deeper rings
          // taper toward 0.55 — keeps the eye reading from the rim
          // inward like a real sunburst.
          .attr("opacity", (d) => Math.max(0.55, 0.95 - (d.depth - 1) * 0.15))
          .attr("stroke", "var(--bg-1, #0a0e18)")
          .attr("stroke-width", 0.5)
          .style("cursor", "pointer");

        wedge
          .append("title")
          .text(
            (d) =>
              `${d
                .ancestors()
                .map((a) => a.data.name)
                .reverse()
                .join(" / ")} - ${(d.value ?? 0).toLocaleString()} LoC`,
          );

        // Breadcrumb on hover, rendered into a state slot above the SVG
        // so users always know what wedge their cursor is over without
        // squinting at the tooltip.
        wedge
          .on("mouseenter", (_evt, d) => {
            const path = d
              .ancestors()
              .map((a) => a.data.name)
              .reverse()
              .join(" / ");
            setBreadcrumb(`${path} - ${(d.value ?? 0).toLocaleString()} LoC`);
          })
          .on("mouseleave", () => setBreadcrumb(""));

        // Label rendering. Only label wedges with enough arc length to
        // hold readable text (roughly > 12px of arc at the wedge's
        // mid-radius). Mirrors graphify which only labels its biggest
        // top-3-ring wedges; CRG labels every wedge regardless and ends
        // up with the same overlap mush mneme had pre-fix.
        svg
          .append("g")
          .attr("class", "vz-sunburst-labels")
          .attr("pointer-events", "none")
          .attr("font-size", 10)
          .attr("fill", "var(--fg-0, #e6ecf6)")
          .selectAll("text")
          .data(
            descendants.filter((d) => {
              const midR = (innerCutout + d.y0 + (innerCutout + d.y1)) / 2;
              const arcLen = Math.abs(d.x1 - d.x0) * midR;
              return arcLen > 18 && d.data.name.length > 0;
            }),
          )
          .join("text")
          .attr("transform", (d) => {
            const midA = (d.x0 + d.x1) / 2;
            const midR = (innerCutout + d.y0 + (innerCutout + d.y1)) / 2;
            const deg = (midA * 180) / Math.PI - 90;
            // Flip labels on the left half so they always read
            // left-to-right.
            const flip = midA > Math.PI ? 180 : 0;
            return `rotate(${deg}) translate(${midR}, 0) rotate(${flip})`;
          })
          .attr("text-anchor", "middle")
          .attr("dy", "0.32em")
          .text((d) => {
            const arcAtR =
              Math.abs(d.x1 - d.x0) *
              ((innerCutout + d.y0 + (innerCutout + d.y1)) / 2);
            // Cap label by arc length so we don't paint long file names
            // over their neighbors.
            const max = Math.max(2, Math.floor(arcAtR / 6));
            return d.data.name.length > max
              ? `${d.data.name.slice(0, max - 1)}...`
              : d.data.name;
          });

        setStatus("ready");

        const elapsed = performance.now() - t0;
        if (elapsed > 500) {
          // eslint-disable-next-line no-console
          console.warn(`sunburst first-paint ${elapsed.toFixed(0)}ms (>500 budget)`);
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
    <div className="vz-view vz-view--sunburst">
      <svg ref={ref} className="vz-view-canvas" />
      {status === "loading" && (
        <div className="vz-view-hint" role="status">
          loading graph.db file tree...
        </div>
      )}
      {status === "empty" && (
        <div className="vz-view-error" role="status">
          no files in shard -- run <code>mneme build .</code> to index the project
        </div>
      )}
      {status === "error" && error && (
        <div className="vz-view-error" role="alert">
          sunburst error: {error}
        </div>
      )}
      {status === "ready" && (
        <>
          {breadcrumb && (
            <div
              className="vz-view-hint"
              role="status"
              style={{
                top: 12,
                bottom: "auto",
                left: 16,
                right: "auto",
                color: "var(--fg-1, #cdd6e4)",
                fontVariantNumeric: "tabular-nums",
              }}
            >
              {breadcrumb}
            </div>
          )}
          <p className="vz-view-hint">{fileCount.toLocaleString()} files - weighted by LoC</p>
        </>
      )}
    </div>
  );
}
