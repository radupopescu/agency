# Working Practices

## Branching model

Each milestone is developed on a dedicated branch and squash-merged into `main`.

```
git checkout -b m2/repl          # start milestone work
# … commits …
git checkout main
git merge --squash m2/repl
git commit -m "M2: multi-turn REPL …"
git tag -a m2-repl -m "M2: …"
```

`main` contains one clean commit per milestone. The feature branch is kept for
reference (article authors can inspect granular history) but is not pushed.

## Tag naming

`m<N>-<slug>` — lowercase, hyphenated. Examples:

```
m0-skeleton
m1-streaming
m2-repl
```

Use annotated tags (`git tag -a`) with a one-line summary. These become the
article anchors.

## Milestone completion checklist

Before committing and squash-merging each milestone, verify all of the
following in order:

1. **Tests pass** — `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`
2. **Live test** — run at least one end-to-end test against LM Studio (or the relevant backend) to confirm the new capability works
3. **Documentation up to date** — update `doc/plan.md` to mark the milestone ✅, update `doc/architecture.md` if new modules or design decisions were introduced, update `README.md` if usage changed
4. **Example present** — an `examples/m<N>_*.rs` file demonstrates the milestone's core capability
5. **Commit and tag** — squash-merge to `main`, annotated tag

## Runnable examples

Every milestone adds at least one example under `examples/` demonstrating its
core capability. Readers should be able to:

```sh
git checkout m2-repl
cargo run --example repl
```

Examples use `AGENCY_*` env vars for configuration so they work without flags.

## Remote

The user pushes to `origin` (GitHub) manually. Claude Code commits locally only
and never runs `git push`.

## Implementation model

Milestones are implemented with **Claude Sonnet** (switch via `/model` in the
session). Subagents are used only when clearly beneficial (parallel research,
isolated library lookups) — heavy subagent use hides implementation detail that
the article reader needs to follow.

## Dependencies

Add new dependencies with `cargo add` (resolves latest compatible version
automatically) rather than editing `Cargo.toml` by hand.

## Commit message style

```
M<N>: <slug> — <one-line summary>

- bullet describing each significant change
- one bullet per file or logical unit
- new deps listed with versions
```
