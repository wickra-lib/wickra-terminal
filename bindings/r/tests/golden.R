## Golden-parity cases for the wickra-terminal R binding.
##
## Lives outside run_tests.R and outside the built tarball (see .Rbuildignore).
## The corpus is at the repository root, above this package, so these cases only
## resolve when run from there -- which CI does explicitly. `R CMD check` runs
## everything under tests/ from the built tarball, where the corpus does not
## exist, and that is what r-universe runs on every platform. Shipping this
## block is what left wickraexchange 0.1.1 with 0 of 13 platform binaries.
##
## The other assertions in run_tests.R build their own data and travel fine.

library(wickraterminal)

config <- paste0(
  '{"sources":[{"Synth":{"seed":1}}],',
  '"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}'
)

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
stopifnot(length(scenarios) >= 12)

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

cat("wickra.terminal R golden parity passed\n")
