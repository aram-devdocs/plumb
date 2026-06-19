# noise-kitchen-sink

A single page that reproduces every known Plumb **false-positive**
pattern alongside **true-positives that must survive** a precision
change. It is the reference page for `scripts/noise-scoreboard.sh`.

It is intentionally *not* wired into the `crates/plumb-e2e` count
harness: it leans on a few user-agent defaults (e.g. an unstyled
`<h1>` margin) whose exact pixels vary by Chromium version. The
deterministic regression guards live in
`crates/plumb-core/tests/golden_*.rs`.

## What each block exercises

False positives (a correct precision pass drives these to ~0):

- text-less containers → `color/contrast-aa` / `color/palette-conformance`
  must not judge a node that paints no text;
- an inline prose `<a>` → `a11y/touch-target` WCAG 2.5.8 inline exemption;
- a heading next to body text → `sibling/height-consistency` is scoped to
  interactive button-like peers;
- `<br>` and `<svg><path>` → `edge/near-alignment` skips non-layout boxes;
- a bare `<h1>` margin within 0.5px of the grid → `spacing/grid-conformance`
  sub-pixel tolerance.

True positives (these must remain):

- an 18×18 `<button>` → undersized real tap target;
- `#aaa` text on white → genuine low contrast;
- 13px padding on a paragraph → genuine off-grid author value;
- `#2e7d2e` text → genuine off-palette color.
