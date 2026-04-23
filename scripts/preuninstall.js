#!/usr/bin/env node
/*
 * datatree preuninstall hook.
 * Stops the daemon before plugin files are removed. Does NOT touch
 * user data — the uninstall script handles that, gated by --purge.
 */
'use strict';

const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

function log(msg) { process.stdout.write('[datatree-preuninstall] ' + msg + '\n'); }

const scriptsDir = __dirname;
let cmd, args;
if (os.platform() === 'win32') {
    cmd  = 'powershell.exe';
    args = ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', path.join(scriptsDir, 'stop-daemon.ps1')];
} else {
    cmd  = '/bin/sh';
    args = [path.join(scriptsDir, 'stop-daemon.sh')];
}

log('stopping daemon: ' + cmd + ' ' + args.join(' '));
const result = spawnSync(cmd, args, { stdio: 'inherit', env: process.env });
if (result.status !== 0) {
    process.stderr.write('[datatree-preuninstall] WARN: stop-daemon exited ' + result.status + '\n');
}
process.exit(0);
