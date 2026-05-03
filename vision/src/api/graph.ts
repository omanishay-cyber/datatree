// vision/src/api/graph.ts
//
// Client-side shard API for the Vision app.
//
// These helpers hit the Bun server's /api/graph/* endpoints, which in turn
// read the project's SQLite shards directly via `bun:sqlite` (see
// `vision/server/shard.ts`). The server discovery logic mirrors the pattern
// used by `mcp/src/store.ts` — derive ProjectId from the cwd, open the
// shard `.db` read-only, answer scoped queries.
//
// Kept client-only so Vite does not try to bundle `bun:sqlite` into the
// browser chunk. The matching server-side module is `vision/server/shard.ts`.

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
/*  Client fetchers                                                            */
/* -------------------------------------------------------------------------- */

async function getJson<T>(url: string, signal?: AbortSignal): Promise<T> {
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
  return (await res.json()) as T;
}

export async function fetchNodes(signal?: AbortSignal, limit = 2000): Promise<NodesResponse> {
  return getJson<NodesResponse>(`/api/graph/nodes?limit=${limit}`, signal);
}

export async function fetchEdges(signal?: AbortSignal, limit = 8000): Promise<EdgesResponse> {
  return getJson<EdgesResponse>(`/api/graph/edges?limit=${limit}`, signal);
}

export async function fetchFiles(signal?: AbortSignal, limit = 2000): Promise<FilesResponse> {
  return getJson<FilesResponse>(`/api/graph/files?limit=${limit}`, signal);
}

export async function fetchFindings(
  signal?: AbortSignal,
  limit = 2000,
): Promise<FindingsResponse> {
  return getJson<FindingsResponse>(`/api/graph/findings?limit=${limit}`, signal);
}

export async function fetchStatus(signal?: AbortSignal): Promise<GraphStatsPayload> {
  return getJson<GraphStatsPayload>("/api/graph/status", signal);
}

export async function fetchDaemonHealth(
  signal?: AbortSignal,
): Promise<DaemonHealthPayload> {
  return getJson<DaemonHealthPayload>("/api/daemon/health", signal);
}

export async function fetchFileTree(
  signal?: AbortSignal,
  limit = 4000,
): Promise<FileTreeResponse> {
  return getJson<FileTreeResponse>(`/api/graph/file-tree?limit=${limit}`, signal);
}

export async function fetchKindFlow(
  signal?: AbortSignal,
  limit = 50000,
): Promise<KindFlowResponse> {
  return getJson<KindFlowResponse>(`/api/graph/kind-flow?limit=${limit}`, signal);
}

export async function fetchDomainFlow(
  signal?: AbortSignal,
  limit = 50000,
): Promise<DomainFlowResponse> {
  return getJson<DomainFlowResponse>(`/api/graph/domain-flow?limit=${limit}`, signal);
}

export async function fetchCommunityMatrix(
  signal?: AbortSignal,
): Promise<CommunityMatrixResponse> {
  return getJson<CommunityMatrixResponse>("/api/graph/community-matrix", signal);
}

export async function fetchCommits(
  signal?: AbortSignal,
  limit = 500,
): Promise<CommitsResponse> {
  return getJson<CommitsResponse>(`/api/graph/commits?limit=${limit}`, signal);
}

export async function fetchHeatmap(
  signal?: AbortSignal,
  limit = 120,
): Promise<HeatmapResponse> {
  return getJson<HeatmapResponse>(`/api/graph/heatmap?limit=${limit}`, signal);
}

export async function fetchLayerTiers(
  signal?: AbortSignal,
): Promise<LayerTierResponse> {
  return getJson<LayerTierResponse>("/api/graph/layers", signal);
}

export async function fetchGalaxy3D(
  signal?: AbortSignal,
  limit = 4000,
): Promise<Galaxy3DResponse> {
  return getJson<Galaxy3DResponse>(`/api/graph/galaxy-3d?limit=${limit}`, signal);
}

export async function fetchTestCoverage(
  signal?: AbortSignal,
  limit = 2000,
): Promise<TestCoverageResponse> {
  return getJson<TestCoverageResponse>(`/api/graph/test-coverage?limit=${limit}`, signal);
}

export async function fetchThemeSwatches(
  signal?: AbortSignal,
  limit = 2000,
): Promise<ThemeSwatchResponse> {
  return getJson<ThemeSwatchResponse>(`/api/graph/theme-palette?limit=${limit}`, signal);
}

export async function fetchHierarchy(
  signal?: AbortSignal,
  limit = 4000,
): Promise<HierarchyResponse> {
  return getJson<HierarchyResponse>(`/api/graph/hierarchy?limit=${limit}`, signal);
}
