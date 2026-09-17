# Wickra Terminal examples — R

Runnable R examples for the [Wickra Terminal R binding](../../bindings/r). The package compiles a thin
`.Call` glue layer against the C ABI library, so build the library and install
the package first (the CI examples job does exactly this):

```bash
cargo build -p wickra-terminal-c --release
R CMD INSTALL bindings/r
```

## Run

```bash
Rscript examples/r/<example>.R
```

## The examples

| Example | What it does |
|---------|--------------|
| `synth_terminal.R` | A runnable R example: drive a synthetic feed and print a frame. |
| `time_machine.R` | A runnable example against this binding. |
