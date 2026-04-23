import { useEffect, useRef } from "react";
import * as d3 from "d3";
import { fetchGraph } from "../api";

export function HeatmapGrid(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;
    fetchGraph("heatmap-grid", { signal: ac.signal }).then((payload) => {
      if (cancelled || !ref.current) return;
      const cols = Math.max(1, Math.ceil(Math.sqrt(payload.nodes.length)));
      const rows = Math.max(1, Math.ceil(payload.nodes.length / cols));
      const cellSize = 28;
      const width = cols * cellSize + 20;
      const height = rows * cellSize + 20;

      const color = d3.scaleSequential(d3.interpolateInferno).domain([0, 100]);

      const svg = d3.select(ref.current).attr("viewBox", `0 0 ${width} ${height}`);
      svg.selectAll("*").remove();

      const g = svg.append("g").attr("transform", "translate(10,10)");
      g.selectAll("rect")
        .data(payload.nodes)
        .join("rect")
        .attr("x", (_d, i) => (i % cols) * cellSize)
        .attr("y", (_d, i) => Math.floor(i / cols) * cellSize)
        .attr("width", cellSize - 2)
        .attr("height", cellSize - 2)
        .attr("rx", 3)
        .attr("fill", (d) => color((d.size ?? 1) * 8))
        .append("title")
        .text((d) => `${d.label ?? d.id}: churn=${d.size ?? 0}`);
    });
    return () => {
      cancelled = true;
      ac.abort();
    };
  }, []);

  return (
    <div className="vz-view vz-view--heatmap">
      <svg ref={ref} className="vz-view-canvas" />
    </div>
  );
}
