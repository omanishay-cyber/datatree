#!/usr/bin/env node
/*
 * mneme plugin postinstall wrapper. Delegates to repo-root scripts/postinstall.js.
 * See preinstall.js in this directory for the rationale.
 */
'use strict';

const path = require('path');
const fs = require('fs');
const { spawnSync } = require('child_process');

const here = __dirname;
const candidates = [
    path.resolve(here, '..', '..', 'scripts', 'postinstall.js'),
    path.resolve(here, '..', 'scripts', 'postinstall.js'),
    path.resolve(here, '..', '..', '..', 'scripts', 'postinstall.js'),
];

let target = null;
for (const c of candidates) {
    if (fs.existsSync(c)) { target = c; break; }
}

if (!target) {
    process.stderr.write('[mneme-plugin-postinstall] cannot locate canonical postinstall.js — searched:\n');
    for (const c of candidates) process.stderr.write('  - ' + c + '\n');
    process.exit(1);
}

const result = spawnSync(process.execPath, [target], { stdio: 'inherit', env: process.env });
if (result.error) { process.stderr.write(result.error.message + '\n'); process.exit(1); }
process.exit(result.status || 0);
