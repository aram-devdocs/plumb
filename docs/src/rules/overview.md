# Rules — overview

Plumb's rules are the catalog of design-system checks the engine runs
against each page snapshot. Every rule has:

- A **stable id**, slash-separated (`<category>/<id>`).
- A **default severity** (`info`, `warning`, `error`).
- A **docs page** — the one you get from `plumb explain <id>`.

## Built-in rules

- [`a11y/touch-target`](./a11y-touch-target.md) — flags interactive
  elements smaller than `a11y.touch_target`.
- [`radius/scale-conformance`](./radius-scale-conformance.md) — flags
  border-radius values that aren't members of `radius.scale`.
- [`spacing/grid-conformance`](./spacing-grid-conformance.md) — flags
  spacing values that aren't multiples of `spacing.base_unit`.
- [`spacing/scale-conformance`](./spacing-scale-conformance.md) —
  flags spacing values that aren't members of `spacing.scale`.
- [`type/scale-conformance`](./type-scale-conformance.md) — flags
  `font-size` values that aren't members of `type.scale`.

## Coming soon

The PRD lists the rest of the initial rule set — color, alignment,
a11y. Each will land with its own docs page and a golden snapshot test.
