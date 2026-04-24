## Summary

{{SUMMARY}}

## PRD references

{{PRD_REFS}}

## Acceptance criteria

{{ACCEPTANCE_CRITERIA}}

## Batches

{{BATCHES}}

## Phase gate

{{PHASE_GATE_CRITERION}}

**Unblocks:** {{UNBLOCKS}}

## How to execute

1. Create GitHub milestone `{{MILESTONE}}` if it doesn't already exist.
2. Run `bash create-issues.sh` from this directory to create all child issues.
3. Dispatch `/gh-issue <child-number>` for each batch in order; sessions within a batch run in parallel.
4. Close this parent issue when every child is merged and the phase gate holds.

## Generated

- Generated at: {{GENERATED_AT}}
- Source spec: `{{SPEC_PATH}}`
- Repo: `{{REPO}}`
