package org.wickra.terminal;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class TerminalTest {
    private static final String CONFIG =
            "{\"sources\":[{\"Synth\":{\"seed\":1}}],"
                    + "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}";

    @Test
    void versionIsNonEmpty() {
        assertFalse(Terminal.version().isEmpty());
    }

    @Test
    void subscribeThenTickReturnsChartFrame() {
        try (Terminal term = new Terminal(CONFIG)) {
            term.command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
            String raw = "";
            for (int i = 0; i < 30; i++) {
                raw = term.command("{\"type\":\"Tick\"}");
            }
            assertTrue(raw.contains("\"panel\":\"chart\""), raw);
        }
    }

    @Test
    void invalidConfigThrows() {
        assertThrows(IllegalArgumentException.class, () -> new Terminal("not json"));
    }

    @Test
    void invalidCommandThrows() {
        try (Terminal term = new Terminal(CONFIG)) {
            assertThrows(IllegalStateException.class,
                    () -> term.command("{\"type\":\"Nope\"}"));
        }
    }
}
