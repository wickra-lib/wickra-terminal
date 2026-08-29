/* Cross-language golden parity for the C ABI, driven by golden/manifest.json.
 *
 * The C ABI is the hub every other binding routes through, so a drift here
 * surfaces everywhere at once. It used to check one scenario while the eight
 * other language suites checked all of them, which made the hub the least
 * covered surface rather than the most.
 *
 * The manifest is walked by splitting on the quote character rather than with a
 * JSON parser, exactly as the Java and R suites do and for the same reason: this
 * example deliberately links nothing but the C ABI header. Splitting on quotes
 * is enough and needs no regular expressions, because every value in the
 * manifest is a plain path and none of them contains a quote — the command
 * sequences live in files of their own precisely so that stays true.
 *
 * No JSON parser is needed for the comparison either: the ABI returns the core's
 * compact output verbatim, so byte equality against the expected file is the
 * whole check. This file is C, and the C++ example links the same header, so
 * passing here covers both.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "wickra_terminal.h"

#define MAX_SCENARIOS 32
#define PATH_MAX_LEN 512

typedef struct {
    char name[64];
    char config[160];
    char commands[160];
    char expected[160];
} Scenario;

/* Read a whole file into a NUL-terminated buffer the caller frees. */
static char *read_file(const char *path) {
    FILE *file = fopen(path, "rb");
    if (!file) {
        return NULL;
    }
    if (fseek(file, 0, SEEK_END) != 0) {
        fclose(file);
        return NULL;
    }
    long size = ftell(file);
    if (size < 0) {
        fclose(file);
        return NULL;
    }
    rewind(file);

    char *buffer = malloc((size_t)size + 1);
    if (!buffer) {
        fclose(file);
        return NULL;
    }
    size_t read = fread(buffer, 1, (size_t)size, file);
    fclose(file);
    buffer[read] = '\0';
    return buffer;
}

/* Trim trailing whitespace in place; the expected file has no trailing newline
 * but an editor may add one. */
static void trim_end(char *text) {
    size_t len = strlen(text);
    while (len > 0 && (text[len - 1] == '\n' || text[len - 1] == '\r' ||
                       text[len - 1] == ' ' || text[len - 1] == '\t')) {
        text[--len] = '\0';
    }
}

/* The prefix that reaches the golden directory from the working directory.
 *
 * ctest runs from the build tree, and how deep that is depends on the
 * generator, so this probes upwards the way every other binding's golden test
 * does. */
static const char *golden_prefix(void) {
    static const char *prefixes[] = {"",          "../",          "../../",
                                     "../../../", "../../../../", "../../../../../"};
    static char probe[PATH_MAX_LEN];
    for (size_t i = 0; i < sizeof(prefixes) / sizeof(prefixes[0]); i++) {
        snprintf(probe, sizeof(probe), "%sgolden/manifest.json", prefixes[i]);
        FILE *file = fopen(probe, "rb");
        if (file) {
            fclose(file);
            return prefixes[i];
        }
    }
    return NULL;
}

/* Walk the manifest by splitting on the quote character.
 *
 * Quoted tokens alternate between keys and values. A token that matches one of
 * the known keys selects what the next one means; a scenario is complete when
 * its name has been read, which is why the generator writes name last. Mutates
 * `raw` in place, terminating each token where its closing quote was.
 *
 * Returns how many scenarios the manifest HOLDS, storing the first `max` of
 * them. The caller compares the two: this used to stop counting at `max` and
 * drop the rest without a word, so a manifest that outgrew this file would have
 * left the hub -- the one binding every other routes through -- quietly checking
 * a prefix of the corpus while reporting a pass. Truncating is the failure this
 * file exists to make impossible. */
static size_t parse_manifest(char *raw, Scenario *out, size_t max) {
    static const char *keys[] = {"scenarios", "commands", "config", "expected", "name"};
    Scenario current;
    memset(&current, 0, sizeof current);
    const char *key = NULL;
    size_t count = 0;

    for (char *cursor = raw; *cursor;) {
        if (*cursor != '"') {
            cursor++;
            continue;
        }
        char *token = ++cursor;
        while (*cursor && *cursor != '"') {
            cursor++;
        }
        if (!*cursor) {
            break;
        }
        *cursor++ = '\0';

        const char *matched = NULL;
        for (size_t k = 0; k < sizeof(keys) / sizeof(keys[0]); k++) {
            if (strcmp(token, keys[k]) == 0) {
                matched = keys[k];
                break;
            }
        }
        if (matched) {
            key = matched;
            continue;
        }
        if (!key) {
            continue;
        }
        if (strcmp(key, "config") == 0) {
            snprintf(current.config, sizeof current.config, "%s", token);
        } else if (strcmp(key, "commands") == 0) {
            snprintf(current.commands, sizeof current.commands, "%s", token);
        } else if (strcmp(key, "expected") == 0) {
            snprintf(current.expected, sizeof current.expected, "%s", token);
        } else if (strcmp(key, "name") == 0) {
            snprintf(current.name, sizeof current.name, "%s", token);
            if (count < max) {
                out[count] = current;
            }
            count++;
            memset(&current, 0, sizeof current);
            key = NULL;
        }
    }
    return count;
}

