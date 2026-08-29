/* Throughput benchmark for the wickra-terminal C++ API.
 *
 * What this measures is the boundary, not the core -- and, next to the C row,
 * the price of the ownership layer. The C++ wrapper calls the same five
 * functions the C benchmark calls; the difference between the two numbers is
 * what RAII costs: one std::string copy of the returned frame, and a guard
 * whose destructor frees the original. If that difference is large, the wrapper
 * is doing something it should not.
 *
 * Built by bindings/cpp/benchmarks/CMakeLists.txt:
 *
 *     cargo build -p wickra-terminal-c --release
 *     cmake -S bindings/cpp/benchmarks -B bindings/cpp/benchmarks/build
 *     cmake --build bindings/cpp/benchmarks/build --config Release
 *     ./bindings/cpp/benchmarks/build/throughput [ticks]
 */
#include <algorithm>
#include <array>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <string>

#include "wickra_terminal.hpp"

using wickra::terminal::Terminal;

namespace {

/* Shared by all nine binding benchmarks, so the numbers compare. */
const char *const CONFIG =
    R"({"sources":[{"Synth":{"seed":1}}],)"
    R"("layout":{"panels":[)"
    R"({"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":40}},)"
    R"({"kind":"Book","rect":{"x":0,"y":40,"w":50,"h":30}},)"
    R"({"kind":"Tape","rect":{"x":50,"y":40,"w":50,"h":30}}]}})";
const char *const SUBSCRIBE = R"({"type":"Subscribe","source":0,"symbol":"BTC/USDT"})";
const char *const TICK = R"({"type":"Tick"})";
const char *const LIST = R"({"type":"ListIndicators"})";

/* The catalogue response is ~30 kB, so a hundred of them is a noisy sample. */
constexpr long CATALOGUE_REPS = 1000;

double median_ns(Terminal &term, const std::string &command, long count) {
    const auto drive = [&] {
        for (long i = 0; i < count; i++) {
            (void)term.command(command);
        }
    };
    drive();
    std::array<double, 3> samples{};
    for (auto &sample : samples) {
        const auto start = std::chrono::steady_clock::now();
        drive();
        sample = static_cast<double>(
            std::chrono::duration_cast<std::chrono::nanoseconds>(
                std::chrono::steady_clock::now() - start)
                .count());
    }
    std::sort(samples.begin(), samples.end());
    return samples[1];
}

}  // namespace

int main(int argc, char **argv) {
    long ticks = argc > 1 ? std::strtol(argv[1], nullptr, 10) : 20000;
    if (ticks < 100) {
        ticks = 20000;
    }

    Terminal term(CONFIG);
    (void)term.command(SUBSCRIBE);
    const std::size_t frame_bytes = term.command(TICK).size();
    const std::size_t catalogue_bytes = term.command(LIST).size();

    const double tick_ns = median_ns(term, TICK, ticks);
    const double list_ns = median_ns(term, LIST, CATALOGUE_REPS);

    std::printf("wickra-terminal C++ throughput - %ld commands (median of 3)\n\n", ticks);
    std::printf("%-18s%14s%14s%12s\n", "Command", "per second", "us/command", "payload");
    std::printf("----------------------------------------------------------\n");
    std::printf("%-18s%14.0f%14.2f%11zuB\n", "Tick", static_cast<double>(ticks) / (tick_ns / 1e9),
                tick_ns / static_cast<double>(ticks) / 1e3, frame_bytes);
    std::printf("%-18s%14.0f%14.2f%11zuB\n", "ListIndicators", static_cast<double>(CATALOGUE_REPS) / (list_ns / 1e9),
                list_ns / static_cast<double>(CATALOGUE_REPS) / 1e3, catalogue_bytes);
    std::printf("\nOne command crosses the boundary once. Higher is better, and the numbers\n"
                "are machine-dependent -- compare bindings on one machine, never across two.\n");
    return 0;
}
