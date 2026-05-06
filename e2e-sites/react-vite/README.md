# react-vite

React 18 + Vite + Tailwind v4. The component renders the same control
card / off-grid hero / off-palette alert as the other fixtures so the
harness asserts identical violation counts across every stack.

## What this site mimics

A small SPA landing page authored in JSX with utility classes. The
React tree is hydrated client-side; Plumb captures the post-hydration
DOM via Chromium DevTools.

## Intentional violations

| Rule                        | Source                                                  | Count |
| --------------------------- | ------------------------------------------------------- | ----- |
| `spacing/grid-conformance`  | `<section className="p-[13px]">` (4 longhands)          | 4     |
| `spacing/scale-conformance` | `<section className="p-[13px]">` (4 longhands)          | 4     |
| `color/palette-conformance` | `<p className="text-[#2e7d2e]">`                        | 1     |

`border-[#0b0b0b]` on the alert pins border-color to a palette token
so the four `border-*-color` longhands stay clean.

## Build

```sh
just build  # `npm ci` + `vite build`, output to dist/
just clean  # remove dist/ and node_modules/
```

`package-lock.json` is committed for reproducible CI builds.
