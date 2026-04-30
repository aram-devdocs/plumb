# GitHub Code Scanning

GitHub Code Scanning consumes SARIF. Plumb already emits SARIF, so the
workflow is:

1. Run Plumb with `--format sarif --output plumb.sarif`.
2. Upload that file with `github/codeql-action/upload-sarif@v3`.
3. Fail the job after the upload if Plumb reported violations.

The important detail is step ordering. `plumb lint` returns a nonzero
exit code when it finds violations, but you still want the SARIF upload
step to run so the findings show up in GitHub's Security tab.

## Minimal workflow

```yaml
name: plumb-code-scanning

on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: read
  security-events: write

jobs:
  plumb:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Plumb
        run: cargo install --git https://github.com/aram-devdocs/plumb plumb

      - name: Run Plumb
        id: plumb
        shell: bash
        run: |
          set +e
          plumb lint https://example.com --format sarif --output plumb.sarif
          status=$?
          echo "exit_code=$status" >> "$GITHUB_OUTPUT"
          exit 0

      - name: Upload SARIF
        if: always()
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: plumb.sarif

      - name: Fail if Plumb reported violations
        if: steps.plumb.outputs.exit_code != '0'
        run: exit "${{ steps.plumb.outputs.exit_code }}"
```

## Why the extra step exists

If you let `plumb lint` fail the job directly, GitHub skips the SARIF
upload and you lose the code scanning result. Capturing the exit code in
one step and failing later keeps both behaviors:

- the SARIF file is uploaded every time;
- the workflow still ends nonzero when Plumb reports violations.

`continue-on-error: true` on the Plumb step is also fine. The example
above uses explicit exit-code capture because it makes the control flow
obvious in the YAML.

## Notes

- `security-events: write` is required for `upload-sarif`.
- `github/codeql-action/upload-sarif@v3` only uploads the report. It
  does not decide whether your lint step should pass.
- Plumb writes the SARIF file directly with:

```bash
plumb lint https://example.com --format sarif --output plumb.sarif
```
