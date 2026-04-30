import { useVisionStore } from "../store";

const TYPE_FACETS = ["module", "page", "store", "util", "test", "asset"];
const DOMAIN_FACETS = ["src", "electron", "tests", "docs"];

export function FilterBar(): JSX.Element {
  const filters = useVisionStore((s) => s.filters);
  const setFilter = useVisionStore((s) => s.setFilter);

  const toggle = (key: "type" | "domain", value: string): void => {
    const current = filters[key];
    const next = current.includes(value)
      ? current.filter((v) => v !== value)
      : [...current, value];
    setFilter(key, next);
  };

  return (
    <div className="vz-filterbar">
      <label className="vz-filterbar-search">
        <span className="vz-sr-only">search</span>
        <input
          type="search"
          placeholder="search nodes…"
          value={filters.search}
          onChange={(e) => setFilter("search", e.target.value)}
        />
      </label>

      <div className="vz-facets">
        <span className="vz-facets-label">type</span>
        {TYPE_FACETS.map((t) => (
          <button
            type="button"
            key={t}
            className={`vz-chip ${filters.type.includes(t) ? "is-active" : ""}`}
            onClick={() => toggle("type", t)}
          >
            {t}
          </button>
        ))}
      </div>

      <div className="vz-facets">
        <span className="vz-facets-label">domain</span>
        {DOMAIN_FACETS.map((d) => (
          <button
            type="button"
            key={d}
            className={`vz-chip ${filters.domain.includes(d) ? "is-active" : ""}`}
            onClick={() => toggle("domain", d)}
          >
            {d}
          </button>
        ))}
      </div>

      <label className="vz-risk-slider">
        <span>risk ≥ {filters.riskMin}</span>
        <input
          type="range"
          min={0}
          max={100}
          step={1}
          value={filters.riskMin}
          onChange={(e) => setFilter("riskMin", Number(e.target.value))}
        />
      </label>
    </div>
  );
}
