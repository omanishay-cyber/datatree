"""
Basic test suite for the mneme_parsers Python SDK.

Run with:  pytest sdk/python/tests/

These tests require the extension to be built first:
  cd sdk/python && maturin develop
"""

from __future__ import annotations

import os
import textwrap
import pathlib
import tempfile
import pytest

import mneme_parsers as mp


# ---------------------------------------------------------------------------
# parse_source — happy paths
# ---------------------------------------------------------------------------


def test_parse_source_rust_extracts_functions() -> None:
    src = textwrap.dedent("""\
        pub fn add(a: i32, b: i32) -> i32 { a + b }
        fn helper() -> i32 { 42 }
    """)
    g = mp.parse_source("rust", src)
    assert isinstance(g, mp.Graph)
    fn_names = {n.name for n in g.nodes if n.kind == "function"}
    assert "add" in fn_names, f"add missing; got {fn_names}"
    assert "helper" in fn_names, f"helper missing; got {fn_names}"


def test_parse_source_python_extracts_class_and_method() -> None:
    src = textwrap.dedent("""\
        class Dog:
            def bark(self):
                return 'woof'
    """)
    g = mp.parse_source("python", src)
    class_names = {n.name for n in g.nodes if n.kind == "class"}
    fn_names = {n.name for n in g.nodes if n.kind == "function"}
    assert "Dog" in class_names, f"Dog class missing; got {class_names}"
    assert "bark" in fn_names, f"bark method missing; got {fn_names}"


def test_parse_source_typescript_emits_import_edges() -> None:
    # K7: named imports fan-out to one edge per binding
    src = "import { useState, useEffect } from 'react';\nexport function App() { return null; }\n"
    g = mp.parse_source("typescript", src)
    import_edges = [e for e in g.edges if e.kind == "imports"]
    assert len(import_edges) == 2, (
        f"expected 2 import edges (one per binding); got {len(import_edges)}"
    )


def test_parse_source_javascript_extracts_function() -> None:
    src = "function greet(name) { return `Hello, ${name}`; }\n"
    g = mp.parse_source("javascript", src)
    fn_names = {n.name for n in g.nodes if n.kind == "function"}
    assert "greet" in fn_names


def test_parse_source_go_extracts_function() -> None:
    src = textwrap.dedent("""\
        package main
        func Add(a, b int) int { return a + b }
    """)
    g = mp.parse_source("go", src)
    fn_names = {n.name for n in g.nodes if n.kind == "function"}
    assert "Add" in fn_names, f"Add missing; got {fn_names}"


# ---------------------------------------------------------------------------
# parse_file — reads from disk
# ---------------------------------------------------------------------------


def test_parse_file_detects_language_from_extension() -> None:
    src = "pub fn hello() -> &'static str { \"hi\" }\n"
    with tempfile.NamedTemporaryFile(suffix=".rs", mode="w", delete=False) as f:
        f.write(src)
        tmp_path = f.name

    try:
        g = mp.parse_file(tmp_path)
        fn_names = {n.name for n in g.nodes if n.kind == "function"}
        assert "hello" in fn_names, f"hello missing; got {fn_names}"
    finally:
        os.unlink(tmp_path)


def test_parse_file_accepts_pathlib_path() -> None:
    src = "def compute():\n    return 42\n"
    with tempfile.NamedTemporaryFile(suffix=".py", mode="w", delete=False) as f:
        f.write(src)
        tmp_path = pathlib.Path(f.name)

    try:
        g = mp.parse_file(tmp_path)  # pathlib.Path, not str
        fn_names = {n.name for n in g.nodes if n.kind == "function"}
        assert "compute" in fn_names, f"compute missing; got {fn_names}"
    finally:
        tmp_path.unlink()


# ---------------------------------------------------------------------------
# Error paths
# ---------------------------------------------------------------------------


def test_parse_source_unknown_language_raises_value_error() -> None:
    with pytest.raises(ValueError, match="unknown language"):
        mp.parse_source("brainfuck", "+++")


def test_parse_file_nonexistent_path_raises_value_error() -> None:
    with pytest.raises(ValueError):
        mp.parse_file("/nonexistent/path/that/does/not/exist.rs")


def test_parse_file_unknown_extension_raises_value_error() -> None:
    with tempfile.NamedTemporaryFile(suffix=".xyz999", delete=False) as f:
        f.write(b"something")
        tmp_path = f.name
    try:
        with pytest.raises(ValueError):
            mp.parse_file(tmp_path)
    finally:
        os.unlink(tmp_path)


# ---------------------------------------------------------------------------
# Graph invariants
# ---------------------------------------------------------------------------


def test_graph_always_contains_file_node() -> None:
    g = mp.parse_source("rust", "fn x() {}")
    file_nodes = [n for n in g.nodes if n.kind == "file"]
    assert len(file_nodes) >= 1, "every graph must contain a File root node"


def test_node_attributes_are_accessible() -> None:
    g = mp.parse_source("python", "def f(): pass\n")
    fn_nodes = [n for n in g.nodes if n.kind == "function"]
    assert fn_nodes, "expected at least one function node"
    n = fn_nodes[0]
    assert isinstance(n.id, str) and n.id
    assert isinstance(n.name, str) and n.name == "f"
    assert isinstance(n.path, str)
    assert isinstance(n.line, int) and n.line >= 1
    assert isinstance(n.end_line, int) and n.end_line >= n.line


def test_edge_attributes_are_accessible() -> None:
    g = mp.parse_source("python", "class Foo:\n    def bar(self): pass\n")
    contains_edges = [e for e in g.edges if e.kind == "contains"]
    assert contains_edges, "expected at least one contains edge"
    e = contains_edges[0]
    assert isinstance(e.source, str) and e.source
    assert isinstance(e.target, str) and e.target
    assert isinstance(e.kind, str) and e.kind


def test_repr_is_informative() -> None:
    g = mp.parse_source("rust", "fn x() {}")
    r = repr(g)
    assert "Graph" in r and "nodes" in r and "edges" in r


# ---------------------------------------------------------------------------
# Language alias coverage
# ---------------------------------------------------------------------------


@pytest.mark.parametrize("alias", ["ts", "typescript", "TypeScript", "TS"])
def test_parse_source_accepts_typescript_aliases(alias: str) -> None:
    g = mp.parse_source(alias, "const x: number = 1;\n")
    assert g.nodes, f"alias `{alias}` produced empty graph"
