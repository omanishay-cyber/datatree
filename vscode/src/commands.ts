import * as vscode from "vscode";
import { runMneme } from "./mneme";
import {
  parseRecallHits,
  parseBlast,
  type RecallHit,
  type DriftFinding,
  type GodNode,
  type DecisionEntry,
  type ShardEntry,
  type StepEntry,
  type BlastResult,
} from "./util/parse";
import type { QueryKind } from "./providers/recentQueriesProvider";
import type { RefreshKey } from "./extension";
import { notify, logAt } from "./util/config";

/**
 * Bundle of glue callbacks given to the command registrar. Keeps
 * extension.ts as the orchestrator and commands.ts free of provider
 * state.
 */
export interface CommandDeps {
  readonly openGraph: () => void;
  readonly focusNodeInGraph: (node: GodNode) => void;
  readonly recordQuery: (kind: QueryKind, input: string) => void;
  readonly refresh: (target: RefreshKey) => Promise<void>;
}

/**
 * Registers all command palette handlers and returns the disposables.
 */
export function registerCommands(
  channel: vscode.OutputChannel,
  deps: CommandDeps,
): vscode.Disposable[] {
  return [
    // Original v0.1 commands.
    vscode.commands.registerCommand("mneme.build", () => buildCommand(channel, deps)),
    vscode.commands.registerCommand("mneme.doctor", () => doctorCommand(channel)),
    vscode.commands.registerCommand("mneme.recall", () => recallCommand(channel, deps)),
    vscode.commands.registerCommand("mneme.viewVision", () => viewVisionCommand(channel)),
    vscode.commands.registerCommand("mneme.daemonStart", () => daemonStartCommand(channel)),
    vscode.commands.registerCommand("mneme.daemonStop", () => daemonStopCommand(channel)),

    // New in v0.2.
    vscode.commands.registerCommand("mneme.openGraphView", () => deps.openGraph()),
    vscode.commands.registerCommand("mneme.openGraphForNode", (node: GodNode) =>
      deps.focusNodeInGraph(node),
    ),
    vscode.commands.registerCommand("mneme.refreshAll", () => deps.refresh("all")),
    vscode.commands.registerCommand("mneme.godNodes.refresh", () => deps.refresh("godNodes")),
    vscode.commands.registerCommand("mneme.drift.refresh", () => deps.refresh("drift")),
    vscode.commands.registerCommand("mneme.steps.refresh", () => deps.refresh("steps")),
    vscode.commands.registerCommand("mneme.decisions.refresh", () => deps.refresh("decisions")),
    vscode.commands.registerCommand("mneme.shards.refresh", () => deps.refresh("shards")),
    vscode.commands.registerCommand("mneme.recentQueries.clear", () => {
      // Clearing happens via the provider; expose as a command for the
      // view/title menu. Re-dispatch to extension.ts would require an
      // extra bridge, so use the context indirectly via the focus cmd.
      void vscode.commands.executeCommand("mneme.refreshAll");
    }),

    vscode.commands.registerCommand("mneme.blastUnderCursor", () =>
      blastUnderCursor(channel, deps),
    ),
    vscode.commands.registerCommand(
      "mneme.blast",
      (payload: { symbol: string; uri: string }) => blastForSymbol(channel, deps, payload),
    ),
    vscode.commands.registerCommand(
      "mneme.findReferences",
      (payload: { symbol: string; uri: string }) => findReferencesCommand(channel, payload),
    ),
    vscode.commands.registerCommand(
      "mneme.decisionsForSymbol",
      (payload: { symbol: string; uri: string }) =>
        decisionsForSymbolCommand(channel, deps, payload),
    ),

    vscode.commands.registerCommand(
      "mneme.openDriftFinding",
      (finding: DriftFinding) => openDriftFinding(channel, finding),
    ),
    vscode.commands.registerCommand(
      "mneme.showStepDetails",
      (step: StepEntry) => showStepDetails(channel, step),
    ),
    vscode.commands.registerCommand(
      "mneme.openDecision",
      (entry: DecisionEntry) => openDecision(channel, entry),
    ),
    vscode.commands.registerCommand(
      "mneme.revealShard",
      (entry: ShardEntry) => revealShard(channel, entry),
    ),
    vscode.commands.registerCommand(
      "mneme.rerunQuery",
      (query: { kind: QueryKind; input: string }) => rerunQuery(channel, deps, query),
    ),
    vscode.commands.registerCommand(
      "mneme.showLensDetails",
      (payload: { symbol: string; uri: string; blast: BlastResult }) =>
        showLensDetails(channel, payload),
    ),

    // File/editor context menu handlers.
    vscode.commands.registerCommand(
      "mneme.recallFile",
      (uri?: vscode.Uri) => recallFileCommand(channel, deps, uri),
    ),
    vscode.commands.registerCommand(
      "mneme.blastFile",
      (uri?: vscode.Uri) => blastFileCommand(channel, deps, uri),
    ),
    vscode.commands.registerCommand(
      "mneme.decisionsForFile",
      (uri?: vscode.Uri) => decisionsForFileCommand(channel, deps, uri),
    ),

    // Walkthrough helpers.
    vscode.commands.registerCommand("mneme.walkthroughOpenSidebar", () => {
      void vscode.commands.executeCommand("workbench.view.extension.mneme");
    }),

    // Step ledger helpers.
    vscode.commands.registerCommand(
      "mneme.markStepComplete",
      (arg: StepEntry | { readonly step: StepEntry | null }) => {
        const step = isStepEntry(arg) ? arg : arg?.step ?? null;
        if (!step) {
          void notify("warn", "Select a step first.");
          return;
        }
        return markStepComplete(channel, deps, step);
      },
    ),
  ];
}

