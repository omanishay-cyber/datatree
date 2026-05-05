/**
 * @mneme/parsers — tree-sitter-based code graph extraction for Node.js
 *
 * No daemon required. Runs entirely in-process.
 *
 * @example
 * ```ts
 * import { parseFile, parseSource } from '@mneme/parsers';
 *
 * // From a file path — language detected from extension
 * const g = await parseFile('src/main.rs');
 * console.log(`${g.nodes.length} nodes, ${g.edges.length} edges`);
 *
 * // From an in-memory string — language supplied explicitly
 * const g2 = await parseSource('python', 'def greet(name):\n    return f"Hi, {name}"\n');
 * const fns = g2.nodes.filter(n => n.kind === 'function');
 * console.log(fns[0].name); // 'greet'
 * ```
 */

// ---------------------------------------------------------------------------
// NodeKind values
// ---------------------------------------------------------------------------

/**
 * The type of a code element.
 *
 * - `"function"` — function or top-level arrow function
 * - `"method"` — class method
 * - `"class"` — class, struct, or trait (language-dependent)
 * - `"struct"` — struct (where the grammar distinguishes from class)
 * - `"trait"` — trait or interface
 * - `"interface"` — interface
 * - `"enum"` — enum
 * - `"module"` — module or namespace
 * - `"variable"` — variable binding
 * - `"constant"` — constant
 * - `"decorator"` — decorator / attribute
 * - `"comment"` — comment block
 * - `"import"` — import statement
 * - `"file"` — the file root node (always present; anchors the graph)
 */
export type NodeKind =
  | 'function'
  | 'method'
  | 'class'
  | 'struct'
  | 'trait'
  | 'interface'
  | 'enum'
  | 'module'
  | 'variable'
  | 'constant'
  | 'decorator'
  | 'comment'
  | 'import'
  | 'file';

// ---------------------------------------------------------------------------
// EdgeKind values
// ---------------------------------------------------------------------------

/**
 * The type of a directed relationship between two nodes.
 *
 * - `"calls"` — caller calls callee
 * - `"contains"` — outer scope contains inner element
 * - `"imports"` — file imports a module or binding
 * - `"inherits"` — class inherits from parent
 * - `"implements"` — class implements a trait or interface
 * - `"decorated_by"` — function is decorated by a decorator
 * - `"generic"` — any other relationship
 */
export type EdgeKind =
  | 'calls'
  | 'contains'
  | 'imports'
  | 'inherits'
  | 'implements'
  | 'decorated_by'
  | 'generic';

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/** A single code element extracted from a source file. */
export interface Node {
  /** Stable content-addressed identifier (blake3 hash prefix). */
  id: string;
  /** What kind of code element this is. */
  kind: NodeKind;
  /** Human-readable name (empty string for anonymous elements). */
  name: string;
  /** Absolute or synthetic file path. */
  path: string;
  /** 1-indexed start line. */
  line: number;
  /** 1-indexed end line (inclusive). */
  endLine: number;
}

/** A directed relationship between two nodes in the code graph. */
export interface Edge {
  /** Node `id` of the origin. */
  source: string;
  /** Node `id` of the destination. */
  target: string;
  /** What kind of relationship this is. */
  kind: EdgeKind;
}

/** The result of a parse operation. */
export interface Graph {
  /** All code elements extracted from the parsed source. */
  nodes: Node[];
  /** All directed relationships between nodes. */
  edges: Edge[];
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

/**
 * Parse the file at `path` and return its code graph.
 *
 * The language is detected automatically from the file extension.
 *
 * @param path - Absolute or relative path to the source file.
 * @returns Promise that resolves to a `Graph`.
 * @throws If the language cannot be determined or the file cannot be read.
 *
 * @example
 * ```ts
 * const g = await parseFile('src/lib.rs');
 * console.log(g.nodes.filter(n => n.kind === 'function').map(n => n.name));
 * ```
 */
export declare function parseFile(path: string): Promise<Graph>;

/**
 * Parse an in-memory `source` string for the given `language`.
 *
 * `language` is case-insensitive. Common values:
 * `"rust"`, `"python"`, `"typescript"` / `"ts"`, `"tsx"`,
 * `"javascript"` / `"js"`, `"jsx"`, `"go"`, `"java"`, `"c"`, `"cpp"`,
 * `"csharp"`, `"ruby"`, `"php"`, `"bash"`, `"json"`, `"toml"`, `"yaml"`,
 * `"markdown"`, `"swift"`, `"kotlin"`, `"scala"`, `"julia"`, `"zig"`,
 * `"haskell"`, `"svelte"`, `"solidity"`.
 *
 * @param language - Language identifier string.
 * @param source   - The source code to parse.
 * @returns Promise that resolves to a `Graph`.
 * @throws If the language is unknown or not compiled into this build.
 *
 * @example
 * ```ts
 * const g = await parseSource('python', 'def greet(name):\n    return f"Hello, {name}"\n');
 * const fns = g.nodes.filter(n => n.kind === 'function');
 * console.log(fns[0].name); // 'greet'
 * ```
 */
export declare function parseSource(language: string, source: string): Promise<Graph>;
