import { Suspense, useEffect, useMemo, useState } from "react";
import { useVisionStore } from "./store";
import { VIEWS, getView, type ViewId } from "./views";
import { FilterBar } from "./components/FilterBar";
import { SidePanel } from "./components/SidePanel";
import { TimelineScrubber } from "./components/TimelineScrubber";
import { Minimap } from "./components/Minimap";
import { CommandCenter } from "./command-center/CommandCenter";
import {
  fetchDaemonHealth,
  fetchStatus,
  type DaemonHealthPayload,
  type GraphStatsPayload,
} from "./api/graph";

function NavGroupHeader({ label }: { label: string }): JSX.Element {
  return (
    <div className="vz-nav-group">
      <span className="vz-nav-group-label">{label}</span>
    </div>
  );
}

interface RouteState {
  route: "view" | "command-center";
}

function readRoute(): RouteState {
  if (typeof window === "undefined") return { route: "view" };
  return window.location.pathname.startsWith("/command-center")
    ? { route: "command-center" }
    : { route: "view" };
}

function formatRelative(iso: string | null): string {
  if (!iso) return "never";
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return iso;
  const secs = Math.max(0, Math.floor((Date.now() - t) / 1000));
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h ago`;
  return `${Math.floor(secs / 86400)}d ago`;
}

/** Compact status bar rendered inside the topbar. */
function StatusBar({ status }: { status: GraphStatsPayload | null }): JSX.Element {
  if (!status) {
    return (
      <div className="vz-statusbar" aria-label="project status" data-state="loading">
        <span className="vz-statusbar-project">loading shard…</span>
      </div>
    );
  }
  if (!status.ok) {
    return (
      <div className="vz-statusbar" data-state="missing" aria-label="project status">
        <span className="vz-statusbar-project">shard missing</span>
        <span className="vz-statusbar-sep">·</span>
        <span className="vz-statusbar-hint">
          run <code>mneme build .</code>
        </span>
      </div>
    );
  }
  return (
    <div className="vz-statusbar" data-state="ok" aria-label="project status">
      <span className="vz-statusbar-project">{status.project ?? "unknown"}</span>
      <span className="vz-statusbar-sep">·</span>
      <span className="vz-statusbar-metric">{status.nodes.toLocaleString()} nodes</span>
      <span className="vz-statusbar-sep">·</span>
      <span className="vz-statusbar-metric">{status.edges.toLocaleString()} edges</span>
      <span className="vz-statusbar-sep">·</span>
      <span className="vz-statusbar-metric">{status.files.toLocaleString()} files</span>
      <span className="vz-statusbar-sep">·</span>
      <span className="vz-statusbar-metric">indexed {formatRelative(status.lastIndexAt)}</span>
    </div>
  );
}

function DaemonBanner({ health }: { health: DaemonHealthPayload | null }): JSX.Element | null {
  if (!health) return null;
  if (health.ok) {
    return (
      <div className="vz-daemon-banner" data-state="running" role="status">
        <span className="vz-daemon-dot" aria-hidden="true" />
        daemon running
      </div>
    );
  }
  return (
    <div className="vz-daemon-banner" data-state="missing" role="alert">
      <span className="vz-daemon-dot" aria-hidden="true" />
      daemon missing — run <code>mneme-daemon start</code>
    </div>
  );
}

export function App(): JSX.Element {
  const activeView = useVisionStore((s) => s.activeView);
  const setActiveView = useVisionStore((s) => s.setActiveView);

  const [status, setStatus] = useState<GraphStatsPayload | null>(null);
  const [daemon, setDaemon] = useState<DaemonHealthPayload | null>(null);

  // Tiny route handler — keeps deps minimal (no react-router for v1).
  const route = useMemo(readRoute, []);

  useEffect(() => {
    const onPop = (): void => {
      window.location.reload();
    };
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
  }, []);

  // Boot probes: status bar + daemon banner. Status refreshes every 30s.
  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;

    const load = async (): Promise<void> => {
      try {
        const [s, h] = await Promise.all([
          fetchStatus(ac.signal).catch(() => null),
          fetchDaemonHealth(ac.signal).catch(() => null),
        ]);
        if (!cancelled) {
          setStatus(s);
          setDaemon(h);
        }
      } catch {
        /* aborted */
      }
    };

    load();
    const timer = setInterval(load, 30_000);
    return () => {
      cancelled = true;
      ac.abort();
      clearInterval(timer);
    };
  }, []);

  if (route.route === "command-center") {
    return <CommandCenter />;
  }

  const grouped = useMemo(() => {
    const groups: Record<string, typeof VIEWS> = {};
    for (const v of VIEWS) {
      const key = v.group;
      if (!groups[key]) groups[key] = [];
      groups[key]!.push(v);
    }
    return groups;
  }, []);

  const ActiveView = getView(activeView).component;

  const onPickView = (id: ViewId): void => {
    setActiveView(id);
  };

  return (
    <div className="vz-app">
      <aside className="vz-nav" aria-label="View navigation">
        <header className="vz-nav-header">
          <span className="vz-brand-mark" aria-hidden="true" />
          <span className="vz-brand-text">mneme · vision</span>
        </header>
        {Object.entries(grouped).map(([group, items]) => (
          <div key={group}>
            <NavGroupHeader label={group} />
            <ul className="vz-nav-list">
              {items.map((v) => (
                <li key={v.id}>
                  <button
                    type="button"
                    className={`vz-nav-item ${v.id === activeView ? "is-active" : ""}`}
                    onClick={() => onPickView(v.id)}
                    title={v.description}
                  >
                    {v.label}
                  </button>
                </li>
              ))}
            </ul>
          </div>
        ))}
        <div className="vz-nav-footer">
          <a className="vz-nav-link" href="/command-center">
            Command Center →
          </a>
        </div>
      </aside>

      <header className="vz-topbar">
        <StatusBar status={status} />
        <DaemonBanner health={daemon} />
        <FilterBar />
      </header>

      <main className="vz-canvas" role="main">
        <Suspense fallback={<div className="vz-loading">loading view…</div>}>
          <ActiveView />
        </Suspense>
        <Minimap />
      </main>

      <aside className="vz-detail" aria-label="Selection detail">
        <SidePanel />
      </aside>

      <footer className="vz-timeline">
        <TimelineScrubber />
      </footer>
    </div>
  );
}
