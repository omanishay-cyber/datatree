/**
 * Datatree MCP — shared type definitions and zod schemas.
 *
 * Every MCP tool input/output is validated against a zod schema declared here.
 * Hooks emit a strict JSON shape consumed by Claude Code (and 17 other AI
 * harnesses); the schemas in this file are the single source of truth.
 *
 * Conventions:
 *   - All times are RFC3339 strings ("2026-04-23T10:11:12.345Z").
 *   - All file paths are absolute, OS-native (forward slashes on Windows OK).
 *   - All ids ("step_id", "session_id", "snapshot_id") are opaque strings.
 *   - Severity ladder: "info" | "low" | "medium" | "high" | "critical".
 */

import { z } from "zod";

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

export const SeverityEnum = z.enum(["info", "low", "medium", "high", "critical"]);
export type Severity = z.infer<typeof SeverityEnum>;

export const StepStatusEnum = z.enum([
  "not_started",
  "in_progress",
  "completed",
  "blocked",
  "failed",
]);
export type StepStatus = z.infer<typeof StepStatusEnum>;

export const DbLayerEnum = z.enum([
  "history",
  "decisions",
  "constraints",
  "tasks",
  "findings",
  "graph",
  "semantic",
  "memory",
  "tool_cache",
  "audit",
  "telemetry",
  "corpus",
  "multimodal",
]);
export type DbLayer = z.infer<typeof DbLayerEnum>;

// ---------------------------------------------------------------------------
// Tool I/O — Recall family (§5.1)
// ---------------------------------------------------------------------------

export const RecallDecisionInput = z.object({
  query: z.string().min(1),
  since: z.string().optional(),
  limit: z.number().int().positive().max(100).default(10),
});
export type RecallDecisionInput = z.infer<typeof RecallDecisionInput>;

export const Decision = z.object({
  id: z.string(),
  topic: z.string(),
  problem: z.string(),
  chosen: z.string(),
  reasoning: z.string(),
  rejected: z.array(z.string()).default([]),
  timestamp: z.string(),
  source_file: z.string().nullable().default(null),
  confidence: z.number().min(0).max(1).default(1),
});
export type Decision = z.infer<typeof Decision>;

export const RecallDecisionOutput = z.object({
  decisions: z.array(Decision),
  query_id: z.string(),
  latency_ms: z.number(),
});

export const RecallConversationInput = z.object({
  query: z.string().min(1),
  since: z.string().optional(),
  session_id: z.string().optional(),
  limit: z.number().int().positive().max(50).default(10),
});
export type RecallConversationInput = z.infer<typeof RecallConversationInput>;

export const ConversationTurn = z.object({
  turn_id: z.string(),
  session_id: z.string(),
  role: z.enum(["user", "assistant", "system", "tool"]),
  content: z.string(),
  tool_calls: z.array(z.unknown()).default([]),
  timestamp: z.string(),
  similarity: z.number().min(0).max(1).optional(),
});
export type ConversationTurn = z.infer<typeof ConversationTurn>;

export const RecallConversationOutput = z.object({
  turns: z.array(ConversationTurn),
});

export const RecallConceptInput = z.object({
  query: z.string().min(1),
  modality: z
    .enum(["all", "code", "doc", "image", "audio", "video"])
    .default("all"),
  limit: z.number().int().positive().max(50).default(10),
});

export const Concept = z.object({
  id: z.string(),
  label: z.string(),
  modality: z.string(),
  source_file: z.string().nullable(),
  source_location: z.string().nullable(),
  similarity: z.number().min(0).max(1),
  community_id: z.number().int().nullable(),
});
export type Concept = z.infer<typeof Concept>;

export const RecallConceptOutput = z.object({
  concepts: z.array(Concept),
});

export const RecallFileInput = z.object({
  path: z.string().min(1),
});

export const FileState = z.object({
  path: z.string(),
  exists: z.boolean(),
  hash: z.string().nullable(),
  size_bytes: z.number().int().nullable(),
  language: z.string().nullable(),
  summary: z.string().nullable(),
  last_read_at: z.string().nullable(),
  last_modified_at: z.string().nullable(),
  blast_radius_count: z.number().int().nullable(),
  test_coverage: z.number().min(0).max(1).nullable(),
});
export type FileState = z.infer<typeof FileState>;

