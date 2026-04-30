# FAQ and troubleshooting

## 1. Plumb fails with a Content Security Policy (CSP) error

Plumb drives Chrome through the DevTools Protocol (CDP). Some sites
set strict CSP headers that block CDP's injected scripts from
executing.

**Fix:** This is a known limitation of the CDP snapshot approach. Plumb
reads the rendered DOM *after* the page loads, so most CSP policies do
not interfere. If you hit a CSP block, check whether the site sets
`script-src` to a nonce-only or hash-only policy that explicitly
rejects inline evaluation — CDP uses `Runtime.evaluate` which some
strict policies block.

As a workaround, lint a staging or local build of the same page where
you control the CSP headers.

See: [Install Chromium](./install-chromium.md),
[ADR 0002 — Chromium version range](./adr.md).

## 2. How do I lint a page behind authentication?

Plumb opens a fresh headless browser session with no stored cookies or
credentials. Auth-protected pages return a login screen instead of the
content you want to lint.

**Fix:** Serve the page locally without auth, or use a pre-authenticated
session by passing `--executable-path` to a browser profile that has
valid cookies. Plumb does not manage browser profiles or inject
credentials itself — doing so is out of scope by design.

See: [CLI flags](./cli.md).

## 3. Chromium version not supported

Plumb accepts Chromium major versions 131 through 150 inclusive. If
your browser reports a version outside this range, `plumb lint` exits
with `UnsupportedChromium`.

**Fix:** Install a Chromium or Chrome build whose major version falls
within the range. Use `chromium --version` or
`google-chrome --version` to check. Pass `--executable-path` to select
a specific binary if you have multiple installs.

See: [Install Chromium](./install-chromium.md),
[ADR 0002 — Chromium version range](./adr.md).

## 4. Why doesn't Plumb extract CSS-in-JS runtime styles?

Plumb reads computed styles from the rendered DOM — it does not parse
source CSS, evaluate JavaScript, or trace style injection at build
time. CSS-in-JS libraries (Styled Components, Emotion, Tailwind
runtime) inject styles into the document before render, so Plumb sees
their *output* just like any other computed style.

What Plumb does *not* do is trace which CSS-in-JS call site produced a
given computed value. That would require build-tool integration and
framework-specific parsers, which is outside Plumb's scope. Plumb
lints the rendered result, not the source.

See: [Introduction](./introduction.md).

## 5. I get false positives on off-screen elements

Plumb snapshots the full DOM at each viewport. Elements positioned
off-screen (e.g. a mobile nav drawer translated to `left: -9999px`) are
still part of the layout and can trigger spacing or typography rules
even though users never see them at that breakpoint.

**Fix:** Suppress specific rules for known false positives using
per-rule overrides in `plumb.toml`:

```toml
[rules."spacing/scale-conformance"]
enabled = false
```

Or narrow the scope by adjusting your viewports so off-screen
breakpoint elements are not rendered.

See: [Configuration — per-rule overrides](./configuration.md),
[Rules overview](./rules/overview.md).

## 6. How do I tune performance for large pages?

`plumb lint` snapshots every viewport sequentially by default. Large
pages with deep DOM trees take longer to snapshot.

**Fix:** Reduce the number of viewports in `plumb.toml` to only those
you need. For CI, a single `desktop` viewport is often enough. If
snapshot capture itself is slow, check that the page has finished
loading — Plumb waits for the `load` event before snapshotting, so
slow-loading resources delay the run.

See: [Configuration — viewports](./configuration.md).

## 7. Violations differ between my machine and CI

Plumb's output is deterministic: given the same snapshot and config,
the engine produces byte-identical results. If you see differences
between local and CI runs, the snapshot itself differs — usually
because the page content or Chromium version changed between runs.

**Fix:** Pin the same Chromium major version locally and in CI.
Confirm with `chromium --version`. If the page is dynamic (A/B tests,
personalized content), lint a stable staging build instead.

See: [ADR 0002 — Chromium version range](./adr.md),
[Install Chromium](./install-chromium.md).

## 8. Can I use Plumb with Firefox or Safari?

No. Plumb uses the Chrome DevTools Protocol for DOM snapshotting.
Firefox and Safari use different debugging protocols and are not
supported. Chromium-based browsers (Chrome, Edge, Brave) work as
long as their major version is within the supported range.

See: [Install Chromium](./install-chromium.md).

## 9. How do I suppress a single violation?

Plumb does not support inline suppression comments in HTML. To
suppress violations, use per-rule overrides in `plumb.toml`:

```toml
[rules."spacing/scale-conformance"]
enabled = false
```

This disables the rule entirely. There is currently no per-element
suppression mechanism.

See: [Configuration — per-rule overrides](./configuration.md).

## 10. The MCP server is not found by my AI agent

The agent cannot connect to `plumb mcp` — the server does not appear
in the tool list.

**Fix:** Check that the `plumb` binary is on the `PATH` that the agent
inherits. GUI-launched editors (Cursor, VS Code) often get a minimal
`PATH` that excludes `~/.cargo/bin`. Use an absolute path in your MCP
config if needed. After updating the config, restart the agent or
reload the MCP connection.

See: [MCP server](./mcp.md), [Claude Code setup](./mcp/claude.md),
[Cursor setup](./mcp/cursor.md), [Codex setup](./mcp/codex.md).

## 11. `plumb lint` exits with code 2 but no violations

Exit code 2 means an infrastructure failure, not a lint result. Common
causes: the URL is unreachable, Chromium was not found, the config
file is invalid, or the page timed out during load.

**Fix:** Run with `-v` (or `-vv` for trace logging) to see the
underlying error. Check that the URL is accessible from the machine
running Plumb, that Chromium is installed and in the supported version
range, and that `plumb.toml` parses without errors.

See: [CLI — exit codes](./cli.md).

## 12. How do I integrate Plumb into GitHub Actions CI?

Use `plumb lint` in a workflow step and check the exit code:

| Code | Meaning |
|------|---------|
| 0 | No violations. |
| 1 | One or more `error`-severity violations. |
| 3 | Only `warning`-severity violations (no errors). |
| 2 | CLI or infrastructure failure (bad URL, missing config, etc.). |

```yaml
- name: Lint with Plumb
  run: |
    plumb lint https://staging.example.com \
      --format sarif --output plumb.sarif
    rc=$?
    if [ "$rc" -eq 2 ]; then
      echo "::error::Plumb infrastructure failure"
      exit 1
    fi
    # rc 0 = clean, rc 1 = errors, rc 3 = warnings only
    exit "$rc"

- name: Upload SARIF
  if: always()
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: plumb.sarif
```

By default this step fails on exit code 1 (errors) and exit code 2
(infrastructure failure), but passes on exit code 3 (warnings only).
To also fail on warnings, replace the `exit "$rc"` line with
`exit $( [ "$rc" -eq 3 ] && echo 1 || echo "$rc" )`.

SARIF output integrates with GitHub Code Scanning, so violations
appear as annotations on the PR. The `if: always()` on the upload
step ensures SARIF results reach Code Scanning even when lint finds
errors. For other CI systems, use `--format json` and parse the
output.

See: [GitHub Code Scanning](./ci/github-code-scanning.md),
[CLI — exit codes](./cli.md).
