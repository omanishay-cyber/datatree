import { useEffect, useRef, useState } from "react";
import * as d3 from "d3";
import { fetchLayerTiers, type LayerTierEntry } from "../api/graph";

type Status = "loading" | "empty" | "ready" | "error";

export function LayeredArchitecture(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [stats, setStats] = useState<{ tiers: number; files: number }>({
    tiers: 0,
    files: 0,
  });

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;

    (async (): Promise<void> => {
      try {
        const res = await fetchLayerTiers(ac.signal);
        if (cancelled || !ref.current) return;
        if (res.error) {
          setError(res.error);
          setStatus("error");
          return;
        }
        if (res.entries.length === 0) {
          setStatus("empty");
          return;
        }

        // Group by tier -> domain
        const tierOrder = res.tiers;
        const buckets = new Map<string, Map<string, LayerTierEntry[]>>();
        for (const tier of tierOrder) buckets.set(tier, new Map());
        for (const entry of res.entries) {
          let t = buckets.get(entry.tier);
          if (!t) {
            t = new Map();
            buckets.set(entry.tier, t);
          }
          const arr = t.get(entry.domain) ?? [];
          arr.push(entry);
          t.set(entry.domain, arr);
        }

        const tiersWithContent = tierOrder.filter((t) => (buckets.get(t)?.size ?? 0) > 0);
        setStats({ tiers: tiersWithContent.length, files: res.entries.length });

        const width = 1200;
        const rowHeight = 140;
        const height = tiersWithContent.length * rowHeight + 40;

        const svg = d3.select(ref.current).attr("viewBox", `0 0 ${width} ${height}`);
        svg.selectAll("*").remove();

        const tierColor = d3
          .scaleOrdinal<string>()
          .domain(tierOrder)
          .range([
            "#4191E1",
            "#41E1B5",
            "#22D3EE",
            "#a78bfa",
            "#f59e0b",
            "#7a8aa6",
          ]);

        tiersWithContent.forEach((tier, idx) => {
          const domainMap = buckets.get(tier);
          if (!domainMap) return;
          const y = 20 + idx * rowHeight;

          svg
            .append("rect")
            .attr("x", 10)
            .attr("y", y)
            .attr("width", width - 20)
            .attr("height", rowHeight - 16)
            .attr("rx", 10)
            .attr("fill", "rgba(255,255,255,0.03)")
            .attr("stroke", tierColor(tier))
            .attr("stroke-opacity", 0.45);

          svg
            .append("text")
            .attr("x", 24)
            .attr("y", y + 22)
            .attr("fill", tierColor(tier))
            .attr("font-size", 13)
            .attr("font-weight", 700)
            .text(tier.toUpperCase());

          // Totals for the tier
          let totalFiles = 0;
          let totalLoc = 0;
          for (const arr of domainMap.values()) {
            totalFiles += arr.length;
            totalLoc += arr.reduce((s, e) => s + e.line_count, 0);
          }
          svg
            .append("text")
            .attr("x", 24)
            .attr("y", y + 38)
            .attr("fill", "#7a8aa6")
            .attr("font-size", 11)
            .text(`${totalFiles} files - ${totalLoc.toLocaleString()} LoC`);

          // Stacked bar per domain, sized by LoC
          const domains = Array.from(domainMap.entries()).sort((a, b) => {
            const la = a[1].reduce((s, e) => s + e.line_count, 0);
            const lb = b[1].reduce((s, e) => s + e.line_count, 0);
            return lb - la;
          });
          const maxLoc = Math.max(1, totalLoc);
          const barAreaX = 200;
          const barAreaW = width - barAreaX - 20;
          let cursor = 0;
          for (const [domain, entries] of domains) {
            const loc = entries.reduce((s, e) => s + e.line_count, 0);
            const w = (loc / maxLoc) * barAreaW;
            if (w < 0.5) {
              cursor += w;
              continue;
            }
            svg
              .append("rect")
              .attr("x", barAreaX + cursor)
              .attr("y", y + 50)
              .attr("width", Math.max(1, w - 1))
              .attr("height", rowHeight - 70)
              .attr("rx", 3)
              .attr("fill", tierColor(tier))
              .attr("opacity", 0.75)
              .append("title")
              .text(`${tier} / ${domain}: ${entries.length} files, ${loc.toLocaleString()} LoC`);
            if (w > 60) {
              svg
                .append("text")
                .attr("x", barAreaX + cursor + 6)
                .attr("y", y + 70)
                .attr("fill", "#0a0e18")
                .attr("font-size", 11)
                .attr("font-weight", 600)
                .text(domain);
              svg
                .append("text")
                .attr("x", barAreaX + cursor + 6)
                .attr("y", y + 86)
                .attr("fill", "#0a0e18")
                .attr("font-size", 10)
                .text(`${entries.length} files`);
            }
            cursor += w;
          }
        });

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
    <div className="vz-view vz-view--layers">
      <svg ref={ref} className="vz-view-canvas" />
      {status === "loading" && (
        <div className="vz-view-hint" role="status">
          loading graph.db files, classifying by tier...
        </div>
      )}
      {status === "empty" && (
        <div className="vz-view-error" role="status">
          no files in shard -- run <code>mneme build .</code> to index the project
        </div>
      )}
      {status === "error" && error && (
        <div className="vz-view-error" role="alert">
          layers error: {error}
        </div>
      )}
      {status === "ready" && (
        <p className="vz-view-hint">
          {stats.tiers} tiers - {stats.files.toLocaleString()} files
        </p>
      )}
    </div>
  );
}
