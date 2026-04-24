import * as vscode from "vscode";
import { runMneme } from "./mneme";

/**
 * One entry per recall hit, parsed from `mneme recall` stdout.
 */
interface RecallHit {
  readonly kind: string;
  readonly name: string;
  readonly file: string | null;
  readonly line: number | null;
  readonly raw: string;
}

/**
 * Registers all command palette handlers and returns the disposables.
 */
export function registerCommands(channel: vscode.OutputChannel): vscode.Disposable[] {
  return [
    vscode.commands.registerCommand("mneme.build", () => buildCommand(channel)),
    vscode.commands.registerCommand("mneme.doctor", () => doctorCommand(channel)),
    vscode.commands.registerCommand("mneme.recall", () => recallCommand(channel)),
    vscode.commands.registerCommand("mneme.viewVision", () => viewVisionCommand(channel)),
    vscode.commands.registerCommand("mneme.daemonStart", () => daemonStartCommand(channel)),
    vscode.commands.registerCommand("mneme.daemonStop", () => daemonStopCommand(channel)),
  ];
}

async function buildCommand(channel: vscode.OutputChannel): Promise<void> {
  const folder = await pickWorkspaceFolder();
  if (!folder) {
    return;
  }
  channel.show(true);
  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: `mneme build ${folder.name}`,
      cancellable: false,
    },
    async () => {
      try {
        await runMneme(["build", folder.uri.fsPath, "--yes"], channel, {
          cwd: folder.uri.fsPath,
        });
        void vscode.window.showInformationMessage(`mneme build complete for ${folder.name}`);
      } catch (err) {
        showError("mneme build failed", err);
      }
    },
  );
}

async function doctorCommand(channel: vscode.OutputChannel): Promise<void> {
  channel.show(true);
  try {
    await runMneme(["doctor"], channel);
  } catch (err) {
    showError("mneme doctor failed", err);
  }
}

async function recallCommand(channel: vscode.OutputChannel): Promise<void> {
  const query = await vscode.window.showInputBox({
    prompt: "Recall query",
    placeHolder: "e.g. compaction recovery",
    ignoreFocusOut: true,
  });
  if (!query || query.trim().length === 0) {
    return;
  }

  const folder = vscode.workspace.workspaceFolders?.[0];
  const args = ["recall", query.trim()];
  if (folder) {
    args.push("--project", folder.uri.fsPath);
  }
  args.push("--limit", "10");

  channel.show(true);
  let stdout = "";
  try {
    const result = await runMneme(args, channel, {
      cwd: folder?.uri.fsPath,
      quiet: true,
    });
    stdout = result.stdout;
    channel.append(stdout);
  } catch (err) {
    showError("mneme recall failed", err);
    return;
  }

  const hits = parseRecallHits(stdout);
  if (hits.length === 0) {
    void vscode.window.showInformationMessage(`No recall hits for "${query}".`);
    return;
  }

  const picked = await vscode.window.showQuickPick(
    hits.map((hit) => ({
      label: `[${hit.kind}] ${hit.name}`,
      description: hit.file ?? undefined,
      detail: hit.raw,
      hit,
    })),
    {
      placeHolder: `Top ${hits.length} hits for "${query}"`,
      matchOnDescription: true,
      matchOnDetail: true,
    },
  );

  if (!picked) {
    return;
  }

  await openRecallHit(picked.hit, folder?.uri.fsPath);
}

async function viewVisionCommand(channel: vscode.OutputChannel): Promise<void> {
  channel.appendLine("opening Vision graph (detached)...");
  try {
    await runMneme(["view"], channel, { detached: true });
  } catch (err) {
    showError("mneme view failed", err);
  }
}

async function daemonStartCommand(channel: vscode.OutputChannel): Promise<void> {
  channel.show(true);
  try {
    await runMneme(["daemon", "start"], channel);
    void vscode.window.showInformationMessage("mneme daemon started.");
  } catch (err) {
    showError("mneme daemon start failed", err);
  }
}

async function daemonStopCommand(channel: vscode.OutputChannel): Promise<void> {
  channel.show(true);
  try {
    await runMneme(["daemon", "stop"], channel);
    void vscode.window.showInformationMessage("mneme daemon stopped.");
  } catch (err) {
    showError("mneme daemon stop failed", err);
  }
}

/**
 * Parses lines like `[function] my_func  src/foo.rs:42` produced by
 * `mneme recall`. Tolerant of extra whitespace and missing file:line.
 */
export function parseRecallHits(stdout: string): RecallHit[] {
  const hits: RecallHit[] = [];
  // [<kind>] <name> [optional trailing path:line]
  const lineRegex = /^\s*\[([^\]]+)\]\s+(\S.*?)\s*$/;
  // Capture the last "path:line" or "path:line:col" on the line.
  const locRegex = /(\S+?):(\d+)(?::\d+)?\s*$/;

  for (const raw of stdout.split(/\r?\n/)) {
    const match = raw.match(lineRegex);
    if (!match) {
      continue;
    }
    const kind = match[1].trim();
    let nameAndLoc = match[2].trim();
    let file: string | null = null;
    let line: number | null = null;

    const locMatch = nameAndLoc.match(locRegex);
    if (locMatch && locMatch.index !== undefined) {
      file = locMatch[1];
      const parsed = Number.parseInt(locMatch[2], 10);
      line = Number.isFinite(parsed) ? parsed : null;
      nameAndLoc = nameAndLoc.slice(0, locMatch.index).trim();
    }

    hits.push({
      kind,
      name: nameAndLoc.length > 0 ? nameAndLoc : "(unnamed)",
      file,
      line,
      raw: raw.trim(),
    });
  }
  return hits;
}

async function openRecallHit(hit: RecallHit, workspaceRoot: string | undefined): Promise<void> {
  if (!hit.file) {
    void vscode.window.showWarningMessage(
      "This recall hit has no file location, so it cannot be opened.",
    );
    return;
  }

  const uri = resolveHitUri(hit.file, workspaceRoot);
  try {
    const doc = await vscode.workspace.openTextDocument(uri);
    const editor = await vscode.window.showTextDocument(doc);
    if (hit.line !== null) {
      const lineIndex = Math.max(0, hit.line - 1);
      const position = new vscode.Position(lineIndex, 0);
      editor.selection = new vscode.Selection(position, position);
      editor.revealRange(
        new vscode.Range(position, position),
        vscode.TextEditorRevealType.InCenter,
      );
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    void vscode.window.showErrorMessage(`Could not open ${hit.file}: ${message}`);
  }
}

function resolveHitUri(file: string, workspaceRoot: string | undefined): vscode.Uri {
  if (/^[a-zA-Z]:[\\/]/.test(file) || file.startsWith("/") || file.startsWith("\\")) {
    return vscode.Uri.file(file);
  }
  if (workspaceRoot) {
    return vscode.Uri.file(`${workspaceRoot}/${file}`);
  }
  return vscode.Uri.file(file);
}

async function pickWorkspaceFolder(): Promise<vscode.WorkspaceFolder | undefined> {
  const folders = vscode.workspace.workspaceFolders ?? [];
  if (folders.length === 0) {
    void vscode.window.showErrorMessage("Open a folder before running mneme build.");
    return undefined;
  }
  if (folders.length === 1) {
    return folders[0];
  }
  const picked = await vscode.window.showWorkspaceFolderPick({
    placeHolder: "Which workspace folder should mneme build?",
  });
  return picked;
}

function showError(prefix: string, err: unknown): void {
  const message = err instanceof Error ? err.message : String(err);
  void vscode.window.showErrorMessage(`${prefix}: ${message}`);
}
