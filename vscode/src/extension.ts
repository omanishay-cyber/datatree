import * as vscode from "vscode";
import { startStatusBar } from "./statusBar";
import { registerCommands } from "./commands";
import { runMneme } from "./mneme";

let outputChannel: vscode.OutputChannel | null = null;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = vscode.window.createOutputChannel("Mneme");
  context.subscriptions.push(outputChannel);

  const config = vscode.workspace.getConfiguration("mneme");
  if (config.get<boolean>("autoRegisterMCP", true)) {
    await runMneme(
      ["register-mcp", "--platform", "vscode"],
      outputChannel,
    ).catch((err: unknown) => {
      const message = err instanceof Error ? err.message : String(err);
      outputChannel?.appendLine(`auto-register failed: ${message}`);
    });
  }

  context.subscriptions.push(...registerCommands(outputChannel));
  context.subscriptions.push(startStatusBar(outputChannel));
}

export function deactivate(): void {
  outputChannel?.dispose();
  outputChannel = null;
}
