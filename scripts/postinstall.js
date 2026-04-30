#!/usr/bin/env node
/*
 * mneme post-install
 * ─────────────────────
 * Runs automatically after `/plugin install mneme`. Downloads the
 * platform-matching prebuilt binary bundle from the latest GitHub release
 * and unpacks it into ~/.mneme/.
 *
 * Rule: NEVER fail hard. If the network is unavailable or the release is
 * still building, print the manual install path and exit 0 so the plugin
 * still registers. User can run `mneme install-runtime` later to retry.
 */

"use strict";

const { spawnSync } = require("node:child_process");
const { existsSync, mkdirSync, chmodSync, createWriteStream, rmSync, readdirSync } =
  require("node:fs");
const { homedir, platform, arch } = require("node:os");
const { join } = require("node:path");
const https = require("node:https");

const REPO = "omanishay-cyber/mneme";
const MNEME_HOME = join(homedir(), ".mneme");
const MNEME_BIN = join(MNEME_HOME, "bin");
const MNEME_MCP = join(MNEME_HOME, "mcp");
const MNEME_PLUGIN = join(MNEME_HOME, "plugin");

function log(msg) { process.stdout.write(`mneme: ${msg}\n`); }
function warn(msg) { process.stderr.write(`mneme: ${msg}\n`); }
function info(msg) { process.stdout.write(`        · ${msg}\n`); }

// ---------- platform ----------

function detectAsset() {
  const pf = platform();
  const ar = arch();
  if (pf === "win32" && (ar === "x64" || ar === "ia32")) {
    return { name: "mneme-windows-x64", archive: "zip" };
  }
  if (pf === "darwin") {
    // Intel Macs run the arm64 binary under Rosetta 2 (built in since
    // macOS 11). GitHub's hosted macos-13 runner is too queue-starved
    // to justify a separate Intel artifact.
    return { name: "mneme-macos-arm64", archive: "tar.gz" };
  }
  if (pf === "linux" && ar === "x64") {
    return { name: "mneme-linux-x64", archive: "tar.gz" };
  }
  return null;
}

// ---------- HTTPS helpers ----------

function httpsGet(url, headers = {}) {
  return new Promise((resolvePromise, reject) => {
    const req = https.get(
      url,
      {
        headers: {
          "User-Agent": "mneme-postinstall",
          Accept: "application/octet-stream",
          ...headers,
        },
      },
      (res) => {
        if (res.statusCode === 301 || res.statusCode === 302) {
          httpsGet(res.headers.location, headers).then(resolvePromise).catch(reject);
          return;
        }
        if (res.statusCode !== 200) {
          reject(new Error(`HTTP ${res.statusCode} — ${url}`));
          return;
        }
        resolvePromise(res);
      }
    );
    req.on("error", reject);
    req.setTimeout(30000, () => req.destroy(new Error("timeout")));
  });
}

async function fetchJson(url) {
  const res = await httpsGet(url, { Accept: "application/vnd.github+json" });
  return new Promise((resolvePromise, reject) => {
    const chunks = [];
    res.on("data", (c) => chunks.push(c));
    res.on("end", () => {
      try {
        resolvePromise(JSON.parse(Buffer.concat(chunks).toString("utf8")));
      } catch (e) {
        reject(e);
      }
    });
    res.on("error", reject);
  });
}

async function downloadFile(url, destPath) {
  const res = await httpsGet(url);
  await new Promise((resolvePromise, reject) => {
    const out = createWriteStream(destPath);
    res.pipe(out);
    out.on("finish", () => out.close(resolvePromise));
    out.on("error", reject);
    res.on("error", reject);
  });
}

// ---------- extraction ----------

function extractZip(zipPath, destDir) {
  if (platform() === "win32") {
    const r = spawnSync(
      "powershell",
      [
        "-NoProfile",
        "-Command",
        `Expand-Archive -Path "${zipPath}" -DestinationPath "${destDir}" -Force`,
      ],
      { stdio: "inherit" }
    );
    if (r.status !== 0) throw new Error("Expand-Archive failed");
    return;
  }
  const r = spawnSync("tar", ["-xf", zipPath, "-C", destDir], { stdio: "inherit" });
  if (r.status !== 0) throw new Error("tar -xf failed");
}

function extractTarGz(tgzPath, destDir) {
  const r = spawnSync("tar", ["-xzf", tgzPath, "-C", destDir], { stdio: "inherit" });
  if (r.status !== 0) throw new Error("tar -xzf failed");
}

// ---------- main ----------

async function main() {
  log("post-install starting");

  mkdirSync(MNEME_HOME, { recursive: true });
  mkdirSync(MNEME_BIN, { recursive: true });
  mkdirSync(MNEME_MCP, { recursive: true });
  mkdirSync(MNEME_PLUGIN, { recursive: true });

  const asset = detectAsset();
  if (!asset) {
    warn(`no prebuilt binary for ${platform()} ${arch()}`);
    warn("install from source: https://github.com/omanishay-cyber/mneme#-install--in-depth");
    return;
  }

  info(`platform: ${platform()} ${arch()} → asset: ${asset.name}`);

  let releaseUrl, zipUrl;
  try {
    info("fetching latest release metadata");
    const meta = await fetchJson(`https://api.github.com/repos/${REPO}/releases/latest`);
    releaseUrl = meta.html_url;
    const target = meta.assets.find(
      (a) => a.name === `${asset.name}.${asset.archive}`
    );
    if (!target) {
      warn(`release ${meta.tag_name} doesn't yet include ${asset.name}.${asset.archive}`);
      warn("the release workflow may still be building — retry in ~15 min");
      warn(`or install from source: https://github.com/${REPO}#-install--in-depth`);
      return;
    }
    zipUrl = target.browser_download_url;
    info(`found: ${target.name} (${(target.size / 1024 / 1024).toFixed(1)} MB)`);
  } catch (err) {
    warn(`release lookup failed: ${err.message}`);
    warn(`install from source: https://github.com/${REPO}#-install--in-depth`);
    return;
  }

  const tmp = join(MNEME_HOME, `download.${asset.archive}`);
  try {
    info("downloading");
    await downloadFile(zipUrl, tmp);
    info("download complete");
  } catch (err) {
    warn(`download failed: ${err.message}`);
    try { rmSync(tmp, { force: true }); } catch {}
    return;
  }

  try {
    info(`extracting to ${MNEME_HOME}`);
    if (asset.archive === "zip") {
      extractZip(tmp, MNEME_HOME);
    } else {
      extractTarGz(tmp, MNEME_HOME);
    }
    rmSync(tmp, { force: true });
  } catch (err) {
    warn(`extract failed: ${err.message}`);
    return;
  }

  if (platform() !== "win32") {
    try {
      for (const name of readdirSync(MNEME_BIN)) {
        try { chmodSync(join(MNEME_BIN, name), 0o755); } catch {}
      }
    } catch {}
  }

  info(`installed: ${MNEME_BIN}`);
  info(`release:   ${releaseUrl}`);
  log("post-install complete. Next:");
  log("  1. Start the daemon:   mneme-daemon start");
  log("  2. Index your project: mneme build .");
  log("  3. Configure AI tools: mneme install");
}

main().catch((err) => {
  warn(`unexpected error: ${err.message}`);
  warn("plugin registered; run `mneme install-runtime` to retry binaries later");
  process.exitCode = 0;
});
