# Full changelog

The full version history. For the Genesis highlights, see [Genesis — Keystone](./v0.4.0.md).

The canonical source is [`CHANGELOG.md`](https://github.com/omanishay-cyber/mneme/blob/main/CHANGELOG.md) in the repo. We mirror notable entries here with a stable URL the AI host (and human readers) can link to.

## Versioning policy

- **Major** (`x.0.0`) — only when something rendering-breaking happens at the wire format / database schema. v0.x is pre-1.0 — we still call shipping-breaking changes "minor" until we tag 1.0.
- **Minor** (`0.x.0`) — keystone features. v0.4.0 is the recall + token gap closure; v0.3.0 was the install + multi-platform bundle; v0.2.0 was the MCP server.
- **Patch** (`0.x.y`) — bug-only fixes between minors. v0.4.0.x ships PERF-P0-001 batched embed writes + the `mneme update` signed verification.

[Full version policy memo →](https://github.com/omanishay-cyber/mneme/blob/main/docs/dev/versioning.md) (stored in repo for completeness; the bumper script is `scripts/bump-version.sh` and is the only sanctioned way to increment versions across the 14+ files that hold them).

## Recent versions

### v0.4.0 (2026-05-05) — Keystone

[Detailed page →](./v0.4.0.md)

- 7 keystone items shipped end-to-end: symbol resolver chain (Rust + TS + Python), symbol-anchored embeddings, soft-redirect Grep/Read hook, server-pre-computed ForceGalaxy layout, auto-update apply+rollback, schema migration, mneme-hook.exe GUI dispatcher.
- 2 audit-fix waves caught 5 P0 + 14 P1 findings before VM testing. Critical fixes shipped: tar.gz path-traversal hardening (F13), hook fail-open dispatch wrapper (REL-001), Rust+TS default alignment (REL-002), 4 security hardening fixes (SEC-001/005/006/007), 2 perf fixes (P0-002/003).
- 747 tests pass (525 cli + 97 parsers + 125 daemon).

### v0.3.2 hotfix (2026-05-04) — install hardening + 222-bug grind

- 22+ install-pipeline fixes after AWS production testing
- Bug B12 root cause (5 regex bombs in audit scanners) fixed
- B17 — bundled `onnxruntime.dll` 1.24.4, BGE/ORT pipeline stable on Windows
- 4-row install matrix (winget Anish.Mneme + winget Anish.Mnemeos + pip mnemeos + curl|bash for Linux/macOS)
- 200+ subbugs caught + fixed during 1-week grind

### v0.3.0 (2026-04-15) — multi-platform bundle

- First multi-arch GitHub Releases (5 platforms: linux-x64, linux-arm64, darwin-arm64, windows-x64, windows-arm64)
- Bundled BGE-small-en-v1.5 ONNX model
- Bundled phi-3-mini-4k-q4 GGUF (optional LLM)
- Auto-rebuild on path mismatch
- Install scripts for Linux/macOS/Windows
- 14 vision views shipped end-to-end

### v0.2.0 (2026-03-22) — MCP server

- 50 MCP tools registered
- 3-layer self-ping hook system (initial design; Layer 3 was a skeleton until v0.4.0)
- Concept graph extraction
- Step Ledger (compaction-safe task tracker)

### v0.1.0 (2026-02-08) — initial public release

- 22 graph layers, single-writer-per-shard
- Tree-sitter parsers for Rust, TS, Python, Java, Go
- Vision SPA shipped on `127.0.0.1:7777`
- Apache-2.0 license

## Migration guides

- [v0.3 → v0.4](./v0.4.0.md#migration-from-v03x) — schema v1→v2, re-embed pass, the "may take a moment" message you'll see on first build after upgrade.
- [v0.2 → v0.3](https://github.com/omanishay-cyber/mneme/blob/main/CHANGELOG.md) — install matrix expansion, bundled models.

## Where to file issues

[GitHub Issues →](https://github.com/omanishay-cyber/mneme/issues)

When in doubt: include `mneme --version` + `mneme doctor` output + the OS + the exact command that failed.
