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
stopifnot(rows >= 497)
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

## Streaming a feed and re-folding it in one batch reach the same frame.
##
## The terminal reaches a state two ways. Streaming folds one event per tick as
## it arrives; Seek throws the state away and re-folds the whole prefix in a
## single batch. ARCHITECTURE.md calls that re-fold the moat -- it is what makes
## a rewind deterministic and what lets the browser run the time-machine with no
## engine behind it -- so the two must land on byte-identical frames.
##
## Byte-identical, not merely equal: the binding returns the core's compact
## command output verbatim, so string equality here is the exact check with no
## JSON comparison in the way. The Rust suite proves the core re-folds correctly;
## this proves the binding carries the same bytes out.
refold_ticks <- 4L
refold_events <- 8L

refold_config <- function() {
  events <- vapply(seq_len(refold_events), function(i) {
    paste0(
      '{"type":"trade","symbol":{"base":"BTC","quote":"USDT"},',
      '"price":"', 99L + i, '","quantity":"1","aggressor":"Buy","timestamp":', i, '}'
    )
  }, character(1))
  feed <- paste0("[", paste(events, collapse = ","), "]")
  ## The feed travels inside a JSON string, so its quotes are escaped once.
  escaped <- gsub('"', '\\"', feed, fixed = TRUE)
  paste0(
    '{"sources":[{"Replay":{"dataset":"', escaped, '"}}],',
    '"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}'
  )
}

subscribed_replay <- function() {
  handle <- wkterm_new(refold_config())
  invisible(wkterm_command(handle, '{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}'))
  handle
}

streamed <- subscribed_replay()
streamed_frame <- ""
for (i in seq_len(refold_ticks)) {
  streamed_frame <- wkterm_command(streamed, '{"type":"Tick"}')
}

## A second terminal runs the feed out, then re-folds the same prefix in one
## batch. Running past the point first is what makes this a rewind rather than a
## replay of state it still had.
rewound <- subscribed_replay()
for (i in seq_len(refold_events)) {
  invisible(wkterm_command(rewound, '{"type":"Tick"}'))
}
refolded_frame <- wkterm_command(
  rewound,
  paste0('{"type":"Seek","source":0,"index":', refold_ticks, '}')
)

stopifnot(identical(streamed_frame, refolded_frame))

## A guard on the guard: two empty frames are also byte-identical, and an
## equality test that passes on nothing proves nothing.
stopifnot(grepl(
  paste0('"last":', 99L + refold_ticks),
  streamed_frame,
  fixed = TRUE
))


## The recorder, the scrubber and the host feed, end to end through the binding.
##
## Four commands sit on the boundary, are documented in all nine binding READMEs,
## and were driven by almost no binding: SetRecording and ExportRecording by none
## at all, ReplayPosition only by the C example, FeedDerivatives by none. The
## README completeness test proved the promise and nothing checked it was kept,
## so the recorder had never been executed outside Rust.
##
## The round trip is the point: arm the recorder, drive the terminal, export what
## it kept, and hand that straight back as a Replay dataset. A binding that
## mangled the export would be caught by the replay refusing it, which no
## assertion about a string shape would find.
##
## No JSON library here either -- the binding deliberately has none, and this
## file reads every other answer by matching on the wire form.

## A derivatives indicator, so FeedDerivatives is observable in the frame rather
## than merely accepted.
recorder_config <- paste0(
  '{"sources":["Manual"],',
  '"indicators":[{"kind":"FundingRate","params":[]}],',
  '"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}'
)

recorder_symbol <- "BTC/USDT"

## A recording as a Replay dataset: the whole array becomes one JSON string
## field, so every quote inside it has to be escaped.
replay_config <- function(dataset) {
  paste0(
    '{"sources":[{"Replay":{"dataset":"',
    gsub('"', '\\"', dataset, fixed = TRUE),
    '"}}],"indicators":[],',
    '"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}'
  )
}

subscribed_recorder <- function() {
  term <- wkterm_new(recorder_config)
  invisible(wkterm_command(
    term,
    paste0('{"type":"Subscribe","source":0,"symbol":"', recorder_symbol, '"}')
  ))
  term
}

drive <- function(term, price, timestamp) {
  invisible(wkterm_command(term, paste0(
    '{"type":"Feed","source":0,"event":{"type":"trade",',
    '"symbol":{"base":"BTC","quote":"USDT"},"price":"', price, '",',
    '"quantity":"0.5","aggressor":"Buy","timestamp":', timestamp, '}}'
  )))
  wkterm_command(term, '{"type":"Tick"}')
}

