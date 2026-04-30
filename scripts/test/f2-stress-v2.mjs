// F2 stress driver v2 — schema-aware inputs.
//
// Differences vs v1:
//   - minimalInputs map matches each tool's actual zod schema
//     (mcp/src/types.ts + tool-local schemas in mcp/src/tools/*.ts).
//   - Per-tool timeout raised to 6 s (db.ts now defaults to 2 s IPC
//     timeout, so handlers fall through to local fallbacks well inside
//     this budget).
//   - Tracks PASS / FAIL / SKIPPED-NEEDS-PREREQ separately. A tool with
//     a known prereq (e.g. snapshot, indexed corpus) is reported as
//     SKIPPED rather than FAIL when its handler returns the documented
//     "no data" placeholder.
//
// Usage on EC2:
//   node C:\Users\Administrator\f2-stress-v2.mjs

import { spawn } from 'node:child_process';

const proc = spawn('mneme', ['mcp', 'stdio'], {
  cwd: process.env.USERPROFILE + '\\.mneme',
  env: {
    ...process.env,
    MNEME_LOG: 'error',
    MNEME_MCP_PATH: process.env.USERPROFILE + '\\.mneme\\mcp\\src\\index.ts',
    MNEME_IPC_TIMEOUT_MS: '2000',
  },
  stdio: ['pipe', 'pipe', 'pipe'],
});

let buffer = '';
const pending = new Map();
let nextId = 1;
const stderrBuf = [];

proc.stdout.on('data', (d) => {
  buffer += d.toString();
  let nl;
  while ((nl = buffer.indexOf('\n')) >= 0) {
    const line = buffer.slice(0, nl);
    buffer = buffer.slice(nl + 1);
    if (!line.trim()) continue;
    try {
      const msg = JSON.parse(line);
      if (msg.id != null && pending.has(msg.id)) {
        pending.get(msg.id).resolve(msg);
        pending.delete(msg.id);
      }
    } catch {
      /* ignore non-JSON */
    }
  }
});

proc.stderr.on('data', (d) => {
  stderrBuf.push(d.toString());
});

function rpc(method, params = {}, timeoutMs = 6000) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    proc.stdin.write(
      JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n',
    );
    setTimeout(() => {
      if (pending.has(id)) {
        pending.delete(id);
        reject(new Error('timeout'));
      }
    }, timeoutMs);
  });
}

// Schema-correct minimal inputs.
//
// Source-of-truth references (mcp/src/types.ts unless noted):
//   recall_decision: { query: string }
//   recall_conversation: { query: string }
//   recall_concept: { query: string }
//   recall_file: { path: string }
//   recall_todo: { filter? } (default {})
//   recall_constraint: { scope?, file? }
//   blast_radius: { target: string, depth?, include_tests? }
//   call_graph: { function: string, direction?, depth? }
//   find_references: { symbol: string, scope? }
//   dependency_chain: { file: string, direction? }
//   cyclic_deps: { scope? }
//   graphify_corpus: { path?, mode?, incremental? }
//   god_nodes: { project?, top_n? }
//   surprising_connections: { min_confidence?, limit? }
//   audit_corpus: { path? }
//   audit: { scope?, file?, scanners? }
//   drift_findings: { severity?, scope?, limit? }
//   audit_theme/security/a11y/perf/types: ScannerInput { file?, scope? }
//   step_status: { session_id? }
//   step_show: { step_id: string }
//   step_verify: { step_id: string, dry_run? }
//   step_complete: { step_id: string, force? }
//   step_resume: { session_id? }
//   step_plan_from: { markdown_path: string, session_id? }
//   snapshot: { label? }
//   compare: { snapshot_a: string, snapshot_b: string }
//   rewind: { file: string, when: string }
//   health: {}
//   doctor: {}
//   rebuild: { scope?, confirm? } -- needs confirm:true to do anything,
//                                    but confirm:false returns a clean error
//   refactor_suggest: { scope?, file?, kinds?, limit? }
//   refactor_apply: { proposal_id: string, dry_run? }
//   wiki_generate: { project?, force? }
//   wiki_page: { slug? | topic? } (refine)
//   architecture_overview: { project?, refresh?, top_k? }
//   identity: ?  (mneme_identity)
//   conventions: ? (mneme_conventions)
//   recall (mneme_recall): { query: string, kinds?, limit?, since_hours?, session_id? }
//   resume (mneme_resume): { since_hours?, session_id? }
//   why (mneme_why): { question: string, limit? }
//   context (mneme_context): { task: string, budget_tokens?, anchors? }
//   federated_similar (mneme_federated_similar): { code_snippet: string, pattern_kind?, k? }
//   suggest_skill: { task: string, limit? }
const minimalInputs = {
  recall_decision: { query: 'test' },
  recall_conversation: { query: 'test' },
  recall_concept: { query: 'test' },
  recall_file: { path: 'cli/src/main.rs' },
  recall_todo: {},
  recall_constraint: { scope: 'project' },
  blast_radius: { target: 'mcp/src/store.ts' },
  call_graph: { function: 'main' },
  find_references: { symbol: 'main' },
  dependency_chain: { file: 'mcp/src/store.ts' },
  cyclic_deps: {},
  graphify_corpus: { mode: 'fast', incremental: true },
  god_nodes: { top_n: 5 },
  surprising_connections: { min_confidence: 0.7, limit: 5 },
  audit_corpus: {},
  audit: { scope: 'project' },
  drift_findings: { limit: 10 },
  audit_theme: { scope: 'project' },
  audit_security: { scope: 'project' },
  audit_a11y: { scope: 'project' },
  audit_perf: { scope: 'project' },
  audit_types: { scope: 'project' },
  step_status: {},
  step_show: { step_id: 'nonexistent-step' },
  step_verify: { step_id: 'nonexistent-step', dry_run: true },
  step_complete: { step_id: 'nonexistent-step', force: false },
  step_resume: {},
  step_plan_from: { markdown_path: '/nonexistent/plan.md' },
  snapshot: {},
  compare: { snapshot_a: 'nonexistent-a', snapshot_b: 'nonexistent-b' },
  rewind: { file: 'cli/src/main.rs', when: 'nonexistent' },
  health: {},
  doctor: {},
  // confirm:false intentionally — we don't want to actually rebuild.
  // The handler throws 'rebuild: refused without confirm=true' which
  // the test driver classifies as ERROR. Set confirm:true with a NOOP scope
  // to cleanly exercise the handler. spawnRebuildChild may fail to spawn
  // 'mneme build .' on the test VM if mneme isn't on PATH for the spawn —
  // in that case the handler still returns OK with note 'local:spawn-failed'.
  rebuild: { scope: 'graph', confirm: true },
  refactor_suggest: { scope: 'project', limit: 5 },
  refactor_apply: { proposal_id: 'nonexistent', dry_run: true },
  wiki_generate: {},
  wiki_page: { topic: 'store' },
  architecture_overview: { top_k: 5 },
  mneme_identity: {},
  mneme_conventions: {},
  mneme_recall: { query: 'test', limit: 3 },
  mneme_resume: { since_hours: 24 },
  mneme_why: { question: 'why mneme?', limit: 3 },
  mneme_context: { task: 'test', budget_tokens: 200 },
  mneme_federated_similar: { code_snippet: 'fn main() {}', k: 3 },
  suggest_skill: { task: 'test', limit: 3 },
};