export const RecallTodoInput = z.object({
  filter: z
    .object({
      status: z.enum(["open", "completed", "all"]).default("open"),
      tag: z.string().optional(),
      since: z.string().optional(),
    })
    .default({}),
});

export const Todo = z.object({
  id: z.string(),
  text: z.string(),
  status: z.enum(["open", "completed"]),
  created_at: z.string(),
  completed_at: z.string().nullable(),
  source_file: z.string().nullable(),
  tags: z.array(z.string()).default([]),
});
export type Todo = z.infer<typeof Todo>;

export const RecallTodoOutput = z.object({ todos: z.array(Todo) });

export const RecallConstraintInput = z.object({
  scope: z.enum(["global", "project", "file"]).default("project"),
  file: z.string().optional(),
});

export const Constraint = z.object({
  id: z.string(),
  rule: z.string(),
  scope: z.string(),
  source: z.string(),
  severity: SeverityEnum,
  enforcement: z.enum(["warn", "block"]),
});
export type Constraint = z.infer<typeof Constraint>;

export const RecallConstraintOutput = z.object({
  constraints: z.array(Constraint),
});

// ---------------------------------------------------------------------------
// Tool I/O — Code Graph (§5.2)
// ---------------------------------------------------------------------------

export const BlastRadiusInput = z.object({
  target: z.string().min(1),
  depth: z.number().int().positive().max(10).default(3),
  include_tests: z.boolean().default(true),
});

export const BlastRadiusOutput = z.object({
  target: z.string(),
  affected_files: z.array(z.string()),
  affected_symbols: z.array(z.string()),
  test_files: z.array(z.string()),
  total_count: z.number().int(),
  critical_paths: z.array(z.string()).default([]),
});

export const CallGraphInput = z.object({
  function: z.string().min(1),
  direction: z.enum(["callers", "callees", "both"]).default("both"),
  depth: z.number().int().positive().max(10).default(3),
});

export const CallGraphNode = z.object({
  id: z.string(),
  label: z.string(),
  file: z.string(),
  line: z.number().int(),
});

export const CallGraphEdge = z.object({
  source: z.string(),
  target: z.string(),
  call_count: z.number().int().default(1),
});

export const CallGraphOutput = z.object({
  nodes: z.array(CallGraphNode),
  edges: z.array(CallGraphEdge),
});

export const FindReferencesInput = z.object({
  symbol: z.string().min(1),
  scope: z.enum(["project", "workspace"]).default("project"),
});

export const ReferenceHit = z.object({
  file: z.string(),
  line: z.number().int(),
  column: z.number().int(),
  context: z.string(),
  kind: z.enum(["definition", "call", "import", "usage"]),
});

export const FindReferencesOutput = z.object({
  symbol: z.string(),
  hits: z.array(ReferenceHit),
});

export const DependencyChainInput = z.object({
  file: z.string().min(1),
  direction: z.enum(["forward", "reverse", "both"]).default("both"),
});

export const DependencyChainOutput = z.object({
  file: z.string(),
  forward: z.array(z.string()),
  reverse: z.array(z.string()),
});

export const CyclicDepsInput = z.object({
  scope: z.enum(["project", "workspace"]).default("project"),
});

export const CyclicDepsOutput = z.object({
  cycles: z.array(z.array(z.string())),
  count: z.number().int(),
});

// ---------------------------------------------------------------------------
// Tool I/O — Multimodal (§5.3)
// ---------------------------------------------------------------------------

export const GraphifyCorpusInput = z.object({
  path: z.string().optional(),
  mode: z.enum(["fast", "deep"]).default("fast"),
  incremental: z.boolean().default(true),
});

export const GraphifyCorpusOutput = z.object({
  nodes_count: z.number().int(),
  edges_count: z.number().int(),
  hyperedges_count: z.number().int(),
  communities_count: z.number().int(),
  duration_ms: z.number(),
  report_path: z.string(),
});

export const GodNodesInput = z.object({
  project: z.string().optional(),
  top_n: z.number().int().positive().max(100).default(10),
});

export const GodNode = z.object({
  id: z.string(),
  label: z.string(),
  degree: z.number().int(),
  betweenness: z.number(),
  community_id: z.number().int().nullable(),
});

export const GodNodesOutput = z.object({ gods: z.array(GodNode) });

