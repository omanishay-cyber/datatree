import * as vscode from "vscode";
import { runMneme } from "../mneme";
import { parseGodNodes, type GodNode } from "../util/parse";
import { getConfig, logAt } from "../util/config";

/**
 * Tree view for the top-K most-connected concepts in the active project.
 *
 * Populated by `mneme god-nodes --top N --format tsv`. Refreshed on
 * graph.updated livebus events and on demand via the refresh command.
 */

export class GodNodesProvider implements vscode.TreeDataProvider<GodNodeItem> {
  private readonly emitter = new vscode.EventEmitter<GodNodeItem | undefined>();
  public readonly onDidChangeTreeData = this.emitter.event;

  private nodes: GodNode[] = [];
  private loading = false;
  private errorMessage: string | null = null;

  public constructor(private readonly channel: vscode.OutputChannel | null) {}

  public async refresh(): Promise<void> {
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (!folder) {
      this.nodes = [];
      this.errorMessage = null;
      this.emitter.fire(undefined);
      return;
    }
    this.loading = true;
    this.errorMessage = null;
    this.emitter.fire(undefined);

    const topN = getConfig().godNodeCount;
    try {
      const result = await runMneme(
        ["god-nodes", "--top", String(topN), "--format", "tsv", "--project", folder.uri.fsPath],
        this.channel,
        { quiet: true, cwd: folder.uri.fsPath },
      );
      this.nodes = parseGodNodes(result.stdout);
      this.errorMessage = null;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      logAt(this.channel, "warn", `god-nodes refresh failed: ${message}`);
      this.errorMessage = message;
      this.nodes = [];
    } finally {
      this.loading = false;
      this.emitter.fire(undefined);
    }
  }

  public getTreeItem(element: GodNodeItem): vscode.TreeItem {
    return element;
  }

  public getChildren(element?: GodNodeItem): GodNodeItem[] {
    if (element) {
      return [];
    }
    if (this.loading) {
      return [GodNodeItem.placeholder("Loading god nodes...", "loading~spin")];
    }
    if (this.errorMessage) {
      return [GodNodeItem.placeholder(this.errorMessage, "warning")];
    }
    if (this.nodes.length === 0) {
      return [];
    }
    return this.nodes.map((node, idx) => GodNodeItem.fromNode(node, idx));
  }
}

export class GodNodeItem extends vscode.TreeItem {
  public readonly node: GodNode | null;

  private constructor(label: string, node: GodNode | null) {
    super(label, vscode.TreeItemCollapsibleState.None);
    this.node = node;
  }

  public static fromNode(node: GodNode, rank: number): GodNodeItem {
    const item = new GodNodeItem(`${rank + 1}. ${node.name}`, node);
    item.description = `${node.kind} | deg ${node.degree}`;
    item.tooltip = new vscode.MarkdownString(
      [
        `**${node.name}** (${node.kind})`,
        "",
        `- Degree: ${node.degree}`,
        node.file ? `- File: \`${node.file}\`` : "",
        "",
        "Click to open the concept graph.",
      ]
        .filter((l) => l.length > 0)
        .join("\n"),
    );
    item.iconPath = iconForKind(node.kind);
    item.contextValue = "mneme.godNode";
    item.command = {
      command: "mneme.openGraphForNode",
      title: "Open graph",
      arguments: [node],
    };
    return item;
  }

  public static placeholder(text: string, codicon: string): GodNodeItem {
    const item = new GodNodeItem(text, null);
    item.iconPath = new vscode.ThemeIcon(codicon);
    item.contextValue = "mneme.placeholder";
    return item;
  }
}

function iconForKind(kind: string): vscode.ThemeIcon {
  const normalised = kind.toLowerCase();
  if (normalised.includes("fn") || normalised.includes("function")) {
    return new vscode.ThemeIcon("symbol-method");
  }
  if (normalised.includes("struct") || normalised.includes("class")) {
    return new vscode.ThemeIcon("symbol-class");
  }
  if (normalised.includes("trait") || normalised.includes("interface")) {
    return new vscode.ThemeIcon("symbol-interface");
  }
  if (normalised.includes("mod") || normalised.includes("module")) {
    return new vscode.ThemeIcon("symbol-namespace");
  }
  if (normalised.includes("enum")) {
    return new vscode.ThemeIcon("symbol-enum");
  }
  return new vscode.ThemeIcon("symbol-constant");
}
