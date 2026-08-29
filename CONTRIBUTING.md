# Contributing to wickra-terminal

Thanks for your interest. Issues, bug reports, ideas and pull requests are all
welcome at <https://github.com/wickra-lib/wickra-terminal>. For larger changes,
open an issue first so we can agree on the approach.

## License of contributions

wickra-terminal is dual-licensed under the [MIT](LICENSE-MIT) and
[Apache-2.0](LICENSE-APACHE) licences; users may choose either. Unless you state
otherwise, any contribution you intentionally submit for inclusion, as defined
in the Apache-2.0 licence, is dual licensed as above, with no additional terms
or conditions.

## Orientation

- The core — the `DataSource` trait, `AppState`, panels and the view-model
  machinery — lives in `crates/wickra-terminal-core`. It is renderer-agnostic: panels
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
.github/scripts/check-cbindgen.sh   # skips cleanly if cbindgen is not installed
python scripts/check_binding_surface.py
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

A new **panel** implements the `Panel` trait in `crates/wickra-terminal-core/src/panels/`,
adds a `PanelView` variant in `src/view.rs`, and gets a widget in the TUI and a
canvas renderer in the Web front-end — the core stays the single source of truth.
A new **data source** implements the `DataSource` trait in
`crates/wickra-terminal-core/src/source/`, registers in `build_source`, and ships a
golden replay fixture. See `docs/INDICATORS.md`, `docs/PANELS.md` and `docs/SOURCES.md`.

## Building each binding

The dev loop above covers the Rust workspace. Each binding has its own, and none
of them needs the others:

| Binding | Build | Test |
| --- | --- | --- |
| Python | `maturin develop -m bindings/python/Cargo.toml` | `pytest bindings/python/tests` |
| Node | `cd bindings/node && npm run build` | `npm test` |
| WASM | in `bindings/wasm`: `wasm-pack build --target nodejs --release --out-dir pkg-node` | `node --test bindings/wasm/tests/*.test.cjs` |
| C | `cargo build -p wickra-terminal-c --release` | `cargo test -p wickra-terminal-c` |
| C++ | links the C header; built with the C examples | `ctest` in the example build tree |
| C# | `dotnet build bindings/csharp` | `dotnet test bindings/csharp/WickraTerminal.Tests/WickraTerminal.Tests.csproj` |
| Go | needs the C ABI built first | `cd bindings/go && go test ./...` |
| Java | `mvn -f bindings/java package` | `mvn -f bindings/java test` |
| R | `R CMD INSTALL bindings/r` | `Rscript bindings/r/tests/run_tests.R` |

`maturin` wants the virtualenv named in `VIRTUAL_ENV`; setting it is more
reliable than activating the environment, particularly under git-bash on
Windows. The Go, C#, Java and R suites all load the C ABI, so build
`wickra-terminal-c` before running them; `WKTERM_INC` and `WKTERM_LIB` point the
R package at a local build instead of a downloaded release.

Eight of the nine non-Rust suites drive the same golden corpus through
`golden/manifest.json`, so a scenario added there is picked up by all of them
without touching a single binding test.

## Lockfile policy

| Component | Lockfile | Tracked? | Why |
| --- | --- | --- | --- |
| Workspace (Rust) | `Cargo.lock` | **yes** | The workspace ships binaries — the TUI, the examples — and CI builds them, so the graph is pinned for reproducible builds. |
| `bindings/node` | `package-lock.json` | **yes** | Reproducible installs for the native binding. |
| `examples/node` | `package-lock.json` | **yes** | The runnable examples link the binding through a `file:` dependency. |
| `web` | `package-lock.json` | **yes** | The browser renderer is built in CI and by Cloudflare, which both need the same tree. |
| `bindings/python` | — | n/a | The published package declares `dependencies = []`; its native code is pinned through the workspace `Cargo.lock`. |
| `.github/requirements` | `*.txt` (hash-pinned) | **yes** | The CI build and test tooling, locked with `uv pip compile --generate-hashes` and installed with `--require-hashes`. Split per Python version because pytest 9 carries the PYSEC-2026-1845 fix but needs 3.10, while 3.9 is the abi3 floor. |
| `fuzz` | `fuzz/Cargo.lock` | **no** (ignored) | `fuzz/` is a detached crate and `cargo-fuzz init` ignores its lock. The smoke job resolves fresh, so nothing depends on it. |
| `bindings/{csharp,java,go,r}` | — | n/a | NuGet, Maven, Go modules and R resolve from their own manifests; none of them adds a lockfile this repository would carry. |

To refresh every committed lockfile at once, run
[`./scripts/update-lockfiles.sh`](scripts/update-lockfiles.sh). It uses `uv`
for the Python locks, and bootstraps it on Linux and macOS if it is absent,
so each target version's hashed closure can be regenerated without that
interpreter installed here.

When you add a committed Node package, commit its `package-lock.json` and remove
any ignore rule that would hide it. Do not add a lockfile at the repository
root: the root is not an npm package.

## Commit and pull-request workflow

- One logical change per commit, with a message that says what changed and why
  it needed changing. The why is the part a reader cannot reconstruct.
- Commits must be **signed**. `git log --format='%G?'` shows `G` for a good
  signature.
- No AI attribution and no `Co-authored-by` trailers for tooling.
- Run `cargo fmt --all` before committing. CI checks formatting on three
  operating systems and will fail on all of them at once.
- Open a pull request rather than pushing to `main`, so the full matrix runs
  before the change lands.
- A change that adds or alters a guard should say, in the commit message, how it
  was shown to fail. A guard nobody has watched fail is a guard nobody knows
  works.

## Governance

Roles, how decisions are made, and what happens if the maintainer stops:
[`GOVERNANCE.md`](GOVERNANCE.md).

## Developer Certificate of Origin

Contributions are accepted under the [DCO](DCO); sign off your commits with
`git commit -s`. By contributing you agree your work is dual-licensed under
`MIT OR Apache-2.0`.
