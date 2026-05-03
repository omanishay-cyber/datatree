// vision/src/api/graph.ts
//
// Client-side shard API for the Vision app.
//
// These helpers hit the daemon's /api/graph/* endpoints. The shipped
// supervisor in v0.3.2 returns one of two wire shapes per route:
//
//   * Bare arrays for the row-style endpoints — `[node, ...]` instead
//     of the documented `{nodes:[...]}` envelope. This affects
//     /api/graph/{nodes, edges, files, findings, commits, test-coverage,
//     theme-palette}.
//   * Bare objects for the tree-style endpoints — `{name,children,...}`
//     instead of `{tree:{...}}`. This affects /api/graph/{file-tree,
//     hierarchy}.
//   * snake_case field names on /api/graph/status — `shard_root`,
//     `last_index_at`, `by_kind` instead of camelCase, plus no `ok`
//     field, plus no top-level `project` value.
//
// The view components all expect the documented envelope shape, so
// reading `res.nodes.length` on a bare array crashes with "Cannot read
// properties of undefined (reading 'length')". The fix is to normalize
// every response client-side at the fetch boundary so views see a
// consistent contract — same defensive pattern projects.ts uses for
// the {id,path,has_graph_db} vs {hash,display_name,...} mismatch.
//
// Each fetch helper below:
//   1. Hits the same URL it always did.
//   2. Catches network/parse errors → returns an empty envelope with
//      the error string attached so views render an "empty/error" state
//      instead of crashing.
//   3. Accepts BOTH the envelope shape AND the bare-shape, normalizing
//      to the envelope so views always read `res.nodes`, `res.tree`,
//      etc., uniformly.

import type { GraphNode, GraphEdge } from "../api";
import { API_BASE } from "../api";
import { withProject } from "../projectSelection";

/* -------------------------------------------------------------------------- */
/*  Shared types                                                               */
/* -------------------------------------------------------------------------- */

export interface ShardFileRow {
  path: string;
  language: string | null;
  line_count: number | null;
  byte_count: number | null;
  last_parsed_at: string | null;
}

export interface ShardFindingRow {
  id: number;
  rule_id: string;
  scanner: string;
  severity: "critical" | "high" | "medium" | "low" | string;
  file: string;
  line_start: number;
  line_end: number;
  message: string;
  suggestion: string | null;
  created_at: string;
}

export interface GraphStatsPayload {
  ok: boolean;
  project: string | null;
  shardRoot: string | null;
  nodes: number;
  edges: number;
  files: number;
  byKind: Record<string, number>;
  lastIndexAt: string | null;
  error?: string;
}

export interface DaemonHealthPayload {
  ok: boolean;
  status: "running" | "missing" | "error";
  url: string;
  detail?: string;
  error?: string;
}

export interface NodesResponse {
  nodes: GraphNode[];
  error?: string;
}
export interface EdgesResponse {
  edges: GraphEdge[];
  error?: string;
}
export interface FilesResponse {
  files: ShardFileRow[];
  error?: string;
}
export interface FindingsResponse {
  findings: ShardFindingRow[];
  error?: string;
}

/* -------------------------------------------------------------------------- */
/*  Shapes for the newly-wired views                                           */
/* -------------------------------------------------------------------------- */

export interface FileTreeNode {
  name: string;
  value?: number;
  language?: string | null;
  children?: FileTreeNode[];
}
export interface FileTreeResponse {
  tree: FileTreeNode;
  error?: string;
}

export interface KindFlowNode {
  id: string;
  kind: string;
  side: string;
}
export interface KindFlowLink {
  source: string;
  target: string;
  value: number;
  edgeKind: string;
}
export interface KindFlowPayload {
  nodes: KindFlowNode[];
  links: KindFlowLink[];
}
export interface KindFlowResponse extends KindFlowPayload {
  error?: string;
}

