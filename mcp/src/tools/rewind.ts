/**
 * MCP tool: rewind
 *
 * Show a file's content as of a past snapshot.
 *
 * v0.1 (review P2): snapshots live at
 * `~/.mneme/projects/<id>/snapshots/<snapshot_id>/<layer>.db`. We walk the
 * `graph.db` inside the requested snapshot and return the `files.*`
 * metadata for the requested path. Actual file content (pre-supervisor-
 * blob-store) isn't stored in v0.1, so we return a typed placeholder
 * with the hash — which is what historical comparisons actually need.
 *
 * When the "when" argument doesn't match any snapshot we fall back to
 * returning the list of available snapshot ids in the `content` field
 * as a newline-separated string, so the caller can pick one.
 */

import { existsSync } from "node:fs";
import { Database } from "bun:sqlite";
import {
  RewindInput,
  RewindOutput,
  type ToolDescriptor,
} from "../types.ts";
import { listSnapshotsFs, snapshotLayerPath } from "../store.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof RewindInput.parse>,
  ReturnType<typeof RewindOutput.parse>
> = {
  name: "rewind",
  description:
    "Show file content as of a past timestamp (RFC3339) or named snapshot id. Returns content + hash. Use to compare current state against any historical version.",
  inputSchema: RewindInput,
  outputSchema: RewindOutput,
  category: "time",
  async handler(input) {
    const snapshots = listSnapshotsFs();
    // Resolve `when` to a snapshot id — accept either an exact id match
    // or the newest snapshot created at-or-before an RFC3339 timestamp.
    let chosen = snapshots.find((s) => s.id === input.when);
    if (!chosen) {
      const ts = Date.parse(input.when);
      if (!Number.isNaN(ts)) {
        chosen = snapshots.find(
          (s) => Date.parse(s.captured_at) <= ts,
        );
      }
    }
    if (!chosen) {
      const listing = snapshots.map((s) => s.id).join("\n");
      return {
        file: input.file,
        when: input.when,
        // No matching snapshot — return the available list so the caller
        // can pick one. Bytes are unavailable.
        content_summary: listing,
        content_available: false,
        hash: "",
      };
    }

    const graphDb = snapshotLayerPath(chosen.id, "graph");
    if (!graphDb || !existsSync(graphDb)) {
      return {
        file: input.file,
        when: chosen.id,
        content_summary: "",
        content_available: false,
        hash: "",
      };
    }

    const db = new Database(graphDb, { readonly: true });
    try {
      const row = db
        .prepare(
          `SELECT sha256, language, line_count, byte_count, last_parsed_at
           FROM files WHERE path = ? LIMIT 1`,
        )
        .get(input.file) as
        | {
            sha256: string;
            language: string | null;
            line_count: number | null;
            byte_count: number | null;
            last_parsed_at: string;
          }
        | undefined;
      if (!row) {
        return {
          file: input.file,
          when: chosen.id,
          content_summary: "",
          content_available: false,
          hash: "",
        };
      }
      // TODO(v0.4): when snapshots store file bytes, return them here and
      // set `content_available: true`. Until then this string is a
      // metadata-only summary — callers must not treat it as the
      // file's actual content.
      const summary =
        `(snapshot metadata for ${input.file} @ ${chosen.id})\n` +
        `language: ${row.language ?? "?"}\n` +
        `lines: ${row.line_count ?? "?"}\n` +
        `bytes: ${row.byte_count ?? "?"}\n` +
        `parsed_at: ${row.last_parsed_at}\n`;
      return {
        file: input.file,
        when: chosen.id,
        content_summary: summary,
        content_available: false,
        hash: row.sha256,
      };
    } finally {
      try {
        db.close();
      } catch {
        // ignore
      }
    }
  },
};
