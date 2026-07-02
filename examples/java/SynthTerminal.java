// A runnable Java example: drive a synthetic feed and print a frame.
//
//   cargo build -p wickra-terminal-c
//   mvn -f bindings/java/pom.xml -q package -DskipTests
//   javac -cp bindings/java/target/classes examples/java/SynthTerminal.java -d examples/java/out
//   java --enable-native-access=ALL-UNNAMED \
//        -Dnative.lib.dir=target/debug \
//        -cp "bindings/java/target/classes;examples/java/out" SynthTerminal
import org.wickra.terminal.Terminal;

public final class SynthTerminal {
    private static final String CONFIG =
            "{\"sources\":[{\"Synth\":{\"seed\":1}}],"
                    + "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}";

    public static void main(String[] args) {
        try (Terminal term = new Terminal(CONFIG)) {
            term.command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
            String raw = "";
            for (int i = 0; i < 20; i++) {
                raw = term.command("{\"type\":\"Tick\"}");
            }
            System.out.println("wickra-terminal " + Terminal.version());
            System.out.println(raw);
        }
    }
}