export interface DomainFlowNode {
  id: string;
  domain: string;
}
export interface DomainFlowLink {
  source: string;
  target: string;
  value: number;
}
export interface DomainFlowPayload {
  nodes: DomainFlowNode[];
  links: DomainFlowLink[];
}
export interface DomainFlowResponse extends DomainFlowPayload {
  error?: string;
}

export interface CommunityInfo {
  id: number;
  name: string;
  size: number;
  language: string | null;
}
export interface CommunityMatrixPayload {
  communities: CommunityInfo[];
  matrix: number[][];
}
export interface CommunityMatrixResponse extends CommunityMatrixPayload {
  error?: string;
}

export interface CommitRow {
  sha: string;
  author: string | null;
  date: string;
  message: string;
  files_changed: number;
  insertions: number;
  deletions: number;
}
export interface CommitsResponse {
  commits: CommitRow[];
  error?: string;
}

export interface HeatmapFileRow {
  file: string;
  language: string | null;
  line_count: number;
  complexity: number;
  severities: { critical: number; high: number; medium: number; low: number };
}
export interface HeatmapPayload {
  severities: string[];
  files: HeatmapFileRow[];
}
export interface HeatmapResponse extends HeatmapPayload {
  error?: string;
}

export interface LayerTierEntry {
  file: string;
  language: string | null;
  line_count: number;
  tier: string;
  domain: string;
}
export interface LayerTierPayload {
  tiers: string[];
  entries: LayerTierEntry[];
}
export interface LayerTierResponse extends LayerTierPayload {
  error?: string;
}

export interface Galaxy3DNode {
  id: string;
  label: string;
  kind: string;
  file_path: string | null;
  degree: number;
  community_id: number | null;
}
export interface Galaxy3DEdge {
  source: string;
  target: string;
  kind: string;
}
export interface Galaxy3DPayload {
  nodes: Galaxy3DNode[];
  edges: Galaxy3DEdge[];
}
export interface Galaxy3DResponse extends Galaxy3DPayload {
  error?: string;
}

export interface TestCoverageRow {
  file: string;
  language: string | null;
  line_count: number;
  test_file: string | null;
  test_count: number;
  covered: boolean;
}
export interface TestCoverageResponse {
  rows: TestCoverageRow[];
  error?: string;
}

export interface ThemeSwatchRow {
  file: string;
  line: number;
  declaration: string;
  value: string;
  severity: string;
  message: string;
  used_count: number;
}
export interface ThemeSwatchResponse {
  swatches: ThemeSwatchRow[];
  error?: string;
}

export interface HierarchyNode {
  name: string;
  kind?: string;
  file_path?: string | null;
  children?: HierarchyNode[];
}
export interface HierarchyResponse {
  tree: HierarchyNode;
  error?: string;
}

/* -------------------------------------------------------------------------- */
/*  Shape-normalisation helpers                                                */
/* -------------------------------------------------------------------------- */
//
// The shipped daemon and the documented client wire shapes have
// drifted. Rather than force a daemon rebuild + zip re-upload every
// time we adjust one route, we accept BOTH shapes here and project to
// the one views consume. Each helper is a single responsibility:
//
//   * `asArray(v, key)` — return `v[key]` if v is an envelope, else `v`
//     itself if it's already an array, else `[]`.
//   * `asObject(v, key, fallback)` — return `v[key]` if v is an
//     envelope carrying that key, else `v` itself if it looks like the
//     bare-object shape, else `fallback`.
//
// Both helpers also look for an `error` string at either level so the
// views' `if (res.error) ...` paths still light up when the daemon
// reports a failure inline.

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return v !== null && typeof v === "object" && !Array.isArray(v);
}

function pickError(...candidates: unknown[]): string | undefined {
  for (const c of candidates) {
    if (typeof c === "string" && c.length > 0) return c;
    if (isPlainObject(c) && typeof c.error === "string" && c.error.length > 0) {
      return c.error;
    }
  }
  return undefined;
}

