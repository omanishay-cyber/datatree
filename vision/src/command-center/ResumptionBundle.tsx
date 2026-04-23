import { useMemo } from "react";
import type { CommandCenterState } from "../store";

interface ResumptionBundleProps {
  commandCenter: CommandCenterState;
}

// Preview of the payload that would be injected into a fresh context if the
// supervisor compacts now. Mirrors the supervisor's resumption schema.
export function ResumptionBundle({ commandCenter }: ResumptionBundleProps): JSX.Element {
  const bundle = useMemo(() => {
    return {
      generatedAt: new Date().toISOString(),
      activeGoal:
        commandCenter.goals.find((g) => g.status === "active") ??
        commandCenter.goals[commandCenter.goals.length - 1] ??
        null,
      pendingSteps: commandCenter.steps.filter((s) => s.status === "todo" || s.status === "doing"),
      lastDecision: commandCenter.decisions[commandCenter.decisions.length - 1] ?? null,
      constraints: commandCenter.constraints,
      filesTouched: commandCenter.filesTouched.slice(-25),
      driftScore: commandCenter.driftScore,
    };
  }, [commandCenter]);

  return (
    <div className="vz-bundle">
      <p className="vz-bundle-summary">
        injected payload: {bundle.pendingSteps.length} pending step(s),{" "}
        {bundle.filesTouched.length} file(s), drift {bundle.driftScore}.
      </p>
      <pre className="vz-bundle-pre">{JSON.stringify(bundle, null, 2)}</pre>
    </div>
  );
}
