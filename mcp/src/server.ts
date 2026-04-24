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

// ---------------------------------------------------------------------------
// Server-level instructions — loaded into every client's first-turn context
// when the MCP server boots. This is the MCP-native alternative to per-turn
// hook nudges: one string, delivered once, zero crash surface per tool call.
// ---------------------------------------------------------------------------

const SERVER_INSTRUCTIONS = `You have access to Mneme — a local persistent memory + code-graph MCP. Prefer Mneme tools over Grep / Glob / Read for any question about code structure, history, decisions, blast radius, conventions, or drift. They are cheaper (tokens) and smarter (structural, not textual).

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
// zod-to-jsonschema (minimal, just what MCP needs)
// ---------------------------------------------------------------------------

function toMcpInputSchema(descriptor: ToolDescriptor): {
  type: "object";
  properties: Record<string, unknown>;
  required?: string[];
} {
  // The MCP SDK accepts any JSON-Schema-shaped object. zod doesn't ship
  // first-party JSON schema emission; we describe inputs as a generic
  // "object" shape and rely on zod's runtime validation in the handler.
  // Concrete shape per tool is documented in types.ts.
  const shape =
    "shape" in descriptor.inputSchema
      ? (descriptor.inputSchema as unknown as { shape: Record<string, unknown> }).shape
      : {};
  const properties: Record<string, { type: string; description?: string }> = {};
  for (const key of Object.keys(shape)) {
    properties[key] = { type: "string" };
  }
  return {
    type: "object",
    properties,
  };
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
        version: "0.2.0",
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
        // no crash surface, one source of truth.
        instructions: SERVER_INSTRUCTIONS,
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
