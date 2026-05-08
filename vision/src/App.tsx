import { Suspense, useEffect, useMemo, useRef, useState } from "react";
import { useVisionStore } from "./store";
import { VIEWS, getView, type ViewId } from "./views";
import { FilterBar } from "./components/FilterBar";
import { SidePanel } from "./components/SidePanel";
import { TimelineScrubber } from "./components/TimelineScrubber";
import { Minimap } from "./components/Minimap";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { CommandCenter } from "./command-center/CommandCenter";
import {
  fetchDaemonHealth,
  fetchStatus,
  type DaemonHealthPayload,
  type GraphStatsPayload,
} from "./api/graph";
import { fetchProjects, type ProjectSummary } from "./api/projects";

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

/**
 * Header dropdown for picking which indexed project to view. Reads the
 * project list from `/api/projects`, mirrors the selection into the
 * shared zustand store (which `projectSelection.ts` keeps in sync with
 * the URL + localStorage), and auto-selects the first project on first
 * load when no choice was persisted.
 */
function ProjectPicker({ daemonOk }: { daemonOk: boolean }): JSX.Element {
  const projectHash = useVisionStore((s) => s.projectHash);
  const setProjectHash = useVisionStore((s) => s.setProjectHash);
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  // HIGH-FE-6 fix (2026-05-05 audit): the previous version captured
  // `projectHash` synchronously inside the load() closure. The 30s
  // recurring `setTimeout(() => load(false), 30_000)` re-fired with
  // a STALE projectHash value from initial render. If the user
  // manually picked B after the initial auto-select happened, the
  // stale-closure 30s tick re-checked `if (!projectHash && ...)`
  // against the empty-on-mount value and overwrote B back to A.
  //
  // Track `projectHash` in a ref so load() reads the CURRENT value
  // each time it fires, not the captured-at-effect-mount value.
  const projectHashRef = useRef<string>(projectHash);
  useEffect(() => {
    projectHashRef.current = projectHash;
  }, [projectHash]);

  // A6-016: refresh on 30s cadence AND whenever the daemon health flips
  // from missing -> running. The previous []-deps fetch never re-ran,
  // so newly-built projects never appeared in the dropdown until the
  // user reloaded the page.
  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;
    let nextTimer: ReturnType<typeof setTimeout> | null = null;

    const load = async (initial: boolean): Promise<void> => {
      try {
        const r = await fetchProjects(ac.signal);
        if (cancelled) return;
        setProjects(r.projects);
        if (r.error) setError(r.error);
        else setError(null);
        // Auto-select the first project with a built shard when nothing
        // was picked yet — matches the legacy "show the only shard"
        // behaviour for single-project installs.
        //
        // HIGH-FE-6: read CURRENT projectHash via the ref so a 30s
        // recurring tick respects the user's manual selection rather
        // than overwriting from a stale closure capture.
        if (!projectHashRef.current && r.projects.length > 0) {
          const firstReady = r.projects.find((p) => p.has_graph_db) ?? r.projects[0];
          if (firstReady) setProjectHash(firstReady.hash);
        }
      } catch (err) {
        if ((err as Error).name === "AbortError") return;
        if (!cancelled && initial) setError(String(err));
        // On refresh ticks, swallow errors silently — we keep the last
        // successful list rendered rather than blanking the dropdown.
      } finally {
        if (!cancelled) {
          if (initial) setLoading(false);
          nextTimer = setTimeout(() => load(false), 30_000);
        }
      }
    };

    load(true);
    return () => {
      cancelled = true;
      ac.abort();
      if (nextTimer !== null) clearTimeout(nextTimer);
    };
    // Re-run when daemon transitions to running -- newly-built shards
    // become visible immediately. `projectHash` deliberately omitted to
    // avoid loops (we read it via projectHashRef instead).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [daemonOk]);

  const onChange = (e: React.ChangeEvent<HTMLSelectElement>): void => {
    setProjectHash(e.target.value);
  };

  if (loading) {
    return (
      <div className="vz-project-picker" data-state="loading" aria-label="project selector">
        <span className="vz-project-picker-label">project:</span>
        <span className="vz-project-picker-loading">loading…</span>
      </div>
    );
  }
  if (projects.length === 0) {
    return (
      <div className="vz-project-picker" data-state="empty" aria-label="project selector">
        <span className="vz-project-picker-label">project:</span>
        <span className="vz-project-picker-empty">
          no projects — run <code>mneme build</code>
          {error ? ` (${error})` : ""}
        </span>
      </div>
    );
  }
  return (
    <div className="vz-project-picker" data-state="ok" aria-label="project selector">
      <label htmlFor="vz-project-select" className="vz-project-picker-label">
        project:
      </label>
      <select
        id="vz-project-select"
        className="vz-project-picker-select"
        value={projectHash}
        onChange={onChange}
      >
        {projects.map((p) => (
          <option key={p.hash} value={p.hash} disabled={!p.has_graph_db}>
            {p.display_name}
            {p.has_graph_db ? ` (${p.indexed_files.toLocaleString()} files)` : " (no shard)"}
          </option>
        ))}
      </select>
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
      daemon missing — run <code>mneme daemon start</code>
    </div>
  );
}

