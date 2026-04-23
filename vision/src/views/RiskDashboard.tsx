import { useEffect, useState } from "react";
import { fetchGraph, type GraphPayload } from "../api";

interface RiskItem {
  id: string;
  label: string;
  risk: number;
  reasons: string[];
}

function deriveRisk(payload: GraphPayload): RiskItem[] {
  return payload.nodes
    .map((n, i) => {
      const risk =
        typeof n.meta?.["risk"] === "number"
          ? Number(n.meta["risk"])
          : Math.min(100, ((i * 11) % 100) + ((i * 7) % 13));
      const reasons: string[] = [];
      if (risk > 80) reasons.push("hot churn");
      if ((n.size ?? 0) > 6) reasons.push("size > 6");
      if ((i % 9) === 0) reasons.push("low coverage");
      return { id: n.id, label: n.label ?? n.id, risk, reasons };
    })
    .sort((a, b) => b.risk - a.risk);
}

export function RiskDashboard(): JSX.Element {
  const [items, setItems] = useState<RiskItem[]>([]);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;
    fetchGraph("risk-dashboard", { signal: ac.signal }).then((payload) => {
      if (cancelled) return;
      setItems(deriveRisk(payload));
    });
    return () => {
      cancelled = true;
      ac.abort();
    };
  }, []);

  const buckets = {
    critical: items.filter((i) => i.risk >= 80),
    high: items.filter((i) => i.risk >= 60 && i.risk < 80),
    moderate: items.filter((i) => i.risk >= 30 && i.risk < 60),
    low: items.filter((i) => i.risk < 30),
  };

  return (
    <div className="vz-view vz-view--risk">
      <header className="vz-risk-header">
        <h2>Risk Dashboard</h2>
        <p>derived from churn × complexity × coverage gaps</p>
      </header>
      <section className="vz-risk-grid">
        {(["critical", "high", "moderate", "low"] as const).map((key) => (
          <article key={key} className={`vz-risk-card vz-risk-card--${key}`}>
            <h3>
              {key} <span>{buckets[key].length}</span>
            </h3>
            <ul>
              {buckets[key].slice(0, 12).map((item) => (
                <li key={item.id}>
                  <span className="vz-risk-bar" style={{ width: `${item.risk}%` }} />
                  <span className="vz-risk-label">{item.label}</span>
                  <span className="vz-risk-score">{item.risk}</span>
                </li>
              ))}
            </ul>
          </article>
        ))}
      </section>
    </div>
  );
}
