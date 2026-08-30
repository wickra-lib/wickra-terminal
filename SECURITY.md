# Security Policy

`wickra-terminal` renders market data. It places no orders, holds no credentials
and keeps no position: the `Live` source connects to public endpoints with empty
credentials, and the exchange client it wraps is used here only for public market
data. See [THREAT_MODEL.md](THREAT_MODEL.md) for what that leaves in scope.

## Supported versions

Security fixes are applied to the latest released version, `0.1.0`, only; please
upgrade to the newest release before reporting an issue.

| Version | Supported |
| --- | --- |
| 0.1.0 (latest) | :white_check_mark: |

## Reporting a vulnerability

**Do not open a public issue, pull request or discussion for a security
vulnerability.**

Report it privately through one of:

- GitHub's [private vulnerability reporting](https://github.com/wickra-lib/wickra-terminal/security/advisories/new)
  ("Report a vulnerability" under the repository's *Security* tab), or
- email to **support@wickra.org** with a subject line starting with
  `[wickra-terminal security]`.

Please include:

- the affected version or commit, and the platform / language binding,
- a description of the issue and its impact,
- steps to reproduce, ideally a minimal proof of concept.

## What to expect

We aim to acknowledge within a few days, agree a disclosure timeline, and credit
reporters who wish to be named once a fix ships.

## Scope

In scope:

- **Memory safety across the C ABI.** Ten language bindings call five exported
  functions, four of them across raw pointers; a null, a use-after-free or an unwind across that
  boundary is the highest-impact class of bug here. The release profile is
  `panic = "abort"` precisely because unwinding through C is undefined behaviour.
- **Untrusted input handling.** Config TOML and JSON, command JSON and feed
  events all come from outside. Malformed input must yield a typed error, never a
  panic or a hang.
- **Supply chain.** A compromised dependency, a tampered release artefact, or a
  workflow that could be made to publish one.

Out of scope:

- Vulnerabilities in the exchanges themselves.
- Trading losses. The terminal displays data; what a reader does with it is not a
  security boundary.

Execution, credentials and order flow are **not** in scope because they are not
implemented. If that changes, this document and the threat model change with it —
the scope here describes the code as it is, not as it may become.

## Security assurance case

A short, evidence-backed argument for why this terminal can be run safely.

**Security requirements.** wickra-terminal reads public market data and renders
it. It stores no credentials, authenticates no users, places no orders, and
implements no cryptography of its own. The requirements are therefore: (1)
memory safety across ten language boundaries, (2) robust handling of hostile or
degenerate feed input without panics or unbounded memory, (3) integrity of the
published artefacts, and (4) a healthy dependency supply chain.

**How the requirements are met.**

- *Memory safety* — the workspace sets `unsafe_code = "forbid"`
  ([`Cargo.toml`](Cargo.toml)), and the core contains no `unsafe` at all. Two
  crates relax it and say why in their own manifests: `bindings/c` allows it
  because a C ABI dereferences caller-supplied pointers, and `bindings/node`
  denies rather than forbids because napi-rs generates it. Neither adds
  indicator logic, so the safe core's guarantees still cover every computation.
- *Panics do not cross a boundary* — the workspace builds with
  `panic = "unwind"` specifically so the FFI layers can catch. Every entry point
  in `bindings/c` that runs code is wrapped in `catch_unwind`, pyo3 converts a
  panic into a Python exception, and napi-rs is told to catch with
  `#[napi(catch_unwind)]`. A test reads the C ABI's own source and fails if an
  entry point is added without one. WASM is the exception and cannot be covered:
  an unwind traps on `wasm32`.
- *Input robustness* — the config parser, the event fold, the state machine and
  the view-model layer each have a coverage-guided fuzz target
  ([`fuzz/`](fuzz/)) run in CI. The state fold is additionally pinned by
  property tests, and the footprint ladder is bounded so a hostile tape cannot
  grow it without limit.
- *Static and dynamic analysis* — every push and pull request runs Clippy
  (`clippy::pedantic`, warnings-as-errors) on three operating systems, CodeQL
  across five languages, `zizmor` over every workflow, the fuzz smoke run, and
  the full test suite in ten languages against a shared golden corpus.
- *Artifact integrity* — commits and tags are signed, releases are built in CI
  from a tag, and release artefacts carry build provenance attestations.
- *Supply chain* — dependencies are watched by Dependabot across ten ecosystems
  and audited by `cargo-deny` (licences and advisories) and OSV-Scanner on every
  change. Every GitHub Action is pinned by commit SHA.

**Residual risk.** The `live` feature opens a TLS WebSocket to an exchange using
the platform TLS library, so transport security depends on that library rather
than on this project. Coverage of that socket path is the one acknowledged gap:
it cannot be reached by an offline test suite, and `ci.yml` says so at the
measurement rather than reporting a flattering number. This is not a trading
system and is provided "as is" — see the disclaimers in `README.md` and the
licences.

## Secrets management

No secrets or credentials are stored in version control. What automation needs
is held as encrypted secrets at the GitHub organisation level and referenced
through the `secrets.*` context; nothing is written into the repository, its
logs, or its build artefacts. Secret scanning **with push protection** is
enabled, so a credential is blocked at the push rather than found afterwards.
Secrets are scoped as narrowly as they can be and rotated when a holder changes
or exposure is suspected.

The browser renderer is deployed by Cloudflare's own Git integration rather than
by a workflow, so no deployment token exists in this repository at all.

## Support timeline and end of support

There is no release yet, so nothing is currently supported. When there is: only
the **latest released version** receives security fixes, and publishing a newer
release ends support for the previous one immediately. The supported-versions
table above is authoritative.

## Remediation policy (dependencies and code scanning)

- **Severity threshold.** Vulnerabilities of **medium severity or higher**, in
  this project's code or in a dependency, are fixed promptly and before the next
  release. Lower-severity findings are addressed on a best-effort basis.
- **Automated enforcement (SCA).** Every change is evaluated by `cargo-deny`
  (RUSTSEC advisories and licence policy), OSV-Scanner and Dependabot. A
  known-vulnerable dependency fails CI and **blocks the change** until it is
  resolved or waived with a written justification.
- **Automated enforcement (SAST).** Every change is evaluated by CodeQL, Clippy
  with `-D warnings`, and `zizmor` over the workflows. Findings **block the
  change** in CI until they are fixed.
- **Pre-release gate.** A release is not cut while an unresolved
  medium-or-higher finding is outstanding.

## Vulnerability exploitability (VEX)

An advisory that does not affect this project — the vulnerable path is
unreachable, or the affected feature is not enabled — is triaged and recorded
with its not-affected justification rather than answered with an unnecessary
dependency bump. Those records are the project's VEX statement.

They live in two files, and which one depends on how the scanner reads the
graph:

- [`deny.toml`](deny.toml) — `cargo-deny` resolves the feature graph, so a
  suppression there describes something that is actually compiled.
- [`osv-scanner.toml`](osv-scanner.toml) — OSV-Scanner and the OpenSSF Scorecard
  Vulnerabilities check read `Cargo.lock`, which lists packages that are never
  built. A lockfile-only entry is recorded here and deliberately **not**
  mirrored into `deny.toml`, because `cargo-deny` never reports it and an ignore
  entry matching nothing would only hide the next real finding.

Each entry carries the reasoning that justifies it, so a reader can check the
claim rather than take it.
