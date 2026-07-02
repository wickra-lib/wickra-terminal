# Wickra Terminal — R

R bindings for the `wickra-terminal` data-driven core, over its C ABI hub
(`.Call`). Build a terminal from a JSON config, drive it with command JSON, read
back frame view-models — the same protocol as the native TUI and every other
binding.

## Usage

```r
library(wickraterminal)

config <- paste0(
  '{"sources":[{"Synth":{"seed":1}}],',
  '"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}'
)

term <- wkterm_new(config)
wkterm_command(term, '{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}')
frame <- wkterm_command(term, '{"type":"Tick"}')
cat(frame, "\n")
cat(wkterm_version(), "\n")
```

## Build and test from source

The package links the `wickra_terminal` C ABI, located out-of-tree via two
environment variables:

```bash
cargo build -p wickra-terminal-c --release
export WKTERM_INC="$PWD/bindings/c/include"      # header dir
export WKTERM_LIB="$PWD/target/release"          # library dir
R CMD INSTALL bindings/r
Rscript bindings/r/tests/run_tests.R             # add target/release to PATH so the DLL loads
```
