/*
 * Throughput benchmark for the wickra-terminal Java binding.
 *
 * What this measures is the boundary, not the core. Every binding drives the
 * same Rust terminal through one function -- a command JSON in, a frame JSON
 * out -- so the number is the cost of crossing this boundary once per command.
 * The Java row includes an Arena allocation per call and two string copies
 * across FFM, plus whatever the JIT has decided by the time it is timed, which
 * is why the loop runs a warmup before anything is measured.
 *
 * A single source file rather than a Maven module, deliberately: it is not
 * published, it is not tested, and a second pom for one class would be more
 * scaffolding than benchmark. Compile it against the built binding.
 *
 *   cargo build -p wickra-terminal-c --release
 *   mvn -B -f bindings/java compile
 *   javac -d bindings/java/target/bench -cp bindings/java/target/classes \
 *       bindings/java/benchmarks/Throughput.java
 *   java --enable-native-access=ALL-UNNAMED \
 *       -cp "bindings/java/target/classes;bindings/java/target/bench" \
 *       -Dnative.lib.dir=target/release \
 *       Throughput 20000
 *
 * (Use ':' instead of ';' on the classpath outside Windows, and the matching
 * library name -- libwickra_terminal.so or .dylib.)
 */

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Locale;

import org.wickra.terminal.Terminal;

public final class Throughput {

    /* Shared by all nine binding benchmarks, so the numbers compare. */
    private static final String CONFIG =
            "{\"sources\":[{\"Synth\":{\"seed\":1}}],"
            + "\"layout\":{\"panels\":["
            + "{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":40}},"
            + "{\"kind\":\"Book\",\"rect\":{\"x\":0,\"y\":40,\"w\":50,\"h\":30}},"
            + "{\"kind\":\"Tape\",\"rect\":{\"x\":50,\"y\":40,\"w\":50,\"h\":30}}]}}";
    private static final String SUBSCRIBE =
            "{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}";
    private static final String TICK = "{\"type\":\"Tick\"}";
    private static final String LIST = "{\"type\":\"ListIndicators\"}";

    /* The catalogue response is ~30 kB, so a hundred is a noisy sample. */
    private static final int CATALOGUE_REPS = 1000;

    private Throughput() {
    }

    /* Median of three timed runs, after one warmup. */
    private static double medianNs(Terminal term, String command, int count) {
        drive(term, command, count);
        double[] samples = new double[3];
        for (int i = 0; i < samples.length; i++) {
            long start = System.nanoTime();
            drive(term, command, count);
            samples[i] = System.nanoTime() - start;
        }
        Arrays.sort(samples);
        return samples[1];
    }

    private static void drive(Terminal term, String command, int count) {
        for (int i = 0; i < count; i++) {
            term.command(command);
        }
    }

    private static int bytes(String text) {
        return text.getBytes(StandardCharsets.UTF_8).length;
    }

    public static void main(String[] args) {
        int ticks = 20_000;
        if (args.length > 0) {
            try {
                int parsed = Integer.parseInt(args[0]);
                if (parsed >= 100) {
                    ticks = parsed;
                }
            } catch (NumberFormatException ignored) {
                /* keep the default */
            }
        }

        try (Terminal term = new Terminal(CONFIG)) {
            term.command(SUBSCRIBE);
            int frameBytes = bytes(term.command(TICK));
            int catalogueBytes = bytes(term.command(LIST));

            double tickNs = medianNs(term, TICK, ticks);
            double listNs = medianNs(term, LIST, CATALOGUE_REPS);

            System.out.printf(Locale.ROOT, "wickra-terminal Java throughput - %,d commands (median of 3)%n%n", ticks);
            System.out.printf(Locale.ROOT, "%-18s%14s%14s%12s%n", "Command", "per second", "us/command", "payload");
            System.out.println("-".repeat(58));
            System.out.printf(Locale.ROOT, "%-18s%,14.0f%14.2f%,11dB%n", "Tick",
                    ticks / (tickNs / 1e9), tickNs / ticks / 1e3, frameBytes);
            System.out.printf(Locale.ROOT, "%-18s%,14.0f%14.2f%,11dB%n", "ListIndicators",
                    CATALOGUE_REPS / (listNs / 1e9), listNs / CATALOGUE_REPS / 1e3, catalogueBytes);
            System.out.println();
            System.out.println("One command crosses the boundary once. Higher is better, and the numbers");
            System.out.println("are machine-dependent -- compare bindings on one machine, never across two.");
        }
    }
}
