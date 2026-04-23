# Datatree Benchmarks

Retrieval-quality and token-reduction harness for the `datatree` graph
relative to a cold Claude baseline (approximated by a naive grep over
the repo). Everything runs 100% local: no daemon, no network, no API
keys, no telemetry.

## What we measure

1. **Indexing throughput** — wall-clock time + counts (files, nodes,
   edges) for a full ingest of a project.
2. **Retrieval latency** — per-query time for `blast_radius`,
   `recall_file`, and `find_references`.
3. **Token reduction** — total bytes of file payload datatree returns
   divided by 4 (the common English-plus-code rule of thumb), vs the
   same estimator applied to the cold baseline's top-5 files.
4. **Precision\@5** — integer count of expected files present in the
   returned top-5, summed across the golden set.

The `compare` subcommand produces the markdown table embedded below.

## How to run

```bash
# From the repo root.
cargo build --release --workspace

# Index this repo + run the 10 golden queries + write a markdown table.
./target/release/bench_retrieval compare .

# Only index, dump a JSON report.
./target/release/bench_retrieval index .

# One-off query against an existing shard.
./target/release/bench_retrieval query \
  ~/.datatree/projects/<project_id>/graph.db \
  "PathManager"
```

The `compare` command writes the markdown table to **stdout** and the
full JSON report to **stderr**, so CI can capture either stream
independently.

## Fixtures

`fixtures/golden.json` holds 10 queries and their expected top-5 files
(relative path substrings). Edit that file when the repo layout
changes so the benchmark stays meaningful.

Each fixture entry has the shape:

```json
{
  "query":        "PathManager",
  "kind":         "recall | blast | references",
  "target":       "optional symbol to target when kind is not recall",
  "expected_top": ["common/src/paths.rs", "common/src/lib.rs"]
}
```

## Output format

```
## Retrieval Benchmark

| # | Query | DT top-1 | DT tokens | DT ms | Cold top-1 | Cold tokens | Cold ms | DT p@5 | Cold p@5 |
|---|-------|----------|-----------|-------|------------|-------------|---------|--------|----------|
| 1 | ...   | ...      | ...       | ...   | ...        | ...         | ...     | ...    | ...      |

### Totals

| Metric          | Datatree | Cold baseline |
|-----------------|----------|---------------|
| Tokens (sum)    | ...      | ...           |
| Wall time (ms)  | ...      | ...           |
| Precision@5 (%) | ...      | ...           |
```

## Results

<!-- populated by bench_retrieval compare -->
