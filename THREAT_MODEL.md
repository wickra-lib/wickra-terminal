# Threat Model

`wickra-terminal` reads. It renders live and recorded market data through a
data-driven core reachable from ten languages, and it does nothing else: no
orders, no credentials, no positions, no account state. This document records
what that leaves worth protecting, where the trust boundaries are, and the
guarantees the code is held to.

It is a living document. If execution is ever added, the assets below change
completely and this document changes first.

## Assets

1. **The host process.** Ten language bindings drive the core through five C ABI
   exports, four of which take raw pointers. A crash, an unwind across the
   boundary, or a use-after-free takes down whatever embedded it — a Python notebook, a JVM, a
   Go service. This is the asset with the largest blast radius, and the only one
   an attacker can reach without already being inside.
2. **Untrusted input.** Config TOML and JSON, command JSON, and feed events from
   an exchange socket. All three are parsed, and all three come from outside the
   process.
3. **The supply chain.** The dependency graph, the release pipeline and the
   artefacts it publishes to seven registries.
4. **What is displayed.** Corrupted state shows a wrong price. That is not a
   security boundary, but it is the correctness the golden corpus exists to pin.

## Trust boundaries

- **The C ABI is the boundary that matters.** Everything above it is another
  language's runtime; everything below is Rust. Pointers cross it, so it is where
  memory safety is either kept or lost.
- **The exchange feed is untrusted input.** Reachable over TLS through the
  exchange layer, but its responses are parsed defensively like any other input.
- **The browser renderer is untrusted, and holds nothing worth stealing.** It
  runs the same core compiled to WebAssembly and opens its own WebSocket to a
  public endpoint. It cannot hold a secret, and there is no secret to hold.
- **The network is untrusted.** All live transport is TLS via the exchange layer.

## Guarantees the code is held to

- **No credentials anywhere.** `LiveSource` constructs its client with
  `Credentials::new("", "")`. There is no key handling to get wrong, no key to
  leak into a log, and no code path that reads one from the environment or disk.
- **Panics never unwind across the C ABI.** The release profile is
  `panic = "abort"`, because unwinding through C is undefined behaviour. Every
  export null-checks its pointers and returns a typed error code.
- **Defensive parsing, fuzzed.** Config, command JSON, the state fold and the
  view-model build each have a fuzz target (`config_parse`, `feed_event`,
  `state_fold`, `view_model`) run on every push. Malformed input yields a typed
  error, never a panic.
- **Exact market arithmetic.** Price and quantity use `rust_decimal::Decimal`,
  not `f64`, through the whole market layer. Values are converted to `f64` only
  at the view-model boundary, where they are being drawn rather than compared.
- **Deterministic state.** `AppState` folds events in O(1) with no recompute over
  history, and the golden corpus pins the produced frame byte-for-byte across all
  ten languages, so a refactor that corrupts state fails loudly rather than
  quietly rendering a wrong number.
- **Bounded memory.** Every accumulating structure is capped: the tape ring, the
  price history, each indicator's series. A feed that never stops does not grow
  the process without limit.
- **Signed, attested releases.** Commits are signed; release artefacts carry
  build provenance attestations; the dependency graph is scanned feature-expanded
  by cargo-deny and OSV-Scanner on every push.

## Out of scope

- Vulnerabilities in the exchanges themselves.
- Trading decisions and their outcomes. The terminal displays data.
- Execution, credentials, order flow and account state — **not because they are
  gated, but because they do not exist.** A threat model that inventories assets
  the code cannot hold teaches a reader the wrong thing about what to check.

## If execution is ever added

It would change every section above, and would need at minimum: an asset
inventory covering key material and order flow, a trust boundary that keeps keys
off the browser side entirely, order validation held to the same exactness as the
display arithmetic, and a security review of the path before it ships enabled.
None of that is in place, because none of it is needed yet.

## Operator guidance

Nothing here needs a credential, so there is none to protect. Run the binary, or
embed the library through any binding; the only input it takes is a config and a
feed.
