/**
 * MCP tool: mneme_federated_similar
 *
 * Moat 4 — Federated pattern matching, read path.
 *
 * Given a code snippet, compute a SimHash + MinHash fingerprint LOCALLY
 * (pure JS, zero network) and rank the top-k most-similar fingerprints in
 * the project's opt-in `federated.db` shard. The shard is produced by the
 * Rust `brain::federated` module during `mneme federated scan` — see the
 * CLI subcommand of the same name.
 *
 * # Local-only invariant
 *
 * This tool only reads the shard; it never opens a network socket. The
 * user-level `~/.mneme/federated.optin` marker file governs whether the
 * Rust `mneme federated sync` CLI *would* eventually upload fingerprints
 * (v0.3 feature). This tool itself is local-only regardless of opt-in.
 *
 * # Privacy guarantees
 *
 * - Source code never leaves the caller. The `code_snippet` argument is
 *   consumed in-process to compute the fingerprint, then discarded.
 * - The stored `source_file` column is LOCAL-ONLY and is never returned
 *   in the output — only the fingerprint fields are.
 */

import { createHash } from "node:crypto";
import { z } from "zod";
import { openShardDb } from "../store.ts";
import type { ToolDescriptor } from "../types.ts";

// ---------------------------------------------------------------------------
// Constants (must match brain::federated.rs)
// ---------------------------------------------------------------------------

const MINHASH_K = 128;
const SIMHASH_BITS = 64;

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

const FingerprintOut = z.object({
  pattern_kind: z.string(),
  simhash: z.string(), // u64 stringified to avoid JS precision loss
  ast_shape: z.string(),
  span_tokens: z.number().int().nonnegative(),
  created_at: z.number().int(),
  similarity: z.number().min(0).max(1),
  simhash_similarity: z.number().min(0).max(1),
  jaccard: z.number().min(0).max(1),
});

/**
 * HIGH-21 fix (2026-05-05 audit): cap `code_snippet` at 64 KiB.
 *
 * The previous schema only enforced `.min(1)`. The downstream
 * tokenize / simhash64 / minhashK loops are all O(n) in token count
 * and run synchronously on Bun's main thread — a 100 MB snippet would
 * stall the MCP server for seconds, causing every other concurrent
 * tool call to time out. That's a local DoS reachable from any MCP
 * client (Claude Code, Cursor, etc.). 64 KiB is bigger than any
 * realistic single-symbol snippet (the largest function in the
 * audited mneme repo is ~12 KB) but bounded enough that the
 * synchronous loops complete in milliseconds.
 */
const MAX_CODE_SNIPPET_BYTES = 64 * 1024;

export const FederatedSimilarInput = z.object({
  code_snippet: z.string().min(1).max(MAX_CODE_SNIPPET_BYTES),
  pattern_kind: z.string().optional(),
  k: z.number().int().positive().max(50).default(10),
});

export const FederatedSimilarOutput = z.object({
  total_indexed: z.number().int().nonnegative(),
  query: z.object({
    pattern_kind: z.string(),
    simhash: z.string(),
    ast_shape: z.string(),
    span_tokens: z.number().int().nonnegative(),
  }),
  hits: z.array(FingerprintOut),
});

type FederatedSimilarInputT = z.infer<typeof FederatedSimilarInput>;
type FederatedSimilarOutputT = z.infer<typeof FederatedSimilarOutput>;

// ---------------------------------------------------------------------------
// Hashing primitives — must produce bit-identical results to the Rust side.
// ---------------------------------------------------------------------------

function tokenize(content: string): string[] {
  const tokens: string[] = [];
  let cur = "";
  for (const ch of content) {
    const isAlnum = /[A-Za-z0-9_]/.test(ch);
    if (isAlnum) {
      cur += ch.toLowerCase();
    } else if (cur.length > 0) {
      tokens.push(cur);
      cur = "";
    }
  }
  if (cur.length > 0) tokens.push(cur);
  return tokens;
}

