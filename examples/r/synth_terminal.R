# A runnable R example: drive a synthetic feed and print a frame.
#
#   cargo build --release -p wickra-terminal-c
#   export WKTERM_INC="$PWD/bindings/c/include" WKTERM_LIB="$PWD/target/release"
#   R CMD INSTALL bindings/r
#   Rscript examples/r/synth_terminal.R    # add target/release to PATH so the DLL loads

library(wickraterminal)

config <- paste0(
  '{"sources":[{"Synth":{"seed":1}}],',
  '"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}'
)

term <- wkterm_new(config)
invisible(wkterm_command(term, '{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}'))
raw <- ""
for (i in seq_len(20)) {
  raw <- wkterm_command(term, '{"type":"Tick"}')
}

cat("wickra-terminal", wkterm_version(), "\n")
cat(raw, "\n")
