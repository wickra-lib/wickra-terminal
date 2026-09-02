/* The recorder round-trips: what it exports is what `Replay` takes.
 *
 * Four commands sit on the boundary, are documented in all nine binding
 * READMEs, and were driven by almost no binding: `SetRecording` and
 * `ExportRecording` by none at all, `ReplayPosition` only by the time-machine
 * example beside this one, `FeedDerivatives` by none. The README completeness
 * test proved the promise and nothing checked it was kept, so the recorder had
 * never been executed outside Rust.
 *
 * The round trip is the check, not the shape of any one answer: arm the
 * recorder, feed a market, export what it kept, and build a second terminal
 * from exactly those bytes. A hub that mangled the export would be caught by
 * the replay refusing it, which no assertion about a string would find.
 *
 *   cargo build --release -p wickra-terminal-c
 *   cmake -S examples/c -B examples/c/build
 *   cmake --build examples/c/build --config Release
 *   ./examples/c/build/recorder_test
 */
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "wickra_terminal.h"

#define TRADES 4
#define BUF_CAP 8192

/* Append to a buffer, refusing to walk past its end.
 *
 * snprintf returns the length it *would* have written, not what it did, so
 * accumulating that return value walks the offset past the buffer on
 * truncation -- and `cap - *at` then underflows to an enormous size_t, handing
 * the next call a length far larger than the space that is left. Returning
 * failure instead is the whole of the fix.
 */
static int append(char *buf, size_t cap, size_t *at, const char *fmt, ...) {
    if (*at >= cap) {
        return -1;
    }
    va_list args;
    va_start(args, fmt);
    const int written = vsnprintf(buf + *at, cap - *at, fmt, args);
    va_end(args);
    if (written < 0 || (size_t)written >= cap - *at) {
        return -1;
    }
    *at += (size_t)written;
    return 0;
}

/* Apply a command and hand back the answer, which the caller must free. */
static char *apply(WickraTerminal *term, const char *command) {
    char *out = NULL;
    if (wickra_terminal_command(term, command, &out) != 0) {
        fprintf(stderr, "command failed: %s\n  %s\n", command, out ? out : "(no message)");
        wickra_terminal_free_string(out);
        return NULL;
    }
    return out;
}

/* A manual source with a derivatives indicator, so a fed update is observable
 * in the frame rather than merely accepted. */
static const char *RECORDER_CONFIG =
    "{\"sources\":[\"Manual\"],"
    "\"indicators\":[{\"kind\":\"FundingRate\",\"params\":[]}],"
    "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}";

/* Feed one trade and drain it with a tick. */
static int drive(WickraTerminal *term, int price, int timestamp) {
    char command[512];
    size_t at = 0;
    if (append(command, sizeof command, &at,
               "{\"type\":\"Feed\",\"source\":0,\"event\":{\"type\":\"trade\","
               "\"symbol\":{\"base\":\"BTC\",\"quote\":\"USDT\"},"
               "\"price\":\"%d\",\"quantity\":\"0.5\","
               "\"aggressor\":\"Buy\",\"timestamp\":%d}}",
               price, timestamp) != 0) {
        return -1;
    }
    char *out = apply(term, command);
    if (!out) {
        return -1;
    }
    wickra_terminal_free_string(out);
    out = apply(term, "{\"type\":\"Tick\"}");
    if (!out) {
        return -1;
    }
    wickra_terminal_free_string(out);
    return 0;
}

/* A recording as a `Replay` dataset: the whole array becomes one JSON string
 * field, so every quote inside it has to be escaped. Written by hand rather
 * than with a serialiser, because this example links nothing but the header. */
static int build_replay_config(char *out, size_t cap, const char *recording) {
    size_t at = 0;
    if (append(out, cap, &at, "{\"sources\":[{\"Replay\":{\"dataset\":\"") != 0) {
        return -1;
    }
    for (const char *p = recording; *p; p++) {
        const int written = (*p == '"' || *p == '\\')
                                ? append(out, cap, &at, "\\%c", *p)
                                : append(out, cap, &at, "%c", *p);
        if (written != 0) {
            return -1;
        }
    }
    return append(out, cap, &at,
                  "\"}}],\"indicators\":[],"
                  "\"layout\":{\"panels\":[{\"kind\":\"Chart\","
                  "\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}");
}

