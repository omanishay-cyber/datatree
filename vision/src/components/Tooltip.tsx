import { useEffect, useRef, useState } from "react";

export interface TooltipDetail {
  file: string;
  lines?: number;
  lastCommit?: { hash: string; author: string; ts: number; message: string };
  blastRadius?: number;
  type?: string;
}

interface TooltipProps {
  detail: TooltipDetail | null;
  position: { x: number; y: number } | null;
}

export function Tooltip({ detail, position }: TooltipProps): JSX.Element | null {
  const ref = useRef<HTMLDivElement | null>(null);
  const [adjusted, setAdjusted] = useState<{ x: number; y: number } | null>(null);

  useEffect(() => {
    if (!position || !ref.current) {
      setAdjusted(null);
      return;
    }
    const rect = ref.current.getBoundingClientRect();
    const w = window.innerWidth;
    const h = window.innerHeight;
    const x = Math.min(position.x + 12, w - rect.width - 8);
    const y = Math.min(position.y + 12, h - rect.height - 8);
    setAdjusted({ x, y });
  }, [position]);

  if (!detail || !position) return null;

  return (
    <div
      ref={ref}
      className="vz-tooltip"
      role="tooltip"
      style={{ left: adjusted?.x ?? position.x, top: adjusted?.y ?? position.y }}
    >
      <header>
        <strong>{detail.file}</strong>
        {detail.type && <span className="vz-tooltip-type">{detail.type}</span>}
      </header>
      <dl>
        {typeof detail.lines === "number" && (
          <>
            <dt>lines</dt>
            <dd>{detail.lines}</dd>
          </>
        )}
        {typeof detail.blastRadius === "number" && (
          <>
            <dt>blast radius</dt>
            <dd>{detail.blastRadius}</dd>
          </>
        )}
        {detail.lastCommit && (
          <>
            <dt>last commit</dt>
            <dd>
              <code>{detail.lastCommit.hash.slice(0, 7)}</code> by {detail.lastCommit.author}
              <br />
              <em>{detail.lastCommit.message}</em>
            </dd>
          </>
        )}
      </dl>
    </div>
  );
}
