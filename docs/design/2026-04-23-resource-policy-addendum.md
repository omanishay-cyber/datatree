---
name: Datatree Resource-Policy Addendum
description: Removes artificial resource caps from the v1 design. Datatree uses all RAM, all CPUs, all disk available.
type: design-amendment
date: 2026-04-23
status: approved-by-user-firestart
overrides: 2026-04-23-datatree-design.md
---

# Resource Policy Addendum

Per explicit user direction: "i dont care if it gets bigger but it has to be super functional also database has unlimited space no limit and unlimited pc ram and processor support."

## Caps removed

The following soft caps from the main design are LIFTED. Datatree v1 ships with sensible defaults set high; users who want hard limits can opt in via config.

### Disk

| Original target | New policy |
|---|---|
| Multimodal cache: 5GB max (LRU evict) | **Unlimited** by default. Eviction only on `disk free < 5%` (safety floor). User can set `cache.max_bytes` if they want a cap. |
| Per-project shard: ~50MB target | **No cap**. Grows as needed. |
| Snapshot retention: 24 hourly + 7 daily + 4 weekly | **Configurable**, default unchanged but user can set `snapshots.retain_all = true` to keep every snapshot forever. |
| Embedding cache: implicit cap | **Unlimited**. Vector index file grows without limit. mmap'd so RAM impact is zero. |

### RAM

| Original target | New policy |
|---|---|
| Daemon: <250MB | **No cap**. Workers use as much as their workload demands. |
| Brain (with model loaded): <300MB | **No cap**. Phi-3 + bge-small + embedding cache + concept store all in RAM if available. |
| Conversation history cache: implicit cap | **Unlimited**. Recent N turns kept in memory; rest mmap'd from disk. |

### CPU

| Original target | New policy |
|---|---|
| Parser workers: `num_cpus * 4` | **Unchanged** — already uses all cores by design. |
| Scanner workers: `num_cpus / 2` | **Bumped to `num_cpus`** — use every core. |
| Brain workers: 2 | **Bumped to `num_cpus / 2`** — parallel embedding + clustering. |
| Multimodal sidecar: 1 process | **`min(num_cpus, 8)` worker tasks** within the single Python process (asyncio + thread pool). |

### Throughput

| Original target | New policy |
|---|---|
| Live bus: 10K events/sec | **No artificial throttle**. Async-broadcast crate handles whatever the OS delivers. |
| MCP context-bundle token cap: 5K | **Configurable upper bound `injection.max_total_overhead_per_turn`**, default 5K but no enforced ceiling — user can set 50K if they want. |
| Background parse: <10s for 10k files | **Same target, no upper-bound on file count**. Datatree happily indexes million-file monorepos; users get progress events. |

## Bundling policy

The installer SHIPS WITH everything required, no detection prompts:

| Component | Bundled in installer | Size |
|---|---|---|
| Datatree binaries (Rust) | Yes | ~30MB |
| SQLite | Bundled in `rusqlite` (already) | included |
| ONNX Runtime native libs | Yes (per-platform `libonnxruntime`) | ~30MB |
| bge-small ONNX model | Yes (required for semantic) | ~33MB |
| Tree-sitter grammars | Linked statically into parsers binary | ~20MB |
| Bun runtime | Yes — vendored under `~/.datatree/runtime/bun/` | ~80MB |
| Python embedded | Yes — vendored under `~/.datatree/runtime/python/` (Windows: PythonEmbedded; Mac/Linux: pyenv-style) | ~40MB |
| Tesseract | Yes (per-platform binary + tessdata) | ~50MB |
| ffmpeg | Yes (static build per platform) | ~80MB |
| Phi-3 (optional) | NOT bundled by default; downloaded on user opt-in | 2.4GB |
| faster-whisper base (optional) | NOT bundled by default; downloaded on user opt-in | 140MB |

**Total default install size: ~360MB.** With Phi-3 + Whisper opt-in: ~2.9GB.

User runs ONE command. Everything required for full functionality is on disk afterward. Optional models (Phi-3, Whisper) prompted separately.

## Concurrency policy

Maximize parallelism by default:
- **Parser pool**: `num_cpus * 4` workers (one Tree-sitter Parser instance per worker per language; all cached at startup).
- **Scanner pool**: `num_cpus` workers, each runs an independent scanner; failures isolated.
- **Brain pool**: `num_cpus / 2` workers for embedding + clustering.
- **Tokio runtime**: multi-threaded; default thread count = `num_cpus`.
- **Read connections per shard**: `num_cpus * 2` (was 4).
- **Inter-process IPC**: lock-free MPSC channels; never blocks the runtime.

## Performance redirected to capability

Bytes and cycles freed by removing caps go into:
- **More aggressive pre-warming**: at SessionStart, datatree pre-computes blast-radius + drift-findings + step-resume bundle in parallel even before Claude asks.
- **Larger context bundles when token budget allows**: smart-inject can balloon to 10K tokens when the user is mid-debug, scaled back to 1K for chitchat.
- **Background re-clustering**: Leiden re-runs every N file changes instead of every M minutes — graph stays current.
- **Aggressive embedding**: every new function gets embedded the moment it's parsed, not on-demand.
- **Continuous chaos testing**: in `--dev` mode, datatree randomly kills its own workers every minute to validate self-healing.

## Acceptance criterion update

Section 19 of the main design adds:
13. ✅ Datatree default install bundles every required runtime; user does not need to install Bun/Python/Tesseract/ffmpeg separately.
14. ✅ No `cache.max_bytes` enforced unless user opts in.
15. ✅ Parser/scanner/brain pools scale with `num_cpus` automatically.

---

End of addendum.