/** Normalize any "row-style" payload to `{ <key>: T[], error? }`. */
function asArrayEnvelope<T>(raw: unknown, key: string): { items: T[]; error?: string } {
  // Already an array — server returned bare rows.
  if (Array.isArray(raw)) {
    return { items: raw as T[] };
  }
  // Envelope shape — pull the named field, but only if it's an array.
  if (isPlainObject(raw)) {
    const inner = raw[key];
    const items = Array.isArray(inner) ? (inner as T[]) : [];
    const err = pickError(raw);
    return err ? { items, error: err } : { items };
  }
  // null / undefined / scalar — degrade to empty.
  return { items: [] };
}

/** Normalize any "tree-style" payload to `{ tree: T, error? }`. */
function asTreeEnvelope<T extends { name?: string; children?: unknown[] }>(
  raw: unknown,
  fallback: () => T,
): { tree: T; error?: string } {
  if (isPlainObject(raw)) {
    // Envelope: { tree: {...}, error? }
    if (isPlainObject(raw.tree)) {
      const err = pickError(raw);
      return err ? { tree: raw.tree as T, error: err } : { tree: raw.tree as T };
    }
    // Bare object that already looks like a tree node (has name +
    // optional children) — accept directly.
    if (typeof raw.name === "string") {
      return { tree: raw as unknown as T };
    }
    // Object with an error but no payload — surface the error and
    // fall back to an empty tree.
    const err = pickError(raw);
    return err ? { tree: fallback(), error: err } : { tree: fallback() };
  }
  return { tree: fallback() };
}

/* -------------------------------------------------------------------------- */
/*  Client fetchers                                                            */
/* -------------------------------------------------------------------------- */

async function getJsonRaw(url: string, signal?: AbortSignal): Promise<unknown> {
  // Prepend API_BASE so URLs hit the daemon's HTTP origin when running
  // inside Tauri. In Bun-server dev mode API_BASE is empty so the
  // existing relative-URL behaviour is preserved.
  //
  // Then thread the active project hash through `withProject()` so
  // multi-shard installs can switch between projects via the header
  // dropdown without a full reload. Backend handlers honour
  // `?project=<hash>` and fall back to "first shard alphabetically"
  // when the param is absent — preserving the legacy single-project
  // contract.
  const baseUrl = url.startsWith("http") ? url : API_BASE + url;
  const finalUrl = withProject(baseUrl);
  const res = await fetch(finalUrl, { signal });
  if (!res.ok) throw new Error(`${finalUrl} -> HTTP ${res.status}`);
  return await res.json();
}

/** Re-throw AbortError, otherwise return the message string. */
function describeFetchErr(err: unknown): string {
  if ((err as Error)?.name === "AbortError") throw err;
  return String(err);
}

export async function fetchNodes(signal?: AbortSignal, limit = 2000): Promise<NodesResponse> {
  try {
    const raw = await getJsonRaw(`/api/graph/nodes?limit=${limit}`, signal);
    const { items, error } = asArrayEnvelope<GraphNode>(raw, "nodes");
    return error ? { nodes: items, error } : { nodes: items };
  } catch (err) {
    return { nodes: [], error: describeFetchErr(err) };
  }
}

export async function fetchEdges(signal?: AbortSignal, limit = 8000): Promise<EdgesResponse> {
  try {
    const raw = await getJsonRaw(`/api/graph/edges?limit=${limit}`, signal);
    const { items, error } = asArrayEnvelope<GraphEdge>(raw, "edges");
    return error ? { edges: items, error } : { edges: items };
  } catch (err) {
    return { edges: [], error: describeFetchErr(err) };
  }
}

export async function fetchFiles(signal?: AbortSignal, limit = 2000): Promise<FilesResponse> {
  try {
    const raw = await getJsonRaw(`/api/graph/files?limit=${limit}`, signal);
    const { items, error } = asArrayEnvelope<ShardFileRow>(raw, "files");
    return error ? { files: items, error } : { files: items };
  } catch (err) {
    return { files: [], error: describeFetchErr(err) };
  }
}