/* Drive one scenario. Returns 0 on parity, 1 otherwise. */
static int run_scenario(const char *prefix, const Scenario *scenario) {
    char path[PATH_MAX_LEN];
    int failed = 1;
    char *config = NULL;
    char *expected = NULL;
    char *commands = NULL;
    WickraTerminal *term = NULL;
    char *frame = NULL;

    snprintf(path, sizeof path, "%sgolden/%s", prefix, scenario->config);
    config = read_file(path);
    if (!config) {
        fprintf(stderr, "%s: could not read %s\n", scenario->name, path);
        goto done;
    }
    snprintf(path, sizeof path, "%sgolden/%s", prefix, scenario->expected);
    expected = read_file(path);
    if (!expected) {
        fprintf(stderr, "%s: could not read %s\n", scenario->name, path);
        goto done;
    }
    trim_end(expected);
    snprintf(path, sizeof path, "%sgolden/%s", prefix, scenario->commands);
    commands = read_file(path);
    if (!commands) {
        fprintf(stderr, "%s: could not read %s\n", scenario->name, path);
        goto done;
    }

    term = wickra_terminal_new(config);
    if (!term) {
        fprintf(stderr, "%s: failed to build a terminal from its config\n", scenario->name);
        goto done;
    }

    /* One command per line; the last frame returned is the one to compare. */
    int replayed = 0;
    for (char *line = commands; line && *line;) {
        char *end = strchr(line, '\n');
        if (end) {
            *end = '\0';
        }
        trim_end(line);
        if (*line) {
            wickra_terminal_free_string(frame);
            frame = NULL;
            if (wickra_terminal_command(term, line, &frame) != WICKRA_TERMINAL_OK) {
                fprintf(stderr, "%s: command rejected: %s\n", scenario->name,
                        frame ? frame : line);
                goto done;
            }
            replayed++;
        }
        line = end ? end + 1 : NULL;
    }
    if (replayed == 0) {
        fprintf(stderr, "%s: the command file is empty\n", scenario->name);
        goto done;
    }

    trim_end(frame);
    if (strcmp(frame, expected) != 0) {
        fprintf(stderr, "%s: golden mismatch\n  expected: %s\n  got:      %s\n", scenario->name,
                expected, frame);
        goto done;
    }
    printf("  %-18s %d commands, frame matches %s\n", scenario->name, replayed,
           scenario->expected);
    failed = 0;

done:
    wickra_terminal_free_string(frame);
    if (term) {
        wickra_terminal_free(term);
    }
    free(commands);
    free(expected);
    free(config);
    return failed;
}

int main(void) {
    const char *prefix = golden_prefix();
    if (!prefix) {
        fprintf(stderr, "golden/manifest.json not found from the working directory\n");
        return 1;
    }

    char path[PATH_MAX_LEN];
    snprintf(path, sizeof path, "%sgolden/manifest.json", prefix);
    char *raw = read_file(path);
    if (!raw) {
        fprintf(stderr, "could not read %s\n", path);
        return 1;
    }

    Scenario scenarios[MAX_SCENARIOS];
    size_t count = parse_manifest(raw, scenarios, MAX_SCENARIOS);
    free(raw);

    if (count > MAX_SCENARIOS) {
        fprintf(stderr,
                "the manifest holds %zu scenarios and this example has room for %d; raise "
                "MAX_SCENARIOS rather than checking a prefix\n",
                count, MAX_SCENARIOS);
        return 1;
    }

    /* A manifest that silently shrank would leave this passing while checking a
     * fraction of what it used to, so the count is floored the way the other
     * suites floor theirs. */
    if (count < 12) {
        fprintf(stderr, "only %zu scenarios parsed from the manifest\n", count);
        return 1;
    }

    printf("C golden parity across %zu scenarios:\n", count);
    int failures = 0;
    for (size_t i = 0; i < count; i++) {
        failures += run_scenario(prefix, &scenarios[i]);
    }
    if (failures) {
        fprintf(stderr, "%d of %zu scenarios failed\n", failures, count);
        return 1;
    }
    return 0;
}
