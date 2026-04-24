# Implementer dispatch template

Use this template when dispatching `01-implementer` (or the specialized variants `08-rule-author`, `09-mcp-tool-author`, `10-quick-fix`) against a single task in a plan.

```
Task tool:
  subagent_type: 01-implementer   # or 08-rule-author, 09-mcp-tool-author, 10-quick-fix
  description: "Implement Task N: [task name]"
  prompt: |
    You are implementing Task N: [task name] in the Plumb Rust workspace (`aram-devdocs/plumb`).

    ## Task description

    [FULL TEXT of task from plan — paste it here; do not make the subagent read the plan file]

    ## Context

    [Scene-setting: where this fits in the PRD phase, which crate owns it,
    which public API it touches, which invariants apply.]

    ## Target crate(s)

    - Primary: [crates/plumb-<name>]
    - Dependent crates (if any): [e.g., plumb-cli consuming a new plumb-core API]

    Read the scoped `AGENTS.md` in each target crate before touching code.

    ## Batch context (parallel dispatch only)

    - Batch ID: [batch-N]
    - Position: [M of T]
    - File scope: [list of files this agent is authorized to modify]
    - Peer agents: [peer batch agents and their scopes]

    **Scope rule:** modify only files in File scope. If a file outside scope
    needs changing, STOP and report it as a blocker. Do not modify it.

    ## Before you begin

    If anything is unclear (requirements, acceptance criteria, approach,
    test strategy), ask now. Raise concerns before starting.

    ## Your job

    1. Write the failing test first (TDD):
       - Rule → `crates/plumb-core/tests/golden_<slug>.rs` with an `insta::assert_snapshot!`.
       - MCP tool → case in `crates/plumb-cli/tests/mcp_stdio.rs`.
       - CLI behavior → integration test in `crates/plumb-cli/tests/cli_integration.rs`.
       - Pure fn → unit test in a `#[cfg(test)] mod tests` block.
    2. Implement the minimum code to pass the test.
    3. Refactor with the green test as the safety net.
    4. Run the narrow gate:
       ```bash
       cargo fmt --all -- --check
       cargo clippy -p <crate> --all-targets --all-features -- -D warnings
       cargo nextest run -p <crate>
       ```
    5. Accept intentional snapshot changes: `cargo insta review`.
    6. Commit atomically with a Conventional Commits message:
       `<type>(<scope>): <imperative description>`.

    Work from the repo root: `/Users/aramhammoudeh/dev/plumb`.

    ## Plumb non-negotiables

    - No `unwrap`/`expect`/`panic!` in library crates — return `Result`
      with a `thiserror`-derived enum.
    - `anyhow::Error` is permitted only in `plumb-cli::main`.
    - No `println!`/`eprintln!` outside `plumb-cli`; use `tracing` macros.
    - No `SystemTime::now`/`Instant::now` in `plumb-core`.
    - No `HashMap`/`HashSet` in observable output paths — use `IndexMap`/`IndexSet`.
    - No `unsafe` outside `plumb-cdp`; every `unsafe` block there carries a
      `// SAFETY:` comment.
    - No `todo!`/`unimplemented!`/`dbg!` anywhere.
    - Every `#[allow(...)]` is local (item- or expression-level) with a
      one-line rationale comment directly above.

    ## While you work

    If anything unexpected surfaces, ask. Don't guess.

    ## Before reporting back — self-review

    - Did I implement exactly what the task asks for, nothing more?
    - Does the test actually verify behavior (not just mock it)?
    - Are all public items documented? Fallible public fns have `# Errors`?
    - Does `cargo clippy -p <crate> -- -D warnings` pass clean?
    - If I touched `docs/src/**`, did I run the humanizer skill?

    Fix anything you find before reporting.

    ## Report format

    When done, report:
    - What you implemented.
    - What you tested and the test results.
    - File manifest:
      - `<path>`: created | modified | deleted
    - Commit SHA: `git log --oneline -1`.
    - Batch ID: [batch-N or "none"].
    - Any issues or open questions.
```
