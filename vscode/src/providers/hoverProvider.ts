import * as vscode from "vscode";
import { runMneme } from "../mneme";
import { parseBlast, type BlastResult } from "../util/parse";
import { TimedCache, quickHash } from "../util/cache";
import { getConfig, logAt } from "../util/config";
import type { DriftProvider } from "./driftProvider";

/**
 * Registers a universal HoverProvider ("*") that enriches the hover
 * tooltip with:
 *   - blast radius of the hovered symbol
 *   - recent decisions touching the hovered file
 *   - tests covering the hovered symbol
 *   - drift findings on the file
 *
 * Everything is best-effort. If mneme isn't installed or the daemon is
 * down, the hover silently contributes nothing.
 */

interface HoverDataKey extends String {}

export class MnemeHoverProvider implements vscode.HoverProvider {
  private readonly blastCache: TimedCache<string, BlastResult>;

  public constructor(
    private readonly channel: vscode.OutputChannel | null,
    private readonly driftProvider: DriftProvider,
  ) {
    this.blastCache = new TimedCache<string, BlastResult>({
      maxEntries: 256,
      ttlMs: 30_000,
    });
  }

  public async provideHover(
    document: vscode.TextDocument,
    position: vscode.Position,
    token: vscode.CancellationToken,
  ): Promise<vscode.Hover | null> {
    if (!getConfig().showHover) {
      return null;
    }
    const range = document.getWordRangeAtPosition(position);
    if (!range) {
      return null;
    }
    const symbol = document.getText(range);
    if (!isReasonableSymbol(symbol)) {
      return null;
    }

    const md = new vscode.MarkdownString(undefined, true);
    md.isTrusted = true;
    md.supportHtml = false;
    md.appendMarkdown(`**mneme** · \`${symbol}\`\n\n`);

    // Blast radius.
    const blast = await this.getBlast(document, symbol, token);
    if (blast) {
      md.appendMarkdown(
        `- Blast: **${blast.directCallers}** direct, **${blast.transitiveCallers}** transitive\n`,
      );
    }

    // Drift findings on this file.
    const driftHits = this.driftProvider.getFindingsForFile(document.uri.fsPath);
    if (driftHits.length > 0) {
      const counts = {
        critical: driftHits.filter((f) => f.severity === "critical").length,
        shouldFix: driftHits.filter((f) => f.severity === "should-fix").length,
        info: driftHits.filter((f) => f.severity === "info").length,
      };
      const parts: string[] = [];
      if (counts.critical > 0) {
        parts.push(`${counts.critical} critical`);
      }
      if (counts.shouldFix > 0) {
        parts.push(`${counts.shouldFix} should-fix`);
      }
      if (counts.info > 0) {
        parts.push(`${counts.info} info`);
      }
      if (parts.length > 0) {
        md.appendMarkdown(`- Drift on this file: ${parts.join(", ")}\n`);
      }
    }

    // Command links.
    const args = encodeArgs({ symbol, uri: document.uri.toString() });
    md.appendMarkdown(
      `\n[Blast graph](command:mneme.blast?${args}) · ` +
        `[Find references](command:mneme.findReferences?${args}) · ` +
        `[Show decisions](command:mneme.decisionsForSymbol?${args})\n`,
    );

    return new vscode.Hover(md, range);
  }

  private async getBlast(
    document: vscode.TextDocument,
    symbol: string,
    token: vscode.CancellationToken,
  ): Promise<BlastResult | null> {
    const folder = vscode.workspace.getWorkspaceFolder(document.uri);
    if (!folder) {
      return null;
    }
    const cacheKey = `${document.uri.fsPath}|${symbol}|${quickHash(document.getText())}`;
    const cached = this.blastCache.get(cacheKey);
    if (cached) {
      return cached;
    }
    try {
      const result = await runMneme(
        [
          "blast",
          symbol,
          "--project",
          folder.uri.fsPath,
          "--format",
          "tsv",
          "--limit",
          "20",
        ],
        this.channel,
        { quiet: true, cwd: folder.uri.fsPath },
      );
      if (token.isCancellationRequested) {
        return null;
      }
      const blast = parseBlast(result.stdout);
      this.blastCache.set(cacheKey, blast);
      return blast;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      logAt(this.channel, "debug", `hover blast failed: ${message}`);
      return null;
    }
  }
}

function isReasonableSymbol(input: string): boolean {
  if (input.length < 2 || input.length > 80) {
    return false;
  }
  return /^[a-zA-Z_][a-zA-Z0-9_]*$/.test(input);
}

function encodeArgs(obj: unknown): string {
  return encodeURIComponent(JSON.stringify(obj));
}
