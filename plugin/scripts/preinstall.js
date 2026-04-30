#!/usr/bin/env node
/*
 * mneme plugin preinstall wrapper.
 * Locates and delegates to the canonical preinstall script. The real logic
 * lives at the repo root in `scripts/preinstall.js`. When the plugin is
 * shipped inside a release tarball, both `plugin/` and `scripts/` are
 * siblings under the extracted root, so we walk up from this file.
 *
 * Search order:
 *   1. <plugin-root>/../scripts/preinstall.js   (release tarball layout)
 *   2. <plugin-root>/scripts/preinstall.js      (in-tree dev layout)
 *   3. <plugin-root>/../../scripts/preinstall.js (nested install layout)
 */
'use strict';

const path = require('path');
const fs = require('fs');
const { spawnSync } = require('child_process');

const here = __dirname;
const candidates = [
    path.resolve(here, '..', '..', 'scripts', 'preinstall.js'),
    path.resolve(here, '..', 'scripts', 'preinstall.js'),
    path.resolve(here, '..', '..', '..', 'scripts', 'preinstall.js'),
];

let target = null;
for (const c of candidates) {
    if (fs.existsSync(c)) { target = c; break; }
}

if (!target) {
    process.stderr.write('[mneme-plugin-preinstall] cannot locate canonical preinstall.js — searched:\n');
    for (const c of candidates) process.stderr.write('  - ' + c + '\n');
    process.exit(1);
}

const result = spawnSync(process.execPath, [target], { stdio: 'inherit', env: process.env });
if (result.error) { process.stderr.write(result.error.message + '\n'); process.exit(1); }
process.exit(result.status || 0);
