# Configuration

Plumb reads `plumb.toml` from the working directory by default. Pass
`--config <path>` to `plumb lint` to override. A starter file is
written by `plumb init`; the canonical example is
[`examples/plumb.toml`](https://github.com/aram-devdocs/plumb/blob/main/examples/plumb.toml)
in the repo.

This page is the full reference. Every section is optional. Omit a
section and the rules that depend on it skip silently — they MUST NOT
fire on partially configured input.

## Editor autocomplete

Point your editor at the canonical JSON Schema for inline validation
and hover docs:

```text
https://plumb.aramhammoudeh.com/schemas/plumb.toml.json
```

VS Code with the Even Better TOML extension:

```jsonc
// .vscode/settings.json
{
  "evenBetterToml.schema.associations": {
    "plumb.toml": "https://plumb.aramhammoudeh.com/schemas/plumb.toml.json"
  }
}
```

JetBrains: open Settings → Languages & Frameworks → Schemas and DTDs →
JSON Schema Mappings, and add the URL above against the file pattern
`plumb.toml`.

The schema is for editor association only. Do not add a `$schema`
field to `plumb.toml` or any JSON config file; Plumb rejects unknown
configuration fields.

To vendor the schema locally instead of using the docs URL:

```bash
plumb schema > plumb.schema.json
```

## Top-level shape

```toml
[viewports.<name>]    # one or more required for real URLs
[spacing]             # spacing scale and tokens
[type]                # type scale, families, weights, tokens
[color]               # named color tokens + ΔE tolerance
[radius]              # allowed border-radius values
[alignment]           # grid spec + near-alignment tolerance
[a11y]                # contrast + touch target
[rules."<id>"]        # per-rule overrides
```

The walking skeleton defaults to a single `desktop` viewport at
1280×800 if `[viewports.*]` is omitted. Real runs SHOULD declare every
viewport explicitly.

## `[viewports.<name>]`

```toml
[viewports.mobile]
width = 375
height = 667
device_pixel_ratio = 2.0

[viewports.desktop]
width = 1280
height = 800
device_pixel_ratio = 1.0
```

Each named viewport is a snapshot target. `width` and `height` are CSS
pixels. `device_pixel_ratio` is optional; defaults to `1.0`. The
viewport name appears in the rule output (`[mobile]`, `[desktop]`)
so name them after how you'll read the report.

## `[spacing]`

```toml
[spacing]
base_unit = 4
scale = [0, 4, 8, 12, 16, 24, 32, 48]
tokens = { xs = 4, sm = 8, md = 16, lg = 24, xl = 32, "2xl" = 48 }
```

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `base_unit` | `u32` | `4` | Grid base for `spacing/grid-conformance`. |
| `scale` | `[u32]` | `[]` | Allowed discrete spacing values. Empty disables `spacing/scale-conformance`. |
| `tokens` | `{string => u32}` | `{}` | Named aliases. Slash-prefixed names act as namespaces. |

Consumed by `spacing/grid-conformance` and
`spacing/scale-conformance`.

## `[type]`

```toml
[type]
families = ["Inter", "system-ui"]
weights = [400, 500, 600, 700]
scale = [12, 14, 16, 18, 20, 24, 30, 36, 48]
tokens = { caption = 12, body = 16, heading = 24 }
```

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `families` | `[string]` | `[]` | Allowed `font-family` values. Empty skips the family check. |
| `weights` | `[u16]` | `[]` | Allowed `font-weight` numeric values. |
| `scale` | `[u32]` | `[]` | Allowed `font-size` values in CSS pixels. |
| `tokens` | `{string => u32}` | `{}` | Named font-size aliases. |

Consumed by `type/scale-conformance`.

## `[color]`

```toml
[color]
tokens = { "bg/canvas" = "#ffffff", "fg/primary" = "#0b0b0b", "accent/brand" = "#0b7285" }
delta_e_tolerance = 2.0
```

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `tokens` | `{string => hex}` | `{}` | Named palette colors. Slash-delimited names group by prefix in diagnostics. |
| `delta_e_tolerance` | `f32` | `2.0` | CIEDE2000 ΔE threshold for `color/palette-conformance`. |

Consumed by `color/palette-conformance` (and the contrast rule, which
reads `[a11y].min_contrast_ratio`).

## `[radius]`

```toml
[radius]
scale = [0, 2, 4, 8, 12, 16, 9999]
```

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `scale` | `[u32]` | `[]` | Allowed `border-radius` values. Empty disables the rule. |

Consumed by `radius/scale-conformance`. The sentinel `9999` is the
conventional "fully rounded pill" value.

## `[alignment]`

```toml
[alignment]
grid_columns = 12
gutter_px = 24
tolerance_px = 3
```

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `grid_columns` | `u32?` | `null` | Number of grid columns, if you use one. |
| `gutter_px` | `u32?` | `null` | Gutter width in CSS pixels. |
| `tolerance_px` | `u32` | `3` | Edge-clustering window for `edge/near-alignment` — elements off by `0 < delta <= tolerance_px` get flagged. |

Consumed by `edge/near-alignment`.

## `[a11y]`

```toml
[a11y]
min_contrast_ratio = 4.5

[a11y.touch_target]
min_width_px = 24
min_height_px = 24
```

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `min_contrast_ratio` | `f32?` | `null` | WCAG contrast ratio target. Set `4.5` for AA body text, `7.0` for AAA. |
| `touch_target.min_width_px` | `u32` | `24` | Minimum touch-target width per WCAG 2.5.8. Raise to `44` for AAA. |
| `touch_target.min_height_px` | `u32` | `24` | Minimum touch-target height. |

Consumed by `a11y/touch-target` and the contrast rule.

## `[rules."<category>/<id>"]`

Per-rule overrides. Every rule is enabled by default at its declared
severity.

```toml
[rules."spacing/grid-conformance"]
severity = "error"

[rules."edge/near-alignment"]
enabled = false
```

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `enabled` | `bool` | `true` | Disable the rule entirely. |
| `severity` | `"error" \| "warning" \| "info"` | rule-defined | Promote or demote the severity. |

Use `plumb explain <category>/<id>` (or the
[Rules](./rules/overview.md) chapter) for per-rule docs.

## Schema and `plumb init`

`plumb init` writes the starter `plumb.toml` shown above. Pass
`--force` to overwrite an existing file.

`plumb schema` prints the canonical JSON Schema on stdout. Pipe it to
disk and point your editor at it for autocomplete:

```bash
plumb schema > plumb.schema.json
```

The schema MUST round-trip: every field documented above appears in
the schema with the same defaults and the same constraints. CI checks
this against the rendered `examples/plumb.toml`.

## Where to go next

- [CLI](./cli.md) — flags and exit codes.
- [Rules](./rules/overview.md) — per-rule reference.
- [Quick start](./quickstart.md) — the five-minute path if you skipped
  it.
