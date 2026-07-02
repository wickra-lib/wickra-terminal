package org.wickra.terminal;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import org.junit.jupiter.api.Test;

// Cross-language golden parity: build the terminal from the committed
// golden/config.json, replay the feed, and assert the frame equals
// golden/expected/basic.min.json byte-for-byte. The binding returns the core's
// compact command_json string verbatim, so byte equality against that one file is
// the exact cross-language parity check.
class GoldenTest {
    private static Path goldenDir() {
        Path dir = Paths.get("").toAbsolutePath();
        for (int i = 0; i < 8 && dir != null; i++) {
            Path g = dir.resolve("golden");
            if (Files.isRegularFile(g.resolve("config.json"))) {
                return g;
            }
            dir = dir.getParent();
        }
        throw new IllegalStateException("golden/ not found");
    }

    @Test
    void goldenParityFrameIsByteExact() throws IOException {
        Path g = goldenDir();
        String config = Files.readString(g.resolve("config.json"), StandardCharsets.UTF_8);
        String expected = Files
                .readString(g.resolve("expected").resolve("basic.min.json"), StandardCharsets.UTF_8)
                .strip();

        try (Terminal term = new Terminal(config)) {
            term.command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
            String frame = "";
            for (int i = 0; i < 32; i++) {
                frame = term.command("{\"type\":\"Tick\"}");
            }
            assertEquals(expected, frame.strip());
        }
    }
}
