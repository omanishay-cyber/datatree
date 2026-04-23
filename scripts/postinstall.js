#!/usr/bin/env node
/*
 * datatree postinstall hook.
 * Verifies daemon health, then optionally prompts to seed a graph for the
 * current project. Designed to be safe in non-interactive environments
 * (CI, plugin marketplaces): when stdin is not a TTY, the prompt is
 * skipped and the user is told how to opt in later.
 */
'use strict';

const http = require('http');
const path = require('path');
const fs   = require('fs');
const os   = require('os');
const readline      = require('readline');
const { spawnSync } = require('child_process');

function log(msg)   { process.stdout.write('[datatree-postinstall] ' + msg + '\n'); }
function warn(msg)  { process.stderr.write('[datatree-postinstall] WARN: ' + msg + '\n'); }

const HEALTH_URL  = 'http://127.0.0.1:7777/health';
const TIMEOUT_MS  = 1500;
const MAX_RETRIES = 6;
const RETRY_MS    = 750;

function probeHealth() {
    return new Promise(function (resolve) {
        const req = http.get(HEALTH_URL, { timeout: TIMEOUT_MS }, function (res) {
            let body = '';
            res.on('data', function (chunk) { body += chunk; });
            res.on('end', function () { resolve({ ok: res.statusCode === 200, body: body }); });
        });
        req.on('error',   function () { resolve({ ok: false }); });
        req.on('timeout', function () { req.destroy(); resolve({ ok: false }); });
    });
}

function sleep(ms) { return new Promise(function (r) { setTimeout(r, ms); }); }

function startDaemon() {
    const scriptsDir = __dirname;
    let cmd, args;
    if (os.platform() === 'win32') {
        cmd  = 'powershell.exe';
        args = ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', path.join(scriptsDir, 'start-daemon.ps1')];
    } else {
        cmd  = '/bin/sh';
        args = [path.join(scriptsDir, 'start-daemon.sh')];
    }
    log('starting daemon: ' + cmd + ' ' + args.join(' '));
    const r = spawnSync(cmd, args, { stdio: 'inherit', env: process.env });
    if (r.status !== 0) warn('start-daemon exited ' + r.status);
}

function prompt(question) {
    return new Promise(function (resolve) {
        if (!process.stdin.isTTY) { resolve(null); return; }
        const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
        rl.question(question, function (ans) { rl.close(); resolve(ans); });
    });
}

async function main() {
    let health = await probeHealth();
    if (!health.ok) {
        log('daemon not responding; attempting to start');
        startDaemon();
        for (let i = 0; i < MAX_RETRIES; i++) {
            await sleep(RETRY_MS);
            health = await probeHealth();
            if (health.ok) break;
        }
    }
    if (!health.ok) {
        warn('datatree daemon failed to come up at ' + HEALTH_URL);
        warn('install completed but graph features will be unavailable until the daemon starts.');
        process.exit(0); // do not fail the plugin install
    }
    log('daemon healthy: ' + (health.body || 'OK'));

    const cwd = process.cwd();
    const looksLikeProject = ['.git', 'package.json', 'Cargo.toml', 'pyproject.toml', 'go.mod']
        .some(function (m) { return fs.existsSync(path.join(cwd, m)); });

    if (!looksLikeProject) {
        log('cwd does not look like a project root; skipping graph build prompt.');
        process.exit(0);
    }

    const ans = await prompt('Build datatree graph for ' + cwd + '? [y/N] ');
    if (ans === null) {
        log('non-interactive shell; skipping. Run `datatree build` later to seed the graph.');
        process.exit(0);
    }
    if (/^y(es)?$/i.test(String(ans).trim())) {
        log('queueing initial graph build');
        const r = spawnSync('datatree', ['build', '--path', cwd], { stdio: 'inherit', env: process.env });
        if (r.status !== 0) warn('datatree build exited ' + r.status);
    } else {
        log('skipped. Run `datatree build` later to seed the graph.');
    }
    process.exit(0);
}

main().catch(function (err) {
    warn(err && err.stack ? err.stack : String(err));
    process.exit(0); // never block plugin install
});
