import * as vscode from "vscode";

/**
 * In-memory ring of the last 10 recall / blast / god_nodes queries
 * the user ran during this VS Code session. Not persisted to disk.
 *
 * Supports clicking an entry to re-run it.
 */

const MAX_QUERIES = 10;

export type QueryKind = "recall" | "blast" | "godnodes";

export interface RecentQuery {
  readonly kind: QueryKind;
  readonly input: string;
  readonly timestamp: number;
}

export class RecentQueriesProvider implements vscode.TreeDataProvider<QueryItem> {
  private readonly emitter = new vscode.EventEmitter<QueryItem | undefined>();
  public readonly onDidChangeTreeData = this.emitter.event;

  private queries: RecentQuery[] = [];

  public record(kind: QueryKind, input: string): void {
    const trimmed = input.trim();
    if (trimmed.length === 0) {
      return;
    }
    // Dedupe: if the last query matches, just bump its timestamp.
    const existing = this.queries.findIndex(
      (q) => q.kind === kind && q.input === trimmed,
    );
    if (existing >= 0) {
      this.queries.splice(existing, 1);
    }
    this.queries.unshift({ kind, input: trimmed, timestamp: Date.now() });
    while (this.queries.length > MAX_QUERIES) {
      this.queries.pop();
    }
    this.emitter.fire(undefined);
  }

  public clear(): void {
    this.queries = [];
    this.emitter.fire(undefined);
  }

  public getTreeItem(element: QueryItem): vscode.TreeItem {
    return element;
  }

  public getChildren(element?: QueryItem): QueryItem[] {
    if (element) {
      return [];
    }
    if (this.queries.length === 0) {
      return [];
    }
    return this.queries.map((q) => QueryItem.fromQuery(q));
  }
}

export class QueryItem extends vscode.TreeItem {
  public readonly query: RecentQuery | null;

  private constructor(label: string, query: RecentQuery | null) {
    super(label, vscode.TreeItemCollapsibleState.None);
    this.query = query;
  }

  public static fromQuery(q: RecentQuery): QueryItem {
    const item = new QueryItem(q.input, q);
    item.description = labelForKind(q.kind);
    item.iconPath = iconForKind(q.kind);
    item.contextValue = `mneme.recentQuery.${q.kind}`;
    item.command = {
      command: "mneme.rerunQuery",
      title: "Re-run query",
      arguments: [q],
    };
    return item;
  }
}

function labelForKind(kind: QueryKind): string {
  switch (kind) {
    case "recall":
      return "recall";
    case "blast":
      return "blast";
    case "godnodes":
      return "god nodes";
  }
}

function iconForKind(kind: QueryKind): vscode.ThemeIcon {
  switch (kind) {
    case "recall":
      return new vscode.ThemeIcon("search");
    case "blast":
      return new vscode.ThemeIcon("references");
    case "godnodes":
      return new vscode.ThemeIcon("graph");
  }
}
