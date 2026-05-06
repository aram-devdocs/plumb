# plumb-codegen

Source-tree token inference for [Plumb](https://plumb.aramhammoudeh.com).

Walks a project directory, discovers design-token sources (CSS custom
properties, Tailwind config files, DTCG token JSON), and bootstraps a
best-effort `plumb.toml`. Wired through the `plumb init --from <path>`
subcommand.

## Public API

- `infer_config` — walk a source tree and return an [`InferredConfig`]
  containing a populated [`plumb_core::Config`] and a per-source summary.
- `render_toml` — serialize an [`InferredConfig`] into a `plumb.toml`
  with a header comment describing the inputs.
- `CodegenError` — typed error enum for inference failures.

## Determinism

Source files are walked in sorted order, scales are sorted ascending
with duplicates removed, and tokens land in [`indexmap::IndexMap`]
preserving discovery order. Two runs over the same tree produce
byte-identical output.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT License](LICENSE-MIT) at your option.
