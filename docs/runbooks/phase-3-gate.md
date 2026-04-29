# Phase 3 gate

Internal runbook only. This file is not part of the mdBook site, and
`just phase3-gate-env` is a manual preflight check rather than a CI job.

This gate closes Phase 3. Do not mark the phase done until every item
below is captured in the PR or issue thread.

## Required evidence

Record the command and the result for each check:

1. Environment check:

   ```bash
   just phase3-gate-env
   ```

2. Full validation:

   ```bash
   just validate
   ```

3. Real MCP client session against the live site:

   - Start the server:

     ```bash
     cargo run --quiet -p plumb-cli -- mcp
     ```

   - From a real MCP client such as Claude Code or Cursor, call:
     - `lint_url` with `url=https://plumb.aramhammoudeh.com`
     - `explain_rule` for at least one built-in rule returned by `lint_url`
     - `list_rules`
   - Record the client name, the exact URL, and whether each call
     succeeded.

4. Rules index sync:

   ```bash
   cargo xtask sync-rules-index
   ```

5. Pre-release checks:

   ```bash
   cargo xtask pre-release
   ```

## What to attach

- The `just phase3-gate-env` output, or the exact missing dependency or
  browser error if the environment is incomplete.
- The `just validate` result.
- MCP client evidence that shows `lint_url`, `explain_rule`, and
  `list_rules` all worked against
  `https://plumb.aramhammoudeh.com`.
- Confirmation that `cargo xtask sync-rules-index` passed.
- Confirmation that `cargo xtask pre-release` passed.

## Troubleshooting

### Missing Python packages

Install the dev dependencies with the same interpreter Plumb uses for
runbook tooling:

```bash
python3 -m pip install --requirement requirements-dev.txt
```

If you are working in a virtual environment, activate it first.

On Debian or Ubuntu, `python3 -m venv` may require `python3-venv`.
Installing `python3-yaml` and `python3-jsonschema` also satisfies the
import check if you prefer distro packages.

### Chrome or Chromium not found

Plumb does not install a browser for you. Install one of these binaries:

- `google-chrome`
- `google-chrome-stable`
- `chromium`
- `chromium-browser`

Common install commands:

```bash
# Debian / Ubuntu
sudo apt-get install chromium-browser

# Fedora
sudo dnf install chromium

# macOS
brew install --cask google-chrome
```