/** 64-bit hash via SHA-256 truncation (little-endian, first 8 bytes). */
function hash64(input: string): bigint {
  const h = createHash("sha256").update(input).digest();
  let out = 0n;
  for (let i = 0; i < 8; i++) {
    out |= BigInt(h[i] ?? 0) << BigInt(8 * i);
  }
  return out;
}

/** Salted 32-bit hash used for MinHash permutations. */
function hash32Salted(input: string, salt: number): number {
  const saltBytes = Buffer.alloc(4);
  saltBytes.writeUInt32LE(salt >>> 0, 0);
  const h = createHash("sha256")
    .update(saltBytes)
    .update(input)
    .digest();
  return h.readUInt32LE(0);
}

function simhash64(tokens: string[]): bigint {
  if (tokens.length === 0) return 0n;
  const bits = new Array<number>(SIMHASH_BITS).fill(0);
  for (const tok of tokens) {
    const h = hash64(tok);
    for (let i = 0; i < SIMHASH_BITS; i++) {
      const bit = Number((h >> BigInt(i)) & 1n);
      const cur = bits[i] ?? 0;
      bits[i] = cur + (bit === 1 ? 1 : -1);
    }
  }
  let out = 0n;
  for (let i = 0; i < SIMHASH_BITS; i++) {
    if ((bits[i] ?? 0) > 0) {
      out |= 1n << BigInt(i);
    }
  }
  return out;
}

function minhashK(tokens: string[], k: number): Uint32Array {
  const sketch = new Uint32Array(k).fill(0xffffffff);
  if (tokens.length === 0) return sketch;
  for (const tok of tokens) {
    for (let i = 0; i < k; i++) {
      const h = hash32Salted(tok, i);
      const cur = sketch[i] ?? 0xffffffff;
      if (h < cur) sketch[i] = h;
    }
  }
  return sketch;
}

const KEEP = new Set([
  "fn", "function", "def", "class", "impl", "trait", "struct", "enum", "pub", "async",
  "await", "return", "if", "else", "for", "while", "match", "try", "catch", "throw",
  "Result", "Option", "Vec", "String", "self", "let", "const", "var", "use", "import",
]);

function normaliseShape(content: string): string {
  let out = "";
  let cur = "";
  for (const ch of content) {
    const isAlnum = /[A-Za-z0-9_]/.test(ch);
    if (isAlnum) {
      cur += ch;
    } else {
      if (cur.length > 0) {
        if (KEEP.has(cur)) {
          if (out.length > 0) out += " ";
          out += cur;
        }
        cur = "";
      }
      if ("()[]{}<>,;:&*?!|-=".includes(ch)) {
        out += ch;
      }
    }
    if (out.length > 512) break;
  }
  if (cur.length > 0 && KEEP.has(cur)) {
    if (out.length > 0) out += " ";
    out += cur;
  }
  return out;
}

function popcount64(x: bigint): number {
  let count = 0;
  let n = x;
  while (n > 0n) {
    if ((n & 1n) === 1n) count++;
    n >>= 1n;
  }
  return count;
}

function simhashSimilarity(a: bigint, b: bigint): number {
  const hamming = popcount64(a ^ b);
  return 1 - hamming / SIMHASH_BITS;
}

function jaccard(a: Uint32Array, b: Uint32Array): number {
  if (a.length !== b.length || a.length === 0) return 0;
  let matches = 0;
  for (let i = 0; i < a.length; i++) {
    const av = a[i];
    const bv = b[i];
    if (av !== undefined && bv !== undefined && av === bv) matches++;
  }
  return matches / a.length;
}

// ---------------------------------------------------------------------------
// MinHash blob decoder (mirror of bincode Vec<u32>)
// ---------------------------------------------------------------------------
//
// bincode's default serialisation of `Vec<u32>` is: u64 LE length followed
// by `length` little-endian u32s. The Rust writer uses the default config,
// so this decoder must match byte-for-byte.
function decodeMinhashBlob(blob: Buffer | Uint8Array): Uint32Array {
  const buf = Buffer.isBuffer(blob) ? blob : Buffer.from(blob);
  if (buf.length < 8) return new Uint32Array();
  const lenLo = buf.readUInt32LE(0);
  const lenHi = buf.readUInt32LE(4);
  const len = lenLo + lenHi * 0x1_0000_0000;
  const expected = 8 + len * 4;
  if (buf.length < expected) return new Uint32Array();
  const out = new Uint32Array(len);
  for (let i = 0; i < len; i++) {
    out[i] = buf.readUInt32LE(8 + i * 4);
  }
  return out;
}

