import { useEffect, useRef } from "react";
import * as d3 from "d3";
import { fetchGraph } from "../api";
import { useVisionStore } from "../store";

interface TimelineEvent {
  ts: number;
  file: string;
  kind: "add" | "modify" | "delete";
}

function synthesise(payload: { nodes: { id: string; label?: string }[] }): TimelineEvent[] {
  const out: TimelineEvent[] = [];
  const now = Date.now();
  payload.nodes.forEach((n, i) => {
    out.push({
      ts: now - (payload.nodes.length - i) * 36e5,
      file: n.label ?? n.id,
      kind: i % 5 === 0 ? "add" : i % 7 === 0 ? "delete" : "modify",
    });
  });
  return out;
}

export function Timeline(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);
  const setTimelinePosition = useVisionStore((s) => s.setTimelinePosition);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;
    fetchGraph("timeline", { signal: ac.signal }).then((payload) => {
      if (cancelled || !ref.current) return;
      const events = synthesise(payload);
      const width = 1200;
      const height = 720;
      const margin = { top: 40, right: 30, bottom: 50, left: 160 };

      const files = Array.from(new Set(events.map((e) => e.file))).slice(0, 30);
      const x = d3
        .scaleTime()
        .domain(d3.extent(events, (d) => new Date(d.ts)) as [Date, Date])
        .range([margin.left, width - margin.right]);
      const y = d3
        .scaleBand()
        .domain(files)
        .range([margin.top, height - margin.bottom])
        .padding(0.2);

      const svg = d3.select(ref.current).attr("viewBox", `0 0 ${width} ${height}`);
      svg.selectAll("*").remove();

      svg
        .append("g")
        .attr("transform", `translate(0,${height - margin.bottom})`)
        .call(d3.axisBottom(x))
        .attr("color", "#7a8aa6");

      svg
        .append("g")
        .attr("transform", `translate(${margin.left},0)`)
        .call(d3.axisLeft(y))
        .attr("color", "#7a8aa6")
        .selectAll("text")
        .attr("font-size", 10);

      svg
        .append("g")
        .selectAll("circle")
        .data(events.filter((e) => files.includes(e.file)))
        .join("circle")
        .attr("cx", (d) => x(new Date(d.ts)))
        .attr("cy", (d) => (y(d.file) ?? 0) + y.bandwidth() / 2)
        .attr("r", 4)
        .attr("fill", (d) =>
          d.kind === "add" ? "#41E1B5" : d.kind === "delete" ? "#ef4444" : "#4191E1",
        )
        .on("click", (_evt, d) => setTimelinePosition(d.ts));
    });
    return () => {
      cancelled = true;
      ac.abort();
    };
  }, [setTimelinePosition]);

  return (
    <div className="vz-view vz-view--timeline-detail">
      <svg ref={ref} className="vz-view-canvas" />
    </div>
  );
}
