/* A runnable C example: rewind a recorded feed and watch state re-fold.
 *
 * The time-machine is what makes a recording more than a slow synthetic feed:
 * `Seek` throws the folded state away and rebuilds it from the recording, so a
 * rewind is deterministic rather than approximate. Nothing here is C-specific --
 * it is four JSON commands, and every binding drives the same four.
 *
 *   cargo build --release -p wickra-terminal-c
 *   cmake -S examples/c -B examples/c/build
 *   cmake --build examples/c/build --config Release
 *   ./examples/c/build/time_machine
 */
#include <stdio.h>
#include <string.h>

#include "wickra_terminal.h"

#define TRADES 6
#define CONFIG_CAP 4096

/* The recorded feed, escaped into the `dataset` string field. Written by hand
 * rather than with a serialiser: this example deliberately links nothing but
 * the C ABI header. */
static void build_config(char *out, size_t cap) {
    char feed[2048];
    size_t at = 0;
    at += (size_t)snprintf(feed + at, sizeof feed - at, "[");
    for (int i = 0; i < TRADES; i++) {
        at += (size_t)snprintf(feed + at, sizeof feed - at,
                               "%s{\\\"type\\\":\\\"trade\\\","
                               "\\\"symbol\\\":{\\\"base\\\":\\\"BTC\\\",\\\"quote\\\":\\\"USDT\\\"},"
                               "\\\"price\\\":\\\"%d\\\",\\\"quantity\\\":\\\"1\\\","
                               "\\\"aggressor\\\":\\\"Buy\\\",\\\"timestamp\\\":%d}",
                               i == 0 ? "" : ",", 100 + i, i + 1);
    }
    snprintf(feed + at, sizeof feed - at, "]");
    snprintf(out, cap,
             "{\"sources\":[{\"Replay\":{\"dataset\":\"%s\"}}],"
             "\"layout\":{\"panels\":[{\"kind\":\"Chart\","
             "\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}",
             feed);
}

/* The chart panel's `last`, read without a JSON parser: the ABI returns the
 * core's output verbatim and this example depends on nothing but the header. */
static void print_last(const char *label, const char *frame) {
    const char *at = strstr(frame, "\"last\":");
    if (!at) {
        printf("%s (no chart panel)\n", label);
        return;
    }
    at += 7;
    size_t span = strspn(at, "-0123456789.eE");
    printf("%s%.*s\n", label, (int)span, at);
}

/* Apply a command, printing and returning the frame the caller must free. */
static char *apply(WickraTerminal *term, const char *command) {
    char *out = NULL;
    if (wickra_terminal_command(term, command, &out) != 0) {
        fprintf(stderr, "command failed: %s\n  %s\n", command, out ? out : "(no message)");
        wickra_terminal_free_string(out);
        return NULL;
    }
    return out;
}

int main(void) {
    char config[CONFIG_CAP];
    build_config(config, sizeof config);

    WickraTerminal *term = wickra_terminal_new(config);
    if (!term) {
        fprintf(stderr, "could not build a replay terminal\n");
        return 1;
    }

    char *out = apply(term, "{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
    if (!out) {
        wickra_terminal_free(term);
        return 1;
    }
    wickra_terminal_free_string(out);

    char *frame = NULL;
    for (int i = 0; i < TRADES; i++) {
        wickra_terminal_free_string(frame);
        frame = apply(term, "{\"type\":\"Tick\"}");
        if (!frame) {
            wickra_terminal_free(term);
            return 1;
        }
    }
    print_last("played to the end:   last = ", frame);
    wickra_terminal_free_string(frame);

    char *where = apply(term, "{\"type\":\"ReplayPosition\",\"source\":0}");
    if (where) {
        printf("position:            %s\n", where);
        wickra_terminal_free_string(where);
    }

    /* Rewind to just after the second trade. The state is rebuilt from the
     * recording rather than restored from a snapshot, which is why a rewind
     * lands on exactly the frame the forward pass had at that point. */
    frame = apply(term, "{\"type\":\"Seek\",\"source\":0,\"index\":2}");
    if (frame) {
        print_last("rewound to index 2:  last = ", frame);
        wickra_terminal_free_string(frame);
    }

    /* And forward again from there, over the same events. */
    frame = apply(term, "{\"type\":\"Tick\"}");
    if (frame) {
        print_last("one tick later:      last = ", frame);
        wickra_terminal_free_string(frame);
    }

    wickra_terminal_free(term);
    return 0;
}
