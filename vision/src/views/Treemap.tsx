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

/**
 * Truncate a file name to fit the cell width.
 *
 * Matches graphify's treemap label heuristic: keep the extension when
 * possible (`.tsx` / `.rs` / `.py` are how engineers scan a project at
 * a glance), eat into the basename. CRG just hides the label below
 * 50px which is the ugly behaviour mneme had pre-fix — half the cells
 * read as anonymous coloured rectangles.
 */
function fitLabel(name: string, cellWidth: number): string {
  // ~7px per char for 11px font. Leave 8px padding either side.
  const max = Math.max(2, Math.floor((cellWidth - 16) / 7));
  if (name.length <= max) return name;
  const dot = name.lastIndexOf(".");
  if (dot > 0 && name.length - dot <= 6) {
    const ext = name.slice(dot);
    const headBudget = max - ext.length - 1;
    if (headBudget >= 2) {
      return `${name.slice(0, headBudget)}…${ext}`;
    }
  }
  return `${name.slice(0, max - 1)}…`;
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
      const t0 = performance.now();
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
        // L11 fix (2026-05-05 audit): bind the laid-out root to its
        // post-layout type. d3.treemap() mutates and returns root with
        // x0/y0/x1/y1 fields populated, so the right type is
        // HierarchyRectangularNode<TreemapDatum>. Single binding here =
        // zero downstream `as d3.HierarchyRectangularNode<...>` casts.
        const positioned = d3
          .treemap<TreemapDatum>()
          .size([width, height])
          .paddingOuter(2)
          .paddingTop(18)
          .paddingInner(1)
          .round(true)(root);

        // Color by language. Falls back to per-language palette swatch;
        // unknown languages get the gray.
        const languages = Array.from(
          new Set(
            positioned
              .leaves()
              .map((l) => (l.data.language as string | null | undefined) ?? "unknown"),
          ),
        );
        const color = d3.scaleOrdinal<string>().domain(languages).range(d3.schemeTableau10);

        const svg = d3.select(ref.current).attr("viewBox", `0 0 ${width} ${height}`);
        svg.selectAll("*").remove();

        // Parent nodes — paint their headers as labelled bands so the
        // user can read directory structure inside the treemap. CRG
        // doesn't do this; graphify does, and it's the single biggest
        // legibility win.
        const parents = positioned
          .descendants()
          .filter((d) => d.depth > 0 && d.children);
        const pCell = svg
          .append("g")
          .attr("class", "vz-treemap-parents")
          .selectAll("g")
          .data(parents)
          .join("g")
          .attr("transform", (d) => `translate(${d.x0},${d.y0})`);
        pCell
          .append("rect")
          .attr("width", (d) => Math.max(0, d.x1 - d.x0))
          .attr("height", 16)
          .attr("fill", "rgba(255,255,255,0.04)")
          .attr("stroke", "rgba(255,255,255,0.06)");
        pCell
          .append("text")
          .attr("x", 6)
          .attr("y", 11)
          .attr("fill", "var(--fg-1, #cdd6e4)")
          .attr("font-size", 10)
          .attr("font-weight", 600)
          .text((d) => fitLabel(d.data.name, d.x1 - d.x0));

        const leaves = positioned.leaves();
        const cell = svg
          .append("g")
          .attr("class", "vz-treemap-leaves")
          .selectAll("g")
          .data(leaves)
          .join("g")
          .attr("transform", (d) => `translate(${d.x0},${d.y0})`);
        cell
          .append("rect")
          .attr("width", (d) => Math.max(0, d.x1 - d.x0))
          .attr("height", (d) => Math.max(0, d.y1 - d.y0))
          .attr("fill", (d) => color(String(d.data.language ?? "unknown")))
          .attr("opacity", 0.85)
          .attr("stroke", "var(--bg-1, #0a0e18)")
          .attr("stroke-width", 0.5);
        cell
          .append("title")
          .text(
            (d) =>
              `${d
                .ancestors()
                .reverse()
                .slice(1)
                .map((a) => a.data.name)
                .join("/")}\n` +
              `${d.data.language ?? "unknown"} - ${(d.data.value ?? 0).toLocaleString()} LoC`,
          );
        // Always paint a label, even when the cell is small — auto-trim
        // by available width so we get readable text in 30px-wide cells
        // instead of nothing. Cells under ~14px wide skip the label
        // entirely (no font fits).
        cell
          .filter((d) => d.x1 - d.x0 >= 18 && d.y1 - d.y0 >= 14)
          .append("text")
          .attr("x", 4)
          .attr("y", 12)
          .attr("fill", "var(--bg-1, #0a0e18)")
          .attr("font-size", 11)
          .attr("font-weight", 500)
          .text((d) => fitLabel(d.data.name, d.x1 - d.x0));

        setStatus("ready");

        const elapsed = performance.now() - t0;
        if (elapsed > 500) {
          // eslint-disable-next-line no-console
          console.warn(`treemap first-paint ${elapsed.toFixed(0)}ms (>500 budget)`);
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
