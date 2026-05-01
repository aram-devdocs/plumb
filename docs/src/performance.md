# Performance

Plumb ships a [Criterion](https://bheisler.github.io/criterion.rs/book/) benchmark suite in `crates/plumb-cdp/benches/`.
Use it to measure rule-engine throughput and CDP snapshot latency on your own hardware.

## Benchmark groups

| Group | What it measures | Chromium required? |
|---|---|---|
| `per_rule_dom` | Rule-engine cost on 100, 1 000, and 10 000 node synthetic DOMs | No |
| `cold_start` | Launch Chromium and take a first snapshot | Yes |
| `warm_run` | Subsequent snapshot on a reused browser | Yes |

## Running benchmarks

```sh
# Rule-engine benchmarks only (no Chromium needed):
just bench

# Full suite including CDP cold-start / warm-run:
just bench-full
```

Or with `cargo` directly:

```sh
cargo bench -p plumb-cdp                          # per_rule_dom only
cargo bench -p plumb-cdp --features e2e-chromium  # full suite
```

Criterion writes HTML reports to `target/criterion/`. Open `target/criterion/report/index.html` to browse results.

## Fixtures

The benchmark uses fixed DOM fixtures from `crates/plumb-cdp/benches/fixtures/`:

- `fixed-dom-100-nodes.html` — 100 leaf nodes
- `fixed-dom-1k-nodes.html` — 1 000 leaf nodes
- `fixed-dom-10k-nodes.html` — 10 000 leaf nodes

These are static HTML files with deterministic structure so that benchmark variance reflects engine changes, not fixture drift.
The `per_rule_dom` group builds equivalent DOMs synthetically in code rather than loading the HTML fixtures through Chromium.

## Interpreting results

Criterion reports the mean, median, standard deviation, and confidence intervals for each benchmark.
A statistically significant regression appears as a red entry in the HTML report.

To compare against a baseline:

```sh
cargo bench -p plumb-cdp -- --save-baseline before
# ... make changes ...
cargo bench -p plumb-cdp -- --baseline before
```

## CI

A dedicated workflow (`.github/workflows/benchmarks.yml`) runs the full benchmark suite on every push to `main` and on manual dispatch.
It installs system Chromium, runs `cargo bench -p plumb-cdp --features e2e-chromium`, then checks p50 (median) latency against hard thresholds:

| Benchmark | p50 limit |
|---|---|
| `cold_start` | 2 000 ms |
| `warm_run` | 500 ms |

If either threshold is breached the job fails with an `::error::` annotation.
Criterion HTML reports are uploaded as a build artifact on every run regardless of pass/fail.

The threshold script lives at `scripts/bench-threshold-check.sh` and can be run locally:

```sh
cargo bench -p plumb-cdp --features e2e-chromium
bash scripts/bench-threshold-check.sh
```
