interface DriftIndicatorProps {
  score: number; // 0 = on-track, 100 = severely drifting
}

function bucket(score: number): { label: string; tone: "ok" | "warn" | "crit" } {
  if (score < 25) return { label: "on-track", tone: "ok" };
  if (score < 60) return { label: "drifting", tone: "warn" };
  return { label: "off-track", tone: "crit" };
}

export function DriftIndicator({ score }: DriftIndicatorProps): JSX.Element {
  const { label, tone } = bucket(score);
  const clamped = Math.max(0, Math.min(100, score));
  return (
    <div className={`vz-drift vz-drift--${tone}`} role="status" aria-live="polite">
      <span className="vz-drift-dot" aria-hidden="true" />
      <span className="vz-drift-label">{label}</span>
      <span className="vz-drift-score" aria-label={`drift score ${clamped} of 100`}>
        {clamped}
      </span>
      <span className="vz-drift-bar">
        <span className="vz-drift-fill" style={{ width: `${clamped}%` }} />
      </span>
    </div>
  );
}
