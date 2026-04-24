/**
 * Parsers for mneme CLI output.
 *
 * The CLI deliberately emits a handful of flat, line-oriented text formats
 * that are easy to parse without JSON. Each parser here is defensive: bad
 * lines are skipped, not thrown.
 *
 * The shapes we recognise (informally):
 *   recall:    "[kind] name  path:line"
 *   blast:     "=> callers: N    transitive: M"
 *              "  - path:line  symbol"
 *   godnodes:  "<degree>\t<name>\t<kind>\t<path>"
 *   drift:     "<severity>\t<scanner>\t<path>:<line>\t<message>"
 *   step:      "<status> <id>. <title>"
 *   decision:  "<iso-ts>\t<summary>\t<transcript-path>"
 *   shards:    "<name>\t<size-bytes>\t<iso-ts>\t<path>"
 *
 * If the Rust CLI changes its output shape, update these parsers. Each one
 * is covered by a unit test in ../test/parse.test.ts.
 */

export interface RecallHit {
  readonly kind: string;
  readonly name: string;
  readonly file: string | null;
  readonly line: number | null;
  readonly raw: string;
}

export interface BlastResult {
  readonly directCallers: number;
  readonly transitiveCallers: number;
  readonly sites: ReadonlyArray<BlastSite>;
}

export interface BlastSite {
  readonly file: string;
  readonly line: number;
  readonly symbol: string;
}

export interface GodNode {
  readonly name: string;
  readonly kind: string;
  readonly degree: number;
  readonly file: string | null;
}

export type DriftSeverity = "critical" | "should-fix" | "info";

export interface DriftFinding {
  readonly severity: DriftSeverity;
  readonly scanner: string;
  readonly file: string;
  readonly line: number;
  readonly message: string;
}

export type StepStatus = "pending" | "done" | "verified" | "blocked";

export interface StepEntry {
  readonly id: number;
  readonly status: StepStatus;
  readonly title: string;
}

export interface DecisionEntry {
  readonly timestamp: string;
  readonly summary: string;
  readonly transcriptPath: string | null;
}

export interface ShardEntry {
  readonly name: string;
  readonly sizeBytes: number;
  readonly lastBuiltIso: string;
  readonly path: string;
}

/**
 * Parses lines like `[function] my_func  src/foo.rs:42` produced by
 * `mneme recall`. Tolerant of extra whitespace and missing file:line.
 *
 * Moved from commands.ts in v0.1.0 and extended.
 */
export function parseRecallHits(stdout: string): RecallHit[] {
  const hits: RecallHit[] = [];
  const lineRegex = /^\s*\[([^\]]+)\]\s+(\S.*?)\s*$/;
  const locRegex = /(\S+?):(\d+)(?::\d+)?\s*$/;

  for (const raw of stdout.split(/\r?\n/)) {
    const match = raw.match(lineRegex);
    if (!match) {
      continue;
    }
    const kind = match[1].trim();
    let nameAndLoc = match[2].trim();
    let file: string | null = null;
    let line: number | null = null;

    const locMatch = nameAndLoc.match(locRegex);
    if (locMatch && locMatch.index !== undefined) {
      file = locMatch[1];
      const parsed = Number.parseInt(locMatch[2], 10);
      line = Number.isFinite(parsed) ? parsed : null;
      nameAndLoc = nameAndLoc.slice(0, locMatch.index).trim();
    }

    hits.push({
      kind,
      name: nameAndLoc.length > 0 ? nameAndLoc : "(unnamed)",
      file,
      line,
      raw: raw.trim(),
    });
  }
  return hits;
}

/**
 * Parses mneme blast output, e.g.:
 *   => callers: 4    transitive: 17
 *     - src/foo.rs:12  do_thing
 *     - src/bar.rs:99  helper
 */
export function parseBlast(stdout: string): BlastResult {
  let directCallers = 0;
  let transitiveCallers = 0;
  const sites: BlastSite[] = [];

  const headerRegex = /callers\s*:\s*(\d+).*?transitive\s*:\s*(\d+)/i;
  const siteRegex = /^\s*-\s+(\S+?):(\d+)\s+(.+?)\s*$/;

  for (const raw of stdout.split(/\r?\n/)) {
    const header = raw.match(headerRegex);
    if (header) {
      directCallers = clampNonNegative(Number.parseInt(header[1], 10));
      transitiveCallers = clampNonNegative(Number.parseInt(header[2], 10));
      continue;
    }
    const site = raw.match(siteRegex);
    if (!site) {
      continue;
    }
    const line = Number.parseInt(site[2], 10);
    if (!Number.isFinite(line)) {
      continue;
    }
    sites.push({
      file: site[1],
      line,
      symbol: site[3].trim(),
    });
  }

  return {
    directCallers,
    transitiveCallers,
    sites,
  };
}

/**
 * Parses god_nodes output. Each line: `<degree>\t<name>\t<kind>\t<file>`.
 * Lines with a non-numeric degree are skipped.
 */
export function parseGodNodes(stdout: string): GodNode[] {
  const nodes: GodNode[] = [];
  for (const raw of stdout.split(/\r?\n/)) {
    if (raw.trim().length === 0) {
      continue;
    }
    const parts = raw.split("\t");
    if (parts.length < 3) {
      continue;
    }
    const degree = Number.parseInt(parts[0].trim(), 10);
    if (!Number.isFinite(degree)) {
      continue;
    }
    nodes.push({
      degree,
      name: parts[1].trim(),
      kind: parts[2].trim(),
      file: parts[3]?.trim() || null,
    });
  }
  return nodes;
}

