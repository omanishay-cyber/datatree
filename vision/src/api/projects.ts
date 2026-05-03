// vision/src/api/projects.ts
//
// Client-side fetcher for the daemon's `/api/projects` endpoint. The
// supervisor enumerates every shard under `<MNEME_HOME>/projects/<id>/`
// and returns one entry per project with summary stats. The Vision SPA
// uses this list to populate the header dropdown so the user can pick
// which shard to view (see `stores/project.ts` for the selection store).
//
// Wire shape mirrors the Rust `DiscoveredProject` struct in
// `supervisor/src/api_graph.rs::api_projects`. The legacy Bun dev server
// returns the same envelope so a `bun run serve` shell works identically.

import { API_BASE } from "../api";

/** One discovered project shard, augmented with summary stats. */
export interface ProjectSummary {
  /** Hex SHA-256 of the canonical project root path. */
  hash: string;
  /** Human-readable project name (from `meta.db::projects.name`,
   *  falling back to the last segment of the canonical path). */
  display_name: string;
  /** Absolute filesystem path of the project root, when known. */
  canonical_path: string | null;
  /** Count of `files` rows in `graph.db`, or 0 when the shard is missing. */
  indexed_files: number;
  /** Count of `nodes` rows in `graph.db`. */
  nodes: number;
  /** Count of `edges` rows in `graph.db`. */
  edges: number;
  /** ISO-8601 timestamp of the last build, or null when never built. */
  last_indexed_at: string | null;
  /** True when `graph.db` exists in the shard directory. */
  has_graph_db: boolean;
}

/** Response envelope from `GET /api/projects`. */
export interface ProjectsResponse {
  projects: ProjectSummary[];
  /** Path that was scanned, for diagnostics. */
  projects_root: string;
  /** Optional error string when the daemon couldn't enumerate. */
  error?: string;
}

/**
 * Fetch the list of indexed projects from the daemon.
 *
 * Returns an empty list (with the error message attached) when the
 * daemon is unreachable so the dropdown still renders an "empty" state
 * rather than crashing the whole SPA.
 */
export async function fetchProjects(signal?: AbortSignal): Promise<ProjectsResponse> {
  const url = API_BASE + "/api/projects";
  try {
    const res = await fetch(url, { signal });
    if (!res.ok) {
      return { projects: [], projects_root: "", error: `HTTP ${res.status}` };
    }
    const json = (await res.json()) as ProjectsResponse;
    // Defensive: backfill missing fields so callers can rely on the shape.
    return {
      projects: Array.isArray(json.projects) ? json.projects : [],
      projects_root: json.projects_root ?? "",
    };
  } catch (err) {
    if ((err as Error).name === "AbortError") throw err;
    return { projects: [], projects_root: "", error: String(err) };
  }
}
