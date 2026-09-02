package org.wickra.terminal;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

/**
 * The recorder, the scrubber and the host feed, end to end through the binding.
 *
 * <p>Four commands sit on the boundary, are documented in all nine binding READMEs, and were driven
 * by almost no binding: {@code SetRecording} and {@code ExportRecording} by none at all, {@code
 * ReplayPosition} only by the C example, {@code FeedDerivatives} by none. The README completeness
 * test proved the promise and nothing checked it was kept, so the recorder had never been executed
 * outside Rust.
 *
 * <p>The round trip is the point: arm the recorder, drive the terminal, export what it kept, and
 * hand that straight back as a {@code Replay} dataset. A binding that mangled the export would be
 * caught by the replay refusing it, which no assertion about a string shape would find.
 *
 * <p>Read without a JSON library, like the rest of this suite: the binding deliberately has no
 * runtime dependencies, and adding one for a test would be the tail wagging the dog.
 */
class RecorderTest {

    /**
     * A derivatives indicator, so {@code FeedDerivatives} is observable in the frame rather than
     * merely accepted.
     */
    private static final String CONFIG =
            "{\"sources\":[\"Manual\"],"
                    + "\"indicators\":[{\"kind\":\"FundingRate\",\"params\":[]}],"
                    + "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}";

    private static final String SYMBOL = "BTC/USDT";

    private static Terminal subscribed() {
        Terminal term = new Terminal(CONFIG);
        term.command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"" + SYMBOL + "\"}");
        return term;
    }

    /**
     * A recording as a {@code Replay} dataset: the whole array becomes one JSON string field, so
     * every quote and backslash inside it has to be escaped.
     */
    private static String replayConfig(String dataset) {
        String escaped = dataset.replace("\\", "\\\\").replace("\"", "\\\"");
        return "{\"sources\":[{\"Replay\":{\"dataset\":\""
                + escaped
                + "\"}}],\"indicators\":[],"
                + "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}";
    }

    private static String drive(Terminal term, String price, int timestamp) {
        term.command(
                "{\"type\":\"Feed\",\"source\":0,\"event\":{\"type\":\"trade\","
                        + "\"symbol\":{\"base\":\"BTC\",\"quote\":\"USDT\"},\"price\":\""
                        + price
                        + "\",\"quantity\":\"0.5\",\"aggressor\":\"Buy\",\"timestamp\":"
                        + timestamp
                        + "}}");
        return term.command("{\"type\":\"Tick\"}");
    }

    @Test
    void recorderRoundTripsThroughAReplay() {
        try (Terminal term = subscribed()) {
            // Nothing is kept until the recorder is armed, and asking is not an error.
            assertEquals("[]", term.command("{\"type\":\"ExportRecording\"}"));

            term.command("{\"type\":\"SetRecording\",\"capacity\":64}");
            String[] prices = {"100", "101", "102", "103"};
            for (int i = 0; i < prices.length; i++) {
                drive(term, prices[i], i + 1);
            }

            String recording = term.command("{\"type\":\"ExportRecording\"}");
            assertTrue(recording.startsWith("[{"), recording);
            assertTrue(recording.contains("\"price\":\"100\""), recording);
            assertTrue(recording.contains("\"price\":\"103\""), recording);

            // Straight back in as a dataset: the shape Replay takes is the shape
            // ExportRecording answers with, which is what makes a session keepable.
            try (Terminal replay = new Terminal(replayConfig(recording))) {
                replay.command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"" + SYMBOL + "\"}");
                String raw = "";
                for (int i = 0; i < 4; i++) {
                    raw = replay.command("{\"type\":\"Tick\"}");
                }
                assertTrue(raw.contains("\"last\":103.0"), raw);
            }
        }
    }

    @Test
    void stoppingTheRecorderClearsWhatItHeld() {
        // Both directions clear, so a capacity change never leaves a recording that is part one
        // size and part another.
        try (Terminal term = subscribed()) {
            term.command("{\"type\":\"SetRecording\",\"capacity\":64}");
            drive(term, "100", 1);
            assertNotEquals("[]", term.command("{\"type\":\"ExportRecording\"}"));

            term.command("{\"type\":\"SetRecording\",\"capacity\":null}");
            assertEquals("[]", term.command("{\"type\":\"ExportRecording\"}"));
        }
    }

    @Test
    void replayPositionAnswersForASourceThatCannotBeReplayed() {
        // 0/0 rather than an error, so a renderer can ask about whatever is focused without first
        // knowing what kind of source it is.
        try (Terminal term = subscribed()) {
            assertEquals(
                    "{\"cursor\":0,\"length\":0}",
                    term.command("{\"type\":\"ReplayPosition\",\"source\":0}"));
        }
    }

    @Test
    void replayPositionTracksTheCursorThroughARecording() {
        try (Terminal term = subscribed()) {
            term.command("{\"type\":\"SetRecording\",\"capacity\":64}");
            String[] prices = {"100", "101", "102", "103"};
            for (int i = 0; i < prices.length; i++) {
                drive(term, prices[i], i + 1);
            }

            String recording = term.command("{\"type\":\"ExportRecording\"}");
            try (Terminal replay = new Terminal(replayConfig(recording))) {
                replay.command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"" + SYMBOL + "\"}");
                assertEquals(
                        "{\"cursor\":0,\"length\":4}",
                        replay.command("{\"type\":\"ReplayPosition\",\"source\":0}"));

                for (int i = 0; i < 3; i++) {
                    replay.command("{\"type\":\"Tick\"}");
                }
                assertEquals(
                        "{\"cursor\":3,\"length\":4}",
                        replay.command("{\"type\":\"ReplayPosition\",\"source\":0}"));
            }
        }
    }

    @Test
    void fedDerivativesReachADerivativesIndicator() {
        // Accepting the command proves nothing on its own: the update is folded into the market's
        // microstructure and reaches an indicator only on the next trade, so the reading is what
        // says it arrived.
        try (Terminal term = subscribed()) {
            String before = drive(term, "100", 1);
            assertTrue(before.contains("{\"name\":\"FundingRate\",\"value\":null}"), before);

            // All three prices, or the tick is withheld: a mark without an index and a futures
            // price is not a priced market.
            term.command(
                    "{\"type\":\"FeedDerivatives\",\"source\":0,\"symbol\":\""
                            + SYMBOL
                            + "\",\"update\":{\"funding_rate\":0.0001,\"mark_price\":102.0,"
                            + "\"index_price\":100.0,\"futures_price\":104.0,"
                            + "\"open_interest\":1000.0,\"timestamp\":9}}");

            String after = drive(term, "101", 2);
            assertTrue(after.contains("\"name\":\"FundingRate\",\"value\":0.0001"), after);
        }
    }

    @Test
    void feedingDerivativesToAnUntrackedMarketThrows() {
        try (Terminal term = new Terminal(CONFIG)) {
            assertThrows(
                    IllegalStateException.class,
                    () ->
                            term.command(
                                    "{\"type\":\"FeedDerivatives\",\"source\":0,\"symbol\":\""
                                            + SYMBOL
                                            + "\",\"update\":{\"funding_rate\":0.0001,\"timestamp\":1}}"));
        }
    }
}
