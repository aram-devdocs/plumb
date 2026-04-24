# Rule: dispatch strategy

How to pick between `/gh-issue N` (one session per ticket) and
`/gh-issue N M O` (one session, one PR addressing multiple tickets)
when a batch is ready. The runbook generator emits a recommendation
per batch; this is the reasoning behind it.

## The three strategies

### Split — one session per ticket

Every ticket gets its own `/gh-issue <N> --worktree` session, its own
branch, its own PR.

Choose split when:

- Tickets touch **different crates** (reviewers differ, imports differ).
- Tickets have **different effort sizes** (don't block a fast fix behind a slow one).
- Any ticket needs **security-auditor** review — isolate it.
- A ticket is **L or XL effort** — it deserves focused review alone.
- You want **maximum review isolation**; ESLint / clippy failures in one ticket don't block others.

### Bundle — one session, multiple tickets, one PR

`/gh-issue N M O --worktree` — one branch, one PR, N+M+O issues closed on merge.

Choose bundle when:

- Tickets are **cookie-cutter similar** — e.g. 8 spacing rules that each add `fn check(&self, …)` + a golden test + a docs page. Shared fixture + shared template → one review instead of N.
- Tickets share a **single refactor** (introduce a trait + 3 impls all in one commit).
- Tickets must **merge atomically** (cross-crate type rename; 5-file API change).
- Total expected diff is **< 400 LOC** across all tickets — keeps review tractable.
- All tickets are **S or XS effort**, same crate, same category.

### Cluster — mixed split + bundle in the same batch

Choose cluster when:

- Batch has a **mix of S/M and L** tickets — bundle the small ones, split the large.
- Batch has **one security-sensitive** ticket — split it out, bundle the rest.
- Batch has **two distinct crates** — one session per crate, tickets within a crate bundled.

## Recommendation heuristic

The generator applies this at runbook-render time:

```
if len(batch) == 1:                                             → single
elif every ticket small (XS/S) and same crate and len >= 3
     and no security review:                                    → bundle
elif mix of small and large efforts:                            → cluster
elif same crate and no large:                                   → bundle
else:                                                           → split
```

Shown with a one-line rationale. The recommendation is advisory — the
parent body always shows all viable commands so you can pick.

## Concurrency cap

Aim for **3–5 concurrent sessions** max. Above that, merge-rebase tax
and human coordination cost begin to dominate the parallelism win.

If a gate has 11 tickets (e.g. Phase 7 Gate 1: 7A + 7C), cluster
rather than firing 11 sessions: bundle 7A's 6 small rules into one
session, split the two large rules (`color/contrast-aa`,
`baseline/rhythm`) as their own sessions, bundle 7C's 3 MCP tools —
total 5 sessions.

## Review-gate implications

- **Split**: each PR goes through the 4 Plumb gates independently.
  Security-auditor fires only on the PRs that need it.
- **Bundle**: the 4 gates fire once for the bundled PR. If ANY ticket
  inside triggers security-auditor, the whole bundle gets audited.
- **Cluster**: as split, per the cluster composition.

## Anti-patterns

- **Splitting cookie-cutter work** into 8 PRs when the pattern is
  identical — burns reviewer cycles for no defect-detection gain.
- **Bundling unrelated work** — creates a 2000-LOC PR that skips
  meaningful review. Fails the <400-LOC review quality threshold.
- **Parallelizing at the wrong grain**: 10 concurrent sessions in a
  single-crate batch → merge conflicts on `register_builtin` or
  `Cargo.toml`. Prefer bundle here.

## Runbook contract

Runbook parent bodies (rendered from `docs/runbooks/*-spec.yaml`)
always show all viable dispatch shapes for each batch. The "recommended"
annotation is advisory. Pick the shape that fits the tickets you're
actually looking at today.
