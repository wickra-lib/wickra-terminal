// A runnable Java example: rewind a recorded feed and watch state re-fold.
//
// The time-machine is what makes a recording more than a slow synthetic feed:
// Seek throws the folded state away and rebuilds it from the recording, so a
// rewind is deterministic rather than approximate. Nothing here is Java-specific
// -- it is four JSON commands, and every binding drives the same four.
//
//   cargo build -p wickra-terminal-c
//   mvn -f bindings/java/pom.xml -q package -DskipTests
//   javac -cp bindings/java/target/classes examples/java/TimeMachine.java -d examples/java/out
//   java --enable-native-access=ALL-UNNAMED \
//        -Dnative.lib.dir=target/debug \
//        -cp "bindings/java/target/classes:examples/java/out" TimeMachine
//
//   The classpath separator is `:` on Linux and macOS and `;` on Windows.
import org.wickra.terminal.Terminal;

public final class TimeMachine {
    private static final int TRADES = 6;

    /** The feed as a JSON array, then escaped into the `dataset` string field. */
    private static String config() {
        StringBuilder events = new StringBuilder("[");
        for (int i = 0; i < TRADES; i++) {
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

    /**
     * The chart panel's {@code last}, read without a JSON parser.
     *
     * The binding returns the core's JSON verbatim, and this example
     * deliberately depends on nothing but the binding — the same reason the Java
     * golden test walks the manifest by splitting on quotes.
     */
    private static String lastPrice(String frame) {
        int at = frame.indexOf("\"last\":");
        if (at < 0) {
            return "(none)";
        }
        int end = at + 7;
        while (end < frame.length() && "-0123456789.eE".indexOf(frame.charAt(end)) >= 0) {
            end++;
        }
        return frame.substring(at + 7, end);
    }

    public static void main(String[] args) {
        try (Terminal term = new Terminal(config())) {
            term.command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");

            String raw = "";
            for (int i = 0; i < TRADES; i++) {
                raw = term.command("{\"type\":\"Tick\"}");
            }
            System.out.println("played to the end:   last = " + lastPrice(raw));

            System.out.println("position:            "
                    + term.command("{\"type\":\"ReplayPosition\",\"source\":0}"));

            // Rewind to just after the second trade. The state is rebuilt from
            // the recording rather than restored from a snapshot, which is why a
            // rewind lands on exactly the frame the forward pass had there.
            raw = term.command("{\"type\":\"Seek\",\"source\":0,\"index\":2}");
            System.out.println("rewound to index 2:  last = " + lastPrice(raw));

            // And forward again from there, over the same events.
            raw = term.command("{\"type\":\"Tick\"}");
            System.out.println("one tick later:      last = " + lastPrice(raw));
        }
    }
}
