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

cat("wickra-terminal R tests passed\n")
