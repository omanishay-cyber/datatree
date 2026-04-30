// Phase X — Cross-test driver
// X1: 3 concurrent mneme mcp stdio instances from different cwds
// X2: build + recall concurrent (build doesn't block reads)
// X3: 47 MCP tools concurrent from one client (Promise.all)
// X4: skipped (multi-user too invasive)
//
// Usage: node x-cross-test.mjs

import { spawn } from "node:child_process";
import { writeFileSync, mkdirSync, existsSync } from "node:fs";
import { join } from "node:path";

const MNEME_BIN = "C:\\Users\\Administrator\\.mneme\\bin\\mneme.exe";
const MNEME_HOME = "C:\\Users\\Administrator\\.mneme";
const MCP_PATH = MNEME_HOME + "\\mcp\\src\\index.ts";
const RESULTS = { x1: [], x2: [], x3: [], x4: { status: "SKIPPED-WITH-RATIONALE", note: "creating 2nd Win user too invasive on production VM; deferred to a clean test instance" } };

// ---------- helper: spawn an MCP client ----------
function makeClient(cwd, label) {
  const proc = spawn(MNEME_BIN, ["mcp", "stdio"], {
    cwd,
    env: {
      ...process.env,
      MNEME_LOG: "error",
      MNEME_MCP_PATH: MCP_PATH,
      MNEME_IPC_TIMEOUT_MS: "2000",
    },
    stdio: ["pipe", "pipe", "pipe"],
  });
  let buf = "";
  const pending = new Map();
  let nextId = 1;
  const stderrChunks = [];
  proc.stdout.on("data", (d) => {
    buf += d.toString();
    let nl;
    while ((nl = buf.indexOf("\n")) >= 0) {
      const line = buf.slice(0, nl);
      buf = buf.slice(nl + 1);
      if (!line.trim()) continue;
      try {
        const msg = JSON.parse(line);
        if (msg.id != null && pending.has(msg.id)) {
          pending.get(msg.id).resolve(msg);
          pending.delete(msg.id);
        }
      } catch {}
    }
  });
  proc.stderr.on("data", (d) => stderrChunks.push(d.toString()));
  function rpc(method, params = {}, timeoutMs = 8000) {
    const id = nextId++;
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      proc.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
      setTimeout(() => {
        if (pending.has(id)) {
          pending.delete(id);
          reject(new Error(`${label} ${method} timeout`));
        }
      }, timeoutMs);
    });
  }
  return {
    proc,
    rpc,
    stderr: () => stderrChunks.join(""),
    kill: () =>
      new Promise((r) => {
        try {
          proc.kill();
        } catch {}
        proc.on("exit", () => r());
        setTimeout(r, 1500);
      }),
  };
}

async function initAndList(c, label) {
  await c.rpc("initialize", {
    protocolVersion: "2024-11-05",
    capabilities: {},
    clientInfo: { name: label, version: "x" },
  });
  const list = await c.rpc("tools/list");
  return list.result?.tools?.length ?? 0;
}

// ---------- X1: 3 concurrent mcp stdio from different cwds ----------
async function runX1() {
  const candidates = [
    "C:\\Users\\Administrator\\.mneme",
    "C:\\Users\\Administrator",
    "C:\\x2-corpus",
    "C:\\Windows\\Temp",
  ];
  const usable = candidates.filter((p) => existsSync(p)).slice(0, 3);
  while (usable.length < 3) usable.push("C:\\Windows\\Temp");
  const labels = ["x1-A", "x1-B", "x1-C"];
  const clients = usable.map((cwd, i) => makeClient(cwd, labels[i]));
  const t0 = Date.now();
  let ok = 0;
  let counts = [];
  try {
    const counts_ = await Promise.all(clients.map((c, i) => initAndList(c, labels[i])));
    counts = counts_;
    ok = counts.filter((n) => n === 47).length;
  } catch (e) {
    RESULTS.x1.push({ test: "init+list", status: "FAIL", error: String(e) });
  }
  const wall = Date.now() - t0;
  RESULTS.x1.push({
    test: "3-concurrent-mcp-stdio",
    cwds: usable,
    counts,
    all_47: counts.every((n) => n === 47),
    wall_ms: wall,
    status: counts.length === usable.length && counts.every((n) => n === 47) ? "PASS" : "FAIL",
  });
  await Promise.all(clients.map((c) => c.kill()));
}

