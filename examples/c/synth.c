/* A minimal C example: drive a synthetic feed and print a frame. */
#include <stdio.h>

#include "wickra_terminal.h"

static const char *CONFIG =
    "{\"sources\":[{\"Synth\":{\"seed\":1}}],"
    "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}";

int main(void) {
    WickraTerminal *term = wickra_terminal_new(CONFIG);
    if (!term) {
        fprintf(stderr, "failed to create terminal\n");
        return 1;
    }

    char *out = NULL;
    wickra_terminal_command(
        term, "{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}", &out);
    wickra_terminal_free_string(out);

    for (int i = 0; i < 20; i++) {
        int code = wickra_terminal_command(term, "{\"type\":\"Tick\"}", &out);
        if (code != WICKRA_TERMINAL_OK) {
            fprintf(stderr, "tick failed: %s\n", out ? out : "");
            wickra_terminal_free_string(out);
            wickra_terminal_free(term);
            return 1;
        }
        if (i == 19) {
            printf("wickra-terminal %s\n", wickra_terminal_version());
            printf("frame: %s\n", out);
        }
        wickra_terminal_free_string(out);
    }

    wickra_terminal_free(term);
    return 0;
}
