// A C++ example: a small RAII wrapper over the wickra-terminal C ABI.
#include <cstdio>
#include <stdexcept>
#include <string>

#include "wickra_terminal.h"

// The cbindgen header wraps its declarations in `extern "C"` under __cplusplus,
// so it is usable directly from C++.
class Terminal {
public:
    explicit Terminal(const char *config) : handle_(wickra_terminal_new(config)) {
        if (!handle_) {
            throw std::runtime_error("wickra-terminal: invalid config");
        }
    }

    ~Terminal() { wickra_terminal_free(handle_); }

    Terminal(const Terminal &) = delete;
    Terminal &operator=(const Terminal &) = delete;

    std::string command(const char *cmd) {
        char *out = nullptr;
        int code = wickra_terminal_command(handle_, cmd, &out);
        std::string result = out ? out : "";
        if (out) {
            wickra_terminal_free_string(out);
        }
        if (code != WICKRA_TERMINAL_OK) {
            throw std::runtime_error("wickra-terminal: " + result);
        }
        return result;
    }

private:
    WickraTerminal *handle_;
};

int main() {
    Terminal term(
        "{\"sources\":[{\"Synth\":{\"seed\":1}}],"
        "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}");

    term.command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
    std::string frame;
    for (int i = 0; i < 20; i++) {
        frame = term.command("{\"type\":\"Tick\"}");
    }

    std::printf("wickra-terminal %s\n", wickra_terminal_version());
    std::printf("frame: %s\n", frame.c_str());
    return 0;
}
