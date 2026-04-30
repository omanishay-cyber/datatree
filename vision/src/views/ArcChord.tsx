import { useEffect, useRef, useState } from "react";
import * as d3 from "d3";
import { fetchCommunityMatrix } from "../api/graph";

type Status = "loading" | "empty" | "ready" | "error";

export function ArcChord(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [communityCount, setCommunityCount] = useState(0);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;

    (async (): Promise<void> => {
      try {
        const res = await fetchCommunityMatrix(ac.signal);
        if (cancelled || !ref.current) return;
        if (res.error) {
          setError(res.error);
          setStatus("error");
          return;
        }
        if (res.communities.length === 0 || res.matrix.length === 0) {
          setStatus("empty");
          return;
        }
        const matrix = res.matrix;
        const totalEdges = matrix.reduce(
          (s, row) => s + row.reduce((a, b) => a + b, 0),
          0,
        );
        if (totalEdges === 0) {
          setStatus("empty");
          return;
        }
        setCommunityCount(res.communities.length);

        const labels = res.communities.map((c) => c.name);

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
          .attr("stroke", "#0a0e18")
          .append("title")
          .text((d) => `${labels[d.index] ?? ""} (${d.value})`);

        svg
          .append("g")
          .attr("fill-opacity", 0.55)
          .selectAll("path")
          .data(chords)
          .join("path")
          .attr("d", ribbon)
          .attr("fill", (d) => color(String(d.target.index)))
          .append("title")
          .text(
            (d) =>
              `${labels[d.source.index] ?? "?"} -> ${labels[d.target.index] ?? "?"} : ${d.source.value}`,
          );

        svg
          .append("g")
          .selectAll("text")
          .data(chords.groups)
          .join("text")
          .attr("transform", (d) => {
            const angle = (d.startAngle + d.endAngle) / 2;
            return `rotate(${(angle * 180) / Math.PI - 90}) translate(${outer + 8})${
              angle > Math.PI ? " rotate(180)" : ""
            }`;
          })
          .attr("text-anchor", (d) =>
            (d.startAngle + d.endAngle) / 2 > Math.PI ? "end" : "start",
          )
          .attr("fill", "#cdd6e4")
          .attr("font-size", 10)
          .text((d) => labels[d.index] ?? "");

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
    <div className="vz-view vz-view--chord">
      <svg ref={ref} className="vz-view-canvas" />
      {status === "loading" && (
        <div className="vz-view-hint" role="status">
          loading semantic.db communities + graph.db edges...
        </div>
      )}
      {status === "empty" && (
        <div className="vz-view-error" role="status">
          no communities in semantic.db yet -- run <code>mneme build .</code> and let brain cluster
        </div>
      )}
      {status === "error" && error && (
        <div className="vz-view-error" role="alert">
          chord error: {error}
        </div>
      )}
      {status === "ready" && (
        <p className="vz-view-hint">
          {communityCount.toLocaleString()} communities - cross-edge density
        </p>
      )}
    </div>
  );
}
