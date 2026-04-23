/**
 * MCP tool: step_verify
 *
 * Run the acceptance check for a step.
 *
 * v0.1 (review P2): reads the step's `acceptance_cmd` from `tasks.db`
 * via `bun:sqlite` read-only, then either (a) spawns the command
 * locally via `Bun.spawn` (acceptance checks are trusted, per design
 * §7), or (b) dispatches `step.verify` over IPC when the supervisor
 * is available. Result-writing into `verification_proof` happens via
 * IPC (single-writer invariant) — if IPC fails we still return the
 * captured proof so the model can decide.
 */

import {
  StepVerifyInput,
  StepVerifyOutput,
  type ToolDescriptor,
} from "../types.ts";
import { shardDbPath, singleStep } from "../store.ts";
import { query as dbQuery } from "../db.ts";

function runLocal(cmd: string): Promise<{
  passed: boolean;
  proof: string;
  exit_code: number;
}> {
  // Bun provides a spawnSync; we fall back to Node's child_process when the
  // global isn't available (keeps the type-checker happy under plain Node).
  return new Promise((resolve) => {
    try {
      // Prefer Bun.spawn if present.
      const bunGlobal = (globalThis as { Bun?: unknown }).Bun;
      if (bunGlobal && typeof bunGlobal === "object") {
        const spawn = (bunGlobal as { spawnSync?: unknown }).spawnSync;
        if (typeof spawn === "function") {
          const res = (spawn as (args: unknown) => {
            exitCode: number;
            stdout: { toString(): string };
            stderr: { toString(): string };
          })({
            cmd: ["sh", "-c", cmd],
            stdout: "pipe",
            stderr: "pipe",
          });
          const exit = res.exitCode;
          const proof =
            (res.stdout?.toString?.() ?? "") +
            (res.stderr?.toString?.() ?? "");
          resolve({ passed: exit === 0, proof, exit_code: exit });
          return;
        }
      }
      // Node fallback.
      import("node:child_process")
        .then(({ spawnSync }) => {
          const res = spawnSync("sh", ["-c", cmd], { encoding: "utf8" });
          const exit = res.status ?? 127;
          const proof = (res.stdout ?? "") + (res.stderr ?? "");
          resolve({ passed: exit === 0, proof, exit_code: exit });
        })
        .catch((err: unknown) => {
          resolve({
            passed: false,
            proof: `spawn failed: ${(err as Error).message}`,
            exit_code: 127,
          });
        });
    } catch (err) {
      resolve({
        passed: false,
        proof: `spawn error: ${(err as Error).message}`,
        exit_code: 127,
      });
    }
  });
}

export const tool: ToolDescriptor<
  ReturnType<typeof StepVerifyInput.parse>,
  ReturnType<typeof StepVerifyOutput.parse>
> = {
  name: "step_verify",
  description:
    "Run the acceptance check for a step. Returns passed/proof/exit_code. Does NOT mark complete — call step_complete after a passing verify.",
  inputSchema: StepVerifyInput,
  outputSchema: StepVerifyOutput,
  category: "step",
  async handler(input) {
    const t0 = Date.now();

    // Prefer the supervisor (it knows how to record the proof).
    const ipc = await dbQuery
      .raw<{ passed: boolean; proof: string; exit_code: number }>(
        "step.verify",
        { step_id: input.step_id, dry_run: input.dry_run },
      )
      .catch(() => null);

    if (ipc) {
      return {
        step_id: input.step_id,
        passed: ipc.passed,
        proof: ipc.proof,
        exit_code: ipc.exit_code,
        duration_ms: Date.now() - t0,
      };
    }

    // Fallback: read the acceptance_cmd locally and execute it.
    if (!shardDbPath("tasks")) {
      return {
        step_id: input.step_id,
        passed: false,
        proof: "tasks shard not yet created (run `mneme build .`)",
        exit_code: 127,
        duration_ms: Date.now() - t0,
      };
    }
    const row = singleStep(input.step_id);
    if (!row) {
      return {
        step_id: input.step_id,
        passed: false,
        proof: `no step with id ${input.step_id}`,
        exit_code: 127,
        duration_ms: Date.now() - t0,
      };
    }
    if (!row.acceptance_cmd) {
      return {
        step_id: input.step_id,
        passed: true,
        proof: "(no acceptance_cmd; trivially passing)",
        exit_code: 0,
        duration_ms: Date.now() - t0,
      };
    }
    if (input.dry_run) {
      return {
        step_id: input.step_id,
        passed: true,
        proof: `dry_run: would execute \`${row.acceptance_cmd}\``,
        exit_code: 0,
        duration_ms: Date.now() - t0,
      };
    }

    const res = await runLocal(row.acceptance_cmd);
    return {
      step_id: input.step_id,
      passed: res.passed,
      proof: res.proof,
      exit_code: res.exit_code,
      duration_ms: Date.now() - t0,
    };
  },
};
