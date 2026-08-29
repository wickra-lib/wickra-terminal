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

## Cross-language golden parity, driven by golden/manifest.json.
##
## Each scenario names a config and a command sequence; replaying it must produce
## the frame in its expected file, byte for byte. The binding returns the core's
## compact command output verbatim, so byte equality against that one file is the
## exact parity check.
##
## Reading the manifest rather than naming one scenario is what makes the corpus
## extensible: a scenario added in the Rust suite is picked up here, and in the
## seven other language suites, with no change to any of them.
##
## The manifest is walked by splitting on the quote character rather than with a
## JSON parser, because this package deliberately has no dependencies and adding
## one for a test would be the tail wagging the dog. Splitting on quotes needs no
## regular expressions and is enough here: every value in the manifest is a
## plain path, and none of them contains a quote.
golden_dir <- function() {
  d <- normalizePath(getwd(), mustWork = FALSE)
  for (i in seq_len(8)) {
    g <- file.path(d, "golden")
    if (file.exists(file.path(g, "manifest.json"))) {
      return(g)
    }
    d <- dirname(d)
  }
  stop("golden/ not found")
}

slurp <- function(path) {
  paste(readLines(path, warn = FALSE), collapse = "\n")
}

## Every quoted token in the manifest, in document order. Splitting on the quote
## character puts the quoted values at the even positions of the result.
quoted_tokens <- function(text) {
  parts <- strsplit(text, '"', fixed = TRUE)[[1]]
  if (length(parts) < 2) {
    return(character(0))
  }
  parts[seq(2, length(parts), by = 2)]
}

## Read the manifest as a list of scenarios. The token stream is a flat sequence
## of keys and values; a key switches what the following tokens mean, and a
## scenario ends when its last key has been read.
parse_manifest <- function(text) {
  keys <- c("scenarios", "commands", "config", "expected", "name")
  tokens <- quoted_tokens(text)
  scenarios <- list()
  current <- list()
  key <- NULL
  for (token in tokens) {
    if (token %in% keys) {
      key <- token
      next
    }
    if (is.null(key)) {
      next
    }
    current[[key]] <- token
    if (identical(key, "name")) {
      scenarios[[length(scenarios) + 1L]] <- current
      current <- list()
      key <- NULL
    }
  }
  scenarios
}

g <- golden_dir()
scenarios <- parse_manifest(slurp(file.path(g, "manifest.json")))
stopifnot(length(scenarios) >= 9)

scenario_names <- character(0)
for (scenario in scenarios) {
  commands <- readLines(file.path(g, scenario$commands), warn = FALSE)
  commands <- commands[nzchar(commands)]
  stopifnot(length(commands) > 0)
  cfg <- slurp(file.path(g, scenario$config))
  expected <- trimws(slurp(file.path(g, scenario$expected)))
  gterm <- wkterm_new(cfg)
  frame <- ""
  for (command in commands) {
    frame <- wkterm_command(gterm, command)
  }
  stopifnot(identical(trimws(frame), expected))
  scenario_names <- c(scenario_names, scenario$name)
  cat("  golden parity:", scenario$name, "ok\n")
}

## A manifest that silently shrank to one entry would leave every parity check
## passing while covering a fraction of what it used to.
for (required in c("basic", "book_deltas", "footprint", "indicators", "seek")) {
  stopifnot(required %in% scenario_names)
}

## The indicator registry is reachable from R.
##
## The registry lives in the Rust core and this binding passes JSON through the
## C ABI, so nothing here needed new binding code. That is exactly why it is
## worth checking: "no code changed" is also what a broken pass-through looks
## like. A non-default indicator is used, so finding it proves the config
## reached the registry rather than the built-in overlay looking right by chance.
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
stopifnot(rows >= 492)
stopifnot(grepl('"kind":"Sma"', catalogue, fixed = TRUE))
stopifnot(grepl('"kind":"MacdIndicator"', catalogue, fixed = TRUE))

## An unknown indicator is rejected, and the error names it.
unknown <- try(
  wkterm_command(rterm, '{"type":"AddIndicator","spec":{"kind":"NotReal"}}'),
  silent = TRUE
)
stopifnot(inherits(unknown, "try-error"))
stopifnot(grepl("NotReal", conditionMessage(attr(unknown, "condition")), fixed = TRUE))

## Arguments are checked before they are dereferenced.
##
## `.Call` hands over whatever the caller wrote, and the shim used to index a
## character vector and take CHAR() of the result without asking whether it was
## a character vector or how long it was. Recent R checks both itself, so on
## R 4.6 this surfaced as "attempt access index 0/0 in STRING_ELT" rather than a
## crash -- an internal message naming neither the package nor the argument --
## and older R, which this package declares support for, does not check at all.
## NA was worse than either: CHAR(NA_STRING) is the string "NA", so it reached
## the config parser and came back as "invalid config".
bad_arg <- function(expr) {
  caught <- try(expr, silent = TRUE)
  stopifnot(inherits(caught, "try-error"))
  conditionMessage(attr(caught, "condition"))
}

stopifnot(grepl("config must be a single string", bad_arg(wkterm_new(character(0))), fixed = TRUE))
stopifnot(grepl("config must be a single string", bad_arg(wkterm_new(c("{}", "{}"))), fixed = TRUE))
stopifnot(grepl("config must be a character vector", bad_arg(wkterm_new(42)), fixed = TRUE))
stopifnot(grepl("config must not be NA", bad_arg(wkterm_new(NA_character_)), fixed = TRUE))
stopifnot(grepl("command must be a single string", bad_arg(wkterm_command(rterm, character(0))), fixed = TRUE))
stopifnot(grepl("not a terminal handle", bad_arg(wkterm_command("nope", '{"type":"Tick"}')), fixed = TRUE))

## The guards must not disturb a well-formed call.
stopifnot(grepl('"panels"', wkterm_command(rterm, '{"type":"Tick"}'), fixed = TRUE))

cat("wickra-terminal R tests passed\n")