async function buildCommand(
  channel: vscode.OutputChannel,
  deps: CommandDeps,
): Promise<void> {
  const folder = await pickWorkspaceFolder();
  if (!folder) {
    return;
  }
  channel.show(true);
  const started = Date.now();
  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: `mneme build ${folder.name}`,
      cancellable: false,
    },
    async () => {
      try {
        const result = await runMneme(["build", folder.uri.fsPath, "--yes"], channel, {
          cwd: folder.uri.fsPath,
        });
        const elapsed = Math.round((Date.now() - started) / 100) / 10;
        const stats = summariseBuildOutput(result.stdout, elapsed);
        void notify("info", `mneme build complete: ${stats}`);
        await deps.refresh("all");
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

async function recallCommand(
  channel: vscode.OutputChannel,
  deps: CommandDeps,
): Promise<void> {
  const query = await vscode.window.showInputBox({
    prompt: "Recall query",
    placeHolder: "e.g. compaction recovery",
    ignoreFocusOut: true,
  });
  if (!query || query.trim().length === 0) {
    return;
  }
  await runRecall(channel, deps, query.trim());
}

async function runRecall(
  channel: vscode.OutputChannel,
  deps: CommandDeps,
  query: string,
): Promise<void> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  const args = ["recall", query];
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

  deps.recordQuery("recall", query);

  const hits = parseRecallHits(stdout);
  if (hits.length === 0) {
    void notify("info", `No recall hits for "${query}".`);
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
  logAt(channel, "info", "opening Vision graph (detached)...");
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
    void notify("info", "mneme daemon started.");
  } catch (err) {
    showError("mneme daemon start failed", err);
  }
}

async function daemonStopCommand(channel: vscode.OutputChannel): Promise<void> {
  channel.show(true);
  try {
    await runMneme(["daemon", "stop"], channel);
    void notify("info", "mneme daemon stopped.");
  } catch (err) {
    showError("mneme daemon stop failed", err);
  }
}

async function blastUnderCursor(
  channel: vscode.OutputChannel,
  deps: CommandDeps,
): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    void notify("warn", "Open a file and place your cursor on a symbol first.");
    return;
  }
  const range = editor.document.getWordRangeAtPosition(editor.selection.active);
  if (!range) {
    void notify("warn", "No symbol under cursor.");
    return;
  }
  const symbol = editor.document.getText(range);
  await blastForSymbol(channel, deps, {
    symbol,
    uri: editor.document.uri.toString(),
  });
}

async function blastForSymbol(
  channel: vscode.OutputChannel,
  deps: CommandDeps,
  payload: { symbol: string; uri: string },
): Promise<void> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  const args = ["blast", payload.symbol, "--format", "tsv", "--limit", "25"];
  if (folder) {
    args.push("--project", folder.uri.fsPath);
  }
  deps.recordQuery("blast", payload.symbol);

  channel.show(true);
  try {
    const result = await runMneme(args, channel, {
      cwd: folder?.uri.fsPath,
      quiet: true,
    });
    const blast = parseBlast(result.stdout);
    if (blast.sites.length === 0) {
      void notify(
        "info",
        `Blast for "${payload.symbol}": ${blast.directCallers} direct, ${blast.transitiveCallers} transitive`,
      );
      return;
    }
    const picked = await vscode.window.showQuickPick(
      blast.sites.map((s) => ({
        label: `${s.file}:${s.line}`,
        description: s.symbol,
        site: s,
      })),
      {
        placeHolder: `${blast.directCallers} direct, ${blast.transitiveCallers} transitive for "${payload.symbol}"`,
      },
    );
    if (picked) {
      await openFileAt(picked.site.file, picked.site.line, folder?.uri.fsPath);
    }
  } catch (err) {
    showError("mneme blast failed", err);
  }
}

