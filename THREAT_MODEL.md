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
- **No order path, checked by the compiler rather than by review.** `connect`
  hands back a `dyn Exchange`, which carries order placement and balances
  alongside the reads; the live source narrows it to `dyn MarketData` and holds
  only that. The methods are not there to call, so an edit that reached for one
  fails to build rather than needing to be noticed. This section used to rest on
  the absence of such code, which is a weaker thing: absence is a fact about
  today, and a type is a fact about tomorrow.
- **Panics never unwind across the C ABI.** The release profile is
  `panic = "abort"`, because unwinding through C is undefined behaviour. Every
  export null-checks its pointers and returns a typed error code.
- **Defensive parsing, fuzzed.** Five targets run on every push, and malformed
  input has to yield a typed error rather than a panic in each:

  | target | what it drives |
  |---|---|
  | `config_parse` | `Config::from_json` and `Terminal::from_json` |
  | `feed_event` | an `Event` and a whole feed, deserialized |
  | `state_fold` | folding an arbitrary feed into a fresh `AppState` |
  | `view_model` | `command_json` -- the data-driven boundary itself |
  | `registry_drive` | the registry's construction arms, by name and parameters |

  This list named four of the five and mapped two of them to the wrong target,
  which is worth more than a correction: `registry_drive` is the one that found
  a real fault. An indicator period of 10^20 cast cleanly to a `usize`, the
  indicator asked for a `Vec` that size, and the process aborted on a panic no
  caller could catch -- reachable from the indicator prompt in either renderer.
  Periods are bounded at a million now, in the generator and the generated file
  alike.
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
