import { useEffect, useMemo, useState } from "react";
import { fetchFindings, type ShardFindingRow } from "../api/graph";

type Bucket = "critical" | "high" | "medium" | "low";
type Status = "loading" | "empty" | "ready" | "error";

const BUCKETS: Bucket[] = ["critical", "high", "medium", "low"];

function severityToBucket(sev: string): Bucket {
  if (sev === "critical") return "critical";
  if (sev === "high") return "high";
  if (sev === "medium") return "medium";
  return "low";
}

function severityToScore(sev: string): number {
  switch (sev) {
    case "critical":
      return 95;
    case "high":
      return 75;
    case "medium":
      return 45;
    case "low":
      return 20;
    default:
      return 10;
  }
}

function bucketClass(b: Bucket): string {
  // CSS expects critical / high / moderate / low — map medium -> moderate
  // so we can reuse the existing stylesheet without churn.
  return b === "medium" ? "moderate" : b;
}

export function RiskDashboard(): JSX.Element {
  const [findings, setFindings] = useState<ShardFindingRow[]>([]);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;

    (async (): Promise<void> => {
      try {
        const res = await fetchFindings(ac.signal, 2000);
        if (cancelled) return;
        if (res.error) {
          setError(res.error);
          setStatus("error");
          return;
        }
        setFindings(res.findings);
        setStatus(res.findings.length === 0 ? "empty" : "ready");
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

  const buckets = useMemo(() => {
    const grouped: Record<Bucket, ShardFindingRow[]> = {
      critical: [],
      high: [],
      medium: [],
      low: [],
    };
    for (const f of findings) {
      grouped[severityToBucket(f.severity)].push(f);
    }
    return grouped;
  }, [findings]);

  const totals = useMemo(() => {
    const byScanner: Record<string, number> = {};
    for (const f of findings) {
      byScanner[f.scanner] = (byScanner[f.scanner] ?? 0) + 1;
    }
    return { total: findings.length, byScanner };
  }, [findings]);

  return (
    <div className="vz-view vz-view--risk">
      <header className="vz-risk-header">
        <h2>Risk Dashboard</h2>
        <p>
          {status === "ready"
            ? `${totals.total.toLocaleString()} open findings from findings.db · ${
                Object.keys(totals.byScanner).length
              } scanners`
            : status === "loading"
              ? "loading findings.db…"
              : status === "empty"
                ? "no open findings — shard clean, or not yet scanned"
                : error
                  ? `findings error: ${error}`
                  : ""}
        </p>
      </header>
      {status === "loading" && (
        <section className="vz-risk-grid" aria-busy="true">
          {BUCKETS.map((b) => (
            <article
              key={b}
              className={`vz-risk-card vz-risk-card--${bucketClass(b)}`}
              style={{ opacity: 0.45 }}
            >
              <h3>
                {b} <span>…</span>
              </h3>
            </article>
          ))}
        </section>
      )}
      {status !== "loading" && (
        <section className="vz-risk-grid">
          {BUCKETS.map((b) => {
            const items = buckets[b];
            return (
              <article key={b} className={`vz-risk-card vz-risk-card--${bucketClass(b)}`}>
                <h3>
                  {b} <span>{items.length}</span>
                </h3>
                <ul>
                  {items.slice(0, 12).map((item) => {
                    const score = severityToScore(item.severity);
                    return (
                      <li key={item.id}>
                        <span className="vz-risk-bar" style={{ width: `${score}%` }} />
                        <span className="vz-risk-label" title={item.message}>
                          {item.file}:{item.line_start} · {item.rule_id}
                        </span>
                        <span className="vz-risk-score">{score}</span>
                      </li>
                    );
                  })}
                </ul>
              </article>
            );
          })}
        </section>
      )}
    </div>
  );
}
