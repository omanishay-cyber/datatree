// mcp/src/__tests__/recall-node.test.ts
//
// Coverage for `_recallNodeOnDb` — the FTS5-with-LIKE-fallback
// recall path. Audit fix TEST-NEW-2 (2026-05-06 multi-agent fan-out,
// testing-reviewer): the HIGH-27 root-cause fix introduced four
// branches (sanitisation collapses -> LIKE; FTS5 MATCH succeeds -> rows;
// FTS5 MATCH throws -> silent fall-through; FTS5 returns zero rows ->
// LIKE) and ALL of them shipped untested. A future schema change
// could break the FTS shadow table and the silent fall-through would
// hide the regression behind LIKE results.

import { describe, expect, it } from "bun:test";
import { Database } from "bun:sqlite";
import { _recallNodeOnDb } from "../store.ts";

/**
 * Build an in-memory shard DB matching the canonical
 * store/src/schema.rs::GRAPH_SQL shape: `nodes` table + `nodes_fts`
 * shadow table + the AFTER INSERT/UPDATE/DELETE triggers that
 * keep the two in sync.
 *
 * We keep the schema minimal — just the columns and triggers
 * `_recallNodeOnDb` cares about. Production schema has more
 * columns; the test only exercises the recall path.
 */
function makeShardDb(): Database {
  const db = new Database(":memory:");
  // Each statement run individually (bun:sqlite's multi-statement
  // .exec() reads identically to JS child_process.exec to some
  // static linters; using prepare/run keeps the lints quiet and
  // is just as fast on tiny in-memory schemas).
  db.prepare(
    "CREATE TABLE nodes (" +
      "id INTEGER PRIMARY KEY, " +
      "qualified_name TEXT NOT NULL, " +
      "name TEXT NOT NULL, " +
      "kind TEXT NOT NULL, " +
      "file_path TEXT" +
      ")",
  ).run();
  db.prepare(
    "CREATE VIRTUAL TABLE nodes_fts USING fts5(name, qualified_name, tokenize='porter')",
  ).run();
  db.prepare(
    "CREATE TRIGGER nodes_ai AFTER INSERT ON nodes BEGIN " +
      "INSERT INTO nodes_fts(rowid, name, qualified_name) " +
      "VALUES (new.id, new.name, new.qualified_name); " +
      "END",
  ).run();
  db.prepare(
    "CREATE TRIGGER nodes_ad AFTER DELETE ON nodes BEGIN " +
      "DELETE FROM nodes_fts WHERE rowid = old.id; " +
      "END",
  ).run();
  return db;
}

function makePlainNodesDb(): Database {
  // No nodes_fts shadow table — simulates the case where FTS is
  // missing (legacy build, schema migration mid-flight). The FTS5
  // branch must throw on prepare(), the catch must swallow it,
  // and the LIKE fallback must run.
  const db = new Database(":memory:");
  db.prepare(
    "CREATE TABLE nodes (" +
      "id INTEGER PRIMARY KEY, " +
      "qualified_name TEXT NOT NULL, " +
      "name TEXT NOT NULL, " +
      "kind TEXT NOT NULL, " +
      "file_path TEXT" +
      ")",
  ).run();
  return db;
}

function seedNode(
  db: Database,
  id: number,
  name: string,
  qualified_name: string,
  kind = "function",
  file_path: string | null = "src/lib.rs",
): void {
  db.prepare(
    "INSERT INTO nodes(id, qualified_name, name, kind, file_path) VALUES (?, ?, ?, ?, ?)",
  ).run(id, qualified_name, name, kind, file_path);
}

