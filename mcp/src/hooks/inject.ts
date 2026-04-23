/**
 * Hook: UserPromptSubmit — Mode B smart inject (design §4.2).
 *
 * Embeds the user prompt, semantically searches the per-project shards for
 * the top 5–10 relevant facts, and composes a <datatree-context> block
 * (≤ 2.5K tokens) prepended to the prompt context.
 *
 * Output JSON shape: { additional_context: string }
 */

import { buildSmartInject } from "../composer.ts";
import { livebus } from "../db.ts";
import type { HookOutput } from "../types.ts";

export interface InjectArgs {
  prompt: string;
  sessionId: string;
  cwd: string;
}

export async function runInject(args: InjectArgs): Promise<HookOutput> {
  const t0 = Date.now();
  try {
    const bundle = await buildSmartInject({
      prompt: args.prompt,
      sessionId: args.sessionId,
      cwd: args.cwd,
    });

    void livebus.emit("prompt.injected", {
      session_id: args.sessionId,
      prompt_chars: args.prompt.length,
      bundle_chars: bundle.length,
      duration_ms: Date.now() - t0,
    });

    return {
      additional_context: bundle,
      metadata: {
        hook: "UserPromptSubmit",
        duration_ms: Date.now() - t0,
        session_id: args.sessionId,
      },
    };
  } catch (err) {
    console.error("[datatree-mcp] inject failed:", err);
    return {
      additional_context: "",
      metadata: { hook: "UserPromptSubmit", error: (err as Error).message },
    };
  }
}
