# vue

Vue 3 + Vite + Tailwind v4. The single-file component renders the same
control card / off-grid hero / off-palette alert as the other fixtures.

## What this site mimics

A small SFC-based SPA. Vite compiles `App.vue` into a runtime bundle;
Plumb captures the post-mount DOM via Chromium DevTools.

## Intentional violations

| Rule                        | Source                              | Count |
| --------------------------- | ----------------------------------- | ----- |
| `spacing/grid-conformance`  | `<section class="p-[13px]">`        | 4     |
| `spacing/scale-conformance` | `<section class="p-[13px]">`        | 4     |
| `color/palette-conformance` | `<p class="text-[#2e7d2e]">`        | 1     |

## Build

```sh
just build
just clean
```
