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
 *   - `shardSchemaVersions` reads the schema_version row from each shard
 *     so the model knows whether a migration is pending.
 *   - A 2s GET to `http://127.0.0.1:7777/health` surfaces the SLA.
 *   - An IPC round-trip is implied by the HTTP probe (same supervisor).
 *   - Missing / corrupt / unreachable components become individual checks
 *     with a recommendation instead of a raised exception.
 *
 * Output schema is fixed (see `DoctorOutput` in ../types.ts):
 *   `{ ok, checks: [{name, passed, detail}], recommendations }`
 * — graceful degrade: every failure path surfaces as one check + one
 *   recommendation, never as an unhandled exception.
 */

import {
  DoctorInput,
  DoctorOutput,
  type ToolDescriptor,
} from "../types.ts";
import { doctorShardSweep, shardSchemaVersions } from "../store.ts";

interface Check {
  name: string;
  passed: boolean;
  detail: string;
}

interface SupervisorProbe {
  check: Check;
  workers_green: boolean;
  reachable: boolean;
}

async function probeSupervisorSla(): Promise<SupervisorProbe> {
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
        reachable: true,
      };
    }
    const h = (await res.json()) as {
      overall_uptime_percent: number;
      children: Array<{
        name: string;
        status: string;
        restart_count?: number;
      }>;
    };
    const running = h.children.filter((c) => c.status === "running").length;
    const workers_green = running === h.children.length;
    // Phase A B3: a literal `(uptime 0.0%)` reads like a failure to humans
    // and to the model. Prefer a stability statement: when no worker has
    // restarted in the supervisor's window, surface "no restarts in 24h".
    // Only emit a percentage when we actually have a meaningful number.
    const totalRestarts = h.children.reduce(
      (acc, c) => acc + (typeof c.restart_count === "number" ? c.restart_count : 0),
      0,
    );
    const pct = h.overall_uptime_percent;
    const stabilitySuffix =
      totalRestarts === 0
        ? "no restarts in 24h"
        : Number.isFinite(pct) && pct > 0
        ? `uptime ${pct.toFixed(1)}%`
        : `${totalRestarts} restart${totalRestarts === 1 ? "" : "s"} in 24h`;
    return {
      check: {
        name: "supervisor_sla",
        passed: workers_green,
        detail: workers_green
          ? `All ${h.children.length} workers running (${stabilitySuffix})`
          : `${running}/${h.children.length} workers running`,
      },
      workers_green,
      reachable: true,
    };
  } catch (err) {
    // Connection refused / timeout / DNS failure — daemon almost certainly
    // not running. Emit a dedicated check so the recommendation is precise.
    return {
      check: {
        name: "supervisor_sla",
        passed: false,
        detail: `daemon not running — could not reach supervisor /health: ${(err as Error).message}`,
      },
      workers_green: false,
      reachable: false,
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

    // Per-shard schema-version probe. Missing/stale versions become checks
    // so the model can decide whether to trigger `mneme migrate`.
    let anyUnversioned = false;
    for (const v of shardSchemaVersions()) {
      if (v.version == null) {
        // Don't double-count shards we already marked missing — keeps the
        // check list readable when the project has never been built.
        const already = sweep.find((s) => s.layer === v.layer);
        if (already && !already.exists) continue;
        checks.push({
          name: `schema_${v.layer}`,
          passed: false,
          detail: v.error ?? "schema_version unavailable",
        });
        anyUnversioned = true;
      } else {
        checks.push({
          name: `schema_${v.layer}`,
          passed: true,
          detail: `v${v.version}`,
        });
      }
    }
    if (anyUnversioned) {
      recommendations.push(
        "One or more shards have no schema_version row — run `mneme migrate` to bring them to the latest version.",
      );
    }

    const sla = await probeSupervisorSla();
    checks.push(sla.check);
    if (!sla.check.passed) {
      if (!sla.reachable) {
        recommendations.push(
          "Daemon not running — start it with `mneme daemon start --detach`.",
        );
      } else {
        recommendations.push(
          "Supervisor reachable but not fully healthy — inspect `mneme health` and `mneme logs` for the failing worker.",
        );
      }
    }

    const ok = checks.every((c) => c.passed);

    return { ok, checks, recommendations };
  },
};
