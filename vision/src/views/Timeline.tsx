import { useEffect, useRef, useState } from "react";
import * as d3 from "d3";
import { fetchCommits, type CommitRow } from "../api/graph";
import { useVisionStore } from "../store";

type Status = "loading" | "empty" | "ready" | "error";

export function Timeline(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [commitCount, setCommitCount] = useState(0);
  const setTimelinePosition = useVisionStore((s) => s.setTimelinePosition);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;

    (async (): Promise<void> => {
      try {
        const res = await fetchCommits(ac.signal, 500);
        if (cancelled || !ref.current) return;
        if (res.error) {
          setError(res.error);
          setStatus("error");
          return;
        }
        const commits: CommitRow[] = res.commits;
        if (commits.length === 0) {
          setStatus("empty");
          return;
        }
        setCommitCount(commits.length);

        const width = 1200;
        const height = 720;
        const margin = { top: 40, right: 30, bottom: 50, left: 60 };

        const parsed = commits
          .map((c) => ({
            ...c,
            dateObj: new Date(c.date),
            churn: c.insertions + c.deletions,
          }))
          .filter((c) => !Number.isNaN(c.dateObj.getTime()))
          .sort((a, b) => a.dateObj.getTime() - b.dateObj.getTime());

        if (parsed.length === 0) {
          setStatus("empty");
          return;
        }

        const x = d3
          .scaleTime()
          .domain(d3.extent(parsed, (d) => d.dateObj) as [Date, Date])
          .range([margin.left, width - margin.right]);
        const maxFiles = d3.max(parsed, (d) => d.files_changed) ?? 1;
        const y = d3
          .scaleLinear()
          .domain([0, maxFiles])
          .nice()
          .range([height - margin.bottom, margin.top]);
        const maxChurn = d3.max(parsed, (d) => d.churn) ?? 1;
        const r = d3.scaleSqrt().domain([0, maxChurn]).range([2, 14]);

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
          .attr("color", "#7a8aa6");

        svg
          .append("text")
          .attr("x", margin.left)
          .attr("y", margin.top - 14)
          .attr("fill", "#cdd6e4")
          .attr("font-size", 11)
          .text("y = files changed  -  size = insertions + deletions");

        svg
          .append("g")
          .selectAll("circle")
          .data(parsed)
          .join("circle")
          .attr("cx", (d) => x(d.dateObj))
          .attr("cy", (d) => y(d.files_changed))
          .attr("r", (d) => r(d.churn))
          .attr("fill", "#4191E1")
          .attr("fill-opacity", 0.55)
          .attr("stroke", "#41E1B5")
          .attr("stroke-width", 0.6)
          .style("cursor", "pointer")
          .on("click", (_evt, d) => setTimelinePosition(d.dateObj.getTime()))
          .append("title")
          .text(
            (d) =>
              `${d.sha.slice(0, 7)} - ${d.author ?? "?"}\n${d.message.split("\n")[0] ?? ""}\n${d.files_changed} files, +${d.insertions}/-${d.deletions}`,
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
  }, [setTimelinePosition]);

  return (
    <div className="vz-view vz-view--timeline-detail">
      <svg ref={ref} className="vz-view-canvas" />
      {status === "loading" && (
        <div className="vz-view-hint" role="status">
          loading git.db commits...
        </div>
      )}
      {status === "empty" && (
        <div className="vz-view-error" role="status">
          no commits in git.db yet -- run <code>mneme build .</code> to index the git history
        </div>
      )}
      {status === "error" && error && (
        <div className="vz-view-error" role="alert">
          timeline error: {error}
        </div>
      )}
      {status === "ready" && (
        <p className="vz-view-hint">{commitCount.toLocaleString()} commits from git.db</p>
      )}
    </div>
  );
}
