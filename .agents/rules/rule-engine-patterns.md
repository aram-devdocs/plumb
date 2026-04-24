# Rule: How to add a rule

Every real rule follows this shape. The placeholder at
`crates/plumb-core/src/rules/placeholder.rs` is the working template.

## Steps

### 1. Create the rule module

Pick `<category>/<id>` — e.g. `spacing/hard-coded-gap`, `color/off-palette`.
Place the file at `crates/plumb-core/src/rules/<category>/<id>.rs` (create
`<category>/mod.rs` if the category is new).

```rust
use crate::config::Config;
use crate::report::{Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::snapshot::SnapshotCtx;

pub struct HardCodedGap;

impl Rule for HardCodedGap {
    fn id(&self) -> &'static str { "spacing/hard-coded-gap" }
    fn default_severity(&self) -> Severity { Severity::Warning }
    fn summary(&self) -> &'static str { "Flags `gap`/`margin`/`padding` values that aren't on the spacing scale." }
    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>) {
        // Pure function of (ctx, config). No I/O, no RNG, no wall-clock.
    }
}
```

### 2. Register it

In `crates/plumb-core/src/rules/mod.rs`, add the module and append to
`register_builtin`:

```rust
pub mod spacing;

pub fn register_builtin() -> Vec<Box<dyn Rule>> {
    vec![Box::new(spacing::hard_coded_gap::HardCodedGap)]
}
```

### 3. Add a golden snapshot test

File: `crates/plumb-core/tests/golden_<category>_<id>.rs`. Use `insta` +
a hand-built fixture snapshot (`PlumbSnapshot::canned()`-style).

### 4. Document it

File: `docs/src/rules/<category>-<id>.md`. `plumb explain
<category>/<id>` reads this path. The front matter conventions:

- Status, default severity
- What it checks (precise, English)
- Why it matters
- Example violation (JSON excerpt)
- Configuration knobs
- Suppression guidance
- See also

### 5. Wire `doc_url`

Point at `https://plumb.dev/rules/<category>-<id>`. Consistency matters —
the MCP server's `explain_rule` tool resolves URLs via the same slug.

## What NOT to do

- **Don't emit more than one violation per offending node per viewport.**
  Duplicate detection is caller-side; upstream fan-out burns token budget.
- **Don't introduce new dependencies** for a single rule unless unavoidable.
  Prefer stdlib / already-present crates.
- **Don't log inside `check`.** Rules are pure; the engine's caller is
  where tracing belongs.
