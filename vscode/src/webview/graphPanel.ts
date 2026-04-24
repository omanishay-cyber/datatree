import * as vscode from "vscode";
import * as fs from "node:fs";
import * as path from "node:path";
import { runMneme } from "../mneme";
import { parseGodNodes, parseBlast, type GodNode } from "../util/parse";
import { getConfig, logAt } from "../util/config";

/**
 * The graph WebviewPanel lifecycle manager.
 *
 * Two render paths:
 *   1) If `mneme-livebus` is up, embed an iframe pointing at
 *      http://127.0.0.1:<port>/vision. That gives us the full Vision
 *      experience inside VS Code.
 *   2) Otherwise, fetch a snapshot from `mneme god-nodes` and render a
 *      d3-force graph in the webview, no network needed. The fallback
 *      lives inline in graphView.html / graphView.js.
 *
 * The panel is a singleton. Opening it when it already exists reveals
 * the existing one. We persist last-known view state via
 * webviewPanel.webview.state so restarts feel seamless.
 */

interface PersistedState {
  readonly mode: "iframe" | "fallback";
  readonly selectedNode: string | null;
  readonly zoom: number;
}

const VIEW_TYPE = "mneme.graphPanel";

export class GraphPanel {
  private static current: GraphPanel | null = null;
  private readonly panel: vscode.WebviewPanel;
  private readonly extensionUri: vscode.Uri;
  private readonly channel: vscode.OutputChannel | null;
  private readonly disposables: vscode.Disposable[] = [];

  private constructor(
    panel: vscode.WebviewPanel,
    extensionUri: vscode.Uri,
    channel: vscode.OutputChannel | null,
  ) {
    this.panel = panel;
    this.extensionUri = extensionUri;
    this.channel = channel;

    this.panel.onDidDispose(() => this.dispose(), null, this.disposables);

    this.panel.webview.onDidReceiveMessage(
      (msg: unknown) => this.handleMessage(msg),
      null,
      this.disposables,
    );

    // Re-render when the user flips themes so the colors match.
    vscode.window.onDidChangeActiveColorTheme(
      () => void this.render(),
      null,
      this.disposables,
    );

    void this.render();
  }

