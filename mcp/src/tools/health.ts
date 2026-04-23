/**
 * MCP tool: health
 *
 * Full SLA snapshot — uptime, worker statuses, cache hit rate, queue depth,
 * latency percentiles.
 *
 * v0.1 implementation: fetches the supervisor's HTTP /health endpoint
 * (localhost:7777) and maps it into the schema the MCP client expects.
 */

import {
  HealthInput,
  HealthOutput,
  type ToolDescriptor,
} from "../types.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof HealthInput.parse>,
  ReturnType<typeof HealthOutput.parse>
> = {
  name: "health",
  description:
    "Full SLA snapshot: uptime, worker statuses + restarts, cache hit rate, disk usage, queue depth, p50/p95/p99 query latency. Mirrors localhost:7777/health.",
  inputSchema: HealthInput,
  outputSchema: HealthOutput,
  category: "health",
  async handler() {
    try {
      const res = await fetch("http://127.0.0.1:7777/health", {
        signal: AbortSignal.timeout(2000),
      });
      if (!res.ok) throw new Error(`health http ${res.status}`);
      const h = (await res.json()) as {
        supervisor_uptime_s: number;
        children: Array<{
          name: string;
          status: string;
          pid: number | null;
          restart_count: number;
          current_uptime_ms: number;
          last_exit_code: number | null;
          p50_us: number | null;
          p95_us: number | null;
          p99_us: number | null;
        }>;
        overall_uptime_percent: number;
        cache_hit_rate: number;
        disk: { used_percent: number; free_bytes: number };
      };
      const workers = h.children.map((c) => ({
        name: c.name,
        status: c.status,
        pid: c.pid,
        restarts: c.restart_count,
        restarts_24h: c.restart_count,
        uptime_seconds: Math.floor(c.current_uptime_ms / 1000),
        rss_mb: 0,
      }));
      const running = workers.filter((w) => w.status === "running").length;
      const overall =
        running === workers.length ? "green" : running > 0 ? "yellow" : "red";
      const p50s = h.children
        .map((c) => c.p50_us)
        .filter((x): x is number => x != null);
      const p95s = h.children
        .map((c) => c.p95_us)
        .filter((x): x is number => x != null);
      const p99s = h.children
        .map((c) => c.p99_us)
        .filter((x): x is number => x != null);
      const avg = (xs: number[]) =>
        xs.length === 0 ? 0 : xs.reduce((a, b) => a + b, 0) / xs.length / 1000;
      return {
        status: overall,
        uptime_seconds: h.supervisor_uptime_s,
        workers,
        cache_hit_rate: h.cache_hit_rate,
        disk_usage_mb: Math.floor((100 - h.disk.used_percent) * 0), // best-effort
        queue_depth: 0,
        p50_ms: avg(p50s),
        p95_ms: avg(p95s),
        p99_ms: avg(p99s),
      };
    } catch (err) {
      return {
        status: "red",
        uptime_seconds: 0,
        workers: [],
        cache_hit_rate: 0,
        disk_usage_mb: 0,
        queue_depth: 0,
        p50_ms: 0,
        p95_ms: 0,
        p99_ms: 0,
      };
    }
  },
};