## The recorder round trips through a replay.
rec_term <- subscribed_recorder()

## Nothing is kept until the recorder is armed, and asking is not an error.
stopifnot(identical(wkterm_command(rec_term, '{"type":"ExportRecording"}'), "[]"))

invisible(wkterm_command(rec_term, '{"type":"SetRecording","capacity":64}'))
rec_prices <- c("100", "101", "102", "103")
for (i in seq_along(rec_prices)) {
  invisible(drive(rec_term, rec_prices[i], i))
}

recording <- wkterm_command(rec_term, '{"type":"ExportRecording"}')
stopifnot(startsWith(recording, "[{"))
stopifnot(grepl('"price":"100"', recording, fixed = TRUE))
stopifnot(grepl('"price":"103"', recording, fixed = TRUE))

## Straight back in as a dataset: the shape Replay takes is the shape
## ExportRecording answers with, which is what makes a session keepable.
rec_replay <- wkterm_new(replay_config(recording))
invisible(wkterm_command(
  rec_replay,
  paste0('{"type":"Subscribe","source":0,"symbol":"', recorder_symbol, '"}')
))
rec_frame <- ""
for (i in seq_len(4)) {
  rec_frame <- wkterm_command(rec_replay, '{"type":"Tick"}')
}
stopifnot(grepl('"last":103.0', rec_frame, fixed = TRUE))

## ReplayPosition answers 0/0 for a source that is not a recording -- rather than
## an error, so a renderer can ask about whatever is focused without first
## knowing what kind of source it is.
stopifnot(identical(
  wkterm_command(rec_term, '{"type":"ReplayPosition","source":0}'),
  '{"cursor":0,"length":0}'
))

## And tracks the cursor through one that is.
rec_scrub <- wkterm_new(replay_config(recording))
invisible(wkterm_command(
  rec_scrub,
  paste0('{"type":"Subscribe","source":0,"symbol":"', recorder_symbol, '"}')
))
stopifnot(identical(
  wkterm_command(rec_scrub, '{"type":"ReplayPosition","source":0}'),
  '{"cursor":0,"length":4}'
))
for (i in seq_len(3)) {
  invisible(wkterm_command(rec_scrub, '{"type":"Tick"}'))
}
stopifnot(identical(
  wkterm_command(rec_scrub, '{"type":"ReplayPosition","source":0}'),
  '{"cursor":3,"length":4}'
))

## Stopping the recorder clears what it held: both directions clear, so a
## capacity change never leaves a recording that is part one size and part
## another.
invisible(wkterm_command(rec_term, '{"type":"SetRecording","capacity":null}'))
stopifnot(identical(wkterm_command(rec_term, '{"type":"ExportRecording"}'), "[]"))

## Fed derivatives reach a derivatives indicator. Accepting the command proves
## nothing on its own: the update is folded into the market's microstructure and
## reaches an indicator only on the next trade, so the reading is what says it
## arrived.
deriv_term <- subscribed_recorder()
deriv_before <- drive(deriv_term, "100", 1)
stopifnot(grepl('{"name":"FundingRate","value":null}', deriv_before, fixed = TRUE))

## All three prices, or the tick is withheld: a mark without an index and a
## futures price is not a priced market.
invisible(wkterm_command(deriv_term, paste0(
  '{"type":"FeedDerivatives","source":0,"symbol":"', recorder_symbol, '",',
  '"update":{"funding_rate":0.0001,"mark_price":102.0,"index_price":100.0,',
  '"futures_price":104.0,"open_interest":1000.0,"timestamp":9}}'
)))
deriv_after <- drive(deriv_term, "101", 2)
stopifnot(grepl('"name":"FundingRate","value":0.0001', deriv_after, fixed = TRUE))

## And an untracked market is refused rather than silently folded.
untracked <- wkterm_new(recorder_config)
stopifnot(inherits(
  try(wkterm_command(untracked, paste0(
    '{"type":"FeedDerivatives","source":0,"symbol":"', recorder_symbol, '",',
    '"update":{"funding_rate":0.0001,"timestamp":1}}'
  )), silent = TRUE),
  "try-error"
))

cat("wickra-terminal R tests passed\n")
