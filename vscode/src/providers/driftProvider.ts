import * as vscode from "vscode";
import { runMneme } from "../mneme";
import { parseDrift, type DriftFinding, type DriftSeverity } from "../util/parse";
import { getConfig, logAt, notify } from "../util/config";

/**
 * Drift findings appear in three places simultaneously:
 *   - the tree view (grouped by severity)
 *   - the Problems panel (via a DiagnosticCollection)
 *   - as inline squiggles in editors
 *
 * The provider owns all three. It polls `mneme audit --format tsv` on an
 * interval (default 15s) and on every save. External callers can also
 * invoke `refresh()` e.g. from a livebus drift.finding event.
 */

type Node = SeverityGroup | FindingNode | PlaceholderNode;

export class DriftProvider implements vscode.TreeDataProvider<Node>, vscode.Disposable {
  private readonly emitter = new vscode.EventEmitter<Node | undefined>();
  public readonly onDidChangeTreeData = this.emitter.event;

  private readonly diagnostics: vscode.DiagnosticCollection;
  private readonly disposables: vscode.Disposable[] = [];

  private findings: DriftFinding[] = [];
  private pollTimer: NodeJS.Timeout | null = null;
  private loading = false;
  private errorMessage: string | null = null;
  private disposed = false;
  private lastCriticalKeys = new Set<string>();

  public constructor(private readonly channel: vscode.OutputChannel | null) {
    this.diagnostics = vscode.languages.createDiagnosticCollection("mneme-drift");

    // Refresh on save; cheap, and keeps diagnostics current.
    this.disposables.push(
      vscode.workspace.onDidSaveTextDocument(() => {
        void this.refresh();
      }),
    );

    this.schedulePoll();
  }

  public dispose(): void {
    this.disposed = true;
    if (this.pollTimer) {
      clearTimeout(this.pollTimer);
      this.pollTimer = null;
    }
    this.diagnostics.clear();
    this.diagnostics.dispose();
    for (const d of this.disposables) {
      d.dispose();
    }
    this.emitter.dispose();
  }

  public async refresh(): Promise<void> {
    if (!getConfig().showDrift) {
      this.findings = [];
      this.diagnostics.clear();
      this.emitter.fire(undefined);
      return;
    }
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (!folder) {
      this.findings = [];
      this.diagnostics.clear();
      this.emitter.fire(undefined);
      return;
    }
    this.loading = true;
    this.errorMessage = null;
    this.emitter.fire(undefined);

    try {
      const result = await runMneme(
        ["audit", "--format", "tsv", "--project", folder.uri.fsPath],
        this.channel,
        { quiet: true, cwd: folder.uri.fsPath },
      );
      this.findings = parseDrift(result.stdout);
      this.updateDiagnostics(folder.uri.fsPath);
      this.fireCriticalToasts();
      this.errorMessage = null;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      logAt(this.channel, "warn", `drift refresh failed: ${message}`);
      this.errorMessage = message;
    } finally {
      this.loading = false;
      this.emitter.fire(undefined);
    }
  }

  public getFindings(): ReadonlyArray<DriftFinding> {
    return this.findings;
  }

  public getFindingsForFile(fsPath: string): DriftFinding[] {
    const normalised = fsPath.replace(/\\/g, "/").toLowerCase();
    return this.findings.filter((f) => {
      const file = f.file.replace(/\\/g, "/").toLowerCase();
      return file.endsWith(normalised) || normalised.endsWith(file);
    });
  }

  public getTreeItem(element: Node): vscode.TreeItem {
    return element;
  }

  public getChildren(element?: Node): Node[] {
    if (!element) {
      if (this.loading) {
        return [placeholder("Scanning for drift...", "loading~spin")];
      }
      if (this.errorMessage) {
        return [placeholder(this.errorMessage, "warning")];
      }
      if (this.findings.length === 0) {
        return [];
      }
      const buckets: DriftSeverity[] = ["critical", "should-fix", "info"];
      return buckets
        .map((sev) => {
          const count = this.findings.filter((f) => f.severity === sev).length;
          return count > 0 ? new SeverityGroup(sev, count) : null;
        })
        .filter((x): x is SeverityGroup => x !== null);
    }
    if (element instanceof SeverityGroup) {
      return this.findings
        .filter((f) => f.severity === element.severity)
        .map((f) => new FindingNode(f));
    }
    return [];
  }

  private schedulePoll(): void {
    if (this.disposed) {
      return;
    }
    const intervalSec = getConfig().driftPollInterval;
    this.pollTimer = setTimeout(async () => {
      await this.refresh();
      this.schedulePoll();
    }, intervalSec * 1_000);
  }

