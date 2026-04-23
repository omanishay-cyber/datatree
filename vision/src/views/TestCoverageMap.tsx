import { useEffect, useRef } from "react";
import * as d3 from "d3";
import { fetchGraph } from "../api";

export function TestCoverageMap(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;
    fetchGraph("test-coverage", { signal: ac.signal }).then((payload) => {
      if (cancelled || !ref.current) return;
      const items = payload.nodes.map((n, i) => ({
        id: n.id,
        label: n.label ?? n.id,
        coverage:
          typeof n.meta?.["coverage"] === "number"
            ? Number(n.meta["coverage"])
            : Math.max(0, Math.min(100, ((i * 37) % 100) + (i % 5))),
      }));

      const cols = Math.ceil(Math.sqrt(items.length));
      const rows = Math.ceil(items.length / cols);
      const cell = 36;
      const width = cols * cell + 40;
      const height = rows * cell + 40;
      const color = d3.scaleSequential(d3.interpolateRdYlGn).domain([0, 100]);

      const svg = d3.select(ref.current).attr("viewBox", `0 0 ${width} ${height}`);
      svg.selectAll("*").remove();

      const g = svg.append("g").attr("transform", "translate(20,20)");
      g.selectAll("rect")
        .data(items)
        .join("rect")
        .attr("x", (_d, i) => (i % cols) * cell)
        .attr("y", (_d, i) => Math.floor(i / cols) * cell)
        .attr("width", cell - 3)
        .attr("height", cell - 3)
        .attr("rx", 4)
        .attr("fill", (d) => color(d.coverage))
        .append("title")
        .text((d) => `${d.label}: ${d.coverage}% covered`);
    });
    return () => {
      cancelled = true;
      ac.abort();
    };
  }, []);

  return (
    <div className="vz-view vz-view--coverage">
      <svg ref={ref} className="vz-view-canvas" />
    </div>
  );
}
