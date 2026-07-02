# Security Policy

`wickra-terminal` connects to live markets and, once explicitly enabled, can
place authenticated orders through the Wickra exchange layer. In that mode it
handles **secret key material** and **order flow**, so security is a first-class
concern. Please read [THREAT_MODEL.md](THREAT_MODEL.md) for the asset inventory,
trust boundaries and the split between the native and browser renderers.

## Supported versions

This project is pre-release. Security fixes target the `main` branch and the most
recent published version once a release exists.

| Version | Supported |
|---------|-----------|
| `main`  | ✅        |
| `0.1.x` (upcoming) | ✅ |

## Reporting a vulnerability

**Please do not open a public issue, pull request or discussion for security
problems.** Report privately through either channel:

- GitHub → the repository's **Security** tab → **Report a vulnerability**
  (private advisory), or
- email **support@wickra.org**.

Include a description, affected version/commit, reproduction steps and impact.
**Never include real API keys, secrets or signed request payloads** — redact them.

We aim to acknowledge within a few days, agree a disclosure timeline, and credit
reporters who wish to be named once a fix ships.

## Scope

In scope: leakage of secret material (logs, errors, memory) on the native
execution path, order-rounding/validation flaws that could mis-size or mis-route
an order, and any path that would let the browser renderer obtain a secret key.
Both renderers default to **read-only / paper** mode. Out of scope:
vulnerabilities in third-party exchanges themselves, and any deployment that puts
secret keys in a browser or other untrusted client (explicitly unsupported — see
the threat model).