export const SurprisingConnectionsInput = z.object({
  min_confidence: z.number().min(0).max(1).default(0.7),
  limit: z.number().int().positive().max(50).default(10),
});

export const Surprise = z.object({
  source: z.string(),
  target: z.string(),
  relation: z.string(),
  confidence: z.number(),
  source_community: z.number().int(),
  target_community: z.number().int(),
  reasoning: z.string(),
});

export const SurprisingConnectionsOutput = z.object({
  surprises: z.array(Surprise),
});

export const AuditCorpusInput = z.object({
  path: z.string().optional(),
});

export const AuditCorpusOutput = z.object({
  report_markdown: z.string(),
  report_path: z.string(),
  warnings: z.array(z.string()),
});

// ---------------------------------------------------------------------------
// Tool I/O — Drift & Audit (§5.4)
// ---------------------------------------------------------------------------

export const Finding = z.object({
  id: z.string(),
  scanner: z.string(),
  severity: SeverityEnum,
  file: z.string(),
  line: z.number().int().nullable(),
  rule: z.string(),
  message: z.string(),
  suggestion: z.string().nullable(),
  detected_at: z.string(),
});
export type Finding = z.infer<typeof Finding>;

export const AuditInput = z.object({
  scope: z.enum(["project", "file", "diff"]).default("project"),
  file: z.string().optional(),
  scanners: z.array(z.string()).optional(),
});

export const AuditOutput = z.object({
  findings: z.array(Finding),
  summary: z.object({
    total: z.number().int(),
    by_severity: z.record(z.string(), z.number().int()),
    by_scanner: z.record(z.string(), z.number().int()),
  }),
});

export const DriftFindingsInput = z.object({
  severity: SeverityEnum.optional(),
  scope: z.string().optional(),
  limit: z.number().int().positive().max(500).default(50),
});

export const DriftFindingsOutput = z.object({
  findings: z.array(Finding),
});

export const ScannerInput = z.object({
  file: z.string().optional(),
  scope: z.enum(["project", "file", "diff"]).default("project"),
});

export const ScannerOutput = z.object({
  findings: z.array(Finding),
  scanner: z.string(),
  duration_ms: z.number(),
});

// ---------------------------------------------------------------------------
// Tool I/O — Step Ledger (§5.5)
// ---------------------------------------------------------------------------

export const Step = z.object({
  step_id: z.string(),
  parent_step_id: z.string().nullable(),
  session_id: z.string(),
  description: z.string(),
  acceptance_cmd: z.string().nullable(),
  acceptance_check: z.unknown().nullable(),
  status: StepStatusEnum,
  started_at: z.string().nullable(),
  completed_at: z.string().nullable(),
  verification_proof: z.string().nullable(),
  artifacts: z.unknown().nullable(),
  notes: z.string().nullable(),
  blocker: z.string().nullable(),
  drift_score: z.number().int().default(0),
});
export type Step = z.infer<typeof Step>;

export const StepStatusInput = z.object({
  session_id: z.string().optional(),
});

export const StepStatusOutput = z.object({
  current_step_id: z.string().nullable(),
  steps: z.array(Step),
  drift_score_total: z.number().int(),
  goal_root: z.string().nullable(),
});

export const StepShowInput = z.object({
  step_id: z.string(),
});

export const StepShowOutput = z.object({ step: Step });

export const StepVerifyInput = z.object({
  step_id: z.string(),
  dry_run: z.boolean().default(false),
});

export const StepVerifyOutput = z.object({
  step_id: z.string(),
  passed: z.boolean(),
  proof: z.string(),
  exit_code: z.number().int(),
  duration_ms: z.number(),
});

export const StepCompleteInput = z.object({
  step_id: z.string(),
  force: z.boolean().default(false),
});

export const StepCompleteOutput = z.object({
  step_id: z.string(),
  completed: z.boolean(),
  next_step_id: z.string().nullable(),
});

export const StepResumeInput = z.object({
  session_id: z.string().optional(),
});

export const StepResumeOutput = z.object({
  bundle: z.string(),
  current_step_id: z.string().nullable(),
  total_steps: z.number().int(),
});

export const StepPlanFromInput = z.object({
  markdown_path: z.string().min(1),
  session_id: z.string().optional(),
});

export const StepPlanFromOutput = z.object({
  steps_created: z.number().int(),
  root_step_id: z.string(),
});