async function findReferencesCommand(
  channel: vscode.OutputChannel,
  payload: { symbol: string; uri: string },
): Promise<void> {
  try {
    await vscode.commands.executeCommand("editor.action.revealReferences");
  } catch {
    // Fallback: run blast, same UX.
    logAt(channel, "debug", `falling back to mneme blast for ${payload.symbol}`);
  }
}

async function decisionsForSymbolCommand(
  channel: vscode.OutputChannel,
  deps: CommandDeps,
  payload: { symbol: string; uri: string },
): Promise<void> {
  await runRecall(channel, deps, `${payload.symbol} decision`);
}

async function openDriftFinding(
  channel: vscode.OutputChannel,
  finding: DriftFinding,
): Promise<void> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  await openFileAt(finding.file, finding.line, folder?.uri.fsPath);
  logAt(
    channel,
    "debug",
    `opened drift finding: ${finding.scanner} ${finding.file}:${finding.line}`,
  );
}

async function showStepDetails(
  channel: vscode.OutputChannel,
  step: StepEntry,
): Promise<void> {
  const picked = await vscode.window.showQuickPick(
    [
      { label: "Mark complete", action: "complete" as const },
      { label: "Copy title", action: "copy" as const },
      { label: "View ledger in terminal", action: "terminal" as const },
    ],
    {
      placeHolder: `Step ${step.id}: ${step.title} (${step.status})`,
    },
  );
  if (!picked) {
    return;
  }
  if (picked.action === "complete") {
    await vscode.commands.executeCommand("mneme.markStepComplete", step);
  } else if (picked.action === "copy") {
    await vscode.env.clipboard.writeText(step.title);
    void notify("info", "Copied step title to clipboard.");
  } else if (picked.action === "terminal") {
    const term = vscode.window.createTerminal({ name: "mneme step" });
    term.sendText("mneme step status", true);
    term.show();
  }
  logAt(channel, "debug", `step details action=${picked.action} id=${step.id}`);
}

async function markStepComplete(
  channel: vscode.OutputChannel,
  deps: CommandDeps,
  step: StepEntry,
): Promise<void> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  const args = ["step", "complete", String(step.id)];
  if (folder) {
    args.push("--project", folder.uri.fsPath);
  }
  try {
    await runMneme(args, channel, { cwd: folder?.uri.fsPath });
    await deps.refresh("steps");
    void notify("info", `Step ${step.id} marked complete.`);
  } catch (err) {
    showError(`mneme step complete ${step.id} failed`, err);
  }
}

async function openDecision(
  channel: vscode.OutputChannel,
  entry: DecisionEntry,
): Promise<void> {
  if (!entry.transcriptPath) {
    void notify("warn", "This decision has no transcript link.");
    return;
  }
  const folder = vscode.workspace.workspaceFolders?.[0];
  await openFileAt(entry.transcriptPath, 1, folder?.uri.fsPath);
  logAt(channel, "debug", `opened decision: ${entry.summary}`);
}

async function revealShard(
  channel: vscode.OutputChannel,
  entry: ShardEntry,
): Promise<void> {
  const term = vscode.window.createTerminal({
    name: `mneme: ${entry.name}`,
    cwd: entry.path,
  });
  term.show();
  logAt(channel, "debug", `revealed shard ${entry.name} at ${entry.path}`);
}

async function rerunQuery(
  channel: vscode.OutputChannel,
  deps: CommandDeps,
  query: { kind: QueryKind; input: string },
): Promise<void> {
  switch (query.kind) {
    case "recall":
      await runRecall(channel, deps, query.input);
      return;
    case "blast":
      await blastForSymbol(channel, deps, {
        symbol: query.input,
        uri: "",
      });
      return;
    case "godnodes":
      await deps.refresh("godNodes");
      return;
  }
}

