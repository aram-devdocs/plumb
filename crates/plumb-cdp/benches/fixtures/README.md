# Fixed DOM Benchmark Fixtures

This directory holds static HTML fixtures for later `plumb-cdp`
benchmark work.

## Size labels

The size in each filename refers to the exact number of HTML element
nodes under `<body>`, excluding text nodes, comments, and the `<body>`
element itself:

- `fixed-dom-100-nodes.html` => 100 body descendant elements
- `fixed-dom-1k-nodes.html` => 1,000 body descendant elements
- `fixed-dom-10k-nodes.html` => 10,000 body descendant elements

Each fixture uses the same small card layout so later benchmark slices
can compare larger DOMs without also changing the shape of the content.

## Fixture constraints

- These files are committed stable text, not generated at runtime.
- Keep them hand-maintained and deterministic.
- Use static HTML with inline CSS only.
- Do not add `<script>` tags.
- Do not add external CSS, fonts, images, or any other network
  dependency.
- Keep fixture text stable so benchmark inputs stay comparable across
  revisions.

## Out of scope for this slice

This directory does not yet include:

- a benchmark harness
- node-count reporting commands
- generated fixture builders
- CI wiring
- performance thresholds
- docs or report publishing

That work lands in later slices for issue #61.
