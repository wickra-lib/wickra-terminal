/* Streaming a feed and re-folding it in one batch reach the same frame.
 *
 * The terminal reaches a state two ways. Streaming folds one event per tick as
 * it arrives; `Seek` throws the state away and re-folds the whole prefix in a
 * single batch. ARCHITECTURE.md calls that re-fold the moat -- it is what makes
 * a rewind deterministic and what lets the browser run the time-machine with no
 * engine behind it -- so the two must land on byte-identical frames.
 *
 * Byte-identical, not merely equal: the ABI returns the core's compact output
 * verbatim, so strcmp here is the exact check and no JSON parser is needed. The
 * Rust suite proves the core re-folds correctly; this proves the hub every other
 * binding routes through carries the same bytes out.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "wickra_terminal.h"

#define TICKS 4
#define EVENTS 8
#define CONFIG_CAP 4096

/* The feed as a JSON array, escaped into the `dataset` string field.
 *
 * Written by hand rather than with a serialiser for the same reason the golden
 * example splits the manifest on quotes: this file deliberately links nothing
 * but the C ABI header.
 */
static void build_config(char *out, size_t cap) {
    char feed[2048];
    size_t at = 0;
    at += (size_t)snprintf(feed + at, sizeof feed - at, "[");
    for (int i = 0; i < EVENTS; i++) {
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

/* Apply a command, returning the frame the caller must free, or NULL. */
static char *apply(WickraTerminal *term, const char *command) {
    char *out = NULL;
    if (wickra_terminal_command(term, command, &out) != 0) {
        fprintf(stderr, "command failed: %s\n  %s\n", command, out ? out : "(no message)");
        wickra_terminal_free_string(out);
        return NULL;
    }
    return out;
}

static WickraTerminal *subscribed(const char *config) {
    WickraTerminal *term = wickra_terminal_new(config);
    if (!term) {
        return NULL;
    }
    char *out = apply(term, "{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
    if (!out) {
        wickra_terminal_free(term);
        return NULL;
    }
    wickra_terminal_free_string(out);
    return term;
}

int main(void) {
    char config[CONFIG_CAP];
    build_config(config, sizeof config);

    WickraTerminal *streamed = subscribed(config);
    WickraTerminal *rewound = subscribed(config);
    if (!streamed || !rewound) {
        fprintf(stderr, "could not build a replay terminal\n");
        wickra_terminal_free(streamed);
        wickra_terminal_free(rewound);
        return 1;
    }

    char *frame = NULL;
    for (int i = 0; i < TICKS; i++) {
        wickra_terminal_free_string(frame);
        frame = apply(streamed, "{\"type\":\"Tick\"}");
        if (!frame) {
            break;
        }
    }

    /* The second terminal runs the feed out, then re-folds the same prefix in
     * one batch. Running past the point first is what makes this a rewind
     * rather than a replay of state it still had. */
    char *drained = NULL;
    for (int i = 0; i < EVENTS && frame; i++) {
        wickra_terminal_free_string(drained);
        drained = apply(rewound, "{\"type\":\"Tick\"}");
    }
    wickra_terminal_free_string(drained);

    char seek[64];
    snprintf(seek, sizeof seek, "{\"type\":\"Seek\",\"source\":0,\"index\":%d}", TICKS);
    char *refolded = frame ? apply(rewound, seek) : NULL;

    int failed = 1;
    if (frame && refolded) {
        if (strcmp(frame, refolded) != 0) {
            fprintf(stderr, "streaming and re-fold disagree:\n stream: %s\n refold: %s\n",
                    frame, refolded);
        } else {
            /* A guard on the guard: two empty frames are also byte-identical,
             * and an equality test that passes on nothing proves nothing. */
            char expected[32];
            snprintf(expected, sizeof expected, "\"last\":%d", 100 + TICKS - 1);
            if (!strstr(frame, expected)) {
                fprintf(stderr, "no %s in the compared frame: %s\n", expected, frame);
            } else {
                printf("streaming and batch re-fold agree, %d ticks\n", TICKS);
                failed = 0;
            }
        }
    }

    wickra_terminal_free_string(frame);
    wickra_terminal_free_string(refolded);
    wickra_terminal_free(streamed);
    wickra_terminal_free(rewound);
    return failed;
}
