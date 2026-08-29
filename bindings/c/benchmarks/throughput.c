/* Throughput benchmark for the wickra-terminal C ABI.
 *
 * What this measures is the boundary, not the core. Every binding drives the
 * same Rust terminal through one function -- a command JSON in, a frame JSON
 * out -- so the number is the cost of crossing this boundary once per command.
 * C is the floor the others are measured against: there is no marshalling here
 * beyond passing two pointers, so whatever this costs is the command itself
 * plus one allocation for the returned string.
 *
 * The Rust core runs the identical loop with no boundary at all, as
 * examples/rust --bin throughput. That is the floor; this is the cheapest a
 * boundary gets.
 *
 * Built by bindings/c/benchmarks/CMakeLists.txt:
 *
 *     cargo build -p wickra-terminal-c --release
 *     cmake -S bindings/c/benchmarks -B bindings/c/benchmarks/build
 *     cmake --build bindings/c/benchmarks/build --config Release
 *     ./bindings/c/benchmarks/build/throughput [ticks]
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "wickra_terminal.h"

/* Shared by all nine binding benchmarks, so the numbers compare. */
static const char *const CONFIG =
    "{\"sources\":[{\"Synth\":{\"seed\":1}}],"
    "\"layout\":{\"panels\":["
    "{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":40}},"
    "{\"kind\":\"Book\",\"rect\":{\"x\":0,\"y\":40,\"w\":50,\"h\":30}},"
    "{\"kind\":\"Tape\",\"rect\":{\"x\":50,\"y\":40,\"w\":50,\"h\":30}}]}}";
static const char *const SUBSCRIBE =
    "{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}";
static const char *const TICK = "{\"type\":\"Tick\"}";
static const char *const LIST = "{\"type\":\"ListIndicators\"}";

/* The catalogue response is ~30 kB, so a hundred of them is a noisy sample. */
#define CATALOGUE_REPS 1000

static double now_ns(void) {
    struct timespec ts;
    timespec_get(&ts, TIME_UTC);
    return (double)ts.tv_sec * 1e9 + (double)ts.tv_nsec;
}

/* Drive `count` commands, freeing each frame. Returns 0 on success. */
static int drive(WickraTerminal *term, const char *command, long count) {
    for (long i = 0; i < count; i++) {
        char *out = NULL;
        const int status = wickra_terminal_command(term, command, &out);
        if (status != WICKRA_TERMINAL_OK) {
            fprintf(stderr, "command failed: %s\n", out ? out : command);
            wickra_terminal_free_string(out);
            return 1;
        }
        wickra_terminal_free_string(out);
    }
    return 0;
}

/* Median of three timed runs, after one warmup. */
static double median_ns(WickraTerminal *term, const char *command, long count) {
    double samples[3];
    if (drive(term, command, count) != 0) {
        return -1.0;
    }
    for (int rep = 0; rep < 3; rep++) {
        const double start = now_ns();
        if (drive(term, command, count) != 0) {
            return -1.0;
        }
        samples[rep] = now_ns() - start;
    }
    for (int i = 0; i < 2; i++) {
        for (int j = i + 1; j < 3; j++) {
            if (samples[j] < samples[i]) {
                const double swap = samples[i];
                samples[i] = samples[j];
                samples[j] = swap;
            }
        }
    }
    return samples[1];
}

/* The byte length of one response, for the payload column. */
static size_t payload(WickraTerminal *term, const char *command) {
    char *out = NULL;
    if (wickra_terminal_command(term, command, &out) != WICKRA_TERMINAL_OK) {
        wickra_terminal_free_string(out);
        return 0;
    }
    const size_t length = out ? strlen(out) : 0;
    wickra_terminal_free_string(out);
    return length;
}

int main(int argc, char **argv) {
    long ticks = argc > 1 ? strtol(argv[1], NULL, 10) : 20000;
    if (ticks < 100) {
        ticks = 20000;
    }

    WickraTerminal *term = wickra_terminal_new(CONFIG);
    if (!term) {
        fprintf(stderr, "could not build a terminal from the benchmark config\n");
        return 1;
    }
    if (drive(term, SUBSCRIBE, 1) != 0) {
        wickra_terminal_free(term);
        return 1;
    }

    const size_t frame_bytes = payload(term, TICK);
    const size_t catalogue_bytes = payload(term, LIST);
    const double tick_ns = median_ns(term, TICK, ticks);
    const double list_ns = median_ns(term, LIST, CATALOGUE_REPS);
    wickra_terminal_free(term);

    if (tick_ns < 0.0 || list_ns < 0.0) {
        return 1;
    }

    printf("wickra-terminal C throughput - %ld commands (median of 3)\n\n", ticks);
    printf("%-18s%14s%14s%12s\n", "Command", "per second", "us/command", "payload");
    printf("----------------------------------------------------------\n");
    printf("%-18s%14.0f%14.2f%11zuB\n", "Tick", (double)ticks / (tick_ns / 1e9),
           tick_ns / (double)ticks / 1e3, frame_bytes);
    printf("%-18s%14.0f%14.2f%11zuB\n", "ListIndicators", (double)CATALOGUE_REPS / (list_ns / 1e9),
           list_ns / (double)CATALOGUE_REPS / 1e3, catalogue_bytes);
    printf("\nOne command crosses the boundary once. Higher is better, and the numbers\n"
           "are machine-dependent -- compare bindings on one machine, never across two.\n");
    return 0;
}
