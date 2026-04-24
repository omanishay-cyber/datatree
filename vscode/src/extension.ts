import * as vscode from "vscode";
import { startStatusBar } from "./statusBar";
import { registerCommands } from "./commands";
import { runMneme } from "./mneme";
import { getConfig, logAt, notify, onConfigChange } from "./util/config";
import { GodNodesProvider } from "./providers/godNodesProvider";
import { DriftProvider } from "./providers/driftProvider";
import { StepLedgerProvider } from "./providers/stepLedgerProvider";
import { DecisionsProvider } from "./providers/decisionsProvider";
import {
  RecentQueriesProvider,
  type QueryKind,
} from "./providers/recentQueriesProvider";
import { ShardsProvider } from "./providers/shardsProvider";
import { MnemeHoverProvider } from "./providers/hoverProvider";
import { MnemeCodeLensProvider } from "./providers/codeLensProvider";
import { GraphPanel } from "./webview/graphPanel";
import { LiveBusClient } from "./livebus/sseClient";
import { looksInstalled } from "./util/parse";

let outputChannel: vscode.OutputChannel | null = null;

/**
 * Container holding every live instance so commands can reach them
 * without re-wiring. One of these per extension activation.
 */
interface ExtensionRuntime {
  readonly channel: vscode.OutputChannel;
  readonly godNodes: GodNodesProvider;
  readonly drift: DriftProvider;
  readonly steps: StepLedgerProvider;
  readonly decisions: DecisionsProvider;
  readonly recentQueries: RecentQueriesProvider;
  readonly shards: ShardsProvider;
  readonly codeLens: MnemeCodeLensProvider;
  readonly livebus: LiveBusClient;
}

