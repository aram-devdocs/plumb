# nextjs

Next.js 14 App Router fixture, statically exported via
`output: 'export'`. The page renders the same control card / off-grid
hero / off-palette alert as the rest of the matrix.

## What this site mimics

A small App Router site exported as static HTML. Plumb captures the
post-hydration DOM via Chromium DevTools.

## A note on counts

Next.js injects a hidden `<next-route-announcer>` element with `margin:
-1px` (the standard "visually hidden" trick used to stay accessible
without contributing visible layout). The four longhand margin values
are off-grid against `spacing.base_unit = 4` and off-scale against the
configured `spacing.scale`, so they emit 4 grid + 4 scale violations
on top of the hero's intended 4 + 4. `expected.json` documents the
total target counts as `8` + `8` accordingly.

| Rule                        | Source                                                  | Count |
| --------------------------- | ------------------------------------------------------- | ----- |
| `spacing/grid-conformance`  | `<section className="p-[13px]">` + route announcer      | 8     |
| `spacing/scale-conformance` | `<section className="p-[13px]">` + route announcer      | 8     |
| `color/palette-conformance` | `<p className="text-[#2e7d2e]">`                        | 1     |

## Build

```sh
just build  # `npm ci` + `next build` (output: 'export') → dist/
just clean
```

`output: 'export'` plus `trailingSlash: true` is the supported way to
ship a Next.js App Router site without a runtime Node server. The
exported `out/` directory is renamed to `dist/` so the harness layout
matches the rest of the matrix.

## Readiness sentinel

`expected.json` sets a `wait_for` block that the harness threads onto
`plumb lint --wait-for <selector> --wait-ms <ms>`. The selector is
`html[data-plumb-ready="true"]`; the marker is set from a `useEffect`
in `app/plumb-ready.tsx`, so it appears only after React has hydrated
the tree on the client. Without it Chromium occasionally captures a
half-hydrated DOM under CI load — that flake was the reason the leg
ran with `continue-on-error: true` in `e2e-sites.yml`.
