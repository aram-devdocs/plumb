# plumb-core

Deterministic design-system linter — rule engine and core types.

`plumb-core` is the foundation of the [Plumb](https://plumb.aramhammoudeh.com)
workspace. It defines the `Rule` trait, the violation model, the
`PlumbSnapshot` representation, and the config schema. All output is
byte-identical across runs by design.

## Crate highlights

- **Rule engine** — register rules, run them against a snapshot, collect
  sorted violations.
- **Snapshot types** — `PlumbSnapshot`, `SnapshotCtx`, `ElementNode`,
  `ComputedStyles`, viewport definitions.
- **Config** — `Config` struct with `serde` + `schemars` support.
- **No I/O, no async, no wall-clock** — pure functions of
  `(snapshot, config)`.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT License](LICENSE-MIT) at your option.
