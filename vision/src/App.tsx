import { Suspense, useEffect, useMemo } from "react";
import { useVisionStore } from "./store";
import { VIEWS, getView, type ViewId } from "./views";
import { FilterBar } from "./components/FilterBar";
import { SidePanel } from "./components/SidePanel";
import { TimelineScrubber } from "./components/TimelineScrubber";
import { Minimap } from "./components/Minimap";
import { CommandCenter } from "./command-center/CommandCenter";

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

export function App(): JSX.Element {
  const activeView = useVisionStore((s) => s.activeView);
  const setActiveView = useVisionStore((s) => s.setActiveView);

  // Tiny route handler — keeps deps minimal (no react-router for v1).
  const route = useMemo(readRoute, []);

  useEffect(() => {
    const onPop = (): void => {
      window.location.reload();
    };
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
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
          <span className="vz-brand-text">datatree · vision</span>
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
