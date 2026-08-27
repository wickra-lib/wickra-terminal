package org.wickra.terminal;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.junit.jupiter.api.Test;

/**
 * The indicator registry is reachable from the JVM.
 *
 * <p>The registry lives in the Rust core and this binding passes JSON through the C ABI, so nothing
 * here needed new binding code. That is exactly why it is worth a test: "no code changed" is also
 * what a broken pass-through looks like.
 *
 * <p>The frame is inspected with regular expressions rather than a JSON parser, because the binding
 * deliberately has no runtime dependencies and adding one for a test would be the tail wagging the
 * dog. The assertions are about which names and keys appear, which is what a substring match can
 * answer honestly.
 */
class RegistryTest {
    // A non-default indicator, so finding it proves the config reached the registry rather than the
    // built-in overlay happening to look right.
    private static final String CONFIG =
            "{\"sources\":[{\"Synth\":{\"seed\":1}}],"
                    + "\"indicators\":[{\"kind\":\"Rsi\",\"params\":[14]}]}";

    private static final String TICK = "{\"type\":\"Tick\"}";

    private static Terminal subscribed() {
        Terminal term = new Terminal(CONFIG);
        term.command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
        return term;
    }

    /** Every indicator name in the frame, in order. */
    private static List<String> indicatorNames(String frame) {
        Matcher matcher = Pattern.compile("\"name\":\"([^\"]+)\"").matcher(frame);
        List<String> names = new ArrayList<>();
        while (matcher.find()) {
            names.add(matcher.group(1));
        }
        return names;
    }

    @Test
    void configuredIndicatorReachesTheChart() {
        try (Terminal term = subscribed()) {
            String frame = "";
            for (int i = 0; i < 30; i++) {
                frame = term.command(TICK);
            }
            assertEquals(List.of("Rsi(14)"), indicatorNames(frame));
        }
    }

    @Test
    void indicatorsCanBeAddedAndRemovedAtRunTime() {
        try (Terminal term = subscribed()) {
            term.command("{\"type\":\"AddIndicator\",\"spec\":{\"kind\":\"Atr\",\"params\":[14]}}");
            assertTrue(indicatorNames(term.command(TICK)).contains("Atr(14)"));
            term.command("{\"type\":\"RemoveIndicator\",\"label\":\"Rsi(14)\"}");
            assertEquals(List.of("Atr(14)"), indicatorNames(term.command(TICK)));
        }
    }

    @Test
    void catalogueListsTheWholeRegistry() {
        try (Terminal term = subscribed()) {
            String catalogue = term.command("{\"type\":\"ListIndicators\"}");
            // One "kind" key per row; the catalogue is the only response shaped this way.
            int rows = 0;
            Matcher matcher = Pattern.compile("\"kind\":").matcher(catalogue);
            while (matcher.find()) {
                rows++;
            }
            assertTrue(rows >= 421, "only " + rows + " entries in the catalogue");
            assertTrue(catalogue.contains("\"kind\":\"Sma\""));
            assertTrue(catalogue.contains("\"kind\":\"MacdIndicator\""));
        }
    }

    @Test
    void unknownIndicatorIsRejectedWithItsName() {
        try (Terminal term = subscribed()) {
            RuntimeException err =
                    assertThrows(
                            RuntimeException.class,
                            () ->
                                    term.command(
                                            "{\"type\":\"AddIndicator\",\"spec\":{\"kind\":\"NotReal\"}}"));
            assertTrue(
                    err.getMessage().contains("NotReal"),
                    "error does not name the indicator: " + err.getMessage());
        }
    }

    @Test
    void multiOutputIndicatorReportsNamedFields() {
        try (Terminal term =
                new Terminal(
                        "{\"sources\":[{\"Synth\":{\"seed\":1}}],"
                                + "\"indicators\":[{\"kind\":\"MacdIndicator\",\"params\":[12,26,9]}]}")) {
            term.command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
            String frame = "";
            for (int i = 0; i < 200; i++) {
                frame = term.command(TICK);
            }
            assertTrue(frame.contains("MacdIndicator(12,26,9)"), frame);
            // A single-output indicator omits the key entirely, so its presence here is what
            // says the multi-output path is wired.
            assertTrue(frame.contains("\"fields\":["), "no named fields in the frame: " + frame);
            assertNotEquals(-1, frame.indexOf("\"fields\":[{\"name\":"));
        }
    }
}