  public static showOrReveal(
    extensionUri: vscode.Uri,
    channel: vscode.OutputChannel | null,
  ): GraphPanel {
    const column = vscode.window.activeTextEditor?.viewColumn ?? vscode.ViewColumn.One;
    if (GraphPanel.current) {
      GraphPanel.current.panel.reveal(column);
      return GraphPanel.current;
    }
    const panel = vscode.window.createWebviewPanel(
      VIEW_TYPE,
      "Mneme Graph",
      column,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [vscode.Uri.joinPath(extensionUri, "src", "webview")],
      },
    );
    panel.iconPath = new vscode.ThemeIcon("graph");
    GraphPanel.current = new GraphPanel(panel, extensionUri, channel);
    return GraphPanel.current;
  }

  public async focusNode(node: GodNode): Promise<void> {
    await this.render();
    await this.panel.webview.postMessage({
      type: "focusNode",
      name: node.name,
    });
  }

  public dispose(): void {
    GraphPanel.current = null;
    this.panel.dispose();
    while (this.disposables.length > 0) {
      const next = this.disposables.pop();
      if (next) {
        next.dispose();
      }
    }
  }

  private async render(): Promise<void> {
    const html = await this.buildHtml();
    this.panel.webview.html = html;
  }

  private async buildHtml(): Promise<string> {
    const port = getConfig().graphViewPort;
    const visionUrl = `http://127.0.0.1:${port}/vision`;
    const nodesSnapshot = await this.snapshotForFallback();

    const cssUri = this.panel.webview.asWebviewUri(
      vscode.Uri.joinPath(this.extensionUri, "src", "webview", "graphView.css"),
    );
    const jsUri = this.panel.webview.asWebviewUri(
      vscode.Uri.joinPath(this.extensionUri, "src", "webview", "graphView.js"),
    );

    const nonce = makeNonce();
    const csp = [
      `default-src 'none'`,
      `img-src ${this.panel.webview.cspSource} data:`,
      `style-src ${this.panel.webview.cspSource} 'unsafe-inline'`,
      `script-src 'nonce-${nonce}' ${this.panel.webview.cspSource}`,
      `frame-src http://127.0.0.1:* http://localhost:*`,
      `connect-src http://127.0.0.1:* http://localhost:*`,
    ].join("; ");

    const template = await this.readTemplate();
    return template
      .replace(/{{CSP}}/g, csp)
      .replace(/{{NONCE}}/g, nonce)
      .replace(/{{CSS_URI}}/g, cssUri.toString())
      .replace(/{{JS_URI}}/g, jsUri.toString())
      .replace(/{{VISION_URL}}/g, escapeHtmlAttr(visionUrl))
      .replace(
        /{{SNAPSHOT_JSON}}/g,
        escapeHtmlAttr(JSON.stringify(nodesSnapshot)),
      );
  }

  private async readTemplate(): Promise<string> {
    const filePath = path.join(
      this.extensionUri.fsPath,
      "src",
      "webview",
      "graphView.html",
    );
    try {
      return await fs.promises.readFile(filePath, "utf8");
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      logAt(this.channel, "warn", `failed to read graph template: ${message}`);
      return "<html><body><h2>mneme graph template missing</h2></body></html>";
    }
  }

  private async snapshotForFallback(): Promise<{
    readonly nodes: ReadonlyArray<{ readonly id: string; readonly kind: string; readonly degree: number; readonly file: string | null }>;
    readonly edges: ReadonlyArray<{ readonly source: string; readonly target: string }>;
  }> {
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (!folder) {
      return { nodes: [], edges: [] };
    }
    try {
      const gn = await runMneme(
        [
          "god-nodes",
          "--top",
          String(getConfig().godNodeCount),
          "--format",
          "tsv",
          "--project",
          folder.uri.fsPath,
        ],
        this.channel,
        { quiet: true, cwd: folder.uri.fsPath },
      );
      const nodes = parseGodNodes(gn.stdout).map((n) => ({
        id: n.name,
        kind: n.kind,
        degree: n.degree,
        file: n.file,
      }));
      const edges: Array<{ source: string; target: string }> = [];
      // Walk the top 3 nodes and pull their blast neighbours for context.
      for (const top of nodes.slice(0, 3)) {
        try {
          const blast = await runMneme(
            [
              "blast",
              top.id,
              "--project",
              folder.uri.fsPath,
              "--format",
              "tsv",
              "--limit",
              "8",
            ],
            this.channel,
            { quiet: true, cwd: folder.uri.fsPath },
          );
          const parsed = parseBlast(blast.stdout);
          for (const site of parsed.sites) {
            edges.push({ source: top.id, target: site.symbol });
          }
        } catch {
          // Skip; edges are decorative.
        }
      }
      return { nodes, edges };
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      logAt(this.channel, "warn", `graph snapshot failed: ${message}`);
      return { nodes: [], edges: [] };
    }
  }

  private handleMessage(msg: unknown): void {
    if (!msg || typeof msg !== "object") {
      return;
    }
    const payload = msg as { type?: string; [k: string]: unknown };
    switch (payload.type) {
      case "openFile": {
        const file = typeof payload.file === "string" ? payload.file : "";
        const line = typeof payload.line === "number" ? payload.line : 1;
        if (file.length > 0) {
          void this.openFile(file, line);
        }
        return;
      }
      case "persistState": {
        const state = payload.state;
        if (state && typeof state === "object") {
          // We don't persist anywhere beyond the webview, but log.
          logAt(
            this.channel,
            "debug",
            `graph persist state: ${JSON.stringify(state as PersistedState)}`,
          );
        }
        return;
      }
      case "requestRefresh":
        void this.render();
        return;
      default:
        return;
    }
  }

  private async openFile(file: string, line: number): Promise<void> {
    const folder = vscode.workspace.workspaceFolders?.[0];
    const uri = resolveFileUri(file, folder?.uri.fsPath);
    try {
      const doc = await vscode.workspace.openTextDocument(uri);
      const editor = await vscode.window.showTextDocument(doc, {
        preview: false,
        viewColumn: vscode.ViewColumn.One,
      });
      const target = Math.max(0, line - 1);
      const position = new vscode.Position(target, 0);
      editor.selection = new vscode.Selection(position, position);
      editor.revealRange(
        new vscode.Range(position, position),
        vscode.TextEditorRevealType.InCenter,
      );
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      void vscode.window.showErrorMessage(`Could not open ${file}: ${message}`);
    }
  }
}

function makeNonce(): string {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let out = "";
  for (let i = 0; i < 32; i++) {
    out += alphabet.charAt(Math.floor(Math.random() * alphabet.length));
  }
  return out;
}

function escapeHtmlAttr(input: string): string {
  return input
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function resolveFileUri(file: string, workspaceRoot: string | undefined): vscode.Uri {
  if (/^[a-zA-Z]:[\\/]/.test(file) || file.startsWith("/") || file.startsWith("\\")) {
    return vscode.Uri.file(file);
  }
  if (workspaceRoot) {
    return vscode.Uri.file(path.join(workspaceRoot, file));
  }
  return vscode.Uri.file(file);
}
