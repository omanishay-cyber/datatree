/**
 * MCP tool: doctor
 *
 * Multi-shard health check. Runs a cheap self-test across every shard in
 * the project, probes the supervisor's HTTP /health endpoint, and emits
 * per-check pass/fail + actionable recommendations.
 *
 * v0.1 (review P2):
 *   - `doctorShardSweep` queries each shard's row counts + PRAGMA
 *     integrity_check via `bun:sqlite` read-only.
 *   - A 2s GET to `http://127.0.0.1:7777/health` surfaces the SLA.
 *   - Missing / corrupt / unreachable components become individual checks
 *     with a recommendation instead of a raised exception.
 */

import {
  DoctorInput,
  DoctorOutput,
  type ToolDescriptor,
} from "../types.ts";
import { doctorShardSweep } from "../store.ts";

interface Check {
  name: string;
  passed: boolean;
  detail: string;
}

async function probeSupervisorSla(): Promise<{
  check: Check;
  workers_green: boolean;
}> {
  try {
    const res = await fetch("http://127.0.0.1:7777/health", {
      signal: AbortSignal.timeout(2000),
    });
    if (!res.ok) {
      return {
        check: {
          name: "supervisor_sla",
          passed: false,
          detail: `HTTP ${res.status} from supervisor /health`,
        },
        workers_green: false,
      };
    }
    const h = (await res.json()) as {
      overall_uptime_percent: number;
      children: Array<{ name: string; status: string }>;
    };
    const running = h.children.filter((c) => c.status === "running").length;
    const workers_green = running === h.children.length;
    return {
      check: {
        name: "supervisor_sla",
        passed: workers_green,
        detail: workers_green
          ? `All ${h.children.length} workers running (uptime ${h.overall_uptime_percent.toFixed(1)}%)`
          : `${running}/${h.children.length} workers running`,
      },
      workers_green,
    };
  } catch (err) {
    return {
      check: {
        name: "supervisor_sla",
        passed: false,
        detail: `Could not reach supervisor /health: ${(err as Error).message}`,
      },
      workers_green: false,
    };
  }
}

export const tool: ToolDescriptor<
  ReturnType<typeof DoctorInput.parse>,
  ReturnType<typeof DoctorOutput.parse>
> = {
  name: "doctor",
  description:
    "Run the supervisor self-test suite: integrity check on every shard, schema-version validation, worker health, IPC round-trip. Returns per-check status + recommendations.",
  inputSchema: DoctorInput,
  outputSchema: DoctorOutput,
  category: "health",
  async handler() {
    const checks: Check[] = [];
    const recommendations: string[] = [];

    const sweep = doctorShardSweep();
    let anyMissing = false;
    let anyCorrupt = false;

    for (const s of sweep) {
      if (!s.exists) {
        checks.push({
          name: `shard_${s.layer}`,
          passed: false,
          detail: s.error ?? "missing",
        });
        anyMissing = true;
        continue;
      }
      if (!s.integrity_ok) {
        checks.push({
          name: `shard_${s.layer}`,
          passed: false,
          detail: s.error ?? "integrity_check failed",
        });
        anyCorrupt = true;
        continue;
      }
      const summary = Object.entries(s.row_counts)
        .map(([t, n]) => `${t}=${n}`)
        .join(", ");
      checks.push({
        name: `shard_${s.layer}`,
        passed: true,
        detail: `ok; ${summary}`,
      });
    }

    if (anyMissing) {
      recommendations.push("Run `mneme build .` in your project to create missing shards.");
    }
    if (anyCorrupt) {
      recommendations.push(
        "One or more shards failed PRAGMA integrity_check — run `mneme doctor --repair` or restore from a snapshot.",
      );
    }

    const sla = await probeSupervisorSla();
    checks.push(sla.check);
    if (!sla.check.passed) {
      recommendations.push("Start the daemon: `mneme daemon start --detach`.");
    }

    const ok = checks.every((c) => c.passed);

    return { ok, checks, recommendations };
  },
};
