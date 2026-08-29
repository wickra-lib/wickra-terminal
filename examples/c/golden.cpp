/* Cross-language golden parity for the C++ API, driven by golden/manifest.json.
 *
 * golden.c already holds the C ABI to this corpus, and the C++ wrapper calls
 * the same five functions, so what this adds is narrow and worth having: proof
 * that the ownership layer does not change what comes back. `command` copies
 * the ABI's string into a std::string and frees the original; a wrapper that
 * got that wrong -- copied one byte short, freed before copying, returned the
 * error text on the success path -- would still compile, still run, and produce
 * frames that no longer match. Byte equality against the same expected files
 * catches exactly that.
 *
 * The manifest is walked by splitting on the quote character rather than with a
 * JSON parser, as the C, Java and R suites do and for the same reason: this
 * example links nothing but the C ABI. Every value in the manifest is a plain
 * path and none contains a quote -- the command sequences live in files of
 * their own precisely so that stays true.
 */
#include <cstdio>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

#include "wickra_terminal.hpp"

using wickra::terminal::Error;
using wickra::terminal::Terminal;

namespace {

struct Scenario {
    std::string name;
    std::string config;
    std::string commands;
    std::string expected;
};

/* The whole file, or an empty optional-by-convention (`ok` false). */
bool read_file(const std::string &path, std::string &out) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        return false;
    }
    std::ostringstream buffer;
    buffer << file.rdbuf();
    out = buffer.str();
    return true;
}

void trim_end(std::string &text) {
    const std::string ws = " \t\r\n";
    const std::string::size_type last = text.find_last_not_of(ws);
    text.erase(last == std::string::npos ? 0 : last + 1);
}

/* The prefix that reaches golden/ from the working directory.
 *
 * ctest runs from the build tree and how deep that is depends on the generator,
 * so this probes upwards the way every other binding's golden test does. */
bool golden_prefix(std::string &out) {
    static const char *const prefixes[] = {"",          "../",          "../../",
                                           "../../../", "../../../../", "../../../../../"};
    for (const char *prefix : prefixes) {
        std::string probe;
        if (read_file(std::string(prefix) + "golden/manifest.json", probe)) {
            out = prefix;
            return true;
        }
    }
    return false;
}

/* Every scenario the manifest holds.
 *
 * Quoted tokens alternate between keys and values; a token matching a known key
 * says what the next one means, and a scenario is complete when its name has
 * been read, which is why the generator writes name last. */
std::vector<Scenario> parse_manifest(const std::string &raw) {
    static const char *const keys[] = {"scenarios", "commands", "config", "expected", "name"};
    std::vector<Scenario> found;
    Scenario current;
    std::string key;

    for (std::string::size_type i = 0; i < raw.size();) {
        if (raw[i] != '"') {
            i++;
            continue;
        }
        const std::string::size_type start = ++i;
        while (i < raw.size() && raw[i] != '"') {
            i++;
        }
        if (i >= raw.size()) {
            break;
        }
        const std::string token = raw.substr(start, i - start);
        i++;

        bool is_key = false;
        for (const char *candidate : keys) {
            if (token == candidate) {
                key = candidate;
                is_key = true;
                break;
            }
        }
        if (is_key || key.empty()) {
            continue;
        }
        if (key == "config") {
            current.config = token;
        } else if (key == "commands") {
            current.commands = token;
        } else if (key == "expected") {
            current.expected = token;
        } else if (key == "name") {
            current.name = token;
            found.push_back(current);
            current = Scenario();
            key.clear();
        }
    }
    return found;
}

bool run_scenario(const std::string &prefix, const Scenario &scenario) {
    std::string config;
    std::string commands;
    std::string expected;
    for (const auto &pair : {std::make_pair(&scenario.config, &config),
                             std::make_pair(&scenario.commands, &commands),
                             std::make_pair(&scenario.expected, &expected)}) {
        if (!read_file(prefix + "golden/" + *pair.first, *pair.second)) {
            std::fprintf(stderr, "%s: could not read %s\n", scenario.name.c_str(),
                         pair.first->c_str());
            return false;
        }
    }
    trim_end(expected);

    try {
        Terminal term(config);
        std::string frame;
        int replayed = 0;
        std::istringstream lines(commands);
        std::string line;
        while (std::getline(lines, line)) {
            trim_end(line);
            if (line.empty()) {
                continue;
            }
            frame = term.command(line);
            replayed++;
        }
        if (replayed == 0) {
            std::fprintf(stderr, "%s: the command file is empty\n", scenario.name.c_str());
            return false;
        }
        trim_end(frame);
        if (frame != expected) {
            std::fprintf(stderr, "%s: golden mismatch\n  expected: %s\n  got:      %s\n",
                         scenario.name.c_str(), expected.c_str(), frame.c_str());
            return false;
        }
        std::printf("  %-18s %d commands, frame matches %s\n", scenario.name.c_str(), replayed,
                    scenario.expected.c_str());
        return true;
    } catch (const Error &err) {
        std::fprintf(stderr, "%s: %s\n", scenario.name.c_str(), err.what());
        return false;
    }
}

}  // namespace

int main() {
    std::string prefix;
    if (!golden_prefix(prefix)) {
        std::fprintf(stderr, "golden/manifest.json not found from the working directory\n");
        return 1;
    }
    std::string raw;
    if (!read_file(prefix + "golden/manifest.json", raw)) {
        std::fprintf(stderr, "could not read the manifest\n");
        return 1;
    }

    const std::vector<Scenario> scenarios = parse_manifest(raw);
    /* A manifest that silently shrank would leave this passing while checking a
     * fraction of what it used to, so the count is floored as the other suites
     * floor theirs. Nothing is truncated here -- a vector has no fixed room --
     * so there is no upper check to make. */
    if (scenarios.size() < 9) {
        std::fprintf(stderr, "only %zu scenarios parsed from the manifest\n", scenarios.size());
        return 1;
    }

    std::printf("C++ golden parity across %zu scenarios:\n", scenarios.size());
    int failures = 0;
    for (const Scenario &scenario : scenarios) {
        if (!run_scenario(prefix, scenario)) {
            failures++;
        }
    }
    if (failures != 0) {
        std::fprintf(stderr, "%d of %zu scenarios failed\n", failures, scenarios.size());
        return 1;
    }
    return 0;
}
