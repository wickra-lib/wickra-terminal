# Security Policy

`wickra-terminal` renders market data. It places no orders, holds no credentials
and keeps no position: the `Live` source connects to public endpoints with empty
credentials, and the exchange client it wraps is used here only for public market
data. See [THREAT_MODEL.md](THREAT_MODEL.md) for what that leaves in scope.

## Supported versions

This project is pre-release. Security fixes target the `main` branch; once a
release exists they will target the latest one.

| Version | Supported |
| --- | --- |
| `main` | :white_check_mark: |
| `0.1.0` (unreleased) | :white_check_mark: |

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
