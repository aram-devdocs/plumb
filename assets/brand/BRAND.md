# Plumb brand assets

The Plumb mark is a set square sighting down a plumb line — the carpenter's
test for *straight and true*, which is exactly what the linter checks a
rendered page against.

## Colour

| Token | Hex | Use |
|-------|-----|-----|
| Brand blue | `#1a4faa` | primary mark, links, accents on light backgrounds |
| Brand blue (on dark) | `#6b9bff` | links/accents on dark backgrounds (clears WCAG AA) |
| Paper | `#ffffff` | reverse mark, knockouts |

Do not recolour the mark outside this palette. On photography or busy
backgrounds use the solid app icon (`plumb-icon.svg`).

## Files

| File | What | Where it's used |
|------|------|-----------------|
| `plumb-lockup.svg` | horizontal mark + wordmark, blue | README (light), docs header |
| `plumb-lockup-white.svg` | horizontal lockup, white | README (dark mode), dark surfaces |
| `plumb-lockup-stacked.svg` | stacked mark over wordmark, blue | hero / centred placements |
| `plumb-mark.svg` / `plumb-mark-white.svg` | mark only | favicons, avatars, tight spaces |
| `plumb-icon.svg` | app icon — blue rounded square, white mark | favicon, social avatar |
| `plumb-og.svg` / `plumb-og.png` | 1200×630 social card | Open Graph / Twitter card |

The docs favicon lives at `theme/favicon.{svg,png}` and the OG card is
served from `docs/src/plumb-og.png`. Source artwork (`.ai`, `.eps`, `.pdf`,
print-resolution `.jpg`/`.png`) is kept out of the repo; regenerate the OG
PNG with `rsvg-convert -w 1200 -h 630 plumb-og.svg -o plumb-og.png`.

## Clear space & minimum size

Keep clear space around the lockup equal to the height of the mark's inner
notch. Minimum legible width: 120 px for the lockup, 16 px for the icon.
