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
  the `base_unit` grid or outside the declared `scale`.
- `[type]` — allowed font families, weights, font-size scale, and named
  type tokens.
- `[color]` — named color tokens and the CIEDE2000 tolerance for fuzzy
  matches.
- `[radius]` — allowed border-radius values.
- `[alignment]` — grid columns, gutter, and edge-clustering tolerance.
- `[a11y]` — minimum contrast ratio and touch-target thresholds.
- `[rules."<category>/<id>"]` — per-rule overrides (enable/disable,
  severity bump).

## Section reference

### `[color]`

```toml
[color]
tokens = { "bg/canvas" = "#ffffff", "fg/primary" = "#0b0b0b", "accent/brand" = "#0b7285" }
delta_e_tolerance = 2.0
```

`tokens` is a flat map of name → hex. Slash-delimited names
(`"bg/canvas"`, `"accent/brand"`) act as informal namespaces — TOML
requires the quotes; the rule engine groups by the prefix in
diagnostics.

`delta_e_tolerance` is the CIEDE2000 ΔE threshold for `color/palette-conformance`.
Defaults to `2.0`.

### `[radius]`

```toml
[radius]
scale = [0, 2, 4, 8, 12, 16, 9999]
```

`scale` is the allowed `border-radius` set in pixels. The name matches
`spacing.scale` and `type.scale`. Defaults to `[]` (no enforcement).

### `[alignment]`

```toml
[alignment]
grid_columns = 12
gutter_px = 24
tolerance_px = 3
```

`grid_columns` and `gutter_px` are optional. `tolerance_px` is the
edge-clustering window for `edge/near-alignment` — elements off by
`0 < delta <= tolerance_px` get flagged. Defaults to `3`.

### `[a11y]`

```toml
[a11y]
min_contrast_ratio = 4.5

[a11y.touch_target]
min_width_px = 24
min_height_px = 24
```

`min_contrast_ratio` is optional; set it for the contrast rule (e.g.
`4.5` for WCAG AA body text).

`[a11y.touch_target]` configures the `a11y/touch-target` rule. Defaults
to `24` × `24` CSS pixels per WCAG 2.5.8 (Target Size, Minimum). Raise
to `44` × `44` for AAA.
