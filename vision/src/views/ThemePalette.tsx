import { useEffect, useState } from "react";
import { fetchThemeSwatches, type ThemeSwatchRow } from "../api/graph";

interface Swatch {
  key: string;
  name: string;
  value: string;
  file: string;
  line: number;
  severity: string;
  used_count: number;
  contrast?: number;
  wcag: "AAA" | "AA" | "AA Large" | "fail" | "n/a";
  solid: boolean;
}

type Status = "loading" | "empty" | "ready" | "error";

function relativeLuminance(hex: string): number {
  const m = /^#?([\da-f]{3,8})\b/i.exec(hex);
  if (!m) return 0;
  let h = m[1] ?? "000000";
  if (h.length === 3) {
    h = h
      .split("")
      .map((c) => c + c)
      .join("");
  }
  if (h.length === 8) h = h.slice(0, 6);
  const num = Number.parseInt(h, 16);
  const r = (num >> 16) & 0xff;
  const g = (num >> 8) & 0xff;
  const b = num & 0xff;
  const channel = (c: number): number => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

function contrastRatio(fg: string, bg: string): number {
  const a = relativeLuminance(fg);
  const b = relativeLuminance(bg);
  const lighter = Math.max(a, b);
  const darker = Math.min(a, b);
  return (lighter + 0.05) / (darker + 0.05);
}

function wcagLabel(ratio: number): Swatch["wcag"] {
  if (ratio >= 7) return "AAA";
  if (ratio >= 4.5) return "AA";
  if (ratio >= 3) return "AA Large";
  return "fail";
}

function toSwatch(row: ThemeSwatchRow): Swatch {
  const solid = /^#[0-9a-fA-F]{3,8}$/.test(row.value);
  const contrast = solid ? contrastRatio(row.value, "#0a0e18") : 0;
  return {
    key: `${row.file}:${row.line}:${row.value}`,
    name: row.declaration,
    value: row.value,
    file: row.file,
    line: row.line,
    severity: row.severity,
    used_count: row.used_count,
    contrast: solid ? Math.round(contrast * 10) / 10 : undefined,
    wcag: solid ? wcagLabel(contrast) : "n/a",
    solid,
  };
}

export function ThemePalette(): JSX.Element {
  const [swatches, setSwatches] = useState<Swatch[]>([]);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;
    (async (): Promise<void> => {
      try {
        const res = await fetchThemeSwatches(ac.signal, 2000);
        if (cancelled) return;
        if (res.error) {
          setError(res.error);
          setStatus("error");
          return;
        }
        if (res.swatches.length === 0) {
          setStatus("empty");
          return;
        }
        setSwatches(res.swatches.map(toSwatch));
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
    <div className="vz-view vz-view--palette">
      {status === "loading" && (
        <div className="vz-view-hint" role="status">
          loading findings.db theme scanner output...
        </div>
      )}
      {status === "empty" && (
        <div className="vz-view-error" role="status">
          no theme findings -- either the theme scanner hasn't run, or the project is clean.
          Try <code>mneme audit .</code>
        </div>
      )}
      {status === "error" && error && (
        <div className="vz-view-error" role="alert">
          palette error: {error}
        </div>
      )}
      {status === "ready" && (
        <>
          <div className="vz-palette-grid">
            {swatches.slice(0, 400).map((s) => (
              <article key={s.key} className="vz-swatch">
                <div
                  className="vz-swatch-chip"
                  style={{ background: s.solid ? s.value : "#1c2433" }}
                >
                  {!s.solid && (
                    <span style={{ color: "#cdd6e4", fontSize: 10, padding: 4 }}>
                      {s.value}
                    </span>
                  )}
                </div>
                <div className="vz-swatch-meta">
                  <strong>{s.name}</strong>
                  <span className="vz-swatch-hex">{s.value}</span>
                  <span
                    className={`vz-badge vz-badge--${s.wcag
                      .toLowerCase()
                      .replace(/\s+/g, "-")
                      .replace("/", "")}`}
                  >
                    {s.wcag === "n/a" ? "n/a" : `${s.wcag} - ${s.contrast}:1`}
                  </span>
                  <span style={{ fontSize: 10, color: "#7a8aa6" }}>
                    {s.file}:{s.line} - used {s.used_count}x
                  </span>
                </div>
              </article>
            ))}
          </div>
          <p className="vz-view-hint">
            {swatches.length.toLocaleString()} theme findings - WCAG vs #0a0e18
          </p>
        </>
      )}
    </div>
  );
}
