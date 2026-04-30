# reviewdog

Plumb does not ship a built-in reviewdog formatter. The integration
here converts `plumb lint --format json` output to reviewdog's
`rdjson` format with `jq`.

The committed runner config lives at `contrib/reviewdog-plumb.yaml`.
Copy that file from the Plumb repo into your own project before using
`-conf=contrib/reviewdog-plumb.yaml`.

## What the config does

Plumb reports findings against a rendered target, not a source file in
your repository. reviewdog expects file-based diagnostics. The config
committed in `contrib/reviewdog-plumb.yaml` does four things:

- running `plumb lint plumb-fake://hello --format json` (replace
  `plumb-fake://hello` with the URL you want to lint);
- converting `.violations[]` to `rdjson`;
- attaching each diagnostic to the synthetic path
  `plumb-lint-target:1:1`;
- keeping the rule id, docs URL, selector, viewport, and message.

That makes the output usable for reviewdog reporters such as
`local`, `github-check`, and `github-annotations`. It is a transport
layer, not source mapping.

## Local run

Before running this, copy `contrib/reviewdog-plumb.yaml` from this repo
into your project's `contrib/` directory. Then edit the copied file and
replace `plumb-fake://hello` with the real target URL.

```bash
reviewdog \
  -conf=contrib/reviewdog-plumb.yaml \
  -runners=plumb \
  -reporter=local \
  -filter-mode=nofilter
```

## GitHub Actions example

```yaml
name: plumb-reviewdog

on:
  pull_request:

permissions:
  contents: read
  checks: write

jobs:
  plumb:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Plumb
        run: cargo install --git https://github.com/aram-devdocs/plumb plumb

      - uses: reviewdog/action-setup@v1
        with:
          reviewdog_version: latest

      - name: Run reviewdog
        env:
          REVIEWDOG_GITHUB_API_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          reviewdog \
            -conf=contrib/reviewdog-plumb.yaml \
            -runners=plumb \
            -reporter=github-check \
            -filter-mode=nofilter
```

## The JSON pipeline

The runner config uses this exact pipeline. Replace `plumb-fake://hello`
with the real target URL in your copied config.

```bash
plumb lint plumb-fake://hello --format json | jq -c '
  {
    source: {
      name: "plumb",
      url: "https://plumb.aramhammoudeh.com"
    },
    diagnostics: [
      .violations[] | {
        message: (
          .message
          + " selector=" + (.selector // "<unknown>")
          + " viewport=" + (.viewport // "<unknown>")
        ),
        location: {
          path: "plumb-lint-target",
          range: {
            start: {
              line: 1,
              column: 1
            }
          }
        },
        severity: (
          if .severity == "error" then "ERROR"
          elif .severity == "warning" then "WARNING"
          else "INFO"
          end
        ),
        code: {
          value: .rule_id,
          url: .doc_url
        }
      }
    ]
  }
'
```

On the current `plumb-fake://hello` fixture, `plumb lint` exits `3`
because it found a warning. The config still works because the runner
command is a plain shell pipeline, so the pipeline exit status comes
from `jq`, which exits `0` after producing valid `rdjson`. If you wrap
the same pipeline in a shell that enables `pipefail`, capture or ignore
Plumb's exit code yourself before handing the transformed JSON to
reviewdog.

## Limits

- The synthetic `plumb-lint-target` path is intentional. Plumb is
  linting rendered output, so there is no source file or source line to
  report.
- If you need GitHub Security alerts, use the SARIF workflow from
  [GitHub Code Scanning](./github-code-scanning.md) instead.
