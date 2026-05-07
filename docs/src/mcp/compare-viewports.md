# `compare_viewports`

Capture snapshots at two-or-more viewports of the same URL and return a
deterministic per-node delta. Useful for catching mobile/desktop
regressions: nodes that disappear at the small breakpoint, blocks that
reflow above the threshold, components that swap order, and tracked
computed-style properties that diverge.

## Arguments

```json
{
  "url": "https://example.com/",
  "viewports": [
    { "name": "mobile",  "width": 375,  "height": 800, "dpr": 2.0 },
    { "name": "desktop", "width": 1280, "height": 800, "dpr": 1.0 }
  ],
  "size_threshold_px": 4
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `url` | string | yes | URL to capture. Accepts `http(s)://` and `plumb-fake://`. |
| `viewports` | array | yes | At least 2 viewports. The first is the diff baseline. |
| `viewports[].name` | string | yes | Stable name. MUST be unique. |
| `viewports[].width` | u32 | yes | Viewport width in CSS pixels. MUST be > 0. |
| `viewports[].height` | u32 | yes | Viewport height in CSS pixels. MUST be > 0. |
| `viewports[].dpr` | f32 | yes | Device pixel ratio. |
| `size_threshold_px` | u32 | no | Pixel threshold for size-change diffs. Defaults to 4. |

## Response

```json
{
  "content": [
    {
      "type": "text",
      "text": "compare_viewports https://example.com/ across 2 viewports: 4 diff(s) [missing=1, size=2, reorder=0, style=1]"
    }
  ],
  "isError": false,
  "structuredContent": {
    "url": "https://example.com/",
    "viewports": ["mobile", "desktop"],
    "size_threshold_px": 4,
    "summary": {
      "total": 4,
      "missing": 1,
      "size_changes": 2,
      "reordered": 0,
      "style_changes": 1
    },
    "diffs": [
      {
        "kind": "missing",
        "selector": "html > body > nav",
        "present_in": ["desktop"],
        "absent_in": ["mobile"]
      },
      {
        "kind": "size_change",
        "selector": "html > body",
        "viewport_a": "mobile",
        "viewport_b": "desktop",
        "width_a": 375, "height_a": 800,
        "width_b": 1280, "height_b": 800,
        "delta_px": 905
      },
      {
        "kind": "style_change",
        "selector": "html > body > main",
        "property": "display",
        "viewport_a": "mobile",
        "viewport_b": "desktop",
        "value_a": "block",
        "value_b": "flex"
      }
    ],
    "truncated": false
  }
}
```

## Diff kinds

| `kind` | When emitted |
|--------|-------------|
| `missing` | A selector path exists in some viewports but not others. |
| `size_change` | A node's width or height changed by more than `size_threshold_px` pixels. |
| `reordered` | A node's `dom_order` differs across viewports. |
| `style_change` | A tracked computed-style property differs. Tracked properties: `display`, `flex-direction`, `grid-template-columns`, `font-size`, `color`, `background-color`, `visibility`, `position`. |

## Token budget

`structuredContent` is capped at **10 KB**. Aggregation runs server-side:
the `summary` always reports the full counts, but the `diffs` array is
capped at 200 entries. When the cap fires, `truncated` is `true`. On the
rare path where serialized output exceeds 10 KB even after capping, the
diff list is dropped entirely and `dropped_for_cap: true` is set so the
caller can re-issue with a higher `size_threshold_px`.

## Determinism

Three calls with the same arguments produce byte-identical
`structuredContent`. Diffs are sorted by `(kind, selector, property,
viewport_a, viewport_b)` before serialization.

## Errors

Returned as JSON-RPC `-32602` on the response's `error` field:

- `viewports` shorter than 2.
- Duplicate `viewport.name` values.
- Empty `url` string.
- Any viewport with `width == 0` or `height == 0`.

Driver failures (Chromium not found, version out of range, navigation
error) return a successful response with `isError: true` and a single
text content block describing the error.

## See also

- [MCP server reference](../mcp.md) — single-viewport `lint_url` and
  full tool list.
- [Configuration](../configuration.md) — `plumb.toml` reference.
