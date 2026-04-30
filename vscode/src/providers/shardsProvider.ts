import * as vscode from "vscode";
import { runMneme } from "../mneme";
import { parseShards, type ShardEntry, humanBytes, humanAge } from "../util/parse";
import { logAt } from "../util/config";

/**
 * Shows all indexed projects as listed by `mneme projects list --format tsv`.
 *
 * Clicking a shard reveals its path in an integrated terminal so the user
 * can poke around. Right-click actions (handled via `contextValue` +
 * menus.view.item.context) include rebuild, forget, and open-folder.
 */

export class ShardsProvider implements vscode.TreeDataProvider<ShardItem> {
  private readonly emitter = new vscode.EventEmitter<ShardItem | undefined>();
  public readonly onDidChangeTreeData = this.emitter.event;

  private entries: ShardEntry[] = [];
  private loading = false;
  private errorMessage: string | null = null;

  public constructor(private readonly channel: vscode.OutputChannel | null) {}

  public async refresh(): Promise<void> {
    this.loading = true;
    this.errorMessage = null;
    this.emitter.fire(undefined);

    try {
      const result = await runMneme(
        ["projects", "list", "--format", "tsv"],
        this.channel,
        { quiet: true },
      );
      this.entries = parseShards(result.stdout);
      this.errorMessage = null;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      logAt(this.channel, "warn", `shards refresh failed: ${message}`);
      this.errorMessage = message;
      this.entries = [];
    } finally {
      this.loading = false;
      this.emitter.fire(undefined);
    }
  }

  public getTreeItem(element: ShardItem): vscode.TreeItem {
    return element;
  }

  public getChildren(element?: ShardItem): ShardItem[] {
    if (element) {
      return [];
    }
    if (this.loading) {
      return [ShardItem.placeholder("Loading shards...", "loading~spin")];
    }
    if (this.errorMessage) {
      return [ShardItem.placeholder(this.errorMessage, "warning")];
    }
    if (this.entries.length === 0) {
      return [];
    }
    return this.entries.map((e) => ShardItem.fromEntry(e));
  }
}

export class ShardItem extends vscode.TreeItem {
  public readonly entry: ShardEntry | null;

  private constructor(label: string, entry: ShardEntry | null) {
    super(label, vscode.TreeItemCollapsibleState.None);
    this.entry = entry;
  }

  public static fromEntry(e: ShardEntry): ShardItem {
    const item = new ShardItem(e.name, e);
    item.description = `${humanBytes(e.sizeBytes)} | ${humanAge(e.lastBuiltIso)}`;
    item.iconPath = new vscode.ThemeIcon("database");
    item.contextValue = "mneme.shard";
    item.tooltip = new vscode.MarkdownString(
      [
        `**${e.name}**`,
        "",
        `- Path: \`${e.path}\``,
        `- Size: ${humanBytes(e.sizeBytes)}`,
        `- Last built: ${humanAge(e.lastBuiltIso)}`,
      ].join("\n"),
    );
    item.command = {
      command: "mneme.revealShard",
      title: "Reveal shard path",
      arguments: [e],
    };
    return item;
  }

  public static placeholder(text: string, codicon: string): ShardItem {
    const item = new ShardItem(text, null);
    item.iconPath = new vscode.ThemeIcon(codicon);
    item.contextValue = "mneme.placeholder";
    return item;
  }
}
