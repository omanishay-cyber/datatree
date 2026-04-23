import { useEffect, useState } from "react";
import { fetchGraph } from "../api";

interface Swatch {
  name: string;
  value: string;
  contrast?: number;
  wcag: "AAA" | "AA" | "AA Large" | "fail";
}

function relativeLuminance(hex: string): number {
  const m = /^#?([\da-f]{6})$/i.exec(hex);
  if (!m) return 0;
  const num = parseInt(m[1] ?? "000000", 16);
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

export function ThemePalette(): JSX.Element {
  const [swatches, setSwatches] = useState<Swatch[]>([]);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;
    fetchGraph("theme-palette", { signal: ac.signal }).then((payload) => {
      if (cancelled) return;
      // Synthesise swatches from node hex colors when the daemon doesn't supply tokens.
      const tokens: Swatch[] = payload.nodes.map((n, i) => {
        const value =
          (n.meta?.["value"] as string | undefined) ??
          n.color ??
          `#${(((i + 1) * 1234567) & 0xffffff).toString(16).padStart(6, "0")}`;
        const ratio = contrastRatio(value, "#0a0e18");
        return {
          name: (n.label ?? n.id) as string,
          value,
          contrast: Math.round(ratio * 10) / 10,
          wcag: wcagLabel(ratio),
        };
      });
      setSwatches(tokens);
    });
    return () => {
      cancelled = true;
      ac.abort();
    };
  }, []);

  return (
    <div className="vz-view vz-view--palette">
      <div className="vz-palette-grid">
        {swatches.map((s) => (
          <article key={`${s.name}-${s.value}`} className="vz-swatch">
            <div className="vz-swatch-chip" style={{ background: s.value }} />
            <div className="vz-swatch-meta">
              <strong>{s.name}</strong>
              <span className="vz-swatch-hex">{s.value}</span>
              <span className={`vz-badge vz-badge--${s.wcag.toLowerCase().replace(/\s+/g, "-")}`}>
                {s.wcag} - {s.contrast}:1
              </span>
            </div>
          </article>
        ))}
      </div>
    </div>
  );
}