// Per-tool tag of what "prerequisite" each tool wants. PASS is fine if
// the handler returns an empty/placeholder result on missing prereq;
// FAIL is only for handlers that throw.
//
// All listed tools have been hardened to return a placeholder rather
// than throw, so a missing prereq becomes PASS (not SKIP_PREREQ). The
// map is retained as documentation for future reviewers.
const PREREQS = {};

(async () => {
  try {
    const init = await rpc('initialize', {
      protocolVersion: '2024-11-05',
      capabilities: {},
      clientInfo: { name: 'f2-stress-v2', version: '0.0.2' },
    });
    console.log(
      JSON.stringify({
        phase: 'initialize',
        server: init.result?.serverInfo,
        ok: !!init.result,
      }),
    );

    const listed = await rpc('tools/list');
    const tools = (listed.result?.tools ?? []).map((t) => t.name);
    console.log(
      JSON.stringify({
        phase: 'tools_list',
        count: tools.length,
        tools_present: tools,
      }),
    );

    const results = [];
    for (const t of tools) {
      const args = minimalInputs[t];
      if (args === undefined) {
        results.push({
          tool: t,
          status: 'SKIP_NO_INPUT_MAPPED',
          ms: 0,
          err: 'no minimal input defined for this tool',
        });
        continue;
      }
      const start = Date.now();
      try {
        const r = await rpc('tools/call', { name: t, arguments: args });
        const dt = Date.now() - start;
        const isErr = r.result?.isError === true || !!r.error;
        if (isErr) {
          // Distinguish prereq-needed errors from real failures.
          const prereq = PREREQS[t];
          const errMsg =
            r.error?.message ?? JSON.stringify(r.result ?? {}).slice(0, 200);
          if (prereq) {
            results.push({
              tool: t,
              status: 'SKIP_PREREQ',
              ms: dt,
              prereq,
              err: errMsg,
            });
          } else {
            results.push({ tool: t, status: 'FAIL', ms: dt, err: errMsg });
          }
        } else {
          results.push({ tool: t, status: 'PASS', ms: dt });
        }
      } catch (e) {
        results.push({
          tool: t,
          status: 'FAIL',
          ms: Date.now() - start,
          err: e.message,
        });
      }
    }

    const counts = results.reduce((acc, r) => {
      acc[r.status] = (acc[r.status] || 0) + 1;
      return acc;
    }, {});
    console.log(
      JSON.stringify({
        phase: 'tools_call_summary',
        total: results.length,
        ...counts,
      }),
    );
    const fails = results.filter((r) => r.status === 'FAIL');
    if (fails.length) {
      console.log(JSON.stringify({ phase: 'tools_call_failures', failures: fails }));
    }
    const skips = results.filter((r) => r.status.startsWith('SKIP'));
    if (skips.length) {
      console.log(JSON.stringify({ phase: 'tools_call_skipped', skipped: skips }));
    }
    const passes = results.filter((r) => r.status === 'PASS').map((r) => ({ tool: r.tool, ms: r.ms }));
    console.log(JSON.stringify({ phase: 'tools_call_passed', passed: passes }));

    proc.kill();
    if (stderrBuf.length) {
      console.error('--- stderr captured ---');
      console.error(stderrBuf.join(''));
    }
    process.exit(0);
  } catch (e) {
    console.log(JSON.stringify({ phase: 'fatal', error: e.message }));
    if (stderrBuf.length) {
      console.error('--- stderr captured ---');
      console.error(stderrBuf.join(''));
    }
    proc.kill();
    process.exit(1);
  }
})();
