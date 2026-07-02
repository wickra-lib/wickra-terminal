/* R .Call glue for the wickra-terminal C ABI hub. */
#include <R.h>
#include <Rinternals.h>
#include <R_ext/Rdynload.h>
#include <stdio.h>
#include "wickra_terminal.h"

/* --- handle lifetime ----------------------------------------------------- */

static void wkterm_finalize(SEXP ext) {
    WickraTerminal *h = (WickraTerminal *)R_ExternalPtrAddr(ext);
    if (h) {
        wickra_terminal_free(h);
    }
    R_ClearExternalPtr(ext);
}

static WickraTerminal *handle_of(SEXP ext) {
    WickraTerminal *h = (WickraTerminal *)R_ExternalPtrAddr(ext);
    if (!h) {
        Rf_error("wickra-terminal: handle is closed");
    }
    return h;
}

/* --- exported .Call entries ---------------------------------------------- */

SEXP wkterm_version(void) {
    return Rf_mkString(wickra_terminal_version());
}

SEXP wkterm_new(SEXP config_json) {
    WickraTerminal *h = wickra_terminal_new(CHAR(STRING_ELT(config_json, 0)));
    if (!h) {
        Rf_error("wickra-terminal: invalid config");
    }
    SEXP ext = PROTECT(R_MakeExternalPtr(h, R_NilValue, R_NilValue));
    R_RegisterCFinalizerEx(ext, wkterm_finalize, TRUE);
    UNPROTECT(1);
    return ext;
}

SEXP wkterm_command(SEXP ext, SEXP cmd_json) {
    WickraTerminal *h = handle_of(ext);
    char *out = NULL;
    int code = wickra_terminal_command(h, CHAR(STRING_ELT(cmd_json, 0)), &out);

    if (code != WICKRA_TERMINAL_OK) {
        /* Copy the error message out before freeing, then raise. */
        char msg[512];
        snprintf(msg, sizeof(msg), "wickra-terminal: %s", out ? out : "command failed");
        if (out) {
            wickra_terminal_free_string(out);
        }
        Rf_error("%s", msg);
    }

    SEXP result = PROTECT(Rf_mkString(out ? out : ""));
    if (out) {
        wickra_terminal_free_string(out);
    }
    UNPROTECT(1);
    return result;
}

/* --- registration -------------------------------------------------------- */

static const R_CallMethodDef CallEntries[] = {
    {"wkterm_version", (DL_FUNC)&wkterm_version, 0},
    {"wkterm_new", (DL_FUNC)&wkterm_new, 1},
    {"wkterm_command", (DL_FUNC)&wkterm_command, 2},
    {NULL, NULL, 0}};

void R_init_wickraterminal(DllInfo *dll) {
    R_registerRoutines(dll, NULL, CallEntries, NULL, NULL);
    R_useDynamicSymbols(dll, FALSE);
}