// ---------------------------------------------------------------------------
// Tool descriptor
// ---------------------------------------------------------------------------

interface Row {
  pattern_kind: string;
  simhash: bigint | number;
  minhash: Buffer | Uint8Array;
  ast_shape: string;
  span_tokens: bigint | number;
  created_at: bigint | number;
}

export const tool: ToolDescriptor<FederatedSimilarInputT, FederatedSimilarOutputT> = {
  name: "mneme_federated_similar",
  description:
    "Moat 4: find the top-k locally-indexed patterns similar to a code snippet. Computes a SimHash+MinHash fingerprint in-process (never uploads code), then ranks the opt-in federated shard. Source code NEVER leaves the caller. Requires `mneme federated scan` to have populated the shard first.",
  inputSchema: FederatedSimilarInput,
  outputSchema: FederatedSimilarOutput,
  category: "recall",
  async handler(input, ctx): Promise<FederatedSimilarOutputT> {
    const kind = input.pattern_kind ?? "func_signature";
    const tokens = tokenize(input.code_snippet);
    const querySim = simhash64(tokens);
    const queryMin = minhashK(tokens, MINHASH_K);
    const queryShape = normaliseShape(input.code_snippet);

    const queryOut = {
      pattern_kind: kind,
      simhash: querySim.toString(),
      ast_shape: queryShape,
      span_tokens: tokens.length,
    };

    let rows: Row[] = [];
    let total = 0;
    try {
      const db = openShardDb("federated", ctx.cwd);
      try {
        total = (db.prepare("SELECT COUNT(*) AS c FROM pattern_fingerprints").get() as
          | { c: number }
          | undefined)?.c ?? 0;
        // Use safeIntegers so the 64-bit simhash column round-trips
        // precisely as BigInt rather than a lossy JS number.
        const stmt = db
          .prepare(
            `SELECT pattern_kind, simhash, minhash, ast_shape,
                    span_tokens, created_at
               FROM pattern_fingerprints
               WHERE pattern_kind = ?
               ORDER BY created_at DESC
               LIMIT 512`,
          );
        type WithSafe = { safeIntegers?: (flag: boolean) => unknown };
        const maybeSafe = (stmt as unknown as WithSafe).safeIntegers;
        if (typeof maybeSafe === "function") {
          maybeSafe.call(stmt, true);
        }
        rows = stmt.all(kind) as Row[];
      } finally {
        db.close();
      }
    } catch {
      // Shard missing or not yet built — return empty hits rather than error.
      return { total_indexed: 0, query: queryOut, hits: [] };
    }

    const scored = rows
      .map((r) => {
        const rawSim = r.simhash ?? 0;
        const candidateSim = BigInt.asUintN(
          64,
          typeof rawSim === "bigint" ? rawSim : BigInt(rawSim),
        );
        const candidateMin = decodeMinhashBlob(r.minhash);
        const sim = simhashSimilarity(querySim, candidateSim);
        const jac = jaccard(queryMin, candidateMin);
        const combined = 0.5 * sim + 0.5 * jac;
        return {
          pattern_kind: r.pattern_kind,
          simhash: candidateSim.toString(),
          ast_shape: r.ast_shape,
          span_tokens: Number(r.span_tokens ?? 0),
          created_at: Number(r.created_at ?? 0),
          similarity: combined,
          simhash_similarity: sim,
          jaccard: jac,
        };
      })
      .sort((a, b) => b.similarity - a.similarity)
      .slice(0, input.k);

    return {
      total_indexed: total,
      query: queryOut,
      hits: scored,
    };
  },
};
