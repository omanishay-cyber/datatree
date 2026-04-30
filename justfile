# Mneme task runner. Every recipe here is documented in BENCHMARKS.md.
# Requires a built release binary of `bench_retrieval`:
#   cargo build --release -p benchmarks --bin bench_retrieval

set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]
set positional-arguments

bin := "./target/release/bench_retrieval"

default:
    @just --list

# Build the release binary used by every bench recipe.
build-bench:
    cargo build --release -p benchmarks --bin bench_retrieval

# Token-reduction ratios (mean + p50 + p95) across 10 generic queries.
bench-token-reduction repo="." *args="":
    {{bin}} bench-token-reduction {{repo}} {{args}}

# Cold + warm full-build wall-clock times.
bench-first-build repo="." *args="":
    {{bin}} bench-first-build {{repo}} {{args}}

# Single-file inject p50/p95 over up to 100 files.
bench-incremental repo="." *args="":
    {{bin}} bench-incremental {{repo}} {{args}}

# graph.db bytes per node + per edge.
bench-viz-scale repo="." *args="":
    {{bin}} bench-viz-scale {{repo}} {{args}}

# Precision@10 over a golden-query fixture.
bench-recall repo="." fixture="benchmarks/fixtures/golden.json" *args="":
    {{bin}} bench-recall {{repo}} {{fixture}} {{args}}

# Run every bench and emit one unified CSV. Stderr carries a JSON summary.
bench-all repo="." *args="":
    {{bin}} bench-all {{repo}} {{args}}

# Legacy compare table (markdown by default).
bench-compare repo="." *args="":
    {{bin}} compare {{repo}} {{args}}

# Emit the compare table as CSV to stdout.
bench-compare-csv repo="." *args="":
    {{bin}} compare {{repo}} --format csv {{args}}
