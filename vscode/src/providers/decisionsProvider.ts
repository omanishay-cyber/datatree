import * as vscode from "vscode";
import { runMneme } from "../mneme";
import { parseDecisions, type DecisionEntry, humanAge } from "../util/parse";
import { logAt } from "../util/config";

/**
 * Shows the most recent 20 entries from the decision ledger
 * (`mneme recall --kind=decision --limit 20`). Clicking an entry opens
 * the linked transcript in the editor.
 */

const LIMIT = 20;

export class DecisionsProvider implements vscode.TreeDataProvider<DecisionItem> {
  private readonly emitter = new vscode.EventEmitter<DecisionItem | undefined>();
  public readonly onDidChangeTreeData = this.emitter.event;

  private entries: DecisionEntry[] = [];
  private loading = false;
  private errorMessage: string | null = null;

  public constructor(private readonly channel: vscode.OutputChannel | null) {}

  public async refresh(): Promise<void> {
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (!folder) {
      this.entries = [];
      this.emitter.fire(undefined);
      return;
    }
    this.loading = true;
    this.errorMessage = null;
    this.emitter.fire(undefined);

    try {
      const result = await runMneme(
        [
          "recall",
          "--kind",
          "decision",
          "--limit",
          String(LIMIT),
          "--format",
          "tsv",
          "--project",
          folder.uri.fsPath,
        ],
        this.channel,
        { quiet: true, cwd: folder.uri.fsPath },
      );
      this.entries = parseDecisions(result.stdout);
      this.errorMessage = null;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      logAt(this.channel, "warn", `decisions refresh failed: ${message}`);
      this.errorMessage = message;
      this.entries = [];
    } finally {
      this.loading = false;
      this.emitter.fire(undefined);
    }
  }

  public getTreeItem(element: DecisionItem): vscode.TreeItem {
    return element;
  }

  public getChildren(element?: DecisionItem): DecisionItem[] {
    if (element) {
      return [];
    }
    if (this.loading) {
      return [DecisionItem.placeholder("Loading decisions...", "loading~spin")];
    }
    if (this.errorMessage) {
      return [DecisionItem.placeholder(this.errorMessage, "warning")];
    }
    if (this.entries.length === 0) {
      return [];
    }
    return this.entries.map((e) => DecisionItem.fromEntry(e));
  }
}

export class DecisionItem extends vscode.TreeItem {
  public readonly entry: DecisionEntry | null;

  private constructor(label: string, entry: DecisionEntry | null) {
    super(label, vscode.TreeItemCollapsibleState.None);
    this.entry = entry;
  }

  public static fromEntry(entry: DecisionEntry): DecisionItem {
    const item = new DecisionItem(entry.summary, entry);
    item.description = humanAge(entry.timestamp);
    item.iconPath = new vscode.ThemeIcon("lightbulb");
    item.contextValue = "mneme.decision";
    item.tooltip = new vscode.MarkdownString(
      [
        `**Decision** (${humanAge(entry.timestamp)})`,
        "",
        entry.summary,
        "",
        entry.transcriptPath ? `Transcript: \`${entry.transcriptPath}\`` : "(no transcript)",
      ].join("\n"),
    );
    item.command = {
      command: "mneme.openDecision",
      title: "Open decision transcript",
      arguments: [entry],
    };
    return item;
  }

  public static placeholder(text: string, codicon: string): DecisionItem {
    const item = new DecisionItem(text, null);
    item.iconPath = new vscode.ThemeIcon(codicon);
    item.contextValue = "mneme.placeholder";
    return item;
  }
}
