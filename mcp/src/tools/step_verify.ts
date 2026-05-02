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

/**
 * Bug TS-6 (2026-05-01): pick the right shell for the OS.
 * `sh -c` was the original implementation but `sh` is NOT on stock
 * Windows (you'd need Git Bash or WSL). Step Ledger verification —
 * the "killer feature" — was therefore broken on the primary
 * deployment platform (Windows). Now we use `cmd /c` on Windows and
 * `sh -c` on Unix.
 */
function pickShell(): { exe: string; flag: string } {
  if (process.platform === "win32") {
    return { exe: "cmd.exe", flag: "/c" };
  }
  return { exe: "sh", flag: "-c" };
}

/**
 * Bug SEC-1 (2026-05-01): defense-in-depth check for obviously
 * dangerous acceptance commands. The supervisor IPC path
 * (`step.verify` over IPC) is still the preferred execution route —
 * runLocal is a fallback when supervisor is unavailable. Even so,
 * since `cmd` originates from the tasks shard (which is
 * filesystem-writable by anyone with FS access to ~/.mneme), we
 * reject the most lethal patterns outright. This is NOT a sandbox —
 * it's a tripwire to catch obvious shell injection / destructive
 * commands before they execute.
 */
function rejectDangerousCommand(cmd: string): string | null {
  const trimmed = cmd.trim();
  if (trimmed.length === 0) {
    return "empty acceptance command";
  }
  if (trimmed.length > 4096) {
    return `acceptance command too long (${trimmed.length} > 4096 chars)`;
  }
  // Hard-deny destructive patterns. We deliberately do NOT try to
  // build a complete sandbox here — these patterns just catch the
  // obvious "tasks.db got pwned" cases. Real defense lives at the FS
  // permission layer on ~/.mneme.
  const denyPatterns: RegExp[] = [
    /\brm\s+-[a-z]*r[a-z]*f?\s+\/\s*$/i, // rm -rf /
    /\brm\s+-[a-z]*r[a-z]*f?\s+~/i, //       rm -rf ~
    /\bmkfs\b/i, //                          mkfs (format disk)
    /\bdd\s+if=.+of=\/dev/i, //              dd of=/dev/...
    /:\(\)\{\s*:\|/, //                      fork bomb
    /\bshutdown\s+/i, //                     shutdown
    /\bformat\s+[a-z]:/i, //                 format C:
  ];
  for (const re of denyPatterns) {
    if (re.test(trimmed)) {
      return `acceptance command matches deny-list pattern (${re.source})`;
    }
  }
  return null;
}

function runLocal(cmd: string): Promise<{
  passed: boolean;
  proof: string;
  exit_code: number;
}> {
  // Defense-in-depth: deny-list lethal patterns before spawning.
  const reject = rejectDangerousCommand(cmd);
  if (reject) {
    return Promise.resolve({
      passed: false,
      proof: `step_verify rejected: ${reject}`,
      exit_code: 126,
    });
  }
  const { exe, flag } = pickShell();

  // Bug TS-12 (2026-05-01): typed Bun globals via a proper interface
  // instead of the prior triple `as unknown as` cast chain. Same
  // runtime behavior, but the type system now sees the optional
  // shape so a Bun signature change gets caught at compile.
  interface BunSpawnSyncResult {
    exitCode: number;
    stdout: { toString(): string };
    stderr: { toString(): string };
  }
  interface BunSpawnSyncOpts {
    cmd: string[];
    stdout: "pipe" | "ignore" | "inherit";
    stderr: "pipe" | "ignore" | "inherit";
  }
  interface BunGlobal {
    spawnSync?: (opts: BunSpawnSyncOpts) => BunSpawnSyncResult;
  }
  const bunGlobal = (globalThis as { Bun?: BunGlobal }).Bun;

  // Bun provides a spawnSync; we fall back to Node's child_process when
  // the global isn't available (keeps the type-checker happy under
  // plain Node).
  return new Promise((resolve) => {
    try {
      // Prefer Bun.spawn if present.
      if (bunGlobal && typeof bunGlobal.spawnSync === "function") {
        const res = bunGlobal.spawnSync({
          cmd: [exe, flag, cmd],
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
      // Node fallback.
      import("node:child_process")
        .then(({ spawnSync }) => {
          const res = spawnSync(exe, [flag, cmd], { encoding: "utf8" });
          const exit = res.status ?? 127;
          const proof = (res.stdout ?? "") + (res.stderr ?? "");
          resolve({ passed: exit === 0, proof, exit_code: exit });
        })
        .catch((err: unknown) => {
          resolve({
            passed: false,
            proof: `spawn failed: ${err instanceof Error ? err.message : String(err)}`,
            exit_code: 127,
          });
        });
    } catch (err) {
      resolve({
        passed: false,
        proof: `spawn error: ${err instanceof Error ? err.message : String(err)}`,
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
