# e2e-sites

Real-world test fixtures the Plumb binary lints end-to-end.

Each subdirectory is a minimal one-page site built with a different
front-end stack. Every site renders the **same** design-system spec
([`plumb.toml`](./plumb.toml)) and intentionally introduces the same
set of violations, so the lint output should match across stacks.

## Why six stacks

The matrix proves the lint pipeline is renderer-agnostic. Tailwind's
JIT, React's hydration, Vue's template compilation, Angular's
zone-managed rendering, and Next.js's RSC streaming all produce
different runtime DOM and computed-style behaviors. The harness asserts
identical violation counts on every leg.

## Layout

| Stack         | Tooling                                |
| ------------- | -------------------------------------- |
| html-css      | Vanilla HTML + hand-rolled CSS tokens  |
| tailwind-html | Static HTML with Tailwind classes      |
| react-vite    | React 18 + Vite + Tailwind             |
| vue           | Vue 3 + Vite + Tailwind                |
| angular       | Angular 17 standalone components       |
| nextjs        | Next.js 14 App Router + Tailwind       |

Each fixture has the same shape:

- `README.md` — what the site mimics, which violations are intentional.
- `expected.json` — the violation count + breakdown by `rule_id`. The
  harness asserts equality, not "at least N".
- `Justfile` — `just build` produces `dist/`, `just clean` removes it.
- Source files for that stack.

## How the harness uses these

`crates/plumb-e2e/` builds each fixture's `dist/`, serves it on
`http://127.0.0.1:<port>/`, runs the local `plumb` binary against the
URL with `--config e2e-sites/plumb.toml`, and diffs the violation
breakdown against `expected.json`. Three runs per fixture confirm
byte-identical output.

Run:

```sh
just test-e2e            # all sites
just test-e2e html-css   # one site
```

## Adding a new fixture

1. Pick a stack and create `e2e-sites/<stack>/`.
2. Render one page that exercises the same intentional violations the
   other fixtures use:
   - `padding: 13px` on a hero region (off-grid + off-scale).
   - `color: #c83a3a` on a button (off-palette accent/danger).
   - One control element using the clean tokens.
3. Add a `Justfile` with `build` and `clean` recipes.
4. Add `expected.json` matching the schema in `html-css/expected.json`.
5. Append the stack name to `crates/plumb-e2e/src/sites.rs::SITES`.
6. Add a matrix leg to `.github/workflows/e2e-sites.yml`.

## Determinism

Violation counts are exact, not minimums. If a stack's rendering
differs (e.g. Tailwind base reset injects a `margin: 0` that would
produce extra clean-grid measurements), the fixture's HTML must
neutralize the difference so the count matches the canonical figure.

The HTML / CSS / JS in each fixture is committed as-is — no tooling
mutates the source between runs. Lockfiles (`package-lock.json`,
`pnpm-lock.yaml`, etc.) are committed for every fixture that has
node deps so CI builds are reproducible.

## Public hosting

The Pages workflow copies every fixture's `dist/` to
`https://plumb.aramhammoudeh.com/test-sites/<stack>/`. The Dogfood job
then re-lints the deployed URLs and asserts the local + remote
violation sets agree.
