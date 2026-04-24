# Configuration

Plumb reads `plumb.toml` from the working directory by default. Pass
`--config <path>` to `plumb lint` to override.

A starter file is written by `plumb init`.

## Schema

Run `plumb schema` to emit the canonical JSON Schema. Point your editor
at it for autocomplete:

```bash
plumb schema > plumb.schema.json
```

## Starter `plumb.toml`

The repo's [`examples/plumb.toml`](https://github.com/aram-devdocs/plumb/blob/main/examples/plumb.toml)
is the canonical example. Highlights:

- `[viewports.<name>]` — viewport specs. At least one required in real
  runs; the walking skeleton defaults to `desktop` (1280×800).
- `[spacing]` — the discrete spacing scale. Violations flag values off
  the scale.
- `[type_scale]` — allowed font families, sizes, line-heights, weights.
- `[color]` — named color tokens and the CIEDE2000 tolerance for fuzzy
  matches.
- `[radius]` — allowed border-radii.
- `[alignment]` — grid columns and gutter.
- `[a11y]` — minimum contrast ratio.
- `[rules."<category>/<id>"]` — per-rule overrides (enable/disable,
  severity bump).
