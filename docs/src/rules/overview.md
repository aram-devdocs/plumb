# Rules — overview

Plumb's rules are the catalog of design-system checks the engine runs
against each page snapshot. Every rule has:

- A **stable id**, slash-separated (`<category>/<id>`).
- A **default severity** (`info`, `warning`, `error`).
- A **docs page** — the one you get from `plumb explain <id>`.

## Built-in rules

- [`a11y/touch-target`](./a11y-touch-target.md) — flags interactive
  elements smaller than `a11y.touch_target`.
- [`color/contrast-aa`](./color-contrast-aa.md) — flags text whose
  foreground/background contrast misses WCAG 2.1 AA.
- [`color/palette-conformance`](./color-palette-conformance.md) —
  flags element colors that aren't members of the configured palette.
- [`edge/near-alignment`](./edge-near-alignment.md) — flags element
  edges that almost-but-not-quite line up with sibling edges.
- [`opacity/scale-conformance`](./opacity-scale-conformance.md) —
  flags opacity values that aren't members of `opacity.scale`.
- [`radius/scale-conformance`](./radius-scale-conformance.md) — flags
  border-radius values that aren't members of `radius.scale`.
- [`shadow/scale-conformance`](./shadow-scale-conformance.md) —
  flags box-shadow values that aren't in `shadow.scale`.
- [`sibling/height-consistency`](./sibling-height-consistency.md) —
  flags sibling elements in the same visual row whose heights drift
  from the row's median.
- [`sibling/padding-consistency`](./sibling-padding-consistency.md) —
  flags sibling elements whose padding drifts from the group median.
- [`spacing/grid-conformance`](./spacing-grid-conformance.md) — flags
  spacing values that aren't multiples of `spacing.base_unit`.
- [`spacing/scale-conformance`](./spacing-scale-conformance.md) —
  flags spacing values that aren't members of `spacing.scale`.
- [`type/family-conformance`](./type-family-conformance.md) — flags
  `font-family` values that aren't in `type.families`.
- [`type/scale-conformance`](./type-scale-conformance.md) — flags
  `font-size` values that aren't members of `type.scale`.
- [`type/weight-conformance`](./type-weight-conformance.md) — flags
  `font-weight` values that aren't in `type.weights`.
- [`z/scale-conformance`](./z-scale-conformance.md) — flags z-index
  values that aren't in `z_index.scale`.
