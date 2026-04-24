# Rule: Determinism

Plumb's output must be byte-identical across runs. This is the single
most load-bearing invariant in the codebase; many other rules follow from it.

## What's banned

- `std::time::SystemTime::now` and `std::time::Instant::now` inside any
  library crate. Enforced by `clippy::disallowed_methods` (see `clippy.toml`).
- `std::collections::HashMap` and `HashSet` in any observable output.
  Use `indexmap::IndexMap` / `IndexSet`. The ahash-backed variants in
  `rustc_hash` or `ahash` are fine as internal accelerators **if** their
  iteration order never leaks out.
- Floating-point sort keys. Sort violations by tuples of strings + integers.
- Anything env-dependent: `std::env::var`, `std::env::current_dir` (except
  in `plumb-cli` where they map explicit user inputs).
- `rand::*`. If randomness is truly needed, seed it from a content hash.

## What's required

- Every rule's output is a function of `(snapshot, config)` only.
- The engine sorts violations by `(rule_id, viewport, selector, dom_order)`
  before return.
- Parallel rule evaluation (rayon) is permitted **only** when reductions
  are associative and inputs are already sorted.

## How it's enforced

- Crate-level `clippy::disallowed_methods` in `plumb-core`.
- The `determinism` CI job runs `plumb lint plumb-fake://hello --format
  json` three times and `diff -q`s the results.
- `just determinism-check` is the local version of the same check.

If you need to add a wall-clock source or a nondeterministic dependency,
open an RFC issue first.
