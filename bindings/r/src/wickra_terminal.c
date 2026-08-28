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
    /* R_ExternalPtrAddr on anything that is not an external pointer reads a
     * field that is not there. .Call passes whatever the caller wrote, so the
     * type is checked rather than assumed. */
    if (TYPEOF(ext) != EXTPTRSXP) {
        Rf_error("wickra-terminal: not a terminal handle");
    }
    WickraTerminal *h = (WickraTerminal *)R_ExternalPtrAddr(ext);
    if (!h) {
        Rf_error("wickra-terminal: handle is closed");
    }
    return h;
}

/* The single element of a character argument, as a C string.
 *
 * STRING_ELT(x, 0) on a zero-length vector indexes past its data, and CHAR() on
 * what comes back dereferences whatever was there: `wkterm_new(character(0))`
 * crashed the R session rather than raising. .Call does no checking of its own,
 * so every argument this shim dereferences is checked here. */
static const char *scalar_string(SEXP value, const char *what) {
    if (TYPEOF(value) != STRSXP) {
        Rf_error("wickra-terminal: %s must be a character vector", what);
    }
    if (Rf_xlength(value) != 1) {
        Rf_error("wickra-terminal: %s must be a single string, not %lld",
                 what, (long long)Rf_xlength(value));
    }
    SEXP element = STRING_ELT(value, 0);
    if (element == NA_STRING) {
        Rf_error("wickra-terminal: %s must not be NA", what);
    }
    return CHAR(element);
}

/* --- exported .Call entries ---------------------------------------------- */

SEXP wkterm_version(void) {
    return Rf_mkString(wickra_terminal_version());
}

SEXP wkterm_new(SEXP config_json) {
    WickraTerminal *h = wickra_terminal_new(scalar_string(config_json, "config"));
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
    const char *cmd = scalar_string(cmd_json, "command");
    int code = wickra_terminal_command(h, cmd, &out);

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
