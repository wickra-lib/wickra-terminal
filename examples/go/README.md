# Wickra Terminal examples — Go

Runnable Go examples for the [Wickra Terminal Go binding](../../bindings/go). The binding links against the
prebuilt C ABI library, so build and stage it once before running anything:

```bash
cargo build -p wickra-terminal-c --release
cp target/release/libwickra_terminal.so bindings/go/lib/linux_amd64/   # match your GOOS_GOARCH
```

## Run

As the CI examples job runs it, from the repository root:

```bash
cd examples/go && go run .
```

## The examples

| Example | What it does |
|---------|--------------|
| `synth_terminal.go` | A runnable Go example: drive a synthetic feed and print a frame. |
| `time_machine.go` | The time-machine scenario: rewind a recorded feed and watch state re-fold. |
