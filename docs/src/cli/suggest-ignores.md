# `--suggest-ignores`

`plumb lint --suggest-ignores` appends a suggested `.plumbignore` block
after the normal lint output. The block lists one entry per
`(rule_id, selector_path)` tuple that would suppress every current
violation, sorted by `(rule_id, selector_path)` for byte-identical
output across runs.

The flag is opt-in. Default behavior is unchanged.

## Why

Plumb is most useful on a brownfield codebase, but a 200-violation
first run is too noisy to action. `--suggest-ignores` produces a
ready-made starter ignore file: paste it into `.plumbignore`, fix the
violations as a follow-up, and remove entries one by one.

## Pretty format

```text
$ plumb lint plumb-fake://hello --suggest-ignores
desktop
  spacing/grid-conformance
    html > body
      warning: `html > body` has off-grid padding-top 13px; expected a multiple of 4px.
      ...

stats
  ...

Suggested .plumbignore (would suppress 1 violation):
# Format: <rule_id> <selector_path>
spacing/grid-conformance html > body
```

The footer prints after the existing `stats` block, separated by a
blank line.

## JSON format

`--format json --suggest-ignores` adds a `suggested_ignores` array to
the existing envelope:

```json
{
  "plumb_version": "0.0.x",
  "run_id": "sha256:…",
  "stats": { … },
  "suggested_ignores": [
    { "rule_id": "color/palette-conformance", "selector": "#cta" },
    { "rule_id": "spacing/grid-conformance", "selector": "html > body" }
  ],
  "summary": { … },
  "violations": [ … ]
}
```

Entries are sorted by `(rule_id, selector)`. The `run_id` and
`violations` fields are unchanged — toggling `--suggest-ignores` MUST
NOT shift the run digest.

## SARIF

The SARIF formatter ignores `--suggest-ignores`. SARIF 2.1.0 has no
canonical slot for ignore suggestions, and consumers (GitHub Code
Scanning, IDE plugins) parse the schema strictly. Use the JSON output
for tooling that wants the suggestions.

## File format

The suggested footer follows a deliberately minimal grammar:

```text
# Format: <rule_id> <selector_path>
spacing/grid-conformance .header
spacing/grid-conformance .footer .copyright
color/palette-conformance #cta-button
```

One entry per line, two whitespace-separated fields:

- `<rule_id>` — slash-separated rule identifier (e.g.
  `spacing/grid-conformance`).
- `<selector_path>` — the CSS selector path Plumb attached to the
  violation.

Lines beginning with `#` are comments. Trailing whitespace is
insignificant.
