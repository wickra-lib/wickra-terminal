# Wickra Terminal — C ABI

The C ABI hub for `wickra-terminal`: a `cdylib` + `staticlib` that every
C-capable language (C, C++, C#, Go, Java, R) links against. The surface is a
tiny, JSON-shaped data API — the same shape as the backtester's `run_json`.

## Surface

```c
#include "wickra_terminal.h"

WickraTerminal *wickra_terminal_new(const char *config_json);
void            wickra_terminal_free(WickraTerminal *handle);
int             wickra_terminal_command(WickraTerminal *handle,
                                        const char *cmd_json,
                                        char **out_json);
void            wickra_terminal_free_string(char *s);
const char     *wickra_terminal_version(void); /* static — do not free */
```

- `wickra_terminal_new` builds a terminal from a JSON config; returns `NULL` on a
  null/invalid argument.
- `wickra_terminal_command` applies a command JSON and writes the resulting frame
  JSON to `*out_json`. Returns `0` (`WICKRA_TERMINAL_OK`) on success, `-2`
  (`WICKRA_TERMINAL_ERR`) with the error message in `*out_json`, or `-1`
  (`WICKRA_TERMINAL_ERR_NULL`) if a required pointer is null.
- The caller owns `*out_json` and frees it with `wickra_terminal_free_string`.

## Example

```c
WickraTerminal *t = wickra_terminal_new(
    "{\"sources\":[{\"Synth\":{\"seed\":1}}],"
    "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}");

char *out = NULL;
wickra_terminal_command(t, "{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}", &out);
wickra_terminal_free_string(out);

wickra_terminal_command(t, "{\"type\":\"Tick\"}", &out); /* out = frame JSON */
printf("%s\n", out);
wickra_terminal_free_string(out);

wickra_terminal_free(t);
```

## Build

```bash
cargo build -p wickra-terminal-c --release
# Header (regenerated + committed; a CI job checks it stays in sync):
cbindgen --config bindings/c/cbindgen.toml --crate wickra-terminal-c \
  --output bindings/c/include/wickra_terminal.h
```

The built library is named `wickra_terminal` (`.dll` / `.so` / `.dylib` and a
`.a`/`.lib` static library) under `target/release/`.
