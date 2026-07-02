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

cat("wickra-terminal R tests passed\n")
