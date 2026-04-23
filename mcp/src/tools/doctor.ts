/**
 * MCP tool: doctor
 *
 * Runs the supervisor self-test suite and returns a per-check pass/fail
 * report plus actionable recommendations.
 */

import {
  DoctorInput,
  DoctorOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

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
    const result = await dbQuery
      .raw<ReturnType<typeof DoctorOutput.parse>>("supervisor.doctor", {})
      .catch(() => null);

    return (
      result ?? {
        ok: false,
        checks: [
          {
            name: "ipc_connect",
            passed: false,
            detail: "Could not reach the datatree supervisor. Is the daemon running?",
          },
        ],
        recommendations: ["Start the daemon: `datatree daemon start`"],
      }
    );
  },
};
