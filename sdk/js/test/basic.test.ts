/**
 * Basic test suite for the @mneme/parsers Node.js SDK.
 *
 * Run with: bun test sdk/js/test/
 *
 * These tests require the native addon to be built first:
 *   cd sdk/js && napi build --platform
 */

import { expect, test, describe } from 'bun:test';
import { parseFile, parseSource, type Graph, type Node, type Edge } from '..';
import { writeFileSync, unlinkSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';

// ---------------------------------------------------------------------------
// parseSource — happy paths
// ---------------------------------------------------------------------------

describe('parseSource — happy paths', () => {
  test('extracts Rust functions', async () => {
    const src = `
pub fn add(a: i32, b: i32) -> i32 { a + b }
fn helper() -> i32 { 42 }
`.trim();
    const g: Graph = await parseSource('rust', src);
    expect(g.nodes).toBeDefined();
    expect(g.edges).toBeDefined();

    const fns = g.nodes.filter((n: Node) => n.kind === 'function');
    const names = fns.map((n: Node) => n.name);
    expect(names).toContain('add');
    expect(names).toContain('helper');
  });

  test('extracts Python class and method', async () => {
    const src = 'class Dog:\n    def bark(self):\n        return "woof"\n';
    const g = await parseSource('python', src);
    const classes = g.nodes.filter((n: Node) => n.kind === 'class');
    const fns = g.nodes.filter((n: Node) => n.kind === 'function');
    expect(classes.some((n: Node) => n.name === 'Dog')).toBe(true);
    expect(fns.some((n: Node) => n.name === 'bark')).toBe(true);
  });

  test('emits per-binding import edges for TypeScript (K7)', async () => {
    const src = "import { useState, useEffect } from 'react';\nexport function App() { return null; }\n";
    const g = await parseSource('typescript', src);
    const importEdges = g.edges.filter((e: Edge) => e.kind === 'imports');
    // K7: 2 bindings → 2 edges
    expect(importEdges.length).toBe(2);
  });

  test('extracts JavaScript function', async () => {
    const src = 'function greet(name) { return `Hello, ${name}`; }\n';
    const g = await parseSource('js', src);
    const names = g.nodes.filter((n: Node) => n.kind === 'function').map((n: Node) => n.name);
    expect(names).toContain('greet');
  });

  test('extracts Go function', async () => {
    const src = 'package main\nfunc Add(a, b int) int { return a + b }\n';
    const g = await parseSource('go', src);
    const names = g.nodes.filter((n: Node) => n.kind === 'function').map((n: Node) => n.name);
    expect(names).toContain('Add');
  });

  test('accepts language aliases: ts / typescript / TypeScript', async () => {
    for (const alias of ['ts', 'typescript', 'TypeScript']) {
      const g = await parseSource(alias, 'const x: number = 1;\n');
      expect(g.nodes.length).toBeGreaterThan(0);
    }
  });
});

// ---------------------------------------------------------------------------
// parseFile — reads from disk
// ---------------------------------------------------------------------------

describe('parseFile — reads from disk', () => {
  test('detects language from .rs extension', async () => {
    const tmp = join(tmpdir(), `mneme_sdk_test_${Date.now()}.rs`);
    writeFileSync(tmp, "pub fn hello() -> &'static str { \"hi\" }\n");
    try {
      const g = await parseFile(tmp);
      const names = g.nodes.filter((n: Node) => n.kind === 'function').map((n: Node) => n.name);
      expect(names).toContain('hello');
    } finally {
      unlinkSync(tmp);
    }
  });

  test('detects language from .py extension', async () => {
    const tmp = join(tmpdir(), `mneme_sdk_test_${Date.now()}.py`);
    writeFileSync(tmp, 'def compute():\n    return 42\n');
    try {
      const g = await parseFile(tmp);
      const names = g.nodes.filter((n: Node) => n.kind === 'function').map((n: Node) => n.name);
      expect(names).toContain('compute');
    } finally {
      unlinkSync(tmp);
    }
  });
});

// ---------------------------------------------------------------------------
// Error paths
// ---------------------------------------------------------------------------

describe('error paths', () => {
  test('rejects unknown language', async () => {
    await expect(parseSource('brainfuck', '+++')).rejects.toThrow();
  });

  test('rejects nonexistent file path', async () => {
    await expect(parseFile('/nonexistent/path/that/does/not/exist.rs')).rejects.toThrow();
  });

  test('rejects unknown file extension', async () => {
    const tmp = join(tmpdir(), `mneme_sdk_test_${Date.now()}.xyz999`);
    writeFileSync(tmp, 'something');
    try {
      await expect(parseFile(tmp)).rejects.toThrow();
    } finally {
      unlinkSync(tmp);
    }
  });
});

// ---------------------------------------------------------------------------
// Graph structure invariants
// ---------------------------------------------------------------------------

describe('graph invariants', () => {
  test('every graph contains a file node', async () => {
    const g = await parseSource('rust', 'fn x() {}');
    const fileNodes = g.nodes.filter((n: Node) => n.kind === 'file');
    expect(fileNodes.length).toBeGreaterThanOrEqual(1);
  });

  test('node attributes are all present and typed correctly', async () => {
    const g = await parseSource('python', 'def f(): pass\n');
    const fn_nodes = g.nodes.filter((n: Node) => n.kind === 'function');
    expect(fn_nodes.length).toBeGreaterThan(0);
    const n = fn_nodes[0];
    expect(typeof n.id).toBe('string');
    expect(n.id.length).toBeGreaterThan(0);
    expect(typeof n.name).toBe('string');
    expect(n.name).toBe('f');
    expect(typeof n.path).toBe('string');
    expect(typeof n.line).toBe('number');
    expect(n.line).toBeGreaterThanOrEqual(1);
    expect(typeof n.endLine).toBe('number');
    expect(n.endLine).toBeGreaterThanOrEqual(n.line);
  });

  test('edge attributes are all present', async () => {
    const g = await parseSource('python', 'class Foo:\n    def bar(self): pass\n');
    const containsEdges = g.edges.filter((e: Edge) => e.kind === 'contains');
    expect(containsEdges.length).toBeGreaterThan(0);
    const e = containsEdges[0];
    expect(typeof e.source).toBe('string');
    expect(typeof e.target).toBe('string');
    expect(typeof e.kind).toBe('string');
  });

  test('graph is JSON serialisable', async () => {
    const g = await parseSource('rust', 'fn x() {}');
    const json = JSON.stringify(g);
    const back = JSON.parse(json) as Graph;
    expect(back.nodes.length).toBe(g.nodes.length);
    expect(back.edges.length).toBe(g.edges.length);
  });
});
