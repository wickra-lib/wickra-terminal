/* Streaming a feed and re-folding it in one batch reach the same frame, through
 * the C++ wrapper.
 *
 * The terminal reaches a state two ways. Streaming folds one event per tick as
 * it arrives; `Seek` throws the state away and re-folds the whole prefix in a
 * single batch. ARCHITECTURE.md calls that re-fold the moat, so the two must
 * land on byte-identical frames.
 *
 * The C example next to this one proves the ABI. What this adds is narrow and
 * worth proving on its own: that the ownership layer -- an OwnedString per call,
 * freed by the ABI's own deallocator -- returns exactly the bytes the ABI wrote,
 * for two frames produced by different paths and compared against each other
 * rather than against a fixture.
 */
#include <cstdio>
#include <string>

#include "wickra_terminal.hpp"

namespace {

constexpr int kTicks = 4;
constexpr int kEvents = 8;

/* The feed as a JSON array, escaped into the `dataset` string field. Built with
 * string concatenation rather than a serialiser: this example deliberately
 * links nothing but the header. */
std::string config() {
    std::string feed = "[";
    for (int i = 0; i < kEvents; ++i) {
        if (i > 0) {
            feed += ',';
        }
        feed += R"({\"type\":\"trade\",\"symbol\":{\"base\":\"BTC\",\"quote\":\"USDT\"},)";
        feed += R"(\"price\":\")" + std::to_string(100 + i) + R"(\",\"quantity\":\"1\",)";
        feed += R"(\"aggressor\":\"Buy\",\"timestamp\":)" + std::to_string(i + 1) + "}";
    }
    feed += "]";
    return R"({"sources":[{"Replay":{"dataset":")" + feed +
           R"("}}],"layout":{"panels":[{"kind":"Chart",)"
           R"("rect":{"x":0,"y":0,"w":100,"h":100}}]}})";
}

wickra::terminal::Terminal subscribed() {
    wickra::terminal::Terminal term(config());
    (void)term.command(R"({"type":"Subscribe","source":0,"symbol":"BTC/USDT"})");
    return term;
}

}  // namespace

int main() {
    try {
        wickra::terminal::Terminal streamed = subscribed();
        std::string frame;
        for (int i = 0; i < kTicks; ++i) {
            frame = streamed.command(R"({"type":"Tick"})");
        }

        /* The second terminal runs the feed out, then re-folds the same prefix
         * in one batch. Running past the point first is what makes this a
         * rewind rather than a replay of state it still had. */
        wickra::terminal::Terminal rewound = subscribed();
        for (int i = 0; i < kEvents; ++i) {
            (void)rewound.command(R"({"type":"Tick"})");
        }
        const std::string refolded = rewound.command(
            R"({"type":"Seek","source":0,"index":)" + std::to_string(kTicks) + "}");

        if (frame != refolded) {
            std::fprintf(stderr, "streaming and re-fold disagree:\n stream: %s\n refold: %s\n",
                         frame.c_str(), refolded.c_str());
            return 1;
        }

        /* A guard on the guard: two empty frames are also byte-identical, and an
         * equality test that passes on nothing proves nothing. */
        const std::string expected = "\"last\":" + std::to_string(100 + kTicks - 1);
        if (frame.find(expected) == std::string::npos) {
            std::fprintf(stderr, "no %s in the compared frame: %s\n",
                         expected.c_str(), frame.c_str());
            return 1;
        }

        std::printf("streaming and batch re-fold agree through the C++ wrapper, %d ticks\n",
                    kTicks);
        return 0;
    } catch (const wickra::terminal::Error &err) {
        std::fprintf(stderr, "terminal error: %s\n", err.what());
        return 1;
    }
}
