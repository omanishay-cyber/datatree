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

const localBinary = join(__dirname, 'mneme_parsers.node');

if (existsSync(localBinary)) {
  module.exports = require(localBinary);
} else {
  // Fallback: let @napi-rs/cli's artifact resolution find the right binary.
  const { load } = require('@node-rs/helper');
  module.exports = load('mneme-parsers', __dirname, 7);
}