export async function fetchFindings(
  signal?: AbortSignal,
  limit = 2000,
): Promise<FindingsResponse> {
  try {
    const raw = await getJsonRaw(`/api/graph/findings?limit=${limit}`, signal);
    const { items, error } = asArrayEnvelope<ShardFindingRow>(raw, "findings");
    return error ? { findings: items, error } : { findings: items };
  } catch (err) {
    return { findings: [], error: describeFetchErr(err) };
  }
}

/**
 * `/api/graph/status` ships its keys snake_case (`shard_root`,
 * `last_index_at`, `by_kind`) and omits both `ok` and `project`. The
 * StatusBar reads `status.ok`, `status.project`, `status.lastIndexAt`,
 * etc. Map both casings so the bar renders correctly without the
 * daemon needing a key-rename redeploy.
 */
function normalizeStatus(raw: unknown): GraphStatsPayload {
  const empty: GraphStatsPayload = {
    ok: false,
    project: null,
    shardRoot: null,
    nodes: 0,
    edges: 0,
    files: 0,
    byKind: {},
    lastIndexAt: null,
  };
  if (!isPlainObject(raw)) return empty;

  const r = raw as Record<string, unknown>;
  const nodes = Number(r.nodes ?? 0);
  const edges = Number(r.edges ?? 0);
  const files = Number(r.files ?? 0);
  const shardRoot =
    (typeof r.shardRoot === "string" ? r.shardRoot : null) ??
    (typeof r.shard_root === "string" ? (r.shard_root as string) : null);
  const lastIndexAt =
    (typeof r.lastIndexAt === "string" ? r.lastIndexAt : null) ??
    (typeof r.last_index_at === "string" ? (r.last_index_at as string) : null);
  const byKindRaw = (r.byKind ?? r.by_kind) as unknown;
  const byKind: Record<string, number> = {};
  if (isPlainObject(byKindRaw)) {
    for (const [k, v] of Object.entries(byKindRaw)) byKind[k] = Number(v ?? 0);
  }
  // `project` may arrive as a string or null. When the supervisor omits
  // it, fall back to the basename of `shardRoot` so the bar shows
  // *something* meaningful (the project hash directory name).
  let project: string | null = null;
  if (typeof r.project === "string" && r.project.length > 0) {
    project = r.project;
  } else if (shardRoot) {
    const segs = shardRoot.split(/[\\/]/).filter(Boolean);
    project = segs.length > 0 ? segs[segs.length - 1]! : null;
  }
  // `ok` defaults to true when a shard answered with non-zero counts
  // OR a shardRoot is present (the daemon found a shard but it's
  // empty mid-build). Only mark `ok=false` when we have nothing at
  // all — that's the "shard missing — run mneme build" state.
  const okExplicit = typeof r.ok === "boolean" ? r.ok : undefined;
  const ok = okExplicit ?? (shardRoot !== null || nodes > 0 || edges > 0 || files > 0);
  const error = pickError(r);

  return {
    ok,
    project,
    shardRoot,
    nodes,
    edges,
    files,
    byKind,
    lastIndexAt,
    ...(error ? { error } : {}),
  };
}

export async function fetchStatus(signal?: AbortSignal): Promise<GraphStatsPayload> {
  try {
    const raw = await getJsonRaw("/api/graph/status", signal);
    return normalizeStatus(raw);
  } catch (err) {
    if ((err as Error).name === "AbortError") throw err;
    return {
      ok: false,
      project: null,
      shardRoot: null,
      nodes: 0,
      edges: 0,
      files: 0,
      byKind: {},
      lastIndexAt: null,
      error: String(err),
    };
  }
}

