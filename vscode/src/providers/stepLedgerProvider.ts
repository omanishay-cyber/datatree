import * as vscode from "vscode";
import { runMneme } from "../mneme";
import { parseSteps, type StepEntry, type StepStatus } from "../util/parse";
import { logAt } from "../util/config";

/**
 * Shows the project's current numbered step ledger, surfaced from
 * `mneme step status --format tsv`. Steps are tracked as one of four
 * statuses: pending / done / verified / blocked.
 */

export class StepLedgerProvider implements vscode.TreeDataProvider<StepItem> {
  private readonly emitter = new vscode.EventEmitter<StepItem | undefined>();
  public readonly onDidChangeTreeData = this.emitter.event;

  private steps: StepEntry[] = [];
  private loading = false;
  private errorMessage: string | null = null;

  public constructor(private readonly channel: vscode.OutputChannel | null) {}

  public async refresh(): Promise<void> {
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (!folder) {
      this.steps = [];
      this.emitter.fire(undefined);
      return;
    }
    this.loading = true;
    this.errorMessage = null;
    this.emitter.fire(undefined);

    try {
      const result = await runMneme(
        ["step", "status", "--format", "tsv", "--project", folder.uri.fsPath],
        this.channel,
        { quiet: true, cwd: folder.uri.fsPath },
      );
      this.steps = parseSteps(result.stdout);
      this.errorMessage = null;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      logAt(this.channel, "warn", `step ledger refresh failed: ${message}`);
      this.errorMessage = message;
      this.steps = [];
    } finally {
      this.loading = false;
      this.emitter.fire(undefined);
    }
  }

  public getTreeItem(element: StepItem): vscode.TreeItem {
    return element;
  }

  public getChildren(element?: StepItem): StepItem[] {
    if (element) {
      return [];
    }
    if (this.loading) {
      return [StepItem.placeholder("Loading step ledger...", "loading~spin")];
    }
    if (this.errorMessage) {
      return [StepItem.placeholder(this.errorMessage, "warning")];
    }
    if (this.steps.length === 0) {
      return [];
    }
    return this.steps.map((s) => StepItem.fromStep(s));
  }
}

export class StepItem extends vscode.TreeItem {
  public readonly step: StepEntry | null;

  private constructor(label: string, step: StepEntry | null) {
    super(label, vscode.TreeItemCollapsibleState.None);
    this.step = step;
  }

  public static fromStep(step: StepEntry): StepItem {
    const item = new StepItem(`${step.id}. ${step.title}`, step);
    item.description = statusLabel(step.status);
    item.iconPath = iconFor(step.status);
    item.contextValue = `mneme.step.${step.status}`;
    item.tooltip = new vscode.MarkdownString(
      [
        `**Step ${step.id}** | ${statusLabel(step.status)}`,
        "",
        step.title,
      ].join("\n"),
    );
    item.command = {
      command: "mneme.showStepDetails",
      title: "Show step details",
      arguments: [step],
    };
    return item;
  }

  public static placeholder(text: string, codicon: string): StepItem {
    const item = new StepItem(text, null);
    item.iconPath = new vscode.ThemeIcon(codicon);
    item.contextValue = "mneme.placeholder";
    return item;
  }
}

function statusLabel(s: StepStatus): string {
  switch (s) {
    case "pending":
      return "pending";
    case "done":
      return "done";
    case "verified":
      return "verified";
    case "blocked":
      return "blocked";
  }
}

function iconFor(s: StepStatus): vscode.ThemeIcon {
  switch (s) {
    case "pending":
      return new vscode.ThemeIcon("circle-large-outline");
    case "done":
      return new vscode.ThemeIcon("check");
    case "verified":
      return new vscode.ThemeIcon("verified", new vscode.ThemeColor("charts.green"));
    case "blocked":
      return new vscode.ThemeIcon("error", new vscode.ThemeColor("charts.red"));
  }
}
