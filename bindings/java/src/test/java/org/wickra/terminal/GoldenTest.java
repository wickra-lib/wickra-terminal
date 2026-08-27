package org.wickra.terminal;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.DynamicTest;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestFactory;

/**
 * Cross-language golden parity, driven by {@code golden/manifest.json}.
 *
 * <p>Each scenario names a config and a command sequence; replaying it must produce the frame in
 * its expected file, byte for byte. Because the binding returns the core's compact {@code
 * command_json} string verbatim, byte equality against that one file is the exact parity check.
 *
 * <p>Reading the manifest rather than naming one scenario is what makes the corpus extensible: a
 * scenario added in the Rust suite is picked up here, and in the seven other language suites, with
 * no change to any of them.
 *
 * <p>The manifest is read without a JSON library, because the binding deliberately has no runtime
 * dependencies and adding one for a test would be the tail wagging the dog. See {@link #manifest()}
 * for how.
 */
class GoldenTest {

    private record Scenario(String name, String config, String expected, String commands) {}

    private static Path goldenDir() {
        Path dir = Paths.get("").toAbsolutePath();
        for (int i = 0; i < 8; i++) {
            Path candidate = dir.resolve("golden");
            if (Files.isRegularFile(candidate.resolve("manifest.json"))) {
                return candidate;
            }
            dir = dir.getParent();
            if (dir == null) {
                break;
            }
        }
        throw new IllegalStateException("golden/ not found");
    }

    private static String read(Path path) {
        try {
            return new String(Files.readAllBytes(path), StandardCharsets.UTF_8);
        } catch (IOException err) {
            throw new IllegalStateException("could not read " + path, err);
        }
    }

    /**
     * Read the manifest by splitting on the quote character.
     *
     * No JSON library, because the binding deliberately has no runtime dependencies and adding one
     * for a test would be the tail wagging the dog. Splitting on quotes is enough and needs no
     * regular expressions: every value in the manifest is a plain path, and none of them contains a
     * quote. The command sequences live in files of their own for exactly this reason.
     */
    private static List<Scenario> manifest() {
        String[] parts = read(goldenDir().resolve("manifest.json")).split("\"", -1);
        List<String> keys = List.of("scenarios", "commands", "config", "expected", "name");
        List<Scenario> scenarios = new ArrayList<>();
        Map<String, String> current = new HashMap<>();
        String key = null;
        // Splitting on the quote puts the quoted values at the odd indices.
        for (int i = 1; i < parts.length; i += 2) {
            String token = parts[i];
            if (keys.contains(token)) {
                key = token;
                continue;
            }
            if (key == null) {
                continue;
            }
            current.put(key, token);
            if (key.equals("name")) {
                scenarios.add(
                        new Scenario(
                                current.get("name"),
                                current.get("config"),
                                current.get("expected"),
                                current.get("commands")));
                current = new HashMap<>();
                key = null;
            }
        }
        if (scenarios.isEmpty()) {
            throw new IllegalStateException("no scenarios parsed from the manifest");
        }
        return scenarios;
    }

    @TestFactory
    List<DynamicTest> goldenParity() {
        Path golden = goldenDir();
        List<DynamicTest> tests = new ArrayList<>();
        for (Scenario scenario : manifest()) {
            tests.add(
                    DynamicTest.dynamicTest(
                            scenario.name(),
                            () -> {
                                String config = read(golden.resolve(scenario.config()));
                                String expected = read(golden.resolve(scenario.expected())).trim();
                                List<String> commands =
                                        read(golden.resolve(scenario.commands()))
                                                .lines()
                                                .filter(line -> !line.isBlank())
                                                .toList();
                                assertTrue(!commands.isEmpty(), scenario.name());
                                try (Terminal term = new Terminal(config)) {
                                    String frame = "";
                                    for (String command : commands) {
                                        frame = term.command(command);
                                    }
                                    assertEquals(expected, frame.trim(), scenario.name());
                                }
                            }));
        }
        return tests;
    }

    @Test
    void corpusCoversMoreThanOneScenario() {
        // A manifest that silently shrank to one entry would leave every parity test passing
        // while checking a fraction of what it used to.
        List<String> names = manifest().stream().map(Scenario::name).toList();
        assertTrue(names.size() >= 7, "only " + names.size() + " scenarios");
        for (String expected : List.of("basic", "book_deltas", "footprint", "indicators", "seek")) {
            assertTrue(names.contains(expected), expected + " missing from the manifest");
        }
    }
}
