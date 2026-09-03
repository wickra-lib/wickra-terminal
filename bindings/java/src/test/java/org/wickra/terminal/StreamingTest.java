package org.wickra.terminal;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

/**
 * Streaming a feed and re-folding it in one batch reach the same frame.
 *
 * <p>The terminal reaches a state two ways. Streaming folds one event per tick
 * as it arrives; {@code Seek} throws the state away and re-folds the whole
 * prefix in a single batch. ARCHITECTURE.md calls that re-fold the moat -- it is
 * what makes a rewind deterministic and what lets the browser run the
 * time-machine with no engine behind it -- so the two must land on
 * byte-identical frames.
 *
 * <p>Byte-identical, not merely equal: the binding returns the core's compact
 * command output verbatim, so string equality here is the exact check with no
 * JSON comparison in the way. The Rust suite proves the core re-folds correctly;
 * this proves the binding carries the same bytes out.
 */
class StreamingTest {
    private static final int TICKS = 4;
    private static final int EVENTS = 8;

    /** The feed as a JSON array, then escaped into the `dataset` string field. */
    private static String config() {
        StringBuilder events = new StringBuilder("[");
        for (int i = 0; i < EVENTS; i++) {
            if (i > 0) {
                events.append(',');
            }
            events.append("{\"type\":\"trade\",\"symbol\":{\"base\":\"BTC\",\"quote\":\"USDT\"},")
                    .append("\"price\":\"").append(100 + i).append("\",\"quantity\":\"1\",")
                    .append("\"aggressor\":\"Buy\",\"timestamp\":").append(i + 1).append('}');
        }
        events.append(']');
        String feed = events.toString().replace("\\", "\\\\").replace("\"", "\\\"");
        return "{\"sources\":[{\"Replay\":{\"dataset\":\"" + feed + "\"}}],"
                + "\"layout\":{\"panels\":[{\"kind\":\"Chart\","
                + "\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}";
    }

    private static Terminal subscribed() {
        Terminal term = new Terminal(config());
        term.command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
        return term;
    }

    @Test
    void streamingAndBatchRefoldAgree() {
        String frame = "";
        try (Terminal streamed = subscribed()) {
            for (int i = 0; i < TICKS; i++) {
                frame = streamed.command("{\"type\":\"Tick\"}");
            }
        }

        // A second terminal runs the feed out, then re-folds the same prefix in
        // one batch. Running past the point first is what makes this a rewind
        // rather than a replay of state it still had.
        String refolded;
        try (Terminal rewound = subscribed()) {
            for (int i = 0; i < EVENTS; i++) {
                rewound.command("{\"type\":\"Tick\"}");
            }
            refolded = rewound.command(
                    "{\"type\":\"Seek\",\"source\":0,\"index\":" + TICKS + "}");
        }

        assertEquals(frame, refolded);
    }

    @Test
    void theComparedFrameIsNotEmpty() {
        // A guard on the guard: two empty frames are also byte-identical, and an
        // equality test that passes on nothing proves nothing.
        try (Terminal term = subscribed()) {
            String raw = "";
            for (int i = 0; i < TICKS; i++) {
                raw = term.command("{\"type\":\"Tick\"}");
            }
            assertTrue(raw.contains("\"last\":" + (100 + TICKS - 1)), raw);
        }
    }
}
