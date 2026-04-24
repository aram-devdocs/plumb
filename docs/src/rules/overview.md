# Rules — overview

Plumb's rules are the catalog of design-system checks the engine runs
against each page snapshot. Every rule has:

- A **stable id**, slash-separated (`<category>/<id>`).
- A **default severity** (`info`, `warning`, `error`).
- A **docs page** — the one you get from `plumb explain <id>`.

## Walking skeleton

Only one rule ships today:

- [`placeholder/hello-world`](./placeholder-hello-world.md) — the
  end-to-end wiring test. Removed when the first real rule lands.

## Coming soon

The PRD lists the initial rule set — spacing, type scale, color,
radius, alignment, a11y. Each will land with its own docs page and a
golden snapshot test.
