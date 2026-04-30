/**
 * MCP server wrapper around @modelcontextprotocol/sdk.
 *
 * Responsibilities:
 *   - Register every tool descriptor exposed by the registry.
 *   - Translate MCP `CallTool` requests into validated handler invocations.
 *   - React to hot-reload events from the registry by re-publishing the tool
 *     list to the connected client.
 *   - Serve over stdio (the only transport every harness supports).
 */

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  ListResourcesRequestSchema,
  ReadResourceRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { readFileSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { registry } from "./tools/index.ts";
import type { ToolContext, ToolDescriptor } from "./types.ts";
import { zodToJsonSchema } from "zod-to-json-schema";
import { getLastIndexed, graphStats, shardDbPath } from "./store.ts";

// ---------------------------------------------------------------------------
// Copyright line — matches mcp/package.json `author`. Wife-name placeholder
// dropped pending Anish's confirmation of canonical surname (Trivedi vs
// Patel). Do not remove the banner; only the contested second name.
// ---------------------------------------------------------------------------

const COPYRIGHT = "© 2026 Anish Trivedi & Kruti Trivedi";

// ---------------------------------------------------------------------------
// Server-level instructions — loaded into every client's first-turn context
// when the MCP server boots. This is the MCP-native alternative to per-turn
// hook nudges: one string, delivered once, zero crash surface per tool call.
// ---------------------------------------------------------------------------

const SERVER_INSTRUCTIONS = `╔══════════════════════════════════════════════════════════════╗
║                                                              ║
║   ███╗   ███╗███╗   ██╗███████╗███╗   ███╗███████╗          ║
║   ████╗ ████║████╗  ██║██╔════╝████╗ ████║██╔════╝          ║
║   ██╔████╔██║██╔██╗ ██║█████╗  ██╔████╔██║█████╗            ║
║   ██║╚██╔╝██║██║╚██╗██║██╔══╝  ██║╚██╔╝██║██╔══╝            ║
║   ██║ ╚═╝ ██║██║ ╚████║███████╗██║ ╚═╝ ██║███████╗          ║
║   ╚═╝     ╚═╝╚═╝  ╚═══╝╚══════╝╚═╝     ╚═╝╚══════╝          ║
║                                                              ║
║   persistent memory · code graph · drift detector · 47 tools ║
║   100% local · Apache-2.0 · connected ✓                      ║
║                                                              ║
║   ${COPYRIGHT.padEnd(58)}║
║                                                              ║
╚══════════════════════════════════════════════════════════════╝

You have access to Mneme — a local persistent memory + code-graph MCP. Prefer Mneme tools over Grep / Glob / Read for any question about code structure, history, decisions, blast radius, conventions, or drift. They are cheaper (tokens) and smarter (structural, not textual).

Decision tree — reach for Mneme first:
  "where is X?"             -> mneme_recall / recall_file / find_references
  "what breaks if I change?" -> blast_radius (returns risk + decisions assumed)
  "who calls this?"         -> call_graph (callers / callees / both)
  "why does this exist?"    -> mneme_why  (ledger + git + concepts)
  "did we decide this?"     -> recall_decision
  "TODOs / open questions?" -> recall_todo
  "rules for this file?"    -> recall_constraint
  "import cycles?"          -> cyclic_deps
  "what's the architecture?"-> architecture_overview + wiki_page
  "minimal context pls"     -> mneme_context (budget_tokens, anchors)
  "resume after compaction" -> mneme_resume / step_resume

Multi-step tasks: track with step_plan_from -> step_show -> step_verify -> step_complete. Call step_resume() after every context compaction or session restart. One fix = one step.

Budget: <= 5 Mneme tool calls per task, <= 800 tokens of graph-injected context per turn. Fall back to Grep/Read only when Mneme doesn't cover the question.

Full reference: read the MCP resource \`mneme://commands\` on demand.`;

// ---------------------------------------------------------------------------
// Phase A D4: dynamic context line. The static banner is identical for every
// project — to disambiguate which shard the model is talking to, append a
// one-line "Last indexed: <ago> · <N> nodes" suffix when a shard exists for
// the cwd. Skip silently when no shard, when the meta DB is unreadable, or
// when graphStats throws — the banner must never block startup.
//
// Phase A B1: when NO graph shard exists for the current project, append a
// short onboarding banner so first-run users see exactly what to do. The
// shard-missing branch takes priority over the "Last indexed" suffix.
// ---------------------------------------------------------------------------

const FIRST_RUN_BANNER =
  "First time on this project — run `mneme build .` then re-invoke any tool. Indexing typically takes ~10s per 100 files.";

function humanAgo(iso: string): string {
  const ts = Date.parse(iso);
  if (!Number.isFinite(ts)) return iso;
  const deltaMs = Date.now() - ts;
  if (deltaMs < 0) return "just now";
  const sec = Math.floor(deltaMs / 1000);
  if (sec < 60) return `${sec}s ago`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const day = Math.floor(hr / 24);
  if (day < 30) return `${day}d ago`;
  const mo = Math.floor(day / 30);
  if (mo < 12) return `${mo}mo ago`;
  const yr = Math.floor(day / 365);
  return `${yr}y ago`;
}

function buildDynamicBannerSuffix(): string {
  try {
    // Phase A B1: no shard for this cwd yet → show first-run onboarding.
    if (!shardDbPath("graph")) {
      return `\n\n${FIRST_RUN_BANNER}`;
    }
    let nodeCount = 0;
    try {
      nodeCount = graphStats().nodes;
    } catch {
      return "";
    }
    const lastIndexed = getLastIndexed();
    const ago = lastIndexed ? humanAgo(lastIndexed) : "unknown";
    return `\n\nLast indexed: ${ago} · ${nodeCount.toLocaleString()} nodes`;
  } catch {
    return "";
  }
}

function buildServerInstructions(): string {
  return SERVER_INSTRUCTIONS + buildDynamicBannerSuffix();
}

// Resolve the path to MNEME-COMMANDS.md — the full reference the
// mneme://commands resource serves. Look in the release payload (mcp/../plugin)
// first, then the dev tree.
function resolveCommandsPath(): string | null {
  const here = typeof import.meta.url === "string" ? fileURLToPath(import.meta.url) : "";
  if (!here) return null;
  const candidates = [
    resolve(dirname(here), "..", "..", "plugin", "MNEME-COMMANDS.md"),
    resolve(dirname(here), "..", "plugin", "MNEME-COMMANDS.md"),
    resolve(process.cwd(), "plugin", "MNEME-COMMANDS.md"),
  ];
  for (const c of candidates) {
    if (existsSync(c)) return c;
  }
  return null;
}

// ---------------------------------------------------------------------------
// zod -> JSON Schema. The MCP SDK advertises every tool's input shape to the
// client; previously we flattened everything to `{ type: "string" }` which
// broke clients that infer argument types from the schema (numbers became
// strings, booleans became strings, nested objects became strings, etc.).
// We delegate to `zod-to-json-schema` which understands the full zod
// vocabulary (numbers, booleans, arrays, unions, refinements, defaults, ...).
// ---------------------------------------------------------------------------

type JsonSchemaObject = {
  type: "object";
  properties?: Record<string, unknown>;
  required?: string[];
  additionalProperties?: boolean | unknown;
  [key: string]: unknown;
};

function toMcpInputSchema(descriptor: ToolDescriptor): JsonSchemaObject {
  const converted = zodToJsonSchema(descriptor.inputSchema, {
    target: "jsonSchema7",
    $refStrategy: "none",
  }) as Record<string, unknown>;
  // zod-to-json-schema may emit a top-level $schema / definitions wrapper;
  // strip non-MCP-relevant noise. MCP only needs an object-shaped schema.
  const { $schema: _schema, definitions: _definitions, ...rest } = converted;
  // Some zod constructs (z.union(...), z.discriminatedUnion(...)) emit a
  // root that isn't `{ type: "object" }`. Wrap it so MCP clients always see
  // an object-shaped advertised schema even though zod will still validate
  // the actual union at handler invocation time.
  if (rest.type !== "object") {
    return {
      type: "object",
      properties: {},
      additionalProperties: true,
    };
  }
  return rest as JsonSchemaObject;
}

// ---------------------------------------------------------------------------
// Server class
// ---------------------------------------------------------------------------

export class MnemeMcpServer {
  private server: Server;
  private transport: StdioServerTransport | null = null;
  private ctx: ToolContext;

  constructor(ctx: ToolContext) {
    this.ctx = ctx;
    this.server = new Server(
      {
        name: "mneme",
        version: "0.3.0",
      },
      {
        capabilities: {
          tools: {
            listChanged: true,
          },
          // Expose `mneme://commands` so any MCP client can fetch the full
          // command reference on demand without hooks.
          resources: {
            listChanged: false,
          },
        },
        // MCP-native channel for AI-facing guidance. Loaded into the
        // client's context on connection — zero per-tool-call overhead,
        // no crash surface, one source of truth. Phase A D4: append a
        // one-line dynamic context (last indexed + node count) so the
        // banner is no longer identical across projects.
        instructions: buildServerInstructions(),
      },
    );

    this.wire();
  }

  private wire(): void {
    this.server.setRequestHandler(ListToolsRequestSchema, async () => {
      const tools = registry.list().map((t) => ({
        name: t.name,
        description: t.description,
        inputSchema: toMcpInputSchema(t),
      }));
      return { tools };
    });

    // `mneme://commands` — the full human-readable reference. Clients that
    // want the decision tree + every tool's when-to-use can read this once
    // and cache it. No hook required; the client pulls on demand.
    this.server.setRequestHandler(ListResourcesRequestSchema, async () => {
      return {
        resources: [
          {
            uri: "mneme://commands",
            name: "Mneme command reference",
            description:
              "Full reference: decision tree, 47 MCP tools (all wired), 25 CLI commands, 13 slash commands, hook behavior, data locations.",
            mimeType: "text/markdown",
          },
          {
            uri: "mneme://identity",
            name: "Project identity kernel",
            description:
              "Auto-detected stack + domain summary + conventions + recent goals + open questions for the current project.",
            mimeType: "text/markdown",
          },
        ],
      };
    });

    this.server.setRequestHandler(ReadResourceRequestSchema, async (req) => {
      const uri = req.params.uri;
      if (uri === "mneme://commands") {
        const path = resolveCommandsPath();
        const text = path
          ? readFileSync(path, "utf8")
          : "Mneme command reference not found on disk. Run `mneme install` to populate.";
        return {
          contents: [{ uri, mimeType: "text/markdown", text }],
        };
      }
      if (uri === "mneme://identity") {
        // Delegate to the identity MCP tool, which knows how to assemble
        // the current project's identity kernel.
        try {
          const out = await registry.invoke(
            "mneme_identity",
            { scope: "project" },
            this.ctx,
          );
          const text = typeof out === "string" ? out : JSON.stringify(out, null, 2);
          return { contents: [{ uri, mimeType: "text/markdown", text }] };
        } catch (err) {
          const msg = err instanceof Error ? err.message : String(err);
          return {
            contents: [
              {
                uri,
                mimeType: "text/plain",
                text: `Identity not yet available: ${msg}. Run \`mneme build .\` first.`,
              },
            ],
          };
        }
      }
      throw new Error(`Unknown resource URI: ${uri}`);
    });

    this.server.setRequestHandler(CallToolRequestSchema, async (req) => {
      const { name, arguments: args } = req.params;
      const start = Date.now();
      try {
        const out = await registry.invoke(name, args ?? {}, this.ctx);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(out),
            },
          ],
          isError: false,
          _meta: { duration_ms: Date.now() - start },
        };
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify({ error: message }),
            },
          ],
          isError: true,
          _meta: { duration_ms: Date.now() - start },
        };
      }
    });

    // When the registry hot-reloads a tool, push a list-changed notification
    // so the client knows to re-fetch the tool catalog.
    const onChange = (): void => {
      void this.server.notification({
        method: "notifications/tools/list_changed",
      });
    };
    registry.on("registered", onChange);
    registry.on("unregistered", onChange);
  }

  async start(): Promise<void> {
    this.transport = new StdioServerTransport();
    await this.server.connect(this.transport);
  }

  async stop(): Promise<void> {
    await this.server.close().catch(() => {});
  }
}
