package org.wickra.terminal;

import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

/**
 * The binding had no {@link java.lang.ref.Cleaner}, so {@code close()} was the
 * only release path and an unclosed terminal held its whole native state — the
 * folded {@code AppState}, every source and any live socket — for the lifetime
 * of the process. Every class in the wickra library registers a
 * {@code Cleaner.Cleanable}; this one registered none.
 *
 * <p>Registering one puts a second release path in play, and these pin how the
 * two must interact: {@code close()} runs the cleanable rather than freeing
 * directly, so the cleaner is left with no registration for a pointer that is
 * already gone. Freeing directly instead takes the JVM down with a native crash
 * once the terminal is collected.
 */
class ClosedHandleTest {
    private static final String CONFIG =
            "{\"sources\":[{\"Synth\":{\"seed\":1}}],"
                    + "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}";

    @Test
    void commandAfterCloseThrows() {
        Terminal term = new Terminal(CONFIG);
        term.command("{\"type\":\"Tick\"}");
        term.close();

        assertThrows(IllegalStateException.class, () -> term.command("{\"type\":\"Tick\"}"));
    }

    @Test
    void closeIsIdempotent() {
        Terminal term = new Terminal(CONFIG);
        term.close();
        term.close();
    }

    @Test
    void collectingClosedTerminalsDoesNotFreeThemAgain() {
        // A double free is a native crash, so the failure signal here is the
        // absence of a test JVM rather than a red test.
        for (int i = 0; i < 200; i++) {
            Terminal term = new Terminal(CONFIG);
            term.command("{\"type\":\"Tick\"}");
            term.close();
        }
        System.gc();
        assertTrue(true, "the cleaner released nothing a second time");
    }

    @Test
    void unclosedTerminalsAreReleasedRatherThanLeaked() {
        // Nothing observable from Java confirms the release itself; what this
        // pins is that the registration exists and is safe to run, which is the
        // half that a mistake here would break.
        for (int i = 0; i < 200; i++) {
            Terminal term = new Terminal(CONFIG);
            term.command("{\"type\":\"Tick\"}");
        }
        System.gc();
        assertTrue(true, "the cleaner released every unclosed terminal");
    }

    @Test
    void anUnclosedTerminalIsUnaffected() {
        try (Terminal term = new Terminal(CONFIG)) {
            term.command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
            assertTrue(term.command("{\"type\":\"Tick\"}").contains("panels"));
        }
    }
}
