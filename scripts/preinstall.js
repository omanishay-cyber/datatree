#!/usr/bin/env node
/*
 * datatree preinstall hook.
 * Invoked by the Claude Code plugin install lifecycle (and equivalent
 * lifecycles on other platforms). Detects the host OS and dispatches to
 * the matching shell installer with the bundled binary path.
 *
 * Idempotent: child scripts are themselves idempotent. Failures are
 * surfaced as non-zero exit codes so the marketplace can show a real
 * error instead of silently continuing.
 */
'use strict';

const os = require('os');
const path = require('path');
const fs = require('fs');
const { spawnSync } = require('child_process');

function log(msg) {
    process.stdout.write('[datatree-preinstall] ' + msg + '\n');
}
function die(msg, code) {
    process.stderr.write('[datatree-preinstall] ERROR: ' + msg + '\n');
    process.exit(typeof code === 'number' ? code : 1);
}

const platform = os.platform();
const arch     = os.arch();
log('platform=' + platform + ' arch=' + arch + ' node=' + process.version);

const scriptsDir = __dirname;
const repoRoot   = path.resolve(scriptsDir, '..');
const distDir    = path.join(repoRoot, 'dist', 'supervisor');

if (!fs.existsSync(distDir)) {
    log('No prebuilt supervisor bundle found at ' + distDir + ' — assuming dev install.');
}

let cmd, args;
if (platform === 'win32') {
    const ps = path.join(scriptsDir, 'install-supervisor.ps1');
    cmd  = 'powershell.exe';
    args = ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', ps];
} else if (platform === 'darwin' || platform === 'linux') {
    const sh = path.join(scriptsDir, 'install-supervisor.sh');
    try { fs.chmodSync(sh, 0o755); } catch (_) { /* best effort */ }
    cmd  = '/bin/sh';
    args = [sh];
} else {
    die('Unsupported platform: ' + platform, 2);
}

log('exec: ' + cmd + ' ' + args.join(' '));
const result = spawnSync(cmd, args, { stdio: 'inherit', env: process.env });
if (result.error) die(result.error.message, 1);
if (result.status !== 0) die('installer exited with code ' + result.status, result.status || 1);

log('preinstall complete');
process.exit(0);
