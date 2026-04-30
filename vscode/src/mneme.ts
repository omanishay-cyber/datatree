import * as vscode from "vscode";
import { spawn, SpawnOptions } from "child_process";

/**
 * Resolves the path to the mneme binary.
 *
 * If the user has set `mneme.binaryPath` to a non-default value, return it
 * as-is. Otherwise return "mneme" and rely on PATH resolution by the OS.
 */
export function resolveBinary(): string {
  const config = vscode.workspace.getConfiguration("mneme");
  const configured = config.get<string>("binaryPath", "mneme");
  if (typeof configured === "string" && configured.trim().length > 0) {
    return configured.trim();
  }
  return "mneme";
}

/**
 * Result of a completed mneme command run.
 */
export interface MnemeRunResult {
  readonly stdout: string;
  readonly stderr: string;
  readonly exitCode: number;
}

/**
 * Options for `runMneme`.
 */
export interface RunMnemeOptions {
  /** Working directory for the spawned process. Defaults to the first workspace folder. */
  readonly cwd?: string;
  /** Detach the process so VS Code does not keep it open. Used for `mneme view`. */
  readonly detached?: boolean;
  /** Quiet mode suppresses appending stdout to the channel. stderr is always captured. */
  readonly quiet?: boolean;
}

/**
 * Spawn `mneme <args>` and stream output to the given channel.
 *
 * Resolves with stdout/stderr/exitCode on exit code 0. Rejects with an Error
 * containing the same fields when the exit code is non-zero, or when the
 * process fails to spawn at all.
 */
export function runMneme(
  args: readonly string[],
  channel: vscode.OutputChannel | null,
  options: RunMnemeOptions = {},
): Promise<MnemeRunResult> {
  return new Promise((resolve, reject) => {
    const binary = resolveBinary();
    const cwd =
      options.cwd ?? vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? process.cwd();

    const spawnOptions: SpawnOptions = {
      cwd,
      shell: false,
      detached: options.detached === true,
      stdio: options.detached === true ? "ignore" : ["ignore", "pipe", "pipe"],
      windowsHide: true,
    };

    channel?.appendLine(`> ${binary} ${args.join(" ")}`);

    let child;
    try {
      child = spawn(binary, [...args], spawnOptions);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      channel?.appendLine(`failed to spawn ${binary}: ${message}`);
      reject(new Error(`failed to spawn ${binary}: ${message}`));
      return;
    }

    if (options.detached === true) {
      child.unref();
      resolve({ stdout: "", stderr: "", exitCode: 0 });
      return;
    }

    let stdout = "";
    let stderr = "";

    child.stdout?.setEncoding("utf8");
    child.stderr?.setEncoding("utf8");

    child.stdout?.on("data", (chunk: string) => {
      stdout += chunk;
      if (!options.quiet) {
        channel?.append(chunk);
      }
    });

    child.stderr?.on("data", (chunk: string) => {
      stderr += chunk;
      channel?.append(chunk);
    });

    child.on("error", (err: Error) => {
      channel?.appendLine(`process error: ${err.message}`);
      reject(err);
    });

    child.on("close", (code: number | null) => {
      const exitCode = code ?? -1;
      if (exitCode === 0) {
        resolve({ stdout, stderr, exitCode });
      } else {
        const summary = `mneme ${args.join(" ")} exited with code ${exitCode}`;
        channel?.appendLine(summary);
        const error = new Error(summary);
        (error as Error & { stdout?: string; stderr?: string; exitCode?: number }).stdout =
          stdout;
        (error as Error & { stdout?: string; stderr?: string; exitCode?: number }).stderr =
          stderr;
        (error as Error & { stdout?: string; stderr?: string; exitCode?: number }).exitCode =
          exitCode;
        reject(error);
      }
    });
  });
}
