import * as vscode from "vscode";
import { runMneme } from "../mneme";
import { parseBlast, type BlastResult } from "../util/parse";
import { TimedCache, quickHash } from "../util/cache";
import { getConfig, logAt } from "../util/config";

/**
 * Adds CodeLens entries above function/class/method declarations, showing:
 *   - N callers      (click: quickpick of call sites)
 *   - M tests        (click: open test files covering the symbol)
 *   - K recent edits (click: open git-blame-style history)
 *
 * We can't reuse VS Code's language server symbol table directly (it's
 * per-language and not available during extension activation on every
 * language), so we do a cheap regex pass over the document to find
 * candidate symbol lines. The mneme CLI is the source of truth for the
 * count - the regex just seeds where to ANCHOR the lens.
 *
 * Updates are debounced by document version + content hash to avoid
 * spamming the CLI on every keystroke.
 */

interface LensData {
  readonly callers: number;
  readonly tests: number;
  readonly edits: number;
  readonly blast: BlastResult;
}

interface SymbolAnchor {
  readonly name: string;
  readonly range: vscode.Range;
}

const SYMBOL_REGEXES: RegExp[] = [
  /\bfn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:<[^>]*>)?\s*\(/g,
  /\bfunction\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(/g,
  /^\s*(?:public\s+|private\s+|protected\s+|async\s+|static\s+)*([a-zA-Z_][a-zA-Z0-9_]*)\s*\(/gm,
  /\b(?:class|struct|interface|trait|enum)\s+([a-zA-Z_][a-zA-Z0-9_]*)/g,
  /\bdef\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(/g,
];

export class MnemeCodeLensProvider implements vscode.CodeLensProvider, vscode.Disposable {
  private readonly emitter = new vscode.EventEmitter<void>();
  public readonly onDidChangeCodeLenses = this.emitter.event;

  private readonly dataCache: TimedCache<string, LensData>;
  private debounceTimer: NodeJS.Timeout | null = null;

  public constructor(private readonly channel: vscode.OutputChannel | null) {
    this.dataCache = new TimedCache<string, LensData>({
      maxEntries: 512,
      ttlMs: 60_000,
    });
  }

  public dispose(): void {
    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
      this.debounceTimer = null;
    }
    this.emitter.dispose();
  }

  public refresh(): void {
    this.dataCache.clear();
    this.emitter.fire();
  }

  public refreshDebounced(delayMs = 500): void {
    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
    }
    this.debounceTimer = setTimeout(() => {
      this.debounceTimer = null;
      this.emitter.fire();
    }, delayMs);
  }

  public async provideCodeLenses(
    document: vscode.TextDocument,
    token: vscode.CancellationToken,
  ): Promise<vscode.CodeLens[]> {
    if (!getConfig().showCodeLens) {
      return [];
    }
    if (document.uri.scheme !== "file") {
      return [];
    }
    const anchors = this.findAnchors(document);
    if (anchors.length === 0) {
      return [];
    }
    const lenses: vscode.CodeLens[] = [];
    for (const anchor of anchors) {
      lenses.push(
        new vscode.CodeLens(anchor.range, {
          title: "$(loading~spin) mneme: loading...",
          command: "",
        }),
      );
    }

    void this.backgroundResolve(document, anchors, token);
    return lenses;
  }

  public async resolveCodeLens(
    lens: vscode.CodeLens,
    token: vscode.CancellationToken,
  ): Promise<vscode.CodeLens> {
    const document = findDocumentForRange(lens.range);
    if (!document) {
      return lens;
    }
    const anchor = this.findAnchors(document).find((a) =>
      a.range.isEqual(lens.range),
    );
    if (!anchor) {
      return lens;
    }
    const data = await this.dataFor(document, anchor, token);
    if (!data) {
      lens.command = {
        title: "mneme",
        command: "",
      };
      return lens;
    }
    lens.command = {
      title: `${data.callers} callers | ${data.tests} tests | ${data.edits} edits`,
      command: "mneme.showLensDetails",
      arguments: [
        {
          symbol: anchor.name,
          uri: document.uri.toString(),
          blast: data.blast,
        },
      ],
    };
    return lens;
  }

  private async backgroundResolve(
    document: vscode.TextDocument,
    anchors: ReadonlyArray<SymbolAnchor>,
    token: vscode.CancellationToken,
  ): Promise<void> {
    let anyChanged = false;
    for (const anchor of anchors) {
      if (token.isCancellationRequested) {
        return;
      }
      const key = this.cacheKey(document, anchor);
      if (this.dataCache.has(key)) {
        continue;
      }
      const data = await this.fetchData(document, anchor, token);
      if (data) {
        this.dataCache.set(key, data);
        anyChanged = true;
      }
    }
    if (anyChanged) {
      this.emitter.fire();
    }
  }

  private async dataFor(
    document: vscode.TextDocument,
    anchor: SymbolAnchor,
    token: vscode.CancellationToken,
  ): Promise<LensData | null> {
    const key = this.cacheKey(document, anchor);
    const cached = this.dataCache.get(key);
    if (cached) {
      return cached;
    }
    const fresh = await this.fetchData(document, anchor, token);
    if (fresh) {
      this.dataCache.set(key, fresh);
    }
    return fresh;
  }

  private async fetchData(
    document: vscode.TextDocument,
    anchor: SymbolAnchor,
    token: vscode.CancellationToken,
  ): Promise<LensData | null> {
    const folder = vscode.workspace.getWorkspaceFolder(document.uri);
    if (!folder) {
      return null;
    }
    try {
      const result = await runMneme(
        [
          "blast",
          anchor.name,
          "--project",
          folder.uri.fsPath,
          "--format",
          "tsv",
          "--include-tests",
          "--include-edits",
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
      const tests = countKind(blast, "test");
      const edits = countKind(blast, "edit");
      return {
        callers: blast.directCallers,
        tests,
        edits,
        blast,
      };
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      logAt(this.channel, "debug", `codelens fetch failed: ${message}`);
      return null;
    }
  }

  private cacheKey(document: vscode.TextDocument, anchor: SymbolAnchor): string {
    return `${document.uri.fsPath}|${anchor.name}|${document.version}|${quickHash(
      document.getText(),
    )}`;
  }

  private findAnchors(document: vscode.TextDocument): SymbolAnchor[] {
    const text = document.getText();
    const anchors = new Map<string, SymbolAnchor>();
    for (const regex of SYMBOL_REGEXES) {
      regex.lastIndex = 0;
      let match: RegExpExecArray | null;
      while ((match = regex.exec(text)) !== null) {
        const name = match[1];
        if (!name || anchors.has(`${name}@${match.index}`)) {
          continue;
        }
        const offset = match.index;
        const pos = document.positionAt(offset);
        const range = new vscode.Range(pos.line, 0, pos.line, 0);
        anchors.set(`${name}@${match.index}`, { name, range });
      }
    }
    return [...anchors.values()].slice(0, 200);
  }
}

function countKind(blast: BlastResult, marker: string): number {
  const lower = marker.toLowerCase();
  return blast.sites.filter((s) => s.symbol.toLowerCase().includes(lower)).length;
}

function findDocumentForRange(range: vscode.Range): vscode.TextDocument | null {
  for (const editor of vscode.window.visibleTextEditors) {
    if (
      editor.document.lineCount > range.start.line &&
      editor.document.lineAt(range.start.line).range.contains(range.start)
    ) {
      return editor.document;
    }
  }
  return vscode.window.activeTextEditor?.document ?? null;
}
