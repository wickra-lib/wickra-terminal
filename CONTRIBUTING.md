# Contributing to wickra-terminal

Thanks for your interest. Issues, bug reports, ideas and pull requests are all
welcome at <https://github.com/wickra-lib/wickra-terminal>. For larger changes,
open an issue first so we can agree on the approach.

## Orientation

- The core — the `DataSource` trait, `AppState`, panels and the view-model
  machinery — lives in `crates/terminal-core`. It is renderer-agnostic: panels
  emit view-models, never renderer commands.
- The two reference renderers consume those view-models: the native TUI in
  `crates/ui-tui` (ratatui) and the Web front-end in `web/` (Vue over the WASM
  binding).
- Every language binding lives under `bindings/<lang>/` and exposes the same
  data-driven surface: a `Terminal` handle plus `command(json) -> json` and
  `version`. Bindings must preserve the **golden-parity invariant**: given the
  recorded feed in `golden/replay/`, the same command produces the byte-identical
  frame in `golden/expected/`.

## The dev loop

Every change runs green locally before a commit:

```bash
cargo fmt --all
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo deny check
```

`cargo fmt --all` and the `clippy -D warnings` gate are enforced in CI on three
operating systems. Tests that hit a live exchange run only against **testnets**,
are gated behind environment variables and are `#[ignore]` by default — never
add a test that uses mainnet or real keys.

## Conventions

- **Commits are signed** and follow Conventional Commits (`feat:`, `fix:`,
  `chore:`, `docs:`…). One logical change per commit. Open a PR against `main`;
  do not push to `main` directly.
- **All public artifacts are in English** — code, comments, commit messages, PR
  titles and bodies, issues and docs.
- **No secrets, ever** — not in code, tests, fixtures, logs, issues or PRs.
  Price/quantity values use `Decimal`, not `f64`.
- **Production code only** — no mocks outside `#[cfg(test)]`, no TODO stubs, and
  no defensive branches that can never run (they fail coverage).

## Adding a panel or a source

A new **panel** implements the `Panel` trait in `crates/terminal-core/src/panels/`,
adds a `PanelView` variant in `src/view.rs`, and gets a widget in the TUI and a
canvas renderer in the Web front-end — the core stays the single source of truth.
A new **data source** implements the `DataSource` trait in
`crates/terminal-core/src/source/`, registers in `build_source`, and ships a
golden replay fixture. See `docs/PANELS.md` and `docs/SOURCES.md`.

## Developer Certificate of Origin

Contributions are accepted under the [DCO](DCO); sign off your commits with
`git commit -s`. By contributing you agree your work is dual-licensed under
`MIT OR Apache-2.0`.
