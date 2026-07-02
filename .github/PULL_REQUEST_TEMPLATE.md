<!-- Keep it short. One logical change per PR. -->

## What

<!-- What does this change and why? -->

## Checklist

- [ ] `cargo fmt --all` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` are clean
- [ ] `cargo test --workspace --all-features` passes
- [ ] Tests added/updated (prefer hand-computed expectations for core changes)
- [ ] Panels emit view-models only — no renderer commands leaked into the core
- [ ] `AppState` fold stays O(1); golden frames regenerated if the schema changed
- [ ] `CHANGELOG.md` updated under `[Unreleased]`
