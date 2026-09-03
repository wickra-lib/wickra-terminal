<!--
Keep it short. One logical change per PR.

Touching the core contract, several bindings at once, or the release pipeline?
Use the long form instead — GitHub offers no picker, so append the query
parameter yourself:

    ?expand=1&template=detailed.md

(.github/PULL_REQUEST_TEMPLATE/detailed.md)
-->

## What

<!-- What does this change and why? -->

## Checklist

- [ ] `cargo fmt --all` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` are clean
- [ ] `cargo test --workspace --all-features` passes
- [ ] Tests added/updated (prefer hand-computed expectations for core changes)
- [ ] Panels emit view-models only — no renderer commands leaked into the core
- [ ] `AppState` fold stays O(1); golden frames regenerated if the schema changed
- [ ] `CHANGELOG.md` updated under `[Unreleased]`