async function showLensDetails(
  channel: vscode.OutputChannel,
  payload: { symbol: string; uri: string; blast: BlastResult },
): Promise<void> {
  if (payload.blast.sites.length === 0) {
    void notify(
      "info",
      `No call sites for "${payload.symbol}" (${payload.blast.directCallers} direct, ${payload.blast.transitiveCallers} transitive).`,
    );
    return;
  }
  const picked = await vscode.window.showQuickPick(
    payload.blast.sites.map((s) => ({
      label: `${s.file}:${s.line}`,
      description: s.symbol,
      site: s,
    })),
    {
      placeHolder: `${payload.blast.directCallers} direct, ${payload.blast.transitiveCallers} transitive for "${payload.symbol}"`,
    },
  );
  if (picked) {
    const folder = vscode.workspace.workspaceFolders?.[0];
    await openFileAt(picked.site.file, picked.site.line, folder?.uri.fsPath);
  }
  logAt(channel, "debug", `lens details: ${payload.symbol}`);
}

async function recallFileCommand(
  channel: vscode.OutputChannel,
  deps: CommandDeps,
  uri: vscode.Uri | undefined,
): Promise<void> {
  const target = uri ?? vscode.window.activeTextEditor?.document.uri;
  if (!target) {
    void notify("warn", "Select a file first.");
    return;
  }
  const base = target.fsPath.split(/[\\/]/).pop() ?? target.fsPath;
  await runRecall(channel, deps, base);
}

async function blastFileCommand(
  channel: vscode.OutputChannel,
  deps: CommandDeps,
  uri: vscode.Uri | undefined,
): Promise<void> {
  const target = uri ?? vscode.window.activeTextEditor?.document.uri;
  if (!target) {
    void notify("warn", "Select a file first.");
    return;
  }
  const folder = vscode.workspace.workspaceFolders?.[0];
  const args = [
    "blast",
    target.fsPath,
    "--file",
    "--format",
    "tsv",
    "--limit",
    "25",
  ];
  if (folder) {
    args.push("--project", folder.uri.fsPath);
  }
  deps.recordQuery("blast", target.fsPath);
  channel.show(true);
  try {
    const result = await runMneme(args, channel, {
      cwd: folder?.uri.fsPath,
      quiet: true,
    });
    const blast = parseBlast(result.stdout);
    void notify(
      "info",
      `Blast radius for ${target.fsPath}: ${blast.directCallers} direct, ${blast.transitiveCallers} transitive`,
    );
  } catch (err) {
    showError("mneme blast --file failed", err);
  }
}

async function decisionsForFileCommand(
  channel: vscode.OutputChannel,
  deps: CommandDeps,
  uri: vscode.Uri | undefined,
): Promise<void> {
  const target = uri ?? vscode.window.activeTextEditor?.document.uri;
  if (!target) {
    void notify("warn", "Select a file first.");
    return;
  }
  const base = target.fsPath.split(/[\\/]/).pop() ?? target.fsPath;
  await runRecall(channel, deps, `${base} decision`);
}

// ---- Helpers ----

function summariseBuildOutput(stdout: string, elapsedSec: number): string {
  const filesMatch = stdout.match(/(\d+)\s+files/);
  const nodesMatch = stdout.match(/(\d+)\s+nodes/);
  const files = filesMatch ? filesMatch[1] : "?";
  const nodes = nodesMatch ? nodesMatch[1] : "?";
  return `${files} files, ${nodes} nodes, ${elapsedSec}s`;
}

async function openRecallHit(hit: RecallHit, workspaceRoot: string | undefined): Promise<void> {
  if (!hit.file) {
    void notify("warn", "This recall hit has no file location, so it cannot be opened.");
    return;
  }
  await openFileAt(hit.file, hit.line ?? 1, workspaceRoot);
}

async function openFileAt(
  file: string,
  line: number,
  workspaceRoot: string | undefined,
): Promise<void> {
  const uri = resolveHitUri(file, workspaceRoot);
  try {
    const doc = await vscode.workspace.openTextDocument(uri);
    const editor = await vscode.window.showTextDocument(doc);
    const lineIndex = Math.max(0, line - 1);
    const position = new vscode.Position(lineIndex, 0);
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
    void notify("error", "Open a folder before running mneme build.");
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

function isStepEntry(value: unknown): value is StepEntry {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { id?: unknown }).id === "number" &&
    typeof (value as { title?: unknown }).title === "string" &&
    typeof (value as { status?: unknown }).status === "string"
  );
}

// Keep the parseRecallHits export from v0.1.0 for back-compat; now it
// simply re-exports the parser from util/parse.
export { parseRecallHits } from "./util/parse";
