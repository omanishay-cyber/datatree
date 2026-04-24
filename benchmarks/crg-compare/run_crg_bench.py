"""Direct CRG benchmark harness against the datatree (mneme) repo.

Runs against the already-built .code-review-graph/graph.db in the repo root.
Emits JSON + CSV to benchmarks/results/crg-YYYY-MM-DD.{json,csv}.
"""

from __future__ import annotations

import csv
import json
import statistics
import time
from pathlib import Path

from code_review_graph.graph import GraphStore
from code_review_graph.incremental import get_db_path


REPO = Path(__file__).resolve().parents[2]  # datatree root
OUT_DIR = REPO / "benchmarks" / "results"
OUT_DIR.mkdir(parents=True, exist_ok=True)


# Same 10 queries used by mneme's compare suite, so the comparison is apples-to-apples.
QUERIES = [
    "where is DbLayer defined",
    "callers of inject_file",
    "drift detection",
    "blast radius implementation",
    "PathManager",
    "build_or_migrate",
    "Store::new",
    "parser pool",
    "embedding store",
    "schema version",
]


def count_tokens(text: str) -> int:
    return len(text) // 4


def main() -> None:
    db = get_db_path(REPO)
    store = GraphStore(db)

    stats = store.get_stats()
    db_bytes = db.stat().st_size

    search_times_ms: list[float] = []
    per_query: list[dict] = []

    for q in QUERIES:
        samples = []
        top_node = None
        top_tokens = 0
        for _ in range(5):
            t0 = time.perf_counter()
            nodes = store.search_nodes(q, limit=10)
            dt_ms = (time.perf_counter() - t0) * 1000.0
            samples.append(dt_ms)
            if top_node is None and nodes:
                top_node = nodes[0]
                # Estimate tokens returned: serialize node metadata to JSON.
                try:
                    payload = {
                        "qualified_name": getattr(top_node, "qualified_name", ""),
                        "file_path": getattr(top_node, "file_path", ""),
                        "node_type": getattr(top_node, "node_type", ""),
                        "signature": getattr(top_node, "signature", ""),
                    }
                    top_tokens = count_tokens(json.dumps(payload))
                except Exception:
                    top_tokens = 0

        p50 = statistics.median(samples)
        mean = statistics.mean(samples)
        search_times_ms.extend(samples)
        per_query.append(
            {
                "query": q,
                "top_node": getattr(top_node, "qualified_name", None) if top_node else None,
                "top_tokens": top_tokens,
                "ms_p50": round(p50, 3),
                "ms_mean": round(mean, 3),
                "ms_min": round(min(samples), 3),
                "ms_max": round(max(samples), 3),
            }
        )

    store.close()

    all_sorted = sorted(search_times_ms)
    summary = {
        "tool": "code-review-graph",
        "version": "2.3.2",
        "fixture": "mneme (datatree) repo",
        "files_indexed": stats.files_count,
        "nodes": stats.total_nodes,
        "edges": stats.total_edges,
        "graph_db_bytes": db_bytes,
        "bytes_per_node": round(db_bytes / max(stats.total_nodes, 1), 1),
        "bytes_per_edge": round(db_bytes / max(stats.total_edges, 1), 1),
        "search_ms_p50": round(statistics.median(all_sorted), 3),
        "search_ms_p95": round(all_sorted[int(len(all_sorted) * 0.95) - 1], 3),
        "search_ms_mean": round(statistics.mean(all_sorted), 3),
        "search_ms_max": round(max(all_sorted), 3),
        "queries": per_query,
    }

    today = time.strftime("%Y-%m-%d")
    json_path = OUT_DIR / f"crg-{today}.json"
    csv_path = OUT_DIR / f"crg-{today}.csv"

    json_path.write_text(json.dumps(summary, indent=2))

    # CSV: one row per query plus an aggregate row.
    with csv_path.open("w", newline="") as f:
        w = csv.writer(f)
        w.writerow(
            [
                "metric",
                "query",
                "top_node",
                "top_tokens",
                "ms_p50",
                "ms_mean",
                "ms_min",
                "ms_max",
            ]
        )
        for row in per_query:
            w.writerow(
                [
                    "per_query",
                    row["query"],
                    row["top_node"] or "",
                    row["top_tokens"],
                    row["ms_p50"],
                    row["ms_mean"],
                    row["ms_min"],
                    row["ms_max"],
                ]
            )
        w.writerow([])
        w.writerow(["metric", "value"])
        w.writerow(["files_indexed", summary["files_indexed"]])
        w.writerow(["nodes", summary["nodes"]])
        w.writerow(["edges", summary["edges"]])
        w.writerow(["graph_db_bytes", summary["graph_db_bytes"]])
        w.writerow(["bytes_per_node", summary["bytes_per_node"]])
        w.writerow(["bytes_per_edge", summary["bytes_per_edge"]])
        w.writerow(["search_ms_p50", summary["search_ms_p50"]])
        w.writerow(["search_ms_p95", summary["search_ms_p95"]])
        w.writerow(["search_ms_mean", summary["search_ms_mean"]])
        w.writerow(["search_ms_max", summary["search_ms_max"]])

    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
