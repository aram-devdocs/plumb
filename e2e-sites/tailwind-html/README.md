# tailwind-html

Static HTML built with the Tailwind v4 CLI. Same intentional violations
as the `html-css` fixture, expressed as Tailwind utility classes —
arbitrary values trigger the JIT path so the build pipeline mirrors a
real Tailwind project.

## What this site mimics

A small marketing-style page authored with utility classes. The
control card uses standard token utilities (`p-4`, `text-2xl`,
`bg-white`); the off-grid hero uses an arbitrary `p-[13px]`; the
off-palette alert uses `text-[#2e7d2e]`.

## Intentional violations

Same surface as `html-css/`:

| Rule                        | Source                                          | Count |
| --------------------------- | ----------------------------------------------- | ----- |
| `spacing/grid-conformance`  | `class="p-[13px]"` on the hero (4 longhands)    | 4     |
| `spacing/scale-conformance` | `class="p-[13px]"` on the hero (4 longhands)    | 4     |
| `color/palette-conformance` | `class="text-[#2e7d2e]"` on the alert           | 1     |

`border-[#0b0b0b]` on the alert pins border-color to a palette token,
so the four `border-*-color` longhands stay clean. Without that, the
off-palette `color` would propagate to `currentColor` and emit four
extra palette violations.

## Build

```sh
just build  # `npm ci` + tailwind CLI compile, output to dist/
just clean  # remove dist/ and node_modules/
```

`package-lock.json` is committed to keep CI deterministic.
