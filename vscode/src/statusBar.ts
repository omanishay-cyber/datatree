import * as vscode from "vscode";
import { runMneme } from "./mneme";

const POLL_INTERVAL_MS = 30_000;

/**
 * Creates the Mneme status bar item and starts the daemon health poll.
 *
 * Returns a Disposable that disposes both the StatusBarItem and the polling
 * timer. The caller should push it onto `context.subscriptions`.
 */
export function startStatusBar(channel: vscode.OutputChannel): vscode.Disposable {
  const item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  item.command = "mneme.doctor";
  item.text = "$(sync~spin) mneme";
  item.tooltip = "Checking mneme daemon status...";
  item.show();

  let disposed = false;

  const poll = async (): Promise<void> => {
    if (disposed) {
      return;
    }
    try {
      const result = await runMneme(["daemon", "status"], channel, { quiet: true });
      if (disposed) {
        return;
      }
      item.text = "$(check) mneme";
      item.tooltip = buildTooltip("daemon up", result.stdout);
    } catch (err) {
      if (disposed) {
        return;
      }
      const stderr =
        err && typeof err === "object" && "stderr" in err
          ? String((err as { stderr?: unknown }).stderr ?? "")
          : err instanceof Error
            ? err.message
            : String(err);
      item.text = "$(error) mneme down";
      item.tooltip = buildTooltip("daemon down", stderr);
    }
  };

  // Kick off the first poll immediately, then on interval.
  void poll();
  const timer = setInterval(() => {
    void poll();
  }, POLL_INTERVAL_MS);

  return new vscode.Disposable(() => {
    disposed = true;
    clearInterval(timer);
    item.dispose();
  });
}

/**
 * Builds the status bar tooltip from a header line plus the last 3 lines of
 * stdout / stderr from the most recent poll.
 */
function buildTooltip(header: string, output: string): string {
  const lines = output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  const tail = lines.slice(-3);
  if (tail.length === 0) {
    return header;
  }
  return `${header}\n${tail.join("\n")}`;
}