export async function fetchDaemonHealth(
  signal?: AbortSignal,
): Promise<DaemonHealthPayload> {
  try {
    const raw = await getJsonRaw("/api/daemon/health", signal);
    if (!isPlainObject(raw)) {
      return { ok: false, status: "error", url: "/api/daemon/health" };
    }
    const ok = Boolean(raw.ok);
    return {
      ok,
      status: ok ? "running" : "missing",
      url: "/api/daemon/health",
      ...(typeof raw.detail === "string" ? { detail: raw.detail } : {}),
    };
  } catch (err) {
    if ((err as Error).name === "AbortError") throw err;
    return {
      ok: false,
      status: "missing",
      url: "/api/daemon/health",
      error: String(err),
    };
  }
}

export async function fetchFileTree(
  signal?: AbortSignal,
  limit = 4000,
): Promise<FileTreeResponse> {
  try {
    const raw = await getJsonRaw(`/api/graph/file-tree?limit=${limit}`, signal);
    const { tree, error } = asTreeEnvelope<FileTreeNode>(raw, () => ({
      name: "project",
      children: [],
    }));
    return error ? { tree, error } : { tree };
  } catch (err) {
    if ((err as Error).name === "AbortError") throw err;
    return { tree: { name: "project", children: [] }, error: String(err) };
  }
}

export async function fetchKindFlow(
  signal?: AbortSignal,
  limit = 50000,
): Promise<KindFlowResponse> {
  try {
    const raw = await getJsonRaw(`/api/graph/kind-flow?limit=${limit}`, signal);
    if (!isPlainObject(raw)) return { nodes: [], links: [] };
    const nodes = Array.isArray(raw.nodes) ? (raw.nodes as KindFlowNode[]) : [];
    const links = Array.isArray(raw.links) ? (raw.links as KindFlowLink[]) : [];
    const error = pickError(raw);
    return error ? { nodes, links, error } : { nodes, links };
  } catch (err) {
    if ((err as Error).name === "AbortError") throw err;
    return { nodes: [], links: [], error: String(err) };
  }
}

export async function fetchDomainFlow(
  signal?: AbortSignal,
  limit = 50000,
): Promise<DomainFlowResponse> {
  try {
    const raw = await getJsonRaw(`/api/graph/domain-flow?limit=${limit}`, signal);
    if (!isPlainObject(raw)) return { nodes: [], links: [] };
    const nodes = Array.isArray(raw.nodes) ? (raw.nodes as DomainFlowNode[]) : [];
    const links = Array.isArray(raw.links) ? (raw.links as DomainFlowLink[]) : [];
    const error = pickError(raw);
    return error ? { nodes, links, error } : { nodes, links };
  } catch (err) {
    if ((err as Error).name === "AbortError") throw err;
    return { nodes: [], links: [], error: String(err) };
  }
}

export async function fetchCommunityMatrix(
  signal?: AbortSignal,
): Promise<CommunityMatrixResponse> {
  try {
    const raw = await getJsonRaw("/api/graph/community-matrix", signal);
    if (!isPlainObject(raw)) return { communities: [], matrix: [] };
    const communities = Array.isArray(raw.communities)
      ? (raw.communities as CommunityInfo[])
      : [];
    const matrix = Array.isArray(raw.matrix) ? (raw.matrix as number[][]) : [];
    const error = pickError(raw);
    return error ? { communities, matrix, error } : { communities, matrix };
  } catch (err) {
    if ((err as Error).name === "AbortError") throw err;
    return { communities: [], matrix: [], error: String(err) };
  }
}

export async function fetchCommits(
  signal?: AbortSignal,
  limit = 500,
): Promise<CommitsResponse> {
  try {
    const raw = await getJsonRaw(`/api/graph/commits?limit=${limit}`, signal);
    const { items, error } = asArrayEnvelope<CommitRow>(raw, "commits");
    return error ? { commits: items, error } : { commits: items };
  } catch (err) {
    return { commits: [], error: describeFetchErr(err) };
  }
}

