## A runnable R example: rewind a recorded feed and watch state re-fold.
##
## The time-machine is what makes a recording more than a slow synthetic feed:
## Seek throws the folded state away and rebuilds it from the recording, so a
## rewind is deterministic rather than approximate. Nothing here is R-specific --
## it is four JSON commands, and every binding drives the same four.
##
##   cargo build --release -p wickra-terminal-c
##   R CMD INSTALL bindings/r
##   Rscript examples/r/time_machine.R

library(wickraterminal)

prices <- 100:105

## The feed as a JSON array, then escaped into the dataset string field. Written
## out rather than serialised, so the example needs no JSON package.
feed <- function() {
  events <- vapply(seq_along(prices), function(i) {
    paste0(
      '{"type":"trade","symbol":{"base":"BTC","quote":"USDT"},',
      '"price":"', prices[i], '","quantity":"1","aggressor":"Buy","timestamp":', i, '}'
    )
  }, character(1))
  paste0("[", paste(events, collapse = ","), "]")
}

config <- function() {
  escaped <- gsub('"', '\\"', feed(), fixed = TRUE)
  paste0(
    '{"sources":[{"Replay":{"dataset":"', escaped, '"}}],',
    '"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}'
  )
}

## The chart panel's `last`, pulled out with a regular expression: the binding
## returns the core's JSON verbatim and this example deliberately depends on no
## JSON reader.
last_price <- function(raw) {
  sub('.*"panel":"chart"[^}]*"last":([-0-9.eE]+).*', "\\1", raw)
}

term <- wkterm_new(config())
invisible(wkterm_command(term, '{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}'))

raw <- ""
for (i in seq_along(prices)) {
  raw <- wkterm_command(term, '{"type":"Tick"}')
}
cat("played to the end:   last =", last_price(raw), "\n")

where <- wkterm_command(term, '{"type":"ReplayPosition","source":0}')
cat("position:           ", where, "\n")

## Rewind to just after the second trade. The state is rebuilt from the
## recording rather than restored from a snapshot, which is why a rewind lands on
## exactly the frame the forward pass had at that point.
raw <- wkterm_command(term, '{"type":"Seek","source":0,"index":2}')
cat("rewound to index 2:  last =", last_price(raw), "\n")

## And forward again from there, over the same events.
raw <- wkterm_command(term, '{"type":"Tick"}')
cat("one tick later:      last =", last_price(raw), "\n")