let runtime: ExtensionRuntime | null = null;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = vscode.window.createOutputChannel("Mneme");
  context.subscriptions.push(outputChannel);
  logAt(outputChannel, "info", "Mneme extension activating...");

  // Probe install silently. If missing, fall back to a friendly toast +
  // do-nothing mode rather than crashing.
  const installed = await probeMnemeInstall(outputChannel);
  await vscode.commands.executeCommand(
    "setContext",
    "mneme.installed",
    installed,
  );

  const config = getConfig();
  if (installed && config.autoRegisterMCP) {
    await runMneme(
      ["register-mcp", "--platform", "vscode"],
      outputChannel,
    ).catch((err: unknown) => {
      const message = err instanceof Error ? err.message : String(err);
      logAt(outputChannel, "warn", `auto-register failed: ${message}`);
    });
  }

  // Core providers.
  const godNodes = new GodNodesProvider(outputChannel);
  const drift = new DriftProvider(outputChannel);
  const steps = new StepLedgerProvider(outputChannel);
  const decisions = new DecisionsProvider(outputChannel);
  const recentQueries = new RecentQueriesProvider();
  const shards = new ShardsProvider(outputChannel);
  const codeLens = new MnemeCodeLensProvider(outputChannel);
  const hover = new MnemeHoverProvider(outputChannel, drift);

  context.subscriptions.push(drift, codeLens);

  // Tree views.
  context.subscriptions.push(
    vscode.window.createTreeView("mneme.godNodes", {
      treeDataProvider: godNodes,
      showCollapseAll: false,
    }),
    vscode.window.createTreeView("mneme.drift", {
      treeDataProvider: drift,
      showCollapseAll: true,
    }),
    vscode.window.createTreeView("mneme.stepLedger", {
      treeDataProvider: steps,
      showCollapseAll: false,
    }),
    vscode.window.createTreeView("mneme.decisions", {
      treeDataProvider: decisions,
      showCollapseAll: false,
    }),
    vscode.window.createTreeView("mneme.recentQueries", {
      treeDataProvider: recentQueries,
      showCollapseAll: false,
    }),
    vscode.window.createTreeView("mneme.shards", {
      treeDataProvider: shards,
      showCollapseAll: false,
    }),
  );

  // Hover + CodeLens for every language.
  context.subscriptions.push(
    vscode.languages.registerHoverProvider({ scheme: "file" }, hover),
    vscode.languages.registerCodeLensProvider({ scheme: "file" }, codeLens),
  );

  // Livebus SSE client.
  const livebus = new LiveBusClient(outputChannel);
  context.subscriptions.push(livebus);
  context.subscriptions.push(
    livebus.onEvent((event) => {
      switch (event.type) {
        case "connected":
          logAt(outputChannel, "info", "livebus: connected");
          void vscode.commands.executeCommand(
            "setContext",
            "mneme.daemonRunning",
            true,
          );
          return;
        case "disconnected":
          logAt(
            outputChannel,
            "debug",
            `livebus: disconnected (${
              (event.data as { reason?: string })?.reason ?? "?"
            })`,
          );
          void vscode.commands.executeCommand(
            "setContext",
            "mneme.daemonRunning",
            false,
          );
          return;
        case "job.complete":
          void shards.refresh();
          return;
        case "drift.finding":
          void drift.refresh();
          return;
        case "step.complete":
          void steps.refresh();
          return;
        case "graph.updated":
          codeLens.refresh();
          void godNodes.refresh();
          return;
        default:
          return;
      }
    }),
  );
  if (installed) {
    livebus.start();
  }

  // Commands.
  runtime = {
    channel: outputChannel,
    godNodes,
    drift,
    steps,
    decisions,
    recentQueries,
    shards,
    codeLens,
    livebus,
  };
  context.subscriptions.push(
    ...registerCommands(outputChannel, {
      openGraph: () => {
        GraphPanel.showOrReveal(context.extensionUri, outputChannel);
      },
      focusNodeInGraph: (node) => {
        void GraphPanel.showOrReveal(context.extensionUri, outputChannel).focusNode(node);
      },
      recordQuery: (kind: QueryKind, input: string) => {
        recentQueries.record(kind, input);
      },
      refresh: (target) => refreshByKey(target),
    }),
  );

  // Status bar (respects showStatusBar).
  const statusBar = startStatusBar(outputChannel);
  context.subscriptions.push(statusBar);

  // React to config changes.
  context.subscriptions.push(
    onConfigChange((cfg) => {
      codeLens.refresh();
      if (!cfg.showDrift) {
        void drift.refresh();
      }
      logAt(outputChannel, "debug", "config changed, refreshing providers");
    }),
  );

  // Kick off initial refresh of every panel once so users see content.
  if (installed) {
    await Promise.allSettled([
      godNodes.refresh(),
      drift.refresh(),
      steps.refresh(),
      decisions.refresh(),
      shards.refresh(),
    ]);
  } else {
    void notify(
      "warn",
      "mneme binary not found on PATH. Install it and reload for full features.",
      "Docs",
    ).then((pick) => {
      if (pick === "Docs") {
        void vscode.env.openExternal(
          vscode.Uri.parse("https://github.com/omanishay-cyber/mneme#readme"),
        );
      }
    });
  }

  logAt(outputChannel, "info", "Mneme extension ready.");
}

export function deactivate(): void {
  if (runtime) {
    runtime.livebus.dispose();
    runtime = null;
  }
  outputChannel?.dispose();
  outputChannel = null;
}

export type RefreshKey =
  | "godNodes"
  | "drift"
  | "steps"
  | "decisions"
  | "shards"
  | "all";

function refreshByKey(key: RefreshKey): Promise<void> {
  if (!runtime) {
    return Promise.resolve();
  }
  switch (key) {
    case "godNodes":
      return runtime.godNodes.refresh();
    case "drift":
      return runtime.drift.refresh();
    case "steps":
      return runtime.steps.refresh();
    case "decisions":
      return runtime.decisions.refresh();
    case "shards":
      return runtime.shards.refresh();
    case "all": {
      const r = runtime;
      return Promise.allSettled([
        r.godNodes.refresh(),
        r.drift.refresh(),
        r.steps.refresh(),
        r.decisions.refresh(),
        r.shards.refresh(),
      ]).then(() => undefined);
    }
  }
}

async function probeMnemeInstall(
  channel: vscode.OutputChannel | null,
): Promise<boolean> {
  try {
    const result = await runMneme(["--version"], channel, { quiet: true });
    const ok = looksInstalled(result.stdout);
    logAt(
      channel,
      "info",
      ok ? "mneme install detected" : "mneme --version output unrecognised",
    );
    return ok;
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    logAt(channel, "info", `mneme not on PATH: ${message}`);
    return false;
  }
}
