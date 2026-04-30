import * as vscode from "vscode";

/**
 * Typed accessor for every setting the extension exposes.
 *
 * Rules:
 *   - Every setting has a sensible default matching package.json.
 *   - `getConfig()` always returns a fresh snapshot (call it inside handlers).
 *   - `onConfigChange()` wraps the event with a single affects-"mneme" guard.
 */

export type LogLevel = "error" | "warn" | "info" | "debug";
export type NotificationLevel = "off" | "errors-only" | "everything";

export interface MnemeConfig {
  readonly binaryPath: string;
  readonly autoRegisterMCP: boolean;
  readonly showStatusBar: boolean;
  readonly showCodeLens: boolean;
  readonly showHover: boolean;
  readonly showDrift: boolean;
  readonly driftPollInterval: number;
  readonly godNodeCount: number;
  readonly logLevel: LogLevel;
  readonly graphViewPort: number;
  readonly notificationLevel: NotificationLevel;
}

const SECTION = "mneme";

export function getConfig(): MnemeConfig {
  const cfg = vscode.workspace.getConfiguration(SECTION);
  return {
    binaryPath: cfg.get<string>("binaryPath", "mneme"),
    autoRegisterMCP: cfg.get<boolean>("autoRegisterMCP", true),
    showStatusBar: cfg.get<boolean>("showStatusBar", true),
    showCodeLens: cfg.get<boolean>("showCodeLens", true),
    showHover: cfg.get<boolean>("showHover", true),
    showDrift: cfg.get<boolean>("showDrift", true),
    driftPollInterval: clampInt(cfg.get<number>("driftPollInterval", 15), 3, 600),
    godNodeCount: clampInt(cfg.get<number>("godNodeCount", 10), 1, 200),
    logLevel: coerceLogLevel(cfg.get<string>("logLevel", "info")),
    graphViewPort: clampInt(cfg.get<number>("graphViewPort", 7777), 1, 65535),
    notificationLevel: coerceNotificationLevel(
      cfg.get<string>("notificationLevel", "everything"),
    ),
  };
}

export function onConfigChange(
  listener: (config: MnemeConfig) => void,
): vscode.Disposable {
  return vscode.workspace.onDidChangeConfiguration((event) => {
    if (!event.affectsConfiguration(SECTION)) {
      return;
    }
    listener(getConfig());
  });
}

function clampInt(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) {
    return min;
  }
  const rounded = Math.round(value);
  if (rounded < min) {
    return min;
  }
  if (rounded > max) {
    return max;
  }
  return rounded;
}

function coerceLogLevel(value: string): LogLevel {
  switch (value) {
    case "error":
    case "warn":
    case "info":
    case "debug":
      return value;
    default:
      return "info";
  }
}

function coerceNotificationLevel(value: string): NotificationLevel {
  switch (value) {
    case "off":
    case "errors-only":
    case "everything":
      return value;
    default:
      return "everything";
  }
}

/**
 * Small wrapper around `vscode.window.showInformationMessage` etc. that
 * respects `mneme.notificationLevel`. Returns undefined if suppressed.
 */
export function notify(
  kind: "info" | "warn" | "error",
  message: string,
  ...items: string[]
): Thenable<string | undefined> {
  const level = getConfig().notificationLevel;
  if (level === "off") {
    return Promise.resolve(undefined);
  }
  if (level === "errors-only" && kind !== "error") {
    return Promise.resolve(undefined);
  }
  switch (kind) {
    case "info":
      return vscode.window.showInformationMessage(message, ...items);
    case "warn":
      return vscode.window.showWarningMessage(message, ...items);
    case "error":
      return vscode.window.showErrorMessage(message, ...items);
  }
}

export function logAt(
  channel: vscode.OutputChannel | null,
  level: LogLevel,
  message: string,
): void {
  if (!channel) {
    return;
  }
  const configured = getConfig().logLevel;
  if (!shouldLog(configured, level)) {
    return;
  }
  const stamp = new Date().toISOString();
  channel.appendLine(`[${stamp}] [${level}] ${message}`);
}

function shouldLog(configured: LogLevel, requested: LogLevel): boolean {
  const order: LogLevel[] = ["error", "warn", "info", "debug"];
  return order.indexOf(requested) <= order.indexOf(configured);
}