/**
 * Parses drift output. Each line: `<severity>\t<scanner>\t<path>:<line>\t<message>`.
 * Unknown severities are clamped to "info".
 */
export function parseDrift(stdout: string): DriftFinding[] {
  const findings: DriftFinding[] = [];
  for (const raw of stdout.split(/\r?\n/)) {
    if (raw.trim().length === 0) {
      continue;
    }
    const parts = raw.split("\t");
    if (parts.length < 4) {
      continue;
    }
    const severity = coerceSeverity(parts[0].trim());
    const scanner = parts[1].trim();
    const loc = parts[2].trim();
    const message = parts.slice(3).join("\t").trim();

    const locMatch = loc.match(/^(.+?):(\d+)$/);
    if (!locMatch) {
      continue;
    }
    const line = Number.parseInt(locMatch[2], 10);
    if (!Number.isFinite(line)) {
      continue;
    }
    findings.push({
      severity,
      scanner,
      file: locMatch[1],
      line,
      message,
    });
  }
  return findings;
}

/**
 * Parses step ledger output. Each line: `<status-marker> <id>. <title>`.
 * Recognised markers: [x] done, [v] verified, [ ] pending, [!] blocked.
 */
export function parseSteps(stdout: string): StepEntry[] {
  const steps: StepEntry[] = [];
  const regex = /^\s*\[([xX vV! ])\]\s+(\d+)\.\s+(.+?)\s*$/;
  for (const raw of stdout.split(/\r?\n/)) {
    const match = raw.match(regex);
    if (!match) {
      continue;
    }
    const id = Number.parseInt(match[2], 10);
    if (!Number.isFinite(id)) {
      continue;
    }
    const marker = match[1];
    let status: StepStatus = "pending";
    if (marker === "x" || marker === "X") {
      status = "done";
    } else if (marker === "v" || marker === "V") {
      status = "verified";
    } else if (marker === "!") {
      status = "blocked";
    }
    steps.push({ id, status, title: match[3].trim() });
  }
  return steps;
}

/**
 * Parses decision ledger output. Each line:
 *   `<iso-ts>\t<summary>\t<transcript-path>`
 */
export function parseDecisions(stdout: string): DecisionEntry[] {
  const entries: DecisionEntry[] = [];
  for (const raw of stdout.split(/\r?\n/)) {
    if (raw.trim().length === 0) {
      continue;
    }
    const parts = raw.split("\t");
    if (parts.length < 2) {
      continue;
    }
    entries.push({
      timestamp: parts[0].trim(),
      summary: parts[1].trim(),
      transcriptPath: parts[2]?.trim() || null,
    });
  }
  return entries;
}

/**
 * Parses project shard listing. Each line:
 *   `<name>\t<size-bytes>\t<iso-ts>\t<path>`
 */
export function parseShards(stdout: string): ShardEntry[] {
  const entries: ShardEntry[] = [];
  for (const raw of stdout.split(/\r?\n/)) {
    if (raw.trim().length === 0) {
      continue;
    }
    const parts = raw.split("\t");
    if (parts.length < 4) {
      continue;
    }
    const size = Number.parseInt(parts[1].trim(), 10);
    entries.push({
      name: parts[0].trim(),
      sizeBytes: Number.isFinite(size) ? size : 0,
      lastBuiltIso: parts[2].trim(),
      path: parts[3].trim(),
    });
  }
  return entries;
}

/**
 * Best-effort detection of whether the `mneme` binary is installed and
 * usable. Looks for a version-like banner in `mneme --version` output.
 */
export function looksInstalled(versionStdout: string): boolean {
  return /mneme\s+\d+\.\d+\.\d+/i.test(versionStdout);
}

function clampNonNegative(n: number): number {
  if (!Number.isFinite(n) || n < 0) {
    return 0;
  }
  return Math.floor(n);
}

function coerceSeverity(raw: string): DriftSeverity {
  const lower = raw.toLowerCase();
  if (lower === "critical" || lower === "error" || lower === "high") {
    return "critical";
  }
  if (lower === "should-fix" || lower === "warn" || lower === "warning" || lower === "medium") {
    return "should-fix";
  }
  return "info";
}

/**
 * Humanise bytes. 2048 -> "2.0 KB", 1 048 576 -> "1.0 MB".
 */
export function humanBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) {
    return "?";
  }
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unitIdx = 0;
  while (value >= 1024 && unitIdx < units.length - 1) {
    value /= 1024;
    unitIdx++;
  }
  return `${value.toFixed(1)} ${units[unitIdx]}`;
}

/**
 * Humanise elapsed time since an ISO timestamp.
 * Falls back to "?" if parsing fails.
 */
export function humanAge(iso: string): string {
  const ts = Date.parse(iso);
  if (!Number.isFinite(ts)) {
    return "?";
  }
  const diffMs = Date.now() - ts;
  if (diffMs < 0) {
    return "just now";
  }
  const sec = Math.floor(diffMs / 1000);
  if (sec < 60) {
    return `${sec}s ago`;
  }
  const min = Math.floor(sec / 60);
  if (min < 60) {
    return `${min}m ago`;
  }
  const hr = Math.floor(min / 60);
  if (hr < 24) {
    return `${hr}h ago`;
  }
  const day = Math.floor(hr / 24);
  if (day < 30) {
    return `${day}d ago`;
  }
  const month = Math.floor(day / 30);
  if (month < 12) {
    return `${month}mo ago`;
  }
  const year = Math.floor(month / 12);
  return `${year}y ago`;
}
