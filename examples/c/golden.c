/* Cross-language golden parity for the C ABI.
 *
 * The other eight language surfaces each assert their output against
 * `golden/expected/basic.min.json`. C and C++ did not, which left the two
 * languages every other binding routes through as the only ones not held to the
 * corpus — the C ABI is the hub, so a drift there would surface everywhere at
 * once and be checked nowhere.
 *
 * No JSON parser is needed: the ABI returns the core's compact output verbatim,
 * so byte equality against that one file is the whole check. This file is C, and
 * the C++ example links the same header, so passing here covers both.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "wickra_terminal.h"

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

/* Find the golden directory by walking up from the working directory, the way
 * every other binding's golden test does — ctest runs from the build tree. */
static char *golden_path(const char *leaf) {
    static char path[512];
    const char *prefixes[] = {"",         "../",          "../../",
                              "../../../", "../../../../", "../../../../../"};
    for (size_t i = 0; i < sizeof(prefixes) / sizeof(prefixes[0]); i++) {
        snprintf(path, sizeof(path), "%sgolden/%s", prefixes[i], leaf);
        FILE *probe = fopen(path, "rb");
        if (probe) {
            fclose(probe);
            return path;
        }
    }
    return NULL;
}

int main(void) {
    const char *config_path = golden_path("config.json");
    if (!config_path) {
        fprintf(stderr, "golden/config.json not found from the working directory\n");
        return 1;
    }
    char *config = read_file(config_path);
    if (!config) {
        fprintf(stderr, "could not read %s\n", config_path);
        return 1;
    }

    const char *expected_path = golden_path("expected/basic.min.json");
    if (!expected_path) {
        fprintf(stderr, "golden/expected/basic.min.json not found\n");
        free(config);
        return 1;
    }
    char *expected = read_file(expected_path);
    if (!expected) {
        fprintf(stderr, "could not read %s\n", expected_path);
        free(config);
        return 1;
    }
    trim_end(expected);

    WickraTerminal *term = wickra_terminal_new(config);
    free(config);
    if (!term) {
        fprintf(stderr, "failed to build a terminal from golden/config.json\n");
        free(expected);
        return 1;
    }

    char *out = NULL;
    if (wickra_terminal_command(
            term, "{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}", &out) !=
        WICKRA_TERMINAL_OK) {
        fprintf(stderr, "subscribe failed: %s\n", out ? out : "");
        wickra_terminal_free_string(out);
        wickra_terminal_free(term);
        free(expected);
        return 1;
    }
    wickra_terminal_free_string(out);

    /* Thirty-two ticks: the replay drains well before that, and the frame is
     * stable afterwards, which is what the Rust golden test pins. */
    char *frame = NULL;
    for (int i = 0; i < 32; i++) {
        wickra_terminal_free_string(frame);
        frame = NULL;
        if (wickra_terminal_command(term, "{\"type\":\"Tick\"}", &frame) !=
            WICKRA_TERMINAL_OK) {
            fprintf(stderr, "tick %d failed: %s\n", i, frame ? frame : "");
            wickra_terminal_free_string(frame);
            wickra_terminal_free(term);
            free(expected);
            return 1;
        }
    }

    trim_end(frame);
    int same = strcmp(frame, expected) == 0;
    if (!same) {
        fprintf(stderr, "golden mismatch\n  expected: %s\n  got:      %s\n", expected, frame);
    } else {
        printf("C golden parity: frame matches golden/expected/basic.min.json\n");
    }

    wickra_terminal_free_string(frame);
    wickra_terminal_free(term);
    free(expected);
    return same ? 0 : 1;
}