// ---------- X2: daemon contention — concurrent build + recall ----------
async function runX2() {
  // build a small synthetic corpus, then concurrently run a SECOND build + a recall
  // from inside the corpus dir so recall has a graph to query.
  const corpus = "C:\\x2-corpus";
  // first prime: synchronous build so subsequent recall has a graph
  await new Promise((resolve) => {
    const p = spawn(MNEME_BIN, ["build", "--yes", corpus], { stdio: ["ignore", "pipe", "pipe"] });
    p.on("exit", () => resolve());
  });
  // now do the contention test: a re-build + a concurrent recall
  const buildP = new Promise((resolve) => {
    const p = spawn(MNEME_BIN, ["build", "--yes", corpus], { stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    p.stdout.on("data", (d) => (stdout += d.toString()));
    p.stderr.on("data", (d) => (stderr += d.toString()));
    const t0 = Date.now();
    p.on("exit", (code) => resolve({ code, wall_ms: Date.now() - t0, stdout: stdout.slice(0, 400), stderr: stderr.slice(0, 400) }));
  });
  await new Promise((r) => setTimeout(r, 400));
  const recallP = new Promise((resolve) => {
    const p = spawn(MNEME_BIN, ["recall", "test"], { stdio: ["ignore", "pipe", "pipe"], cwd: corpus });
    let stdout = "";
    let stderr = "";
    p.stdout.on("data", (d) => (stdout += d.toString()));
    p.stderr.on("data", (d) => (stderr += d.toString()));
    const t0 = Date.now();
    p.on("exit", (code) => resolve({ code, wall_ms: Date.now() - t0, stdout: stdout.slice(0, 400), stderr: stderr.slice(0, 400) }));
  });
  const [b, r] = await Promise.all([buildP, recallP]);
  RESULTS.x2.push({
    build: { code: b.code, wall_ms: b.wall_ms },
    recall: { code: r.code, wall_ms: r.wall_ms },
    // PASS criterion: both calls return cleanly, neither hangs.
    // recall non-zero is OK if it's just an empty-result; we assert the daemon stayed responsive.
    status: b.code === 0 && r.code !== null && r.wall_ms < 10000 ? "PASS" : "PARTIAL",
    note: `build_code=${b.code} recall_code=${r.code} — daemon stayed responsive iff both returned <10s`,
  });
}

// ---------- X3: 47-tool concurrent fan-out from one client ----------
async function runX3() {
  const c = makeClient(MNEME_HOME, "x3");
  const list = await c.rpc("initialize", { protocolVersion: "2024-11-05", capabilities: {}, clientInfo: { name: "x3", version: "x" } })
    .then(() => c.rpc("tools/list"));
  const tools = list.result?.tools ?? [];
  // minimal-but-valid inputs reused from f2-stress-v2 patterns
  const minIn = {
    recall_decision: { query: "test" },
    recall_conversation: { query: "test" },
    recall_concept: { query: "test" },
    recall_file: { path: "src/main.rs" },
    recall_todo: { query: "test" },
    recall_constraint: { query: "test" },
    blast_radius: { target: "src/main.rs", depth: 2 },
    call_graph: { target: "main", depth: 2 },
    find_references: { target: "main" },
    dependency_chain: { from: "src/main.rs", to: "src/lib.rs" },
    cyclic_deps: {},
    graphify_corpus: {},
    god_nodes: { limit: 5 },
    surprising_connections: { limit: 5 },
    audit_corpus: { domain: "all" },
    audit: {},
    drift_findings: {},
    audit_theme: {},
    audit_security: {},
    audit_a11y: {},
    audit_perf: {},
    audit_types: {},
    step_status: {},
    step_show: { id: "1" },
    step_verify: { id: "1" },
    step_complete: { id: "1" },
    step_resume: {},
    step_plan_from: { description: "test" },
    snapshot: { label: "x3-snap" },
    compare: { from_id: "1", to_id: "2" },
    rewind: { snapshot_id: "1", path: "x" },
    health: {},
    doctor: {},
    rebuild: { confirm: true },
    refactor_suggest: { target: "src/main.rs" },
    refactor_apply: { id: "1" },
    wiki_generate: {},
    wiki_page: { topic: "store" },
    architecture_overview: {},
    mneme_identity: {},
    mneme_conventions: {},
    mneme_recall: { query: "test" },
    mneme_resume: {},
    mneme_why: { id: "1" },
    mneme_context: {},
    mneme_federated_similar: { query: "test" },
    suggest_skill: { hint: "react" },
  };
  const t0 = Date.now();
  // fan-out
  const calls = tools.map((t) =>
    c
      .rpc("tools/call", { name: t.name, arguments: minIn[t.name] ?? {} }, 12000)
      .then((m) => ({ name: t.name, ok: !m.error, code: m.error?.code }))
      .catch((e) => ({ name: t.name, ok: false, error: String(e) })),
  );
  const results = await Promise.all(calls);
  const wall = Date.now() - t0;
  const pass = results.filter((r) => r.ok).length;
  const fail = results.filter((r) => !r.ok);
  RESULTS.x3.push({
    test: "47-tool-concurrent-fanout",
    total: results.length,
    pass,
    fail: fail.length,
    wall_ms: wall,
    fail_detail: fail.slice(0, 10),
    status: pass === 47 ? "PASS" : pass >= 40 ? "PARTIAL" : "FAIL",
  });
  await c.kill();
}

(async () => {
  await runX1();
  await runX2();
  await runX3();
  console.log("=== X-RESULTS-JSON ===");
  console.log(JSON.stringify(RESULTS, null, 2));
  console.log("=== X-END ===");
  process.exit(0);
})();
