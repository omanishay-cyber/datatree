import { useEffect, useRef } from "react";
import * as d3 from "d3";
import { fetchGraph } from "../api";

export function ArcChord(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;
    fetchGraph("arc-chord", { signal: ac.signal }).then((payload) => {
      if (cancelled || !ref.current) return;
      const labels = Array.from(
        new Set(payload.nodes.map((n) => (n.label ?? n.id).split(/[/\\]/)[0] ?? "root")),
      );
      const idx = new Map(labels.map((l, i) => [l, i] as const));
      const matrix: number[][] = Array.from({ length: labels.length }, () => Array(labels.length).fill(0));
      for (const e of payload.edges) {
        const sLabel = (payload.nodes.find((n) => n.id === e.source)?.label ?? e.source).split(/[/\\]/)[0] ?? "root";
        const tLabel = (payload.nodes.find((n) => n.id === e.target)?.label ?? e.target).split(/[/\\]/)[0] ?? "root";
        const i = idx.get(sLabel);
        const j = idx.get(tLabel);
        if (i == null || j == null) continue;
        const row = matrix[i];
        if (!row) continue;
        row[j] = (row[j] ?? 0) + (e.weight ?? 1);
      }

      const width = 760;
      const height = 760;
      const outer = Math.min(width, height) * 0.5 - 30;
      const inner = outer - 18;

      const chord = d3.chord().padAngle(0.04).sortSubgroups(d3.descending);
      const chords = chord(matrix);
      const arc = d3.arc<d3.ChordGroup>().innerRadius(inner).outerRadius(outer);
      const ribbon = d3.ribbon<d3.Chord, d3.ChordSubgroup>().radius(inner);
      const color = d3.scaleOrdinal(d3.schemeTableau10);

      const svg = d3
        .select(ref.current)
        .attr("viewBox", `${-width / 2} ${-height / 2} ${width} ${height}`);
      svg.selectAll("*").remove();

      svg
        .append("g")
        .selectAll("path")
        .data(chords.groups)
        .join("path")
        .attr("d", arc)
        .attr("fill", (d) => color(String(d.index)))
        .attr("stroke", "#0a0e18");

      svg
        .append("g")
        .attr("fill-opacity", 0.55)
        .selectAll("path")
        .data(chords)
        .join("path")
        .attr("d", ribbon)
        .attr("fill", (d) => color(String(d.target.index)))
        .append("title")
        .text((d) => `${labels[d.source.index]} → ${labels[d.target.index]} : ${d.source.value}`);

      svg
        .append("g")
        .selectAll("text")
        .data(chords.groups)
        .join("text")
        .attr("transform", (d) => {
          const angle = (d.startAngle + d.endAngle) / 2;
          return `rotate(${(angle * 180) / Math.PI - 90}) translate(${outer + 8})${angle > Math.PI ? " rotate(180)" : ""}`;
        })
        .attr("text-anchor", (d) => ((d.startAngle + d.endAngle) / 2 > Math.PI ? "end" : "start"))
        .attr("fill", "#cdd6e4")
        .attr("font-size", 10)
        .text((d) => labels[d.index] ?? "");
    });
    return () => {
      cancelled = true;
      ac.abort();
    };
  }, []);

  return (
    <div className="vz-view vz-view--chord">
      <svg ref={ref} className="vz-view-canvas" />
    </div>
  );
}
