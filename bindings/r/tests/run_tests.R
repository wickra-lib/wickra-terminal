## Plain-R tests for the wickra-terminal R binding (no testthat dependency).
## Mirrors the Rust/Python/Node/Go/C#/Java tests and doubles as the completeness
## guard: it exercises the full public surface (version + new + command).

library(wickraterminal)

config <- paste0(
  '{"sources":[{"Synth":{"seed":1}}],',
  '"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}'
)

## version
stopifnot(nzchar(wkterm_version()))

## subscribe -> tick -> chart frame
term <- wkterm_new(config)
invisible(wkterm_command(term, '{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}'))
raw <- ""
for (i in seq_len(30)) {
  raw <- wkterm_command(term, '{"type":"Tick"}')
}
stopifnot(grepl('"panel":"chart"', raw, fixed = TRUE))

## invalid config raises
stopifnot(inherits(try(wkterm_new("not json"), silent = TRUE), "try-error"))

## invalid command raises
stopifnot(inherits(
  try(wkterm_command(term, '{"type":"Nope"}'), silent = TRUE),
  "try-error"
))

## cross-language golden parity: build the terminal from the committed
## golden/config.json, replay the feed, and assert the frame equals
## golden/expected/basic.min.json byte-for-byte. The binding returns the core's
## compact command output verbatim, so byte equality against that one file is the
## exact cross-language parity check.
golden_dir <- function() {
  d <- normalizePath(getwd(), mustWork = FALSE)
  for (i in seq_len(8)) {
    g <- file.path(d, "golden")
    if (file.exists(file.path(g, "config.json"))) {
      return(g)
    }
    d <- dirname(d)
  }
  stop("golden/ not found")
}

g <- golden_dir()
golden_config <- paste(
  readLines(file.path(g, "config.json"), warn = FALSE),
  collapse = "\n"
)
golden_expected <- trimws(paste(
  readLines(file.path(g, "expected", "basic.min.json"), warn = FALSE),
  collapse = "\n"
))
gterm <- wkterm_new(golden_config)
invisible(wkterm_command(
  gterm, '{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}'
))
golden_frame <- ""
for (i in seq_len(32)) {
  golden_frame <- wkterm_command(gterm, '{"type":"Tick"}')
}
stopifnot(identical(trimws(golden_frame), golden_expected))

## The indicator registry is reachable from R.
##
## The registry lives in the Rust core and this binding passes JSON through the
## C ABI, so nothing here needed new binding code. That is exactly why it is
## worth checking: "no code changed" is also what a broken pass-through looks
## like. A non-default indicator is used, so finding it proves the config
## reached the registry rather than the built-in overlay looking right by chance.
##
## The frame is inspected with fixed substring matches rather than a JSON
## parser, because the package deliberately has no dependencies and adding one
## for a test would be the tail wagging the dog.
registry_config <- paste0(
  '{"sources":[{"Synth":{"seed":1}}],',
  '"indicators":[{"kind":"Rsi","params":[14]}]}'
)
rterm <- wkterm_new(registry_config)
invisible(wkterm_command(
  rterm, '{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}'
))
registry_frame <- ""
for (i in seq_len(30)) {
  registry_frame <- wkterm_command(rterm, '{"type":"Tick"}')
}
stopifnot(grepl('"name":"Rsi(14)"', registry_frame, fixed = TRUE))
stopifnot(!grepl('"name":"Sma(20)"', registry_frame, fixed = TRUE))

## Added and removed at run time.
invisible(wkterm_command(
  rterm, '{"type":"AddIndicator","spec":{"kind":"Atr","params":[14]}}'
))
added <- wkterm_command(rterm, '{"type":"Tick"}')
stopifnot(grepl('"name":"Atr(14)"', added, fixed = TRUE))
invisible(wkterm_command(rterm, '{"type":"RemoveIndicator","label":"Rsi(14)"}'))
removed <- wkterm_command(rterm, '{"type":"Tick"}')
stopifnot(!grepl('"name":"Rsi(14)"', removed, fixed = TRUE))
stopifnot(grepl('"name":"Atr(14)"', removed, fixed = TRUE))

## The catalogue answers with the whole registry: one "kind" key per row.
catalogue <- wkterm_command(rterm, '{"type":"ListIndicators"}')
rows <- length(gregexpr('"kind":', catalogue, fixed = TRUE)[[1]])
stopifnot(rows >= 421)
stopifnot(grepl('"kind":"Sma"', catalogue, fixed = TRUE))
stopifnot(grepl('"kind":"MacdIndicator"', catalogue, fixed = TRUE))

## An unknown indicator is rejected, and the error names it.
unknown <- try(
  wkterm_command(rterm, '{"type":"AddIndicator","spec":{"kind":"NotReal"}}'),
  silent = TRUE
)
stopifnot(inherits(unknown, "try-error"))
stopifnot(grepl("NotReal", conditionMessage(attr(unknown, "condition")), fixed = TRUE))

cat("wickra-terminal R tests passed\n")
