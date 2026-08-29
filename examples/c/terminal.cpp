// A C++ example, over the shipped RAII header.
//
// It used to declare a Terminal class of its own here, which was the honest
// thing to do while the C++ surface was "the C header, plus write your own
// ownership". It is not any more: `wickra_terminal.hpp` ships that wrapper, so
// an example that redefines it teaches a reader to reimplement what they were
// handed. This drives the shipped API instead.
#include <cstdio>
#include <string>

#include "wickra_terminal.hpp"

using wickra::terminal::Terminal;

int main() {
    Terminal term(
        R"({"sources":[{"Synth":{"seed":1}}],)"
        R"("layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}})");

    // Every command answers with a frame; this one is a subscription and the
    // frame it returns says nothing new, so it is discarded on purpose. The
    // cast is what says "on purpose" -- command() is [[nodiscard]], so a
    // dropped frame is a warning unless it is written down.
    (void)term.command(R"({"type":"Subscribe","source":0,"symbol":"BTC/USDT"})");
    std::string frame;
    for (int i = 0; i < 20; i++) {
        frame = term.command(R"({"type":"Tick"})");
    }

    std::printf("wickra-terminal %s\n", wickra::terminal::version().c_str());
    std::printf("frame: %s\n", frame.c_str());
    return 0;
}
