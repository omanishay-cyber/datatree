import { useEffect, useRef, useState } from "react";
import * as d3 from "d3";
import { fetchTestCoverage, type TestCoverageRow } from "../api/graph";

type Status = "loading" | "empty" | "ready" | "error";

export function TestCoverageMap(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [stats, setStats] = useState<{ total: number; covered: number }>({
    total: 0,
    covered: 0,
  });

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;

    (async (): Promise<void> => {
      try {
        const res = await fetchTestCoverage(ac.signal, 2000);
        if (cancelled || !ref.current) return;
        if (res.error) {
          setError(res.error);
          setStatus("error");
          return;
        }
        const rows: TestCoverageRow[] = res.rows;
        if (rows.length === 0) {
          setStatus("empty");
          return;
        }
        const covered = rows.filter((r) => r.covered).length;
        setStats({ total: rows.length, covered });

        // Sort: uncovered large files first (they need tests most).
        const sorted = rows.slice().sort((a, b) => {
          if (a.covered !== b.covered) return a.covered ? 1 : -1;
          return b.line_count - a.line_count;
        });

        const cols = Math.ceil(Math.sqrt(sorted.length));
        const cell = 32;
        const width = cols * cell + 40;
        const height = Math.ceil(sorted.length / cols) * cell + 60;

        const maxTests = Math.max(1, ...sorted.map((r) => r.test_count));
        const color = d3.scaleSequential(d3.interpolateRdYlGn).domain([0, maxTests]);

        const svg = d3.select(ref.current).attr("viewBox", `0 0 ${width} ${height}`);
        svg.selectAll("*").remove();

        svg
          .append("text")
          .attr("x", 20)
          .attr("y", 20)
          .attr("fill", "#cdd6e4")
          .attr("font-size", 12)
          .text(
            `${covered.toLocaleString()} / ${rows.length.toLocaleString()} files have tests (${Math.round((covered / rows.length) * 100)}%)`,
          );

        const g = svg.append("g").attr("transform", "translate(20,40)");
        g.selectAll("rect")
          .data(sorted)
          .join("rect")
          .attr("x", (_d, i) => (i % cols) * cell)
          .attr("y", (_d, i) => Math.floor(i / cols) * cell)
          .attr("width", cell - 3)
          .attr("height", cell - 3)
          .attr("rx", 4)
          .attr("fill", (d) => (d.covered ? color(d.test_count) : "#ef4444"))
          .attr("opacity", (d) => (d.covered ? 0.9 : 0.55))
          .attr("stroke", (d) => (d.covered ? "none" : "#fca5a5"))
          .attr("stroke-width", 0.5)
          .append("title")
          .text(
            (d) =>
              `${d.file}\n${d.line_count.toLocaleString()} LoC\n${
                d.covered ? `${d.test_count} tests` : "no test file"
              }${d.test_file ? `\n-> ${d.test_file}` : ""}`,
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
    <div className="vz-view vz-view--coverage">
      <svg ref={ref} className="vz-view-canvas" />
      {status === "loading" && (
        <div className="vz-view-hint" role="status">
          mapping source files to tests...
        </div>
      )}
      {status === "empty" && (
        <div className="vz-view-error" role="status">
          no source files in shard -- run <code>mneme build .</code>
        </div>
      )}
      {status === "error" && error && (
        <div className="vz-view-error" role="alert">
          coverage error: {error}
        </div>
      )}
      {status === "ready" && (
        <p className="vz-view-hint">
          {stats.covered.toLocaleString()} / {stats.total.toLocaleString()} files covered
        </p>
      )}
    </div>
  );
}
