import { useEffect, useRef, useState } from "react";
import * as d3 from "d3";
import { fetchCommunityMatrix } from "../api/graph";

type Status = "loading" | "empty-no-communities" | "empty-no-edges" | "ready" | "error";

export function ArcChord(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [communityCount, setCommunityCount] = useState(0);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;

    (async (): Promise<void> => {
      const t0 = performance.now();
      try {
        const res = await fetchCommunityMatrix(ac.signal);
        if (cancelled || !ref.current) return;
        if (res.error) {
          setError(res.error);
          setStatus("error");
          return;
        }
        if (res.communities.length === 0 || res.matrix.length === 0) {
          // Distinguish "semantic.db hasn't been built yet" from "graph
          // has no cross-community edges". Both look the same to a
          // first-time user otherwise — the previous unified empty
          // state silently buried the more common cause (no clustering
          // run yet) under the rarer one.
          setStatus("empty-no-communities");
          return;
        }
        const matrix = res.matrix;
        const totalEdges = matrix.reduce(
          (s, row) => s + row.reduce((a, b) => a + b, 0),
          0,
        );
        if (totalEdges === 0) {
          setStatus("empty-no-edges");
          return;
        }
        setCommunityCount(res.communities.length);

        const labels = res.communities.map((c) => c.name);

        const width = 760;
        const height = 760;
        // Reserve outer label space on the canvas. Earlier code clipped
        // long community names because `outer` ate every pixel up to
        // the SVG edge.
        const labelPad = 90;
        const outer = Math.min(width, height) * 0.5 - labelPad;
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

        const groupArcs = svg
          .append("g")
          .selectAll("path")
          .data(chords.groups)
          .join("path")
          .attr("d", arc)
          .attr("fill", (d) => color(String(d.index)))
          .attr("stroke", "var(--bg-1, #0a0e18)")
          .style("cursor", "pointer");
        groupArcs
          .append("title")
          .text((d) => `${labels[d.index] ?? ""} (${d.value})`);

        const ribbonG = svg
          .append("g")
          .attr("fill-opacity", 0.55)
          .selectAll("path")
          .data(chords)
          .join("path")
          .attr("d", ribbon)
          .attr("fill", (d) => color(String(d.target.index)))
          .attr("stroke", "rgba(255,255,255,0.06)");
        ribbonG
          .append("title")
          .text(
            (d) =>
              `${labels[d.source.index] ?? "?"} -> ${labels[d.target.index] ?? "?"}: ${d.source.value}`,
          );

        // Hover interaction: dim non-incident ribbons + arcs to 0.08.
        // CRG ships this; mneme didn't. Without it the chord diagram is
        // a noise blob and you can't read which community talks to
        // which.
        const dim = (idx: number | null): void => {
          ribbonG.attr("fill-opacity", (d) => {
            if (idx === null) return 0.55;
            return d.source.index === idx || d.target.index === idx ? 0.78 : 0.06;
          });
          groupArcs.attr("opacity", (d) => {
            if (idx === null) return 1;
            return d.index === idx ? 1 : 0.35;
          });
        };
        groupArcs
          .on("mouseenter", (_evt, d) => dim(d.index))
          .on("mouseleave", () => dim(null));

        // Labels: only paint for arcs with enough sweep that the text
        // fits without colliding with neighbours. Mirrors the
        // arc-length-based label gating in Sunburst.tsx so the two
        // radial views feel coherent.
        const labelGroup = svg.append("g").attr("class", "vz-chord-labels");
        chords.groups.forEach((d) => {
          const sweep = d.endAngle - d.startAngle;
          // Need at least 0.04 rad (~2.3°) of sweep to safely fit a
          // short label without overlapping its neighbours.
          if (sweep < 0.04) return;

          const angle = (d.startAngle + d.endAngle) / 2;
          const flipped = angle > Math.PI;
          const label = labels[d.index] ?? "";
          const truncated =
            label.length > 16 ? `${label.slice(0, 15)}…` : label;
          labelGroup
            .append("text")
            .attr(
              "transform",
              `rotate(${(angle * 180) / Math.PI - 90}) translate(${outer + 8})${
                flipped ? " rotate(180)" : ""
              }`,
            )
            .attr("text-anchor", flipped ? "end" : "start")
            .attr("dy", "0.32em")
            .attr("fill", "var(--fg-1, #cdd6e4)")
            .attr("font-size", 11)
            .attr("font-weight", 500)
            .text(truncated)
            .append("title")
            .text(label);
        });

        setStatus("ready");

        const elapsed = performance.now() - t0;
        if (elapsed > 500) {
          // eslint-disable-next-line no-console
          console.warn(`arc-chord first-paint ${elapsed.toFixed(0)}ms (>500 budget)`);
        }
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
      {status === "empty-no-communities" && (
        <div className="vz-view-error" role="status">
          no community clusters yet -- semantic.db is empty.
          Run <code>mneme build .</code> and let the brain cluster, or
          run <code>mneme audit . --semantic</code>.
        </div>
      )}
      {status === "empty-no-edges" && (
        <div className="vz-view-error" role="status">
          {communityCount.toLocaleString()} communities found, but no cross-community
          edges in this slice. Try a wider date range, or run <code>mneme build .</code>.
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
