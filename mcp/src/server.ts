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
} from "@modelcontextprotocol/sdk/types.js";
import { registry } from "./tools/index.ts";
import type { ToolContext, ToolDescriptor } from "./types.ts";

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

export class DatatreeMcpServer {
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
        },
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
