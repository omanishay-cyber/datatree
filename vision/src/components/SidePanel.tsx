import { useEffect, useState } from "react";
import { useVisionStore, shallow } from "../store";
import { API_BASE } from "../api";
import { withProject } from "../projectSelection";

interface FileDetail {
  path: string;
  summary: string;
  lines: number;
  tests: { name: string; status: "pass" | "fail" | "skip" }[];
  history: { hash: string; ts: number; message: string }[];
  preview: string;
}

function placeholderDetail(path: string): FileDetail {
  return {
    path,
    summary: "summary unavailable until daemon is connected.",
    lines: 0,
    tests: [],
    history: [],
    preview: "",
  };
}

export function SidePanel(): JSX.Element {
  // A6-022: shallow equality on selectedNodes (array reference would
  // otherwise change on every store mutation).
  const selected = useVisionStore((s) => s.selectedNodes, shallow);
  const clear = useVisionStore((s) => s.clearSelection);
  const [detail, setDetail] = useState<FileDetail | null>(null);
  const [tab, setTab] = useState<"summary" | "tests" | "history" | "preview">("summary");

  useEffect(() => {
    if (selected.length === 0) {
      setDetail(null);
      return;
    }
    const target = selected[0];
    if (!target) return;
    // Bug #NEW-J (2026-05-04): the previous code fetched
    // `/api/graph?view=file-detail&id=...` which is a stub returning
    // 501. That spammed the console with 501 errors on every node click.
    // No server endpoint exists today for per-node detail, so use the
    // placeholder synchronously. v0.5.0 should add /api/graph/files/:id
    // and switch this back to a real fetch with proper error handling.
    setDetail(placeholderDetail(target.label ?? target.id));
  }, [selected]);

  if (selected.length === 0) {
    return (
      <div className="vz-side vz-side--empty">
        <p>select a node to inspect.</p>
      </div>
    );
  }

  return (
    <div className="vz-side">
      <header className="vz-side-header">
        <h2>{detail?.path ?? selected[0]?.label ?? selected[0]?.id}</h2>
        <button type="button" className="vz-side-close" onClick={() => clear()} aria-label="clear selection">
          ×
        </button>
      </header>
      <nav className="vz-side-tabs" aria-label="detail tabs">
        {(["summary", "tests", "history", "preview"] as const).map((t) => (
          <button
            type="button"
            key={t}
            className={`vz-side-tab ${tab === t ? "is-active" : ""}`}
            onClick={() => setTab(t)}
          >
            {t}
          </button>
        ))}
      </nav>
      <section className="vz-side-body">
        {tab === "summary" && <p>{detail?.summary ?? "—"}</p>}
        {tab === "tests" && (
          <ul className="vz-side-tests">
            {(detail?.tests ?? []).length === 0 && <li className="vz-cc-empty">no tests linked</li>}
            {detail?.tests.map((t) => (
              <li key={t.name} className={`vz-test vz-test--${t.status}`}>
                <span>{t.status}</span>
                {t.name}
              </li>
            ))}
          </ul>
        )}
        {tab === "history" && (
          <ul className="vz-side-history">
            {(detail?.history ?? []).length === 0 && <li className="vz-cc-empty">no history yet</li>}
            {detail?.history.map((h) => (
              <li key={h.hash}>
                <code>{h.hash.slice(0, 7)}</code>
                <time>{new Date(h.ts).toLocaleDateString()}</time>
                <span>{h.message}</span>
              </li>
            ))}
          </ul>
        )}
        {tab === "preview" && (
          <pre className="vz-side-preview">{detail?.preview || "preview unavailable"}</pre>
        )}
      </section>
    </div>
  );
}