int main(void) {
    WickraTerminal *term = wickra_terminal_new(RECORDER_CONFIG);
    if (!term) {
        fprintf(stderr, "could not build a manual terminal\n");
        return 1;
    }

    int status = 1;
    char *out = NULL;
    char *recording = NULL;
    WickraTerminal *replay = NULL;
    char *config = malloc(BUF_CAP);
    if (!config) {
        fprintf(stderr, "out of memory\n");
        goto done;
    }

    out = apply(term, "{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
    if (!out) {
        goto done;
    }
    wickra_terminal_free_string(out);

    /* Nothing is kept until the recorder is armed, and asking is not an error. */
    out = apply(term, "{\"type\":\"ExportRecording\"}");
    if (!out || strcmp(out, "[]") != 0) {
        fprintf(stderr, "an unarmed recorder answered %s\n", out ? out : "(nothing)");
        goto done;
    }
    wickra_terminal_free_string(out);

    /* A manual source is not a recording, and says so rather than erroring: a
     * renderer can ask about whatever is focused without first knowing what
     * kind of source it is. */
    out = apply(term, "{\"type\":\"ReplayPosition\",\"source\":0}");
    if (!out || strcmp(out, "{\"cursor\":0,\"length\":0}") != 0) {
        fprintf(stderr, "a manual source reported %s\n", out ? out : "(nothing)");
        goto done;
    }
    wickra_terminal_free_string(out);

    out = apply(term, "{\"type\":\"SetRecording\",\"capacity\":64}");
    if (!out) {
        goto done;
    }
    wickra_terminal_free_string(out);
    out = NULL;

    for (int i = 0; i < TRADES; i++) {
        if (drive(term, 100 + i, i + 1) != 0) {
            goto done;
        }
    }

    recording = apply(term, "{\"type\":\"ExportRecording\"}");
    if (!recording || strstr(recording, "\"price\":\"103\"") == NULL) {
        fprintf(stderr, "the recorder kept %s\n", recording ? recording : "(nothing)");
        goto done;
    }

    /* Straight back in as a dataset. This is the assertion the whole file is
     * for: the shape `Replay` takes is the shape `ExportRecording` answers
     * with, and a hub that changed the bytes would fail here. */
    if (build_replay_config(config, BUF_CAP, recording) != 0) {
        fprintf(stderr, "the replay config did not fit its buffer\n");
        goto done;
    }
    replay = wickra_terminal_new(config);
    if (!replay) {
        fprintf(stderr, "the exported recording is not a valid feed\n");
        goto done;
    }

    out = apply(replay, "{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
    if (!out) {
        goto done;
    }
    wickra_terminal_free_string(out);

    out = apply(replay, "{\"type\":\"ReplayPosition\",\"source\":0}");
    if (!out || strcmp(out, "{\"cursor\":0,\"length\":4}") != 0) {
        fprintf(stderr, "a fresh replay reported %s\n", out ? out : "(nothing)");
        goto done;
    }
    wickra_terminal_free_string(out);
    out = NULL;

    for (int i = 0; i < 3; i++) {
        char *frame = apply(replay, "{\"type\":\"Tick\"}");
        if (!frame) {
            goto done;
        }
        wickra_terminal_free_string(frame);
    }

    out = apply(replay, "{\"type\":\"ReplayPosition\",\"source\":0}");
    if (!out || strcmp(out, "{\"cursor\":3,\"length\":4}") != 0) {
        fprintf(stderr, "the cursor reported %s after three ticks\n", out ? out : "(nothing)");
        goto done;
    }
    wickra_terminal_free_string(out);

    /* Stopping the recorder clears what it held: both directions clear, so a
     * capacity change never leaves a recording that is part one size and part
     * another. */
    out = apply(term, "{\"type\":\"SetRecording\",\"capacity\":null}");
    if (!out) {
        goto done;
    }
    wickra_terminal_free_string(out);
    out = apply(term, "{\"type\":\"ExportRecording\"}");
    if (!out || strcmp(out, "[]") != 0) {
        fprintf(stderr, "stopping the recorder left %s\n", out ? out : "(nothing)");
        goto done;
    }
    wickra_terminal_free_string(out);
    out = NULL;

    /* And a derivatives update reaches an indicator. Accepting the command
     * proves nothing on its own: the update is folded into the market's
     * microstructure and reaches an indicator only on the next trade. All three
     * prices, or the tick is withheld -- a mark without an index and a futures
     * price is not a priced market. */
    out = apply(term,
                "{\"type\":\"FeedDerivatives\",\"source\":0,\"symbol\":\"BTC/USDT\","
                "\"update\":{\"funding_rate\":0.0001,\"mark_price\":102.0,"
                "\"index_price\":100.0,\"futures_price\":104.0,"
                "\"open_interest\":1000.0,\"timestamp\":9}}");
    if (!out) {
        goto done;
    }
    wickra_terminal_free_string(out);
    out = NULL;

    if (drive(term, 101, 5) != 0) {
        goto done;
    }
    out = apply(term, "{\"type\":\"Tick\"}");
    if (!out || strstr(out, "\"name\":\"FundingRate\",\"value\":0.0001") == NULL) {
        fprintf(stderr, "the funding rate did not reach the indicator:\n  %s\n",
                out ? out : "(nothing)");
        goto done;
    }

    printf("recorder round trip ok: exported %d events and replayed them\n", TRADES);
    status = 0;

done:
    wickra_terminal_free_string(out);
    wickra_terminal_free_string(recording);
    free(config);
    wickra_terminal_free(replay);
    wickra_terminal_free(term);
    return status;
}
