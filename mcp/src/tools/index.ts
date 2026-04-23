/**
 * Tool registry with hot-reload.
 *
 * Each tool lives in its own file inside src/tools/ and exports a `tool`
 * symbol of type ToolDescriptor. The registry:
 *
 *   1. Eagerly imports the bundled tools at startup.
 *   2. Watches src/tools/ for new or changed .ts files.
 *   3. On change: re-imports the file using a cache-busting query string
 *      and atomically swaps the descriptor in the registry.
 *   4. Emits "registered" / "unregistered" events for the MCP server to
 *      forward to the harness.
 *
 * Reloads are CRASH-SAFE: a failing reload logs and keeps the previous
 * descriptor. Writes are LAST-WRITER-WINS — drop a new file, replaces
 * existing tool by name (the file's `tool.name` field).
 */

import { EventEmitter } from "node:events";
import { readdir, stat } from "node:fs/promises";
import { watch, type FSWatcher } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, join, basename } from "node:path";
import type { ToolContext, ToolDescriptor } from "../types.ts";

// ---------------------------------------------------------------------------
// Static module list — kept in sync with the file system on disk.
// ---------------------------------------------------------------------------

const STATIC_TOOL_FILES = [
  "recall_decision",
  "recall_conversation",
  "recall_concept",
  "recall_file",
  "recall_todo",
  "recall_constraint",
  "blast_radius",
  "call_graph",
  "find_references",
  "dependency_chain",
  "cyclic_deps",
  "graphify_corpus",
  "god_nodes",
  "surprising_connections",
  "audit_corpus",
  "audit",
  "drift_findings",
  "audit_theme",
  "audit_security",
  "audit_a11y",
  "audit_perf",
  "audit_types",
  "step_status",
  "step_show",
  "step_verify",
  "step_complete",
  "step_resume",
  "step_plan_from",
  "snapshot",
  "compare",
  "rewind",
  "health",
  "doctor",
  "rebuild",
  "refactor_suggest",
  "refactor_apply",
  "wiki_generate",
  "wiki_page",
  "architecture_overview",
  "identity",
  "conventions",
  // F1 (Step Ledger) + F6 (Why-Chain)
  "recall",
  "resume",
  "why",
  // F2 (Hybrid retrieval)
  "context",
  // Moat 4 (federated pattern matching)
  "federated_similar",
];

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

class ToolRegistry extends EventEmitter {
  private tools = new Map<string, ToolDescriptor>();
  private fileToName = new Map<string, string>();
  private watcher: FSWatcher | null = null;
  private reloadDebounce = new Map<string, ReturnType<typeof setTimeout>>();

  constructor(private readonly toolsDir: string) {
    super();
  }

  async load(): Promise<void> {
    for (const name of STATIC_TOOL_FILES) {
      await this.loadFile(`${name}.ts`);
    }
    await this.scanExtraFiles();
  }

  private async scanExtraFiles(): Promise<void> {
    let entries: string[];
    try {
      entries = await readdir(this.toolsDir);
    } catch {
      return;
    }
    for (const entry of entries) {
      if (!entry.endsWith(".ts")) continue;
      if (entry === "index.ts") continue;
      const stem = entry.replace(/\.ts$/, "");
      if (STATIC_TOOL_FILES.includes(stem)) continue;
      await this.loadFile(entry);
    }
  }

  private async loadFile(filename: string): Promise<void> {
    const fullPath = join(this.toolsDir, filename);
    try {
      // Cache-bust by appending a query string to the import URL so Bun/Node
      // re-evaluate the module body on every load.
      const cacheBuster = `?v=${Date.now()}`;
      const url = pathToFileURL(fullPath).toString() + cacheBuster;
      const mod: { tool?: ToolDescriptor } = await import(url);
      if (!mod.tool || !mod.tool.name) {
        console.error(`[mneme-mcp] ${filename}: no exported \`tool\` descriptor`);
        return;
      }

      const previous = this.fileToName.get(filename);
      if (previous && previous !== mod.tool.name) {
        this.tools.delete(previous);
        this.emit("unregistered", previous);
      }

      this.tools.set(mod.tool.name, mod.tool);
      this.fileToName.set(filename, mod.tool.name);
      this.emit("registered", mod.tool.name);
    } catch (err) {
      console.error(`[mneme-mcp] failed to load ${filename}:`, err);
    }
  }

  /** Watch the tools directory; reload on change with 250ms debounce. */
  watch(): void {
    if (this.watcher) return;
    try {
      this.watcher = watch(this.toolsDir, { persistent: false }, (event, filename) => {
        if (!filename) return;
        const name = basename(filename);
        if (!name.endsWith(".ts") || name === "index.ts") return;

        const prev = this.reloadDebounce.get(name);
        if (prev) clearTimeout(prev);

        this.reloadDebounce.set(
          name,
          setTimeout(async () => {
            this.reloadDebounce.delete(name);
            try {
              const stats = await stat(join(this.toolsDir, name));
              if (stats.isFile()) {
                await this.loadFile(name);
              }
            } catch {
              // File was deleted — unregister.
              const toolName = this.fileToName.get(name);
              if (toolName) {
                this.tools.delete(toolName);
                this.fileToName.delete(name);
                this.emit("unregistered", toolName);
              }
            }
          }, 250),
        );
      });
    } catch (err) {
      console.error(`[mneme-mcp] failed to watch tools dir:`, err);
    }
  }

  unwatch(): void {
    if (this.watcher) {
      this.watcher.close();
      this.watcher = null;
    }
    for (const t of this.reloadDebounce.values()) clearTimeout(t);
    this.reloadDebounce.clear();
  }

  list(): ToolDescriptor[] {
    return Array.from(this.tools.values());
  }

  get(name: string): ToolDescriptor | undefined {
    return this.tools.get(name);
  }

  /** Validate input, run handler, validate output. Throws on validation error. */
  async invoke(name: string, input: unknown, ctx: ToolContext): Promise<unknown> {
    const t = this.tools.get(name);
    if (!t) throw new Error(`Unknown tool: ${name}`);
    const validatedInput = t.inputSchema.parse(input);
    const out = await t.handler(validatedInput, ctx);
    return t.outputSchema.parse(out);
  }
}

// ---------------------------------------------------------------------------
// Default instance — used by the MCP server.
// ---------------------------------------------------------------------------

const defaultToolsDir = dirname(fileURLToPath(import.meta.url));
export const registry = new ToolRegistry(defaultToolsDir);

export type { ToolDescriptor };
