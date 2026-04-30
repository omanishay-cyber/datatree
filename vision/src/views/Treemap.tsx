import { useEffect, useRef, useState } from "react";
import * as d3 from "d3";
import { fetchFiles, type ShardFileRow } from "../api/graph";

interface TreemapDatum {
  name: string;
  value?: number;
  language?: string | null;
  children?: TreemapDatum[];
}

type Status = "loading" | "empty" | "ready" | "error";

function buildTree(files: ShardFileRow[]): TreemapDatum {
  const root: TreemapDatum = { name: "project", children: [] };
  for (const f of files) {
    const segs = f.path.split(/[/\\]/).filter(Boolean);
    let cursor = root;
    for (let i = 0; i < segs.length; i += 1) {
      const seg = segs[i] ?? "";
      cursor.children = cursor.children ?? [];
      let child = cursor.children.find((c) => c.name === seg);
      if (!child) {
        child = { name: seg, children: [] };
        cursor.children.push(child);
      }
      if (i === segs.length - 1) {
        child.value = Math.max(1, f.line_count ?? 1);
        child.language = f.language;
      }
      cursor = child;
    }
  }
  return root;
}

export function Treemap(): JSX.Element {
  const ref = useRef<SVGSVGElement | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [fileCount, setFileCount] = useState(0);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;

    (async (): Promise<void> => {
      try {
        const res = await fetchFiles(ac.signal, 2000);
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

        const data = buildTree(res.files);
        const width = 1200;
        const height = 720;
        const root = d3
          .hierarchy<TreemapDatum>(data)
          .sum((d) => d.value ?? 0)
          .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));
        d3.treemap<TreemapDatum>().size([width, height]).padding(2)(root);

        // Color by language (falls back to first-ancestor name).
        const languages = Array.from(
          new Set(
            root
              .leaves()
              .map((l) => (l.data.language as string | null | undefined) ?? "unknown"),
          ),
        );
        const color = d3.scaleOrdinal<string>().domain(languages).range(d3.schemeTableau10);

        const svg = d3.select(ref.current).attr("viewBox", `0 0 ${width} ${height}`);
        svg.selectAll("*").remove();

        const leaves = root.leaves() as d3.HierarchyRectangularNode<TreemapDatum>[];
        const cell = svg
          .selectAll("g")
          .data(leaves)
          .join("g")
          .attr("transform", (d) => `translate(${d.x0},${d.y0})`);
        cell
          .append("rect")
          .attr("width", (d) => Math.max(0, d.x1 - d.x0))
          .attr("height", (d) => Math.max(0, d.y1 - d.y0))
          .attr("fill", (d) => color(String(d.data.language ?? "unknown")))
          .attr("opacity", 0.85);
        cell
          .append("title")
          .text(
            (d) =>
              `${d.ancestors().reverse().slice(1).map((a) => a.data.name).join("/")}\n` +
              `${d.data.language ?? "unknown"} · ${(d.data.value ?? 0).toLocaleString()} LoC`,
          );
        cell
          .append("text")
          .attr("x", 4)
          .attr("y", 14)
          .attr("fill", "#0a0e18")
          .attr("font-size", 11)
          .text((d) => (d.x1 - d.x0 > 60 ? d.data.name : ""));

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
    <div className="vz-view vz-view--treemap">
      <svg ref={ref} className="vz-view-canvas" />
      {status === "loading" && (
        <div className="vz-view-hint" role="status">
          loading graph.db files…
        </div>
      )}
      {status === "empty" && (
        <div className="vz-view-error" role="status">
          no files in shard — run <code>mneme build .</code> to index the project
        </div>
      )}
      {status === "error" && error && (
        <div className="vz-view-error" role="alert">
          treemap error: {error}
        </div>
      )}
      {status === "ready" && (
        <p className="vz-view-hint">{fileCount.toLocaleString()} files · weighted by LoC</p>
      )}
    </div>
  );
}