// ---------------------------------------------------------------------------
// Tool I/O — Time Machine (§5.6)
// ---------------------------------------------------------------------------

export const SnapshotInput = z.object({
  label: z.string().optional(),
});

export const SnapshotOutput = z.object({
  snapshot_id: z.string(),
  created_at: z.string(),
  size_bytes: z.number().int(),
});

export const CompareInput = z.object({
  snapshot_a: z.string(),
  snapshot_b: z.string(),
});

export const Diff = z.object({
  files_added: z.array(z.string()),
  files_removed: z.array(z.string()),
  files_modified: z.array(z.string()),
  decisions_added: z.number().int(),
  findings_resolved: z.number().int(),
  findings_introduced: z.number().int(),
});

export const CompareOutput = z.object({ diff: Diff });

export const RewindInput = z.object({
  file: z.string().min(1),
  when: z.string().min(1),
});

export const RewindOutput = z.object({
  file: z.string(),
  when: z.string(),
  content: z.string(),
  hash: z.string(),
});

// ---------------------------------------------------------------------------
// Tool I/O — Health (§5.7)
// ---------------------------------------------------------------------------

export const HealthInput = z.object({}).default({});

export const HealthOutput = z.object({
  status: z.enum(["green", "yellow", "red"]),
  uptime_seconds: z.number().int(),
  workers: z.array(
    z.object({
      name: z.string(),
      pid: z.number().int().nullable(),
      restarts_24h: z.number().int(),
      rss_mb: z.number(),
      status: z.string(),
    }),
  ),
  cache_hit_rate: z.number().min(0).max(1),
  disk_usage_mb: z.number(),
  queue_depth: z.number().int(),
  p50_ms: z.number(),
  p95_ms: z.number(),
  p99_ms: z.number(),
});

export const DoctorInput = z.object({}).default({});

export const DoctorOutput = z.object({
  ok: z.boolean(),
  checks: z.array(
    z.object({
      name: z.string(),
      passed: z.boolean(),
      detail: z.string(),
    }),
  ),
  recommendations: z.array(z.string()),
});

export const RebuildInput = z.object({
  scope: z.enum(["graph", "semantic", "all"]).default("graph"),
  confirm: z.boolean().default(false),
});

export const RebuildOutput = z.object({
  rebuilt: z.array(z.string()),
  duration_ms: z.number(),
});

// ---------------------------------------------------------------------------
// Hook outputs
// ---------------------------------------------------------------------------

/** Universal hook envelope returned by every hook to the harness. */
export const HookOutput = z.object({
  additional_context: z.string().optional(),
  skip: z.boolean().optional(),
  result: z.string().optional(),
  metadata: z.record(z.string(), z.unknown()).optional(),
});
export type HookOutput = z.infer<typeof HookOutput>;

// ---------------------------------------------------------------------------
// IPC envelope (CLI <-> MCP)
// ---------------------------------------------------------------------------

export interface IpcRequest {
  id: string;
  method: string;
  params: unknown;
}

export interface IpcResponse<T = unknown> {
  id: string;
  ok: boolean;
  data?: T;
  error?: { code: string; message: string; detail?: unknown };
  latency_ms: number;
  cache_hit: boolean;
  source_db?: DbLayer;
  schema_version?: number;
}

// ---------------------------------------------------------------------------
// MCP tool descriptor (used by the registry)
// ---------------------------------------------------------------------------

export interface ToolDescriptor<I = unknown, O = unknown> {
  /** MCP tool name (snake_case). */
  name: string;
  /** Human description shown to the model. */
  description: string;
  /** zod schema for input validation. */
  inputSchema: z.ZodType<I>;
  /** zod schema for output validation. */
  outputSchema: z.ZodType<O>;
  /** Implementation called by the MCP runtime after validation. */
  handler: (input: I, ctx: ToolContext) => Promise<O>;
  /** Optional category (used by /dt-recall, etc.). */
  category?:
    | "recall"
    | "graph"
    | "multimodal"
    | "drift"
    | "step"
    | "time"
    | "health";
}

export interface ToolContext {
  sessionId: string;
  cwd: string;
  /** Set when the tool was triggered via a hook (vs. direct MCP call). */
  hook?: string;
}

export const TokenBudgets = {
  primer: 1500,
  smart_inject: 2500,
  max_total_per_turn: 5000,
} as const;