export async function fetchHeatmap(
  signal?: AbortSignal,
  limit = 120,
): Promise<HeatmapResponse> {
  try {
    const raw = await getJsonRaw(`/api/graph/heatmap?limit=${limit}`, signal);
    if (!isPlainObject(raw)) {
      return { severities: ["critical", "high", "medium", "low"], files: [] };
    }
    const severities = Array.isArray(raw.severities)
      ? (raw.severities as string[])
      : ["critical", "high", "medium", "low"];
    const files = Array.isArray(raw.files) ? (raw.files as HeatmapFileRow[]) : [];
    const error = pickError(raw);
    return error ? { severities, files, error } : { severities, files };
  } catch (err) {
    if ((err as Error).name === "AbortError") throw err;
    return {
      severities: ["critical", "high", "medium", "low"],
      files: [],
      error: String(err),
    };
  }
}

export async function fetchLayerTiers(
  signal?: AbortSignal,
): Promise<LayerTierResponse> {
  try {
    const raw = await getJsonRaw("/api/graph/layers", signal);
    if (!isPlainObject(raw)) {
      return {
        tiers: ["presentation", "api", "intelligence", "data", "foundation", "other"],
        entries: [],
      };
    }
    const tiers = Array.isArray(raw.tiers)
      ? (raw.tiers as string[])
      : ["presentation", "api", "intelligence", "data", "foundation", "other"];
    const entries = Array.isArray(raw.entries) ? (raw.entries as LayerTierEntry[]) : [];
    const error = pickError(raw);
    return error ? { tiers, entries, error } : { tiers, entries };
  } catch (err) {
    if ((err as Error).name === "AbortError") throw err;
    return {
      tiers: ["presentation", "api", "intelligence", "data", "foundation", "other"],
      entries: [],
      error: String(err),
    };
  }
}

export async function fetchGalaxy3D(
  signal?: AbortSignal,
  limit = 4000,
): Promise<Galaxy3DResponse> {
  try {
    const raw = await getJsonRaw(`/api/graph/galaxy-3d?limit=${limit}`, signal);
    if (!isPlainObject(raw)) return { nodes: [], edges: [] };
    const nodes = Array.isArray(raw.nodes) ? (raw.nodes as Galaxy3DNode[]) : [];
    const edges = Array.isArray(raw.edges) ? (raw.edges as Galaxy3DEdge[]) : [];
    const error = pickError(raw);
    return error ? { nodes, edges, error } : { nodes, edges };
  } catch (err) {
    if ((err as Error).name === "AbortError") throw err;
    return { nodes: [], edges: [], error: String(err) };
  }
}

export async function fetchTestCoverage(
  signal?: AbortSignal,
  limit = 2000,
): Promise<TestCoverageResponse> {
  try {
    const raw = await getJsonRaw(`/api/graph/test-coverage?limit=${limit}`, signal);
    const { items, error } = asArrayEnvelope<TestCoverageRow>(raw, "rows");
    return error ? { rows: items, error } : { rows: items };
  } catch (err) {
    return { rows: [], error: describeFetchErr(err) };
  }
}

export async function fetchThemeSwatches(
  signal?: AbortSignal,
  limit = 2000,
): Promise<ThemeSwatchResponse> {
  try {
    const raw = await getJsonRaw(`/api/graph/theme-palette?limit=${limit}`, signal);
    const { items, error } = asArrayEnvelope<ThemeSwatchRow>(raw, "swatches");
    return error ? { swatches: items, error } : { swatches: items };
  } catch (err) {
    return { swatches: [], error: describeFetchErr(err) };
  }
}

export async function fetchHierarchy(
  signal?: AbortSignal,
  limit = 4000,
): Promise<HierarchyResponse> {
  try {
    const raw = await getJsonRaw(`/api/graph/hierarchy?limit=${limit}`, signal);
    const { tree, error } = asTreeEnvelope<HierarchyNode>(raw, () => ({
      name: "project",
      children: [],
    }));
    return error ? { tree, error } : { tree };
  } catch (err) {
    if ((err as Error).name === "AbortError") throw err;
    return { tree: { name: "project", children: [] }, error: String(err) };
  }
}