  private updateDiagnostics(projectRoot: string): void {
    this.diagnostics.clear();
    if (!getConfig().showDrift) {
      return;
    }
    const byFile = new Map<string, vscode.Diagnostic[]>();
    for (const finding of this.findings) {
      const uri = resolveFindingUri(finding.file, projectRoot);
      const diag = toDiagnostic(finding);
      const key = uri.toString();
      if (!byFile.has(key)) {
        byFile.set(key, []);
      }
      byFile.get(key)!.push(diag);
    }
    for (const [uriStr, diags] of byFile.entries()) {
      this.diagnostics.set(vscode.Uri.parse(uriStr), diags);
    }
  }

  private fireCriticalToasts(): void {
    const level = getConfig().notificationLevel;
    if (level === "off") {
      this.lastCriticalKeys = new Set();
      return;
    }
    const newKeys = new Set<string>();
    const newCriticals: DriftFinding[] = [];
    for (const f of this.findings) {
      if (f.severity !== "critical") {
        continue;
      }
      const key = `${f.scanner}|${f.file}:${f.line}|${f.message}`;
      newKeys.add(key);
      if (!this.lastCriticalKeys.has(key)) {
        newCriticals.push(f);
      }
    }
    this.lastCriticalKeys = newKeys;

    if (newCriticals.length > 0) {
      const first = newCriticals[0];
      const extra = newCriticals.length > 1 ? ` (+${newCriticals.length - 1} more)` : "";
      void notify(
        "error",
        `Critical drift (${first.scanner}): ${first.message}${extra}`,
        "Show",
      ).then((pick) => {
        if (pick === "Show") {
          void vscode.commands.executeCommand("mneme.drift.focus");
        }
      });
    }
  }
}

class SeverityGroup extends vscode.TreeItem {
  public constructor(
    public readonly severity: DriftSeverity,
    public readonly count: number,
  ) {
    super(`${severityLabel(severity)} (${count})`, vscode.TreeItemCollapsibleState.Expanded);
    this.iconPath = iconForSeverity(severity);
    this.contextValue = "mneme.driftGroup";
  }
}

class FindingNode extends vscode.TreeItem {
  public constructor(public readonly finding: DriftFinding) {
    super(finding.message, vscode.TreeItemCollapsibleState.None);
    this.description = `${finding.scanner} | ${finding.file}:${finding.line}`;
    this.tooltip = new vscode.MarkdownString(
      [
        `**${severityLabel(finding.severity)}** · ${finding.scanner}`,
        "",
        finding.message,
        "",
        `\`${finding.file}:${finding.line}\``,
      ].join("\n"),
    );
    this.iconPath = iconForSeverity(finding.severity);
    this.contextValue = "mneme.driftFinding";
    this.command = {
      command: "mneme.openDriftFinding",
      title: "Open finding",
      arguments: [finding],
    };
  }
}

class PlaceholderNode extends vscode.TreeItem {
  public constructor(label: string, codicon: string) {
    super(label, vscode.TreeItemCollapsibleState.None);
    this.iconPath = new vscode.ThemeIcon(codicon);
    this.contextValue = "mneme.placeholder";
  }
}

function placeholder(label: string, codicon: string): PlaceholderNode {
  return new PlaceholderNode(label, codicon);
}

function toDiagnostic(f: DriftFinding): vscode.Diagnostic {
  const line = Math.max(0, f.line - 1);
  const range = new vscode.Range(line, 0, line, 256);
  const diag = new vscode.Diagnostic(range, f.message, toSeverity(f.severity));
  diag.source = "mneme";
  diag.code = f.scanner;
  return diag;
}

function toSeverity(sev: DriftSeverity): vscode.DiagnosticSeverity {
  switch (sev) {
    case "critical":
      return vscode.DiagnosticSeverity.Error;
    case "should-fix":
      return vscode.DiagnosticSeverity.Warning;
    case "info":
      return vscode.DiagnosticSeverity.Information;
  }
}

function severityLabel(sev: DriftSeverity): string {
  switch (sev) {
    case "critical":
      return "Critical";
    case "should-fix":
      return "Should fix";
    case "info":
      return "Info";
  }
}

function iconForSeverity(sev: DriftSeverity): vscode.ThemeIcon {
  switch (sev) {
    case "critical":
      return new vscode.ThemeIcon("error", new vscode.ThemeColor("charts.red"));
    case "should-fix":
      return new vscode.ThemeIcon("warning", new vscode.ThemeColor("charts.yellow"));
    case "info":
      return new vscode.ThemeIcon("info", new vscode.ThemeColor("charts.blue"));
  }
}

function resolveFindingUri(file: string, projectRoot: string): vscode.Uri {
  if (/^[a-zA-Z]:[\\/]/.test(file) || file.startsWith("/")) {
    return vscode.Uri.file(file);
  }
  return vscode.Uri.file(`${projectRoot}/${file}`);
}
