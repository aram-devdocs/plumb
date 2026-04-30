# plumb-config

Config loading and JSON Schema emission for [Plumb](https://plumb.aramhammoudeh.com).

Loads `plumb.toml` (or `.json` / `.yaml`) via
[figment](https://docs.rs/figment) and emits the canonical JSON Schema
via [schemars](https://docs.rs/schemars). Supports DTCG token files and
Tailwind CSS config as design-token sources.

## Public API

- `load` — resolve and merge config from disk.
- `emit_schema` — generate the JSON Schema for `plumb.toml`.
- `ConfigError` — typed error enum for config failures.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT License](LICENSE-MIT) at your option.
