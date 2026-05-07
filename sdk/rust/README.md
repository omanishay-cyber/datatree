# mneme-parsers-rs

Rust SDK for mneme's tree-sitter-based code graph extractor.

Parse source files into a typed graph of nodes (functions, classes, imports,
…) and directed edges (calls, contains, imports, inherits, …) with no daemon
process required.

## Installation

```toml
[dependencies]
mneme-parsers-rs = "0.3"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Usage

```rust
use mneme_parsers_rs::{parse_file, parse_source};

#[tokio::main]
async fn main() -> Result<(), mneme_parsers_rs::ParseError> {
    // Parse a file from disk — language detected from extension
    let graph = parse_file("src/main.rs").await?;
    println!("{} nodes, {} edges", graph.nodes.len(), graph.edges.len());

    // Parse an in-memory string — language supplied explicitly
    let src = "def greet(name):\n    return f'Hello, {name}'\n";
    let graph = parse_source("python", src).await?;
    for node in &graph.nodes {
        println!("{:?}  {}", node.kind, node.name);
    }

    Ok(())
}
```

## Supported languages

**Tier 1 (always available):** TypeScript, TSX, JavaScript, JSX, Python, Rust,
Go, Java, C, C++, C#, Ruby, PHP, Bash, JSON, TOML, YAML, Markdown, Lua

**Tier 2 (enabled by default, can be toggled via Cargo features):** Swift,
Kotlin, Scala, Julia, Zig, Haskell, Svelte, Solidity

## Graph types

| Type | Description |
|------|-------------|
| `Graph` | The extraction result — holds `nodes` and `edges` |
| `Node` | A code element: function, class, import, variable, … |
| `Edge` | A directed relationship between two nodes |
| `NodeKind` | Discriminant: `Function`, `Class`, `Import`, `File`, … |
| `EdgeKind` | Discriminant: `Calls`, `Contains`, `Imports`, `Inherits`, … |

Both `Node` and `Edge` implement `serde::Serialize` / `Deserialize`, so you
can round-trip through JSON with `serde_json`.

## Error handling

All errors are covered by the `ParseError` enum, which you can match
exhaustively:

```rust
use mneme_parsers_rs::ParseError;

match parse_file("unknown.xyz").await {
    Err(ParseError::UnknownLanguage { path }) => eprintln!("no language for {path}"),
    Err(ParseError::LanguageNotEnabled { language }) => eprintln!("{language} not compiled in"),
    Err(ParseError::Io { path, source }) => eprintln!("I/O: {path}: {source}"),
    Err(e) => eprintln!("other error: {e}"),
    Ok(graph) => { /* use graph */ }
}
```

## No daemon required

This SDK runs entirely in-process. It does not start or connect to the mneme
daemon. The full tree-sitter grammar set is linked directly into your binary.

## License

Mneme Personal-Use License — same as the mneme project. Source-available, not open-source.
