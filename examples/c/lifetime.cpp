/* Lifetime tests for the C++ RAII header.
 *
 * The header exists to make three mistakes impossible, so these check that it
 * does rather than that the happy path works -- the other examples cover that.
 *
 *   - A returned string is freed however the scope is left, including the path
 *     that throws.
 *   - A handle has one owner. Copying is deleted at compile time, which is
 *     checked here with a static_assert rather than a runtime case; moving
 *     leaves the source empty, so the moved-from destructor frees nothing.
 *   - A failure is an exception carrying the ABI's own message, not a code a
 *     caller can drop.
 *
 * No test framework: this links the C ABI and nothing else, exactly as the C
 * example does, so the whole file is asserts and a counter. Running it under a
 * leak checker is worthwhile and is not what this file is for -- it is for the
 * ownership rules holding at all.
 */
#include <cstdio>
#include <string>
#include <type_traits>
#include <utility>

#include "wickra_terminal.hpp"

using wickra::terminal::Error;
using wickra::terminal::Terminal;

namespace {

const char *const CONFIG =
    R"({"sources":[{"Synth":{"seed":7}}],)"
    R"("layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":80,"h":24}}]}})";

int failures = 0;

void check(bool condition, const char *what) {
    if (condition) {
        std::printf("  ok    %s\n", what);
    } else {
        std::printf("  FAIL  %s\n", what);
        failures++;
    }
}

/* The type-level guarantees. A copy would give two owners of one handle and the
 * second destructor would free it again; the compiler refuses instead. */
void copying_is_impossible() {
    static_assert(!std::is_copy_constructible<Terminal>::value,
                  "Terminal must not be copy-constructible: two owners double-free");
    static_assert(!std::is_copy_assignable<Terminal>::value,
                  "Terminal must not be copy-assignable: two owners double-free");
    static_assert(std::is_move_constructible<Terminal>::value,
                  "Terminal must be move-constructible to be usable in a container");
    static_assert(std::is_nothrow_move_constructible<Terminal>::value,
                  "moving must not throw, or a container cannot move on reallocation");
    check(true, "copying is rejected at compile time, moving is not");
}

void a_moved_from_terminal_is_empty() {
    Terminal first(CONFIG);
    check(first.valid(), "a constructed terminal is valid");

    Terminal second(std::move(first));
    check(second.valid(), "the move target owns the handle");
    check(!first.valid(), "the moved-from source is empty, so its destructor frees nothing");

    /* Both destructors run at the end of this scope. The empty one must be a
     * no-op; if it were not, this is where the double free would happen. */
}

void move_assignment_frees_the_target_it_replaces() {
    Terminal first(CONFIG);
    Terminal second(CONFIG);
    second = std::move(first);
    check(second.valid() && !first.valid(),
          "move assignment takes the handle and leaves the source empty");

    /* Self-assignment must not free the handle it is about to keep. */
    Terminal &alias = second;
    second = std::move(alias);
    check(second.valid(), "self-move leaves the terminal usable");
}

void a_command_on_a_moved_from_terminal_throws() {
    Terminal first(CONFIG);
    Terminal second(std::move(first));
    bool threw = false;
    try {
        (void)first.command(R"({"type":"Tick"})");
    } catch (const Error &err) {
        threw = err.code() == WICKRA_TERMINAL_ERR_NULL;
    }
    check(threw, "a command on a moved-from terminal throws rather than passing a null handle");
}

void an_invalid_config_throws() {
    bool threw = false;
    try {
        Terminal term("{ not json");
        (void)term;
    } catch (const Error &err) {
        threw = std::string(err.what()).find("config") != std::string::npos;
    }
    check(threw, "an invalid config throws an Error naming the config");
}

void a_failed_command_carries_the_abi_message() {
    Terminal term(CONFIG);
    bool threw = false;
    std::string message;
    try {
        (void)term.command(R"({"type":"NoSuchCommand"})");
    } catch (const Error &err) {
        threw = true;
        message = err.what();
    }
    check(threw, "an unknown command throws");
    check(!message.empty() && message != "the command failed",
          "the exception carries the ABI's own message, not a placeholder");
}

/* The throwing path is the one that leaks when strings are freed by hand: the
 * ABI has already allocated the error message when the throw happens. Running
 * it many times would show a leak as growth; what is asserted here is only that
 * it stays correct, which is what this file can prove without a leak checker. */
void the_throwing_path_stays_correct_when_repeated() {
    Terminal term(CONFIG);
    int thrown = 0;
    for (int i = 0; i < 500; i++) {
        try {
            (void)term.command(R"({"type":"NoSuchCommand"})");
        } catch (const Error &) {
            thrown++;
        }
    }
    check(thrown == 500, "500 failed commands each threw, and the terminal is still usable");
    const std::string frame = term.command(R"({"type":"Tick"})");
    check(!frame.empty(), "a good command still works after 500 failures");
}

void version_is_reported() {
    check(!wickra::terminal::version().empty(), "version() returns a non-empty string");
}

}  // namespace

int main() {
    std::printf("C++ RAII header, lifetime rules:\n");
    copying_is_impossible();
    a_moved_from_terminal_is_empty();
    move_assignment_frees_the_target_it_replaces();
    a_command_on_a_moved_from_terminal_throws();
    an_invalid_config_throws();
    a_failed_command_carries_the_abi_message();
    the_throwing_path_stays_correct_when_repeated();
    version_is_reported();

    if (failures != 0) {
        std::printf("%d check(s) failed\n", failures);
        return 1;
    }
    std::printf("all checks passed\n");
    return 0;
}
