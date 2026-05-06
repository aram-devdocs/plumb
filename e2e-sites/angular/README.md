# angular

Angular 17 standalone component + Tailwind v4. The component renders
the same control card / off-grid hero / off-palette alert as the rest
of the matrix.

## What this site mimics

A small Angular SPA with a single standalone component. Plumb captures
the post-bootstrap DOM via Chromium DevTools.

## Build pipeline

Angular's `application` builder pins to Tailwind 2/3 in its
`@angular-devkit/build-angular` peer-deps, so we compile Tailwind v4
separately with the standalone `@tailwindcss/cli`. The Justfile runs
the Angular build first, flattens `dist/browser/` up into `dist/`, and
then writes `dist/tailwind.css` next to it. `index.html` links the
compiled stylesheet directly.

`npm ci --legacy-peer-deps` is used because `tailwindcss@4.x` is
outside the peer range that `@angular-devkit/build-angular@17.3.x`
declares.

## Intentional violations

| Rule                        | Source                                | Count |
| --------------------------- | ------------------------------------- | ----- |
| `spacing/grid-conformance`  | `class="p-[13px]"`                    | 4     |
| `spacing/scale-conformance` | `class="p-[13px]"`                    | 4     |
| `color/palette-conformance` | `class="text-[#2e7d2e]"`              | 1     |

## Build

```sh
just build
just clean
```
