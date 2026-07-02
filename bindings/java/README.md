# Wickra Terminal — Java

JVM bindings for the `wickra-terminal` data-driven core, over the Wickra C ABI
using the Foreign Function & Memory API (FFM/Panama). Build a `Terminal` from a
JSON config, drive it with command JSON, read back frame view-models — the same
protocol as the native TUI and every other binding.

Requires a JVM with a finalized FFM API (`maven.compiler.release` 22).

## Usage

```java
import org.wickra.terminal.Terminal;

try (Terminal term = new Terminal(
        "{\"sources\":[{\"Synth\":{\"seed\":1}}]," +
        "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}")) {
    term.command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
    String frame = term.command("{\"type\":\"Tick\"}");
    System.out.println(frame);
    System.out.println(Terminal.version());
}
```

## Build and test from source

```bash
cargo build -p wickra-terminal-c          # produces the native wickra_terminal library
mvn -f bindings/java/pom.xml test         # loads it from target/debug via native.lib.dir
```

The native library is located at runtime via the `native.lib.dir` system
property (the workspace `target/debug` directory by default); the release
pipeline stages it per platform.