describe("_recallNodeOnDb", () => {
  it("returns FTS5-matched rows for a simple word query", () => {
    const db = makeShardDb();
    try {
      seedNode(db, 1, "parser", "n_aaa::parser");
      seedNode(db, 2, "tokenizer", "n_bbb::tokenizer");

      const out = _recallNodeOnDb(db, "parser", 10);
      expect(out).toHaveLength(1);
      expect(out[0]?.qualified_name).toBe("n_aaa::parser");
    } finally {
      db.close();
    }
  });

  it("uses prefix wildcards so 'parse' also matches 'parser'", () => {
    const db = makeShardDb();
    try {
      seedNode(db, 1, "parser", "n_aaa::parser");
      seedNode(db, 2, "parserState", "n_bbb::parserState");

      // Per the implementation, every token gets `*` appended for
      // prefix match. So "parse" -> "parse*" -> matches both.
      const out = _recallNodeOnDb(db, "parse", 10);
      const names = out.map((r) => r.qualified_name).sort();
      expect(names).toEqual(["n_aaa::parser", "n_bbb::parserState"]);
    } finally {
      db.close();
    }
  });

  it("strips FTS5-syntax characters during sanitisation (does not throw)", () => {
    const db = makeShardDb();
    try {
      seedNode(db, 1, "parser", "n_aaa::parser");

      // Pre-fix, an input containing FTS5-grammar chars (`:` `(` `)`)
      // was passed straight to FTS5 and triggered a syntax error. The
      // sanitiser now strips them. The function MUST NOT throw —
      // the value contract is "no exception", not a particular hit
      // count (FTS5's default operator is AND, so "foo* parser* bar*"
      // requires all three terms; "parser" alone wouldn't satisfy it
      // and the function falls through to LIKE on zero rows, which
      // also doesn't match because LIKE searches "%foo:parser(bar)%"
      // — neither path crashes).
      let out: ReturnType<typeof _recallNodeOnDb> | undefined;
      expect(() => {
        out = _recallNodeOnDb(db, "foo:parser(bar)", 10);
      }).not.toThrow();
      expect(Array.isArray(out)).toBe(true);
    } finally {
      db.close();
    }
  });

  it("strips FTS5-syntax chars and matches when the punctuation surrounds a real term", () => {
    const db = makeShardDb();
    try {
      seedNode(db, 1, "parser", "n_aaa::parser");

      // Single-token input wrapped in FTS5-grammar chars: after
      // sanitisation -> "parser" -> "parser*" -> matches. This is
      // the load-bearing case the production fix protects: a user
      // typing "parser:" or "(parser)" in a recall query.
      const out = _recallNodeOnDb(db, "(parser)", 10);
      expect(out).toHaveLength(1);
      expect(out[0]?.qualified_name).toBe("n_aaa::parser");
    } finally {
      db.close();
    }
  });

  it("returns empty when nothing matches at all (FTS5 zero, LIKE zero)", () => {
    const db = makeShardDb();
    try {
      seedNode(db, 1, "parser", "n_aaa::parser");

      const out = _recallNodeOnDb(db, "completely_unmatched_xyz", 10);
      expect(out).toEqual([]);
    } finally {
      db.close();
    }
  });

  it("falls back to LIKE when FTS5 shadow table is absent", () => {
    // Build a DB WITHOUT nodes_fts so the FTS5 prepare() throws.
    // The fall-through branch must catch the throw and run LIKE.
    const db = makePlainNodesDb();
    try {
      seedNode(db, 1, "parser", "n_aaa::parser");

      const out = _recallNodeOnDb(db, "parser", 10);
      // LIKE branch is case-insensitive (lower(name) LIKE '%parser%'),
      // so we still get the row back. Without the fall-through this
      // would have thrown.
      expect(out).toHaveLength(1);
      expect(out[0]?.qualified_name).toBe("n_aaa::parser");
    } finally {
      db.close();
    }
  });

  it("falls back to LIKE when sanitisation collapses input to empty", () => {
    // Pure-punctuation input: the sanitiser strips it all, leaving
    // an empty string. The FTS5 branch never runs (the
    // `sanitised.length > 0` guard short-circuits), and we hit the
    // LIKE branch directly with the original (lowercased) query.
    const db = makeShardDb();
    try {
      seedNode(db, 1, "weird::name", "n_aaa::weird::name");
      seedNode(db, 2, "other", "n_bbb::other");

      // Query "::" sanitises to "" — but LIKE '%::%' should still
      // match the qualified_names containing colons.
      const out = _recallNodeOnDb(db, "::", 10);
      expect(out.length).toBeGreaterThanOrEqual(1);
      expect(
        out.some((r) => r.qualified_name === "n_aaa::weird::name"),
      ).toBe(true);
    } finally {
      db.close();
    }
  });

  it("respects the limit parameter", () => {
    const db = makeShardDb();
    try {
      for (let i = 1; i <= 10; i++) {
        seedNode(db, i, `parser${i}`, `n_aaa::parser${i}`);
      }
      const out = _recallNodeOnDb(db, "parser", 3);
      expect(out).toHaveLength(3);
    } finally {
      db.close();
    }
  });
});
