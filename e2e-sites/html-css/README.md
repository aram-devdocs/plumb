# html-css

Vanilla HTML + hand-rolled CSS tokens. The simplest fixture: `just
build` copies the source into `dist/` and the harness serves it.

## What this site mimics

A static landing-page card pattern: a control element with clean
tokens, an off-grid hero region, and an off-palette alert.

## Intentional violations

The fixture targets three rules:

| Rule                       | Source                                         | Count |
| -------------------------- | ---------------------------------------------- | ----- |
| `spacing/grid-conformance` | `.hero { padding: 13px }` → 4 longhand sides   | 4     |
| `spacing/scale-conformance`| `.hero { padding: 13px }` → 4 longhand sides   | 4     |
| `color/palette-conformance`| `.alert { color: #2e7d2e }` (saturated green)  | 1     |

`expected.json` declares the target rule set; the harness ignores
non-target violations (e.g. accidental UA-default residue) so the fixture
stays robust against Chromium upgrades.

## Build

```sh
just build  # copies source files into dist/
just clean  # removes dist/
```

The harness in `crates/plumb-e2e/` builds, serves on a random local
port, lints with `--config e2e-sites/plumb.toml`, and asserts the count
breakdown matches `expected.json`.
