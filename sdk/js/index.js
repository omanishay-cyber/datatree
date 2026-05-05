/**
 * @mneme/parsers — runtime entry point.
 *
 * Loads the platform-specific .node binary built by `napi build`.
 * The binding name is `mneme-parsers` (from package.json `napi.name`).
 */

'use strict';

const { existsSync } = require('fs');
const { join } = require('path');

// napi-rs places the .node file next to this index.js at build time.
// In development (after `napi build --platform`), it uses a canonical name.
// In a published package, the correct platform binary is included in `files`.
//
// Names tried, in order:
//   1. mneme_parsers_node.node — produced when cargo's [lib].name is the
//      collision-safe `mneme_parsers_node` (current source-of-truth) and
//      `napi build` was either not run or skipped the rename step.
//   2. mneme_parsers.node — historical local-dev binary name from before
//      the workspace rename. Kept as a fallback so older worktrees /
//      already-built artifacts still load.
//   3. @node-rs/helper resolution by napi-name `mneme-parsers` — what a
//      published `npm install @mneme/parsers` package uses.
const candidates = [
  join(__dirname, 'mneme_parsers_node.node'),
  join(__dirname, 'mneme_parsers.node'),
];
const found = candidates.find((p) => existsSync(p));

if (found) {
  module.exports = require(found);
} else {
  const { load } = require('@node-rs/helper');
  module.exports = load('mneme-parsers', __dirname, 7);
}
