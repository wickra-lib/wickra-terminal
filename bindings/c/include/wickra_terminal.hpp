/* wickra-terminal C++ API — a header-only RAII wrapper over the C ABI.
 *
 * The C ABI is the hub every binding routes through, and C++ can call it
 * directly: `wickra_terminal.h` is already `extern "C"`-guarded. What it cannot
 * do directly is own anything. Every handle and every returned string has to be
 * freed by hand, on every path including the ones an exception takes, and the
 * shape of that mistake is a leak nobody notices or a double free nobody can
 * reproduce.
 *
 * So this file adds ownership and nothing else. It declares no indicator logic,
 * holds no state of its own, and compiles to the same calls a careful C++
 * author would have written -- it just makes the careless version impossible:
 *
 *   - `Terminal` owns the handle and frees it in its destructor.
 *   - Copying is deleted; two owners of one handle would double-free it.
 *   - Moving is allowed and leaves the source empty, so the destructor of a
 *     moved-from object is a no-op.
 *   - The string `wickra_terminal_command` writes is owned by a guard that
 *     frees it however the scope is left, exception included.
 *   - A failure is an exception carrying the message the ABI produced, rather
 *     than a return code a caller can ignore.
 *
 * Header-only and dependency-free beyond the standard library, so consuming it
 * is one include and one link against the same library the C examples use.
 *
 * Requires C++17.
 *
 *   #include "wickra_terminal.hpp"
 *
 *   wickra::terminal::Terminal term(R"({"symbols":["BTCUSDT"]})");
 *   const std::string frame = term.command(R"({"kind":"frame"})");
 */

#ifndef WICKRA_TERMINAL_HPP
#define WICKRA_TERMINAL_HPP

#pragma once

#include <stdexcept>
#include <string>
#include <utility>

#include "wickra_terminal.h"

namespace wickra::terminal {

/* A failed call, carrying the ABI's status code and its message.
 *
 * `wickra_terminal_command` writes its error text into the same out-pointer it
 * would have written a frame to, so the message is whatever the core said went
 * wrong rather than a category this wrapper invented. */
class Error : public std::runtime_error {
public:
    Error(int code, const std::string &message)
        : std::runtime_error(message), code_(code) {}

    /* WICKRA_TERMINAL_ERR or WICKRA_TERMINAL_ERR_NULL. */
    [[nodiscard]] int code() const noexcept { return code_; }

private:
    int code_;
};

namespace detail {

/* Owns a string the ABI allocated, and frees it however the scope is left.
 *
 * The command path has two exits that both hold an allocation -- a frame on
 * success, an error message on failure -- and the failure exit throws. Freeing
 * by hand would have to happen on both, and the throwing one is the one that
 * gets forgotten. */
class OwnedString {
public:
    OwnedString() noexcept = default;
    ~OwnedString() { wickra_terminal_free_string(text_); }

    OwnedString(const OwnedString &) = delete;
    OwnedString &operator=(const OwnedString &) = delete;
    OwnedString(OwnedString &&) = delete;
    OwnedString &operator=(OwnedString &&) = delete;

    /* Where the ABI writes the pointer it allocated. */
    char **out() noexcept { return &text_; }

    [[nodiscard]] bool empty() const noexcept { return text_ == nullptr; }

    /* A copy the caller owns; the original stays with this guard. */
    [[nodiscard]] std::string copy() const {
        return text_ == nullptr ? std::string() : std::string(text_);
    }

private:
    char *text_ = nullptr;
};

}  // namespace detail

/* A terminal, owning its handle.
 *
 * Move-only: one handle has one owner, and a moved-from Terminal is empty
 * rather than sharing. Calling `command` on an empty one throws instead of
 * reaching the ABI with a null handle. */
class Terminal {
public:
    /* Build from a JSON config.
     *
     * Throws `Error` if the config is not valid. The ABI reports that as a null
     * handle with no message of its own, so the message here names the cause it
     * can name. */
    explicit Terminal(const std::string &config_json)
        : handle_(wickra_terminal_new(config_json.c_str())) {
        if (handle_ == nullptr) {
            throw Error(WICKRA_TERMINAL_ERR,
                        "wickra_terminal_new rejected the config: it is not valid UTF-8, "
                        "not valid JSON, or not a valid terminal config");
        }
    }

    ~Terminal() { wickra_terminal_free(handle_); }

    Terminal(const Terminal &) = delete;
    Terminal &operator=(const Terminal &) = delete;

    Terminal(Terminal &&other) noexcept
        : handle_(std::exchange(other.handle_, nullptr)) {}

    Terminal &operator=(Terminal &&other) noexcept {
        if (this != &other) {
            wickra_terminal_free(handle_);
            handle_ = std::exchange(other.handle_, nullptr);
        }
        return *this;
    }

    /* Apply a command and return the frame JSON it produced.
     *
     * Throws `Error` carrying the ABI's own message if the command fails. */
    [[nodiscard]] std::string command(const std::string &cmd_json) {
        if (handle_ == nullptr) {
            throw Error(WICKRA_TERMINAL_ERR_NULL,
                        "command called on a moved-from Terminal");
        }
        detail::OwnedString out;
        const int status = wickra_terminal_command(handle_, cmd_json.c_str(), out.out());
        if (status != WICKRA_TERMINAL_OK) {
            throw Error(status, out.empty() ? std::string("the command failed") : out.copy());
        }
        return out.copy();
    }

    /* False after this Terminal has been moved from. */
    [[nodiscard]] bool valid() const noexcept { return handle_ != nullptr; }

private:
    WickraTerminal *handle_;
};

/* The library version. Static storage in the ABI, so this only copies it. */
[[nodiscard]] inline std::string version() {
    const char *text = wickra_terminal_version();
    return text == nullptr ? std::string() : std::string(text);
}

}  // namespace wickra::terminal

#endif /* WICKRA_TERMINAL_HPP */
