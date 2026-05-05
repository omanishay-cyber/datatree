"""
mneme_parsers — tree-sitter-based code graph extraction.

This package wraps the Rust ``mneme-parsers`` crate via PyO3. No daemon
process is required; the tree-sitter grammars are linked directly into the
compiled extension module.

Functions
---------
parse_file(path: str | os.PathLike) -> Graph
    Parse a source file from disk. Language is detected from the file
    extension.

parse_source(language: str, source: str) -> Graph
    Parse an in-memory source string. ``language`` is a case-insensitive
    identifier such as ``"rust"``, ``"python"``, ``"typescript"``, etc.

Classes
-------
Graph
    Holds the extraction result.

    Attributes
    ----------
    nodes : list[Node]
    edges : list[Edge]

Node
    A single code element.

    Attributes
    ----------
    id       : str   — stable content-addressed id
    kind     : str   — "function" | "class" | "import" | "file" | ...
    name     : str   — human-readable identifier (empty for anonymous)
    path     : str   — file path
    line     : int   — 1-indexed start line
    end_line : int   — 1-indexed end line (inclusive)

Edge
    A directed relationship between two nodes.

    Attributes
    ----------
    source : str   — origin node id
    target : str   — destination node id
    kind   : str   — "calls" | "contains" | "imports" | "inherits" | ...

Examples
--------
>>> import mneme_parsers as mp
>>> g = mp.parse_source("python", "def greet(name):\\n    return f'Hello, {name}'\\n")
>>> [n.name for n in g.nodes if n.kind == "function"]
['greet']
"""

from __future__ import annotations

import os
from typing import TYPE_CHECKING

# Import the compiled Rust extension.  The .pyd / .so is placed by maturin
# next to this __init__.py at install time.
from .mneme_parsers import (  # type: ignore[import]
    Graph,
    Node,
    Edge,
    parse_file as _parse_file,
    parse_source,
)

__all__ = ["Graph", "Node", "Edge", "parse_file", "parse_source"]


def parse_file(path: "str | os.PathLike[str]") -> Graph:
    """Parse the source file at *path* and return its code graph.

    The language is detected automatically from the file extension.

    Parameters
    ----------
    path:
        Path to the source file, as a ``str`` or ``pathlib.Path``.

    Returns
    -------
    Graph

    Raises
    ------
    ValueError
        If the language cannot be determined or the file cannot be read.

    Examples
    --------
    >>> import mneme_parsers as mp
    >>> g = mp.parse_file("src/lib.rs")
    >>> print(g)
    Graph(nodes=..., edges=...)
    """
    return _parse_file(os.fspath(path))