export function App(): JSX.Element {
  const activeView = useVisionStore((s) => s.activeView);
  const setActiveView = useVisionStore((s) => s.setActiveView);
  // Re-fetch status/daemon health whenever the user picks a different
  // project so the counts in the status bar reflect the active shard.
  const projectHash = useVisionStore((s) => s.projectHash);

  const [status, setStatus] = useState<GraphStatsPayload | null>(null);
  const [daemon, setDaemon] = useState<DaemonHealthPayload | null>(null);

  // Tiny route handler — keeps deps minimal (no react-router for v1).
  // A6-017: route is reactive state so popstate can swap views without
  // a full page reload that would kill the Sigma canvas, in-flight
  // fetches and the WebSocket.
  const [route, setRoute] = useState<RouteState>(() => readRoute());

  useEffect(() => {
    const onPop = (): void => {
      setRoute(readRoute());
    };
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
  }, []);

  // Boot probes: status bar + daemon banner. Status refreshes every 30s
  // and re-runs immediately whenever the chosen project changes so the
  // visible counts always match the shard the views are reading.
  //
  // A6-009: single AbortController scope; the next tick is scheduled with
  // setTimeout AFTER the previous load() resolves so we can never have
  // two in-flight loads racing setState. StrictMode double-mount no
  // longer flickers because the second effect's load() shares the same
  // cancellation flag and AbortController as the first.
  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;
    let nextTimer: ReturnType<typeof setTimeout> | null = null;

    const load = async (): Promise<void> => {
      try {
        const [s, h] = await Promise.all([
          fetchStatus(ac.signal).catch(() => null),
          fetchDaemonHealth(ac.signal).catch(() => null),
        ]);
        if (cancelled) return;
        setStatus(s);
        setDaemon(h);
      } catch {
        /* aborted or upstream failure -- silent on refresh path */
      } finally {
        if (!cancelled) {
          nextTimer = setTimeout(load, 30_000);
        }
      }
    };

    load();
    return () => {
      cancelled = true;
      ac.abort();
      if (nextTimer !== null) clearTimeout(nextTimer);
    };
  }, [projectHash]);

  // CRIT-FE-1 fix (2026-05-05 audit): hooks MUST execute on every render in
  // the same order. Previously the early return for command-center (added
  // when the route was introduced) ran BEFORE the useMemo below, which made
  // the hook count differ between renders. The first navigation between /
  // and /command-center produced React's "Rendered fewer/more hooks than
  // during the previous render" error and crashed the entire SPA — and
  // ErrorBoundary cannot catch hook-order errors above its mount point.
  // Run all hooks first, then return conditionally.
  const grouped = useMemo(() => {
    // TS-5 fix (2026-05-05 audit): bind the bucket locally so we
    // don't need a non-null assertion on the next line. The previous
    // pattern wrote `if (!groups[key]) groups[key] = []; groups[key]!.push(v)`
    // — the `!` was technically necessary under noUncheckedIndexedAccess
    // (the type system can't prove the line above narrowed it) but
    // it's strictly cleaner to bind once and let the local var carry
    // the non-undefined type through the push.
    const groups: Record<string, typeof VIEWS> = {};
    for (const v of VIEWS) {
      const key = v.group;
      const bucket = groups[key] ?? (groups[key] = []);
      bucket.push(v);
    }
    return groups;
  }, []);

  if (route.route === "command-center") {
    return (
      <ErrorBoundary region="command-center">
        <CommandCenter />
      </ErrorBoundary>
    );
  }

  const ActiveView = getView(activeView).component;

  // LOW fix (2026-05-05 audit): debounce view-switch clicks. Each
  // view that uses WebGL (ForceGalaxy/Sigma, ProjectGalaxy3D/deck.gl,
  // Sunburst's canvas back-end) creates and tears down a WebGL
  // context on mount/unmount. Browsers cap concurrent WebGL contexts
  // (Chromium: 16, Firefox: 16) and rapid switching faster than
  // teardown finishes leaks contexts; eventually the next view fails
  // to acquire one and renders blank with the WARNING:Too many
  // active WebGL contexts message in DevTools.
  //
  // 150ms is below the JND for "feels responsive" on UI clicks but
  // long enough that triple-clicking through the nav doesn't stack
  // mounts. The ref tracks the last pick so we only honour the
  // latest in any 150ms burst — same pattern Linear / Notion use for
  // their chrome-tab switchers.
  //
  // UX-18 (2026-05-07 audit): the 150ms debounce was applied
  // unconditionally, including to keyboard navigation (Arrow / Home /
  // End). Keyboard users are deliberate (no triple-tap risk) and
  // expect immediate visual feedback as focus moves; the cumulative
  // 150ms-per-press lag made arrow-tabbing through 14 nav items feel
  // sluggish. Pointer activations still need the debounce to guard
  // WebGL context churn, so split the path: `immediate=true` (passed
  // by keyboard handlers) bypasses the timer and updates synchronously.
  const pickTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const onPickView = (id: ViewId, immediate: boolean = false): void => {
    if (pickTimerRef.current !== null) {
      clearTimeout(pickTimerRef.current);
      pickTimerRef.current = null;
    }
    if (immediate) {
      setActiveView(id);
      return;
    }
    pickTimerRef.current = setTimeout(() => {
      setActiveView(id);
      pickTimerRef.current = null;
    }, 150);
  };

  // UX-19 (2026-05-07 audit): sidebar nav was 14 plain <button>s with
  // no composite-widget keyboard semantics. Tab walked through every
  // item one-at-a-time and there was no way to jump groups. Convert
  // to a roving-tabindex menu pattern (per group: role="menu" +
  // role="menuitem"), so that:
  //   - Tab enters the nav once and Tab again leaves it (focus skips
  //     to the next focusable region, not through 14 buttons).
  //   - ArrowDown / ArrowUp move focus within the current group (wrap).
  //   - ArrowRight / ArrowLeft jump between groups (first item of the
  //     next/previous group), matching the audit fix sketch.
  //   - Home / End jump to the very first / very last nav item across
  //     all groups.
  //   - aria-current="page" marks the active view for assistive tech.
  // Activation (Enter / Space) is handled implicitly by <button>; we
  // route those through `onPickView(id, true)` so keyboard users
  // bypass the 150ms pointer debounce (UX-18 above).
  const navItemRefs = useRef<Map<ViewId, HTMLButtonElement>>(new Map());
  const flatNavOrder = useMemo<ViewId[]>(
    () => Object.values(grouped).flat().map((v) => v.id),
    [grouped],
  );
  const groupOrder = useMemo<ViewId[][]>(
    () => Object.values(grouped).map((items) => items.map((v) => v.id)),
    [grouped],
  );

  const focusNavItem = (id: ViewId): void => {
    const el = navItemRefs.current.get(id);
    if (el) el.focus();
  };

  const onNavKeyDown = (
    event: React.KeyboardEvent<HTMLButtonElement>,
    currentId: ViewId,
  ): void => {
    const groupIndex = groupOrder.findIndex((g) => g.includes(currentId));
    if (groupIndex === -1) return;
    const group = groupOrder[groupIndex];
    if (!group) return;
    const itemIndex = group.indexOf(currentId);

    switch (event.key) {
      case "ArrowDown": {
        event.preventDefault();
        const next = group[(itemIndex + 1) % group.length];
        if (next) focusNavItem(next);
        break;
      }
      case "ArrowUp": {
        event.preventDefault();
        const prev = group[(itemIndex - 1 + group.length) % group.length];
        if (prev) focusNavItem(prev);
        break;
      }
      case "ArrowRight": {
        event.preventDefault();
        const nextGroup = groupOrder[(groupIndex + 1) % groupOrder.length];
        if (nextGroup && nextGroup[0]) focusNavItem(nextGroup[0]);
        break;
      }
      case "ArrowLeft": {
        event.preventDefault();
        const prevGroup =
          groupOrder[(groupIndex - 1 + groupOrder.length) % groupOrder.length];
        if (prevGroup && prevGroup[0]) focusNavItem(prevGroup[0]);
        break;
      }
      case "Home": {
        event.preventDefault();
        const first = flatNavOrder[0];
        if (first) focusNavItem(first);
        break;
      }
      case "End": {
        event.preventDefault();
        const last = flatNavOrder[flatNavOrder.length - 1];
        if (last) focusNavItem(last);
        break;
      }
      default:
        break;
    }
  };

  // Roving tabindex: exactly one nav item is in the tab order at a
  // time. Prefer the active view; fall back to the first item so the
  // group is reachable even when the active view lives elsewhere
  // (e.g. command-center returning).
  const rovingTabId: ViewId | undefined =
    flatNavOrder.find((id) => id === activeView) ?? flatNavOrder[0];

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
            {/* UX-19 (2026-05-07 audit): role="menu" + role="menuitem"
                with roving tabindex turns this <ul> into a composite
                widget. Arrow keys navigate within/between groups,
                Home/End jump to first/last, Tab leaves the nav. */}
            <ul className="vz-nav-list" role="menu" aria-label={`${group} views`}>
              {items.map((v) => {
                const isActive = v.id === activeView;
                const inTabOrder = v.id === rovingTabId;
                return (
                  <li key={v.id} role="none">
                    <button
                      type="button"
                      role="menuitem"
                      ref={(el) => {
                        if (el) navItemRefs.current.set(v.id, el);
                        else navItemRefs.current.delete(v.id);
                      }}
                      className={`vz-nav-item ${isActive ? "is-active" : ""}`}
                      // UX-18: keyboard activation bypasses the 150ms
                      // pointer debounce — Enter/Space fire immediately.
                      onClick={(e) =>
                        onPickView(v.id, e.detail === 0 /* keyboard-synth click */)
                      }
                      onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          onPickView(v.id, true);
                          return;
                        }
                        onNavKeyDown(e, v.id);
                      }}
                      tabIndex={inTabOrder ? 0 : -1}
                      aria-current={isActive ? "page" : undefined}
                      title={v.description}
                    >
                      {v.label}
                    </button>
                  </li>
                );
              })}
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
        <ProjectPicker daemonOk={Boolean(daemon?.ok)} />
        <StatusBar status={status} />
        <DaemonBanner health={daemon} />
        <FilterBar />
      </header>

      <main className="vz-canvas" role="main">
        <ErrorBoundary region={`view:${activeView}`}>
          <Suspense fallback={<div className="vz-loading">loading view…</div>}>
            <ActiveView key={`${activeView}:${projectHash}`} />
          </Suspense>
        </ErrorBoundary>
        <ErrorBoundary region="minimap">
          <Minimap />
        </ErrorBoundary>
      </main>

      <aside className="vz-detail" aria-label="Selection detail">
        <ErrorBoundary region="side-panel">
          <SidePanel />
        </ErrorBoundary>
      </aside>

      <footer className="vz-timeline">
        <ErrorBoundary region="timeline">
          <TimelineScrubber />
        </ErrorBoundary>
      </footer>
    </div>
  );
}
