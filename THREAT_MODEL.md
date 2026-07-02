# Threat Model

`wickra-terminal` renders live market data and, once explicitly enabled, routes
signed orders through the Wickra exchange layer. This document records what it
protects, where the trust boundaries are, and the guarantees the code is held to.
It is a living document — update it when the attack surface changes.

## Assets

1. **Secret key material** — the API key/secret (and where applicable passphrase)
   used by the `Live` execution path. Held **only** by the native renderer on a
   trusted host; never by the browser renderer.
2. **Order flow** — the requests that move capital, once real execution is
   enabled. Corruption (wrong symbol, side, price or quantity) loses money even
   without a key leak.
3. **Account state** — balances, positions and open orders read back from the
   exchange and shown in panels.
4. **Layout / config** — non-sensitive, but user-controlled input parsed from
   TOML/JSON and `localStorage`; it must be parsed defensively (see fuzz targets).

## Trust boundaries

- **The native (TUI) renderer runs on a trusted host** and may hold secret keys
  for the `Live` source. This is the only place authenticated execution happens.
- **The browser (Web) renderer is untrusted** and holds **no secrets**. It runs
  the same core compiled to WebAssembly, but defaults to read-only market data
  and **paper** trading (simulated fills, `localStorage` state). Real execution
  from the browser requires a separate backend that holds the keys — see the
  backend appendix in the architecture docs.
- **Data sources are semi-trusted** — an exchange feed is reachable over TLS, but
  its responses are untrusted input and are parsed defensively.
- **The network is untrusted** — all live transport is TLS via the exchange layer.

## Guarantees the code is held to

- **No secrets in the core or view-models.** `terminal-core` produces view-models
  (values, series, colours) only; it never embeds key material, and the WASM
  binding compiles without any secret-bearing code path.
- **Exact market arithmetic.** Price and quantity use `rust_decimal::Decimal`, not
  `f64`, so panel values and any order rounding are exact and auditable.
- **Deterministic state.** `AppState` folds events in O(1) with no recompute over
  history; golden tests pin the produced frame byte-for-byte, cross-language, so a
  refactor that corrupts state fails loudly.
- **Defensive parsing.** Config, command JSON and feed events are fuzzed
  (`config_parse`, `feed_event`, `state_fold`, `view_model`) with bounded inputs;
  malformed input yields a typed error, never a panic across the C ABI.
- **Read-only / paper by default.** Real execution is opt-in, testnet-first, and
  wired with a dead-man's-switch + reconnect reconciliation inherited from the
  exchange layer.

## Out of scope

- Vulnerabilities in the exchanges themselves.
- Any deployment that places secret keys in a browser or untrusted client — this
  is explicitly unsupported.
- Custody of funds and withdrawal flows (the execution path favours
  withdrawal-disabled keys and does not implement withdrawals).

## Operator guidance

Run in paper mode while developing. For live execution use API keys scoped to
trading only (withdrawals disabled), restrict them by IP where the exchange
allows it, test against testnets before mainnet, and keep keys out of source
control (`.env`, secret managers — never committed).
