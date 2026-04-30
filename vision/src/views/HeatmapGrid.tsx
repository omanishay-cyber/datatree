import { useEffect, useRef, useState } from "react";
import * as d3 from "d3";
import { fetchHeatmap, type HeatmapFileRow } from "../api/graph";

type Status = "loading" | "empty" | "ready" | "error";

type SeverityKey = keyof HeatmapFileRow["severities"];
const SEVERITIES: SeverityKey[] = ["critical", "high", "medium", "low"];

export function HeatmapGrid(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [fileCount, setFileCount] = useState(0);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;

    (async (): Promise<void> => {
      try {
        const res = await fetchHeatmap(ac.signal, 120);
        if (cancelled || !ref.current) return;
        if (res.error) {
          setError(res.error);
          setStatus("error");
          return;
        }
        if (res.files.length === 0) {
          setStatus("empty");
          return;
        }
        setFileCount(res.files.length);

        // Keep the files that have *something* interesting (findings OR complexity > 0)
        const files = res.files
          .slice()
          .sort((a, b) => {
            const ascore =
              a.severities.critical * 4 +
              a.severities.high * 3 +
              a.severities.medium * 2 +
              a.severities.low +
              a.complexity * 0.1;
            const bscore =
              b.severities.critical * 4 +
              b.severities.high * 3 +
              b.severities.medium * 2 +
              b.severities.low +
              b.complexity * 0.1;
            return bscore - ascore;
          })
          .slice(0, 60);

        // 5 columns: 4 severity + 1 complexity
        const cols: string[] = [...SEVERITIES, "complexity"];
        const cellW = 120;
        const cellH = 20;
        const margin = { top: 60, right: 20, bottom: 20, left: 240 };
        const width = margin.left + cols.length * cellW + margin.right;
        const height = margin.top + files.length * cellH + margin.bottom;

        const svg = d3.select(ref.current).attr("viewBox", `0 0 ${width} ${height}`);
        svg.selectAll("*").remove();

        const findingsColor = d3.scaleSequential(d3.interpolateInferno).domain([
          0,
          Math.max(
            1,
            d3.max(files, (f) => Math.max(...SEVERITIES.map((s) => f.severities[s]))) ?? 1,
          ),
        ]);
        const complexityColor = d3
          .scaleSequential(d3.interpolateViridis)
          .domain([0, Math.max(1, d3.max(files, (f) => f.complexity) ?? 1)]);

        // Column headers
        svg
          .append("g")
          .attr("transform", `translate(${margin.left},${margin.top - 8})`)
          .selectAll("text")
          .data(cols)
          .join("text")
          .attr("x", (_d, i) => i * cellW + cellW / 2)
          .attr("text-anchor", "middle")
          .attr("fill", "#cdd6e4")
          .attr("font-size", 12)
          .attr("font-weight", 600)
          .text((c) => c);

        const rows = svg
          .append("g")
          .attr("transform", `translate(${margin.left},${margin.top})`)
          .selectAll("g")
          .data(files)
          .join("g")
          .attr("transform", (_d, i) => `translate(0, ${i * cellH})`);

        rows.each(function (d) {
          const g = d3.select(this);
          SEVERITIES.forEach((sev, i) => {
            const val = d.severities[sev];
            g.append("rect")
              .attr("x", i * cellW + 1)
              .attr("y", 1)
              .attr("width", cellW - 2)
              .attr("height", cellH - 2)
              .attr("rx", 2)
              .attr("fill", val > 0 ? findingsColor(val) : "rgba(255,255,255,0.03)");
            if (val > 0) {
              g.append("text")
                .attr("x", i * cellW + cellW / 2)
                .attr("y", cellH / 2 + 4)
                .attr("text-anchor", "middle")
                .attr("fill", "#0a0e18")
                .attr("font-size", 11)
                .attr("font-weight", 600)
                .text(String(val));
            }
          });
          // complexity column
          g.append("rect")
            .attr("x", SEVERITIES.length * cellW + 1)
            .attr("y", 1)
            .attr("width", cellW - 2)
            .attr("height", cellH - 2)
            .attr("rx", 2)
            .attr("fill", d.complexity > 0 ? complexityColor(d.complexity) : "rgba(255,255,255,0.03)");
          if (d.complexity > 0) {
            g.append("text")
              .attr("x", SEVERITIES.length * cellW + cellW / 2)
              .attr("y", cellH / 2 + 4)
              .attr("text-anchor", "middle")
              .attr("fill", "#0a0e18")
              .attr("font-size", 11)
              .text(String(d.complexity));
          }
        });

        // Row labels (file paths) on the left
        svg
          .append("g")
          .attr("transform", `translate(${margin.left - 8},${margin.top})`)
          .selectAll("text")
          .data(files)
          .join("text")
          .attr("y", (_d, i) => i * cellH + cellH / 2 + 4)
          .attr("text-anchor", "end")
          .attr("fill", "#cdd6e4")
          .attr("font-size", 10)
          .text((d) => {
            const s = d.file;
            return s.length > 38 ? `...${s.slice(-36)}` : s;
          })
          .append("title")
          .text((d) => d.file);

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
    <div className="vz-view vz-view--heatmap">
      <svg ref={ref} className="vz-view-canvas" />
      {status === "loading" && (
        <div className="vz-view-hint" role="status">
          loading findings.db x graph.db join...
        </div>
      )}
      {status === "empty" && (
        <div className="vz-view-error" role="status">
          no files to heat-map -- run <code>mneme build .</code> and re-audit
        </div>
      )}
      {status === "error" && error && (
        <div className="vz-view-error" role="alert">
          heatmap error: {error}
        </div>
      )}
      {status === "ready" && (
        <p className="vz-view-hint">
          {fileCount.toLocaleString()} files scored by severity + complexity
        </p>
      )}
    </div>
  );
}
