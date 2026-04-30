import type { CommandCenterStep } from "../store";

interface StepLedgerProps {
  steps: CommandCenterStep[];
}

const STATUS_LABEL: Record<CommandCenterStep["status"], string> = {
  todo: "todo",
  doing: "doing",
  done: "done",
  skipped: "skipped",
};

export function StepLedger({ steps }: StepLedgerProps): JSX.Element {
  if (steps.length === 0) {
    return <p className="vz-cc-empty">no steps logged</p>;
  }
  return (
    <ol className="vz-step-ledger">
      {steps.map((step) => {
        if (step.isCompactionMarker) {
          return (
            <li key={step.id} className="vz-step vz-step--compaction">
              <span className="vz-step-marker" aria-hidden="true" />
              <div className="vz-step-body">
                <strong>compaction event</strong>
                <span>{step.description}</span>
                <time>{new Date(step.ts).toLocaleString()}</time>
              </div>
            </li>
          );
        }
        return (
          <li key={step.id} className={`vz-step vz-step--${step.status}`}>
            <input
              type="checkbox"
              readOnly
              checked={step.status === "done"}
              aria-label={`step ${step.id} ${step.status}`}
            />
            <div className="vz-step-body">
              <span className="vz-step-desc">{step.description}</span>
              <span className={`vz-badge vz-badge--${step.status}`}>{STATUS_LABEL[step.status]}</span>
              {step.files && step.files.length > 0 && (
                <span className="vz-step-files">
                  {step.files.slice(0, 3).map((f) => (
                    <code key={f}>{f}</code>
                  ))}
                  {step.files.length > 3 && <em>+{step.files.length - 3} more</em>}
                </span>
              )}
              <time>{new Date(step.ts).toLocaleTimeString()}</time>
            </div>
          </li>
        );
      })}
    </ol>
  );
}
