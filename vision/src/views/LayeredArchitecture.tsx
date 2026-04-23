import { useEffect, useRef } from "react";
import * as d3 from "d3";
import { fetchGraph } from "../api";

const LAYERS: Array<{ id: string; match: RegExp }> = [
  { id: "ui", match: /(components?|pages?|views?)/i },
  { id: "state", match: /(stores?|hooks?|context)/i },
  { id: "domain", match: /(services?|domain|features?)/i },
  { id: "infra", match: /(electron|server|ipc|fs|db|adapter)/i },
];

function classify(label: string): string {
  for (const layer of LAYERS) {
    if (layer.match.test(label)) return layer.id;
  }
  return "lib";
}

export function LayeredArchitecture(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;
    fetchGraph("layered-architecture", { signal: ac.signal }).then((payload) => {
      if (cancelled || !ref.current) return;
      const layerOrder = [...LAYERS.map((l) => l.id), "lib"];
      const buckets = new Map<string, { id: string; label: string }[]>();
      for (const id of layerOrder) buckets.set(id, []);
      for (const n of payload.nodes) {
        const layer = classify(n.label ?? n.id);
        buckets.get(layer)!.push({ id: n.id, label: n.label ?? n.id });
      }

      const width = 1200;
      const rowHeight = 130;
      const height = layerOrder.length * rowHeight + 40;

      const svg = d3.select(ref.current).attr("viewBox", `0 0 ${width} ${height}`);
      svg.selectAll("*").remove();

      const layerColor = d3.scaleOrdinal<string>().domain(layerOrder).range([
        "#4191E1",
        "#41E1B5",
        "#22D3EE",
        "#a78bfa",
        "#7a8aa6",
      ]);

      layerOrder.forEach((layer, idx) => {
        const items = buckets.get(layer) ?? [];
        const y = 20 + idx * rowHeight;
        svg
          .append("rect")
          .attr("x", 10)
          .attr("y", y)
          .attr("width", width - 20)
          .attr("height", rowHeight - 16)
          .attr("rx", 8)
          .attr("fill", "rgba(255,255,255,0.04)")
          .attr("stroke", layerColor(layer))
          .attr("stroke-opacity", 0.4);
        svg
          .append("text")
          .attr("x", 24)
          .attr("y", y + 22)
          .attr("fill", layerColor(layer))
          .attr("font-size", 13)
          .attr("font-weight", 600)
          .text(layer.toUpperCase());

        const cellWidth = 110;
        items.slice(0, 80).forEach((item, i) => {
          const cx = 24 + (i % 10) * cellWidth;
          const cy = y + 40 + Math.floor(i / 10) * 26;
          svg
            .append("rect")
            .attr("x", cx)
            .attr("y", cy)
            .attr("width", cellWidth - 8)
            .attr("height", 22)
            .attr("rx", 4)
            .attr("fill", layerColor(layer))
            .attr("opacity", 0.7);
          svg
            .append("text")
            .attr("x", cx + 6)
            .attr("y", cy + 15)
            .attr("fill", "#0a0e18")
            .attr("font-size", 10)
            .text(item.label.slice(0, 16));
        });
      });
    });
    return () => {
      cancelled = true;
      ac.abort();
    };
  }, []);

  return (
    <div className="vz-view vz-view--layers">
      <svg ref={ref} className="vz-view-canvas" />
    </div>
  );
}
