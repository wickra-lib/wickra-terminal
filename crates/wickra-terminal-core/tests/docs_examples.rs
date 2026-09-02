//! The configs and commands in the documentation are the ones the code accepts.
//!
//! Every broken example this repository has shipped was broken the same way: a
//! snippet written once, correct at the time or not, and never executed again.
//! The README's Python example omitted a field the config required and used a
//! command shape the deserialiser rejected; the cookbook's TOML used `;` as a
//! statement separator, which TOML has no concept of. Both looked fine.
//!
//! So this reads the snippets out of the markdown rather than restating them: a
//! test carrying its own copy of an example only proves the copy is right.

use std::fs;
use std::path::{Path, PathBuf};

use wickra_terminal_core::{Config, SourceSpec, Terminal};

/// The documents whose examples must work.
const DOCS: [&str; 8] = [
    "README.md",
    "ARCHITECTURE.md",
    "docs/INDICATORS.md",
    "docs/Cookbook.md",
    "docs/PANELS.md",
    "docs/RENDERERS.md",
    "docs/SOURCES.md",
    "docs/STREAMING.md",
];

/// The eight binding READMEs, which are also the registry landing pages:
/// `PyPI`, `npm`, `NuGet`, Maven Central, `pkg.go.dev` and r-universe render
/// them as the first thing a user of that language reads.
///
/// They are not in [`DOCS`], and adding them there would be decorative: each
/// embeds its config and commands inside a language snippet -- a Python string,
/// a Go raw literal, an escaped C string -- so a guard that reads fenced JSON
/// blocks finds nothing in them. What they do carry uniformly is the command
/// table, which P11.13 had to correct by hand across all eight, and which is
/// checked below.
const BINDING_READMES: [&str; 9] = [
    "bindings/c/README.md",
    "bindings/cpp/README.md",
    "bindings/csharp/README.md",
    "bindings/go/README.md",
    "bindings/java/README.md",
    "bindings/node/README.md",
    "bindings/python/README.md",
    "bindings/r/README.md",
    "bindings/wasm/README.md",
];

/// The repository root, found by walking up from this crate.
fn repo_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..8 {
        if dir.join("README.md").is_file() && dir.join("docs").is_dir() {
            return dir;
        }
        dir = dir
            .parent()
            .unwrap_or_else(|| panic!("no repository root above {}", env!("CARGO_MANIFEST_DIR")))
            .to_path_buf();
    }
    panic!("no repository root found");
}

/// Every fenced block of one language in a markdown file.
fn fenced_blocks(markdown: &str, language: &str) -> Vec<String> {
    let open = format!("```{language}");
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in markdown.lines() {
        match current {
            None if line.trim() == open => current = Some(String::new()),
            None => {}
            Some(_) if line.trim() == "```" => {
                blocks.push(current.take().unwrap_or_default());
            }
            Some(ref mut body) => {
                body.push_str(line);
                body.push('\n');
            }
        }
    }
    blocks
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap_or_else(|err| panic!("{rel}: {err}"))
}

#[test]
fn every_documented_toml_config_parses() {
    let root = repo_root();
    let mut checked = 0;
    for rel in DOCS {
        for (index, block) in fenced_blocks(&read(&root, rel), "toml").iter().enumerate() {
            Config::from_toml(block)
                .unwrap_or_else(|err| panic!("{rel} toml block {index} does not parse: {err}"));
            checked += 1;
        }
    }
    assert!(checked > 0, "no TOML config examples found to check");
}

#[test]
fn every_documented_json_config_parses() {
    let root = repo_root();
    let mut checked = 0;
    for rel in DOCS {
        for (index, block) in fenced_blocks(&read(&root, rel), "json").iter().enumerate() {
            // A block may hold a config, a command, or an example response. Only
            // the configs are checked here; the commands have their own test and
            // a response is not an input at all.
            let Ok(value) = serde_json::from_str::<serde_json::Value>(block) else {
                continue;
            };
            let Some(object) = value.as_object() else {
                continue;
            };
            if !object.contains_key("sources") && !object.contains_key("indicators") {
                continue;
            }
            Config::from_json(block).unwrap_or_else(|err| {
                panic!("{rel} json block {index} is not a valid config: {err}")
            });
            checked += 1;
        }
    }
    assert!(checked > 0, "no JSON config examples found to check");
}

#[test]
fn every_documented_command_is_accepted() {
    let root = repo_root();
    // A fresh terminal per block, not per file. Each fenced block is a
    // self-contained scenario -- one of them adds a source and then addresses it
    // by id -- so replaying every block against one terminal would shift those
    // ids and fail the documentation for being read the way it is written.
    let dataset =
        fs::read_to_string(root.join("golden/replay/basic.json")).expect("the golden replay feed");

    let mut checked = 0;
    for rel in DOCS {
        for (index, block) in fenced_blocks(&read(&root, rel), "json").iter().enumerate() {
            let commands: Vec<&str> = block
                .lines()
                .map(str::trim)
                .filter(|line| line.starts_with('{'))
                .filter(|line| {
                    serde_json::from_str::<serde_json::Value>(line)
                        .ok()
                        .and_then(|v| {
                            v.get("type")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_string)
                        })
                        .is_some()
                })
                .collect();
            if commands.is_empty() {
                continue;
            }

            // Source 0 is a Replay, because the documented commands include
            // `Seek` and only a replayable source accepts it.
            let mut config = Config::default_layout();
            config.sources = vec![SourceSpec::Replay {
                dataset: dataset.clone(),
            }];
            let mut terminal = Terminal::new(&config).expect("the default config must build");
            terminal
                .command_json(r#"{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}"#)
                .expect("subscribe");

            for command in commands {
                terminal.command_json(command).unwrap_or_else(|err| {
                    panic!(
                        "{rel} block {index} documents a command the terminal rejects:
  {command}
  {err}"
                    )
                });
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "no command examples found to check");
}

/// Documents that state how many indicators this terminal reaches.
///
/// Every bare occurrence of the number in these files carries a marker, because
/// the guard is only worth as much as its coverage: three of these documents
/// used to state the count in prose outside any marker, so the test passed while
/// six sites were free to rot.
const COUNTED: [&str; 6] = [
    "README.md",
    "ARCHITECTURE.md",
    "docs/INDICATORS.md",
    "ROADMAP.md",
    "BENCHMARKS.md",
    "docs/STREAMING.md",
];

#[test]
fn the_documented_indicator_count_is_the_real_one() {
    // The indicator library keeps its counts in step with a workflow that
    // rewrites them on every push. This repository has no such workflow, and the
    // count is the single most load-bearing number in its README — it was 514
    // while the core wired two. A marker rather than a loose regular expression
    // because the README also cites the library's own 514 and a sibling
    // project's, and only this terminal's own figure should move with the
    // registry.
    let root = repo_root();
    let actual = wickra_terminal_core::registry::DEFAULTS.len().to_string();

    for rel in COUNTED {
        let text = read(&root, rel);
        let mut found = 0;
        let mut rest = text.as_str();
        while let Some(start) = rest.find(OPEN) {
            let after = &rest[start + OPEN.len()..];
            let end = after
                .find(CLOSE)
                .unwrap_or_else(|| panic!("{rel}: an indicator-count marker is not closed"));
            let claimed = &after[..end];
            assert_eq!(
                claimed, actual,
                "{rel} claims {claimed} indicators; the registry has {actual}"
            );
            found += 1;
            rest = &after[end + CLOSE.len()..];
        }
        // Per file rather than a total: a document that lost its marker used to
        // be masked by another that had gained one.
        assert!(found > 0, "{rel} states no indicator count in a marker");
    }
}

/// The count of indicators wickra-core holds that this terminal cannot reach.
///
/// Not derivable from the registry, which only knows what it registered, so the
/// documents state it and this checks the arithmetic instead: unreachable plus
/// registered must be the library's total. A regeneration that reaches further
/// moves both numbers, and a stale one is caught here rather than by a reader.
#[test]
fn the_unreachable_indicator_count_adds_up() {
    const LIBRARY_TOTAL: usize = 504;
    let root = repo_root();
    // Reachable, not registered. Six of the library's indicators answer with a
    // histogram rather than a reading, so they are carried by the profile
    // surface instead of the registry -- reachable from the terminal all the
    // same, and counting them as unreachable would be a lie in the other
    // direction.
    let registered = wickra_terminal_core::registry::DEFAULTS.len()
        + wickra_terminal_core::registry::PROFILES.len();
    let needle = format!(" of the {LIBRARY_TOTAL}");
    let mut checked = 0;

    for rel in ["docs/INDICATORS.md", "ROADMAP.md"] {
        let text = read(&root, rel);
        for (idx, _) in text.match_indices(&needle) {
            let claimed: usize = text[..idx]
                .rsplit(|c: char| !c.is_ascii_digit())
                .find(|word| !word.is_empty())
                .unwrap_or_else(|| panic!("{rel}: no number before \"{needle}\""))
                .parse()
                .unwrap_or_else(|err| panic!("{rel}: {err}"));
            assert_eq!(
                claimed + registered,
                LIBRARY_TOTAL,
                "{rel} says {claimed} unreachable and the terminal reaches {registered},                  which is not the {LIBRARY_TOTAL} the library ships"
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "no unreachable-count claim found to check");
}

/// `CITATION.cff` states the count in prose, and nothing checked it.
///
/// It cannot carry the marker the other documents use: that marker is an HTML
/// comment, and this is YAML whose abstract is rendered verbatim by GitHub's
/// "Cite this repository" widget and by Zenodo. So the phrase itself is the
/// anchor. It had drifted to 457 while the registry held 455, which is the whole
/// reason the other six documents are guarded rather than trusted.
#[test]
fn the_citation_abstract_states_the_real_indicator_count() {
    // Anchored on the phrase alone, not on the line break before it: the
    // abstract is a folded YAML scalar, so where it wraps is a formatting
    // detail and the count sits on the other side of it.
    const PHRASE: &str = "the Wickra indicators are constructible";

    let root = repo_root();
    let text = read(&root, "CITATION.cff");
    let actual = wickra_terminal_core::registry::DEFAULTS.len().to_string();

    let before = text
        .split(PHRASE)
        .next()
        .unwrap_or_else(|| panic!("CITATION.cff no longer says \"{PHRASE}\""));
    assert_ne!(
        before, text,
        "CITATION.cff no longer states an indicator count"
    );
    let claimed: String = before
        .chars()
        .rev()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    assert_eq!(
        claimed, actual,
        "CITATION.cff claims {claimed} indicators; the registry has {actual}"
    );
}

/// The first released section of the changelog, as `(version, date)`.
///
/// `## [Unreleased]` carries no date and is skipped by the same pattern that
/// finds a release: a released heading is `## [x.y.z] - YYYY-MM-DD`.
fn released_in_changelog(root: &std::path::Path) -> Option<(String, String)> {
    read(root, "CHANGELOG.md").lines().find_map(|line| {
        let rest = line.strip_prefix("## [")?;
        let (version, rest) = rest.split_once("] - ")?;
        let date = rest.trim();
        (date.len() == 10 && date.starts_with("20"))
            .then(|| (version.to_string(), date.to_string()))
    })
}

/// `version` and `date-released` say exactly one thing, and it has to be true.
///
/// They are what GitHub's citation box and Zenodo present as the thing being
/// cited, so carrying them against zero releases dates something that never
/// happened — which is why this file omitted both. The other side is just as
/// real: once a release exists, a citation without them cites nothing in
/// particular, and both keys are release touchpoints nobody would remember on
/// their own.
///
/// So the rule is the pairing rather than either half of it. While the changelog
/// shows no released section both keys must be absent; the moment one is cut,
/// both must be present and agree with it. Cutting a release therefore fails
/// here until the citation is brought along, rather than shipping a stale one.
#[test]
fn the_citation_matches_the_release_state() {
    let root = repo_root();
    let text = read(&root, "CITATION.cff");
    // Line-anchored: `cff-version:` is the schema version and belongs here.
    let value_of = |key: &str| -> Option<String> {
        text.lines()
            .find(|line| line.starts_with(key))?
            .split_once(':')
            .map(|(_, v)| v.trim().trim_matches('"').to_string())
    };

    match released_in_changelog(&root) {
        None => {
            for key in ["version:", "date-released:"] {
                assert!(
                    value_of(key).is_none(),
                    "CITATION.cff carries {key}, which cites a release that does not exist"
                );
            }
        }
        Some((version, date)) => {
            assert_eq!(
                value_of("version:").as_deref(),
                Some(version.as_str()),
                "CHANGELOG released {version}; CITATION.cff does not cite it"
            );
            assert_eq!(
                value_of("date-released:").as_deref(),
                Some(date.as_str()),
                "CHANGELOG dates the release {date}; CITATION.cff does not"
            );
        }
    }
}

/// Every command the READMEs promise is driven by at least one binding suite.
///
/// `every_binding_readme_documents_every_command` below checks the promise is
/// complete. Nothing checked it was kept, and that is exactly how four commands
/// came to be documented in nine READMEs and executed by no binding at all:
/// `SetRecording` and `ExportRecording` by none, `ReplayPosition` only by the C
/// example, `FeedDerivatives` by none. The recorder had never run outside Rust
/// while nine pages described how to use it.
///
/// One suite, not all of them. Holding every command to every language would
/// fail for reasons that are not defects -- the two suites that carry no JSON
/// dependency read answers by matching on the wire form, and some commands are
/// only sensible against a source kind another suite happens to use. What must
/// never be true again is a command that nine pages document and nothing
/// anywhere runs.
#[test]
fn every_documented_command_is_driven_by_a_binding_suite() {
    /// Where a binding proves its reach.
    ///
    /// `examples/c` is the C hub's suite: it is run by ctest in the `c-abi`
    /// job, which is why it counts. `golden/commands` counts for a different
    /// reason -- the scenarios there are driven by all nine suites through the
    /// manifest, so a command in a scenario file is executed in nine languages
    /// even though it appears in no suite's source. Leaving it out reported
    /// `AddSource`, `RemoveSource`, `SetFocus`, `Unsubscribe` and
    /// `SetTimeframe` as undriven, which is the opposite of true: they are the
    /// best-covered commands there are.
    const SUITE_DIRS: [&str; 9] = [
        "bindings/python/tests",
        "bindings/node/__tests__",
        "bindings/wasm/tests",
        "bindings/go",
        "bindings/csharp/WickraTerminal.Tests",
        "bindings/java/src/test/java/org/wickra/terminal",
        "bindings/r/tests",
        "examples/c",
        "golden/commands",
    ];

    let root = repo_root();
    let commands = command_variants(&root);
    assert_eq!(
        commands.len(),
        16,
        "expected sixteen commands: {commands:?}"
    );

    // Read once: eight directories against sixteen names is a hundred and
    // twenty-eight scans of the same files otherwise.
    let mut haystack = String::new();
    for dir in SUITE_DIRS {
        let path = root.join(dir);
        let entries =
            std::fs::read_dir(&path).unwrap_or_else(|err| panic!("{dir} is not readable: {err}"));
        for entry in entries.flatten() {
            if entry.path().is_file() {
                if let Ok(text) = std::fs::read_to_string(entry.path()) {
                    haystack.push_str(&text);
                }
            }
        }
    }
    assert!(
        haystack.len() > 10_000,
        "the suites read as {} bytes, which is not the suites",
        haystack.len()
    );

    let undriven: Vec<&str> = commands
        .iter()
        .copied()
        .filter(|name| !haystack.contains(&format!("\"{name}\"")))
        .collect();
    assert!(
        undriven.is_empty(),
        "documented in every binding README and driven by no binding suite: {undriven:?}"
    );
}

/// The `Command` variants, read out of the source.
///
/// `Command` is private -- it is an implementation detail of the JSON boundary,
/// and making it public to be testable would be the test dictating the API.
fn command_variants(root: &std::path::Path) -> Vec<&'static str> {
    let source = read(root, "crates/wickra-terminal-core/src/terminal.rs");
    let start = source
        .find("enum Command {")
        .expect("the Command enum moved or was renamed");
    let body = &source[start..];
    let body = &body[..body
        .find(
            "
}",
        )
        .expect("an unterminated enum")];

    // One variant per line at four spaces of indent, which is how the enum is
    // written; deeper indents are a variant's fields.
    body.lines()
        .skip(1)
        .filter_map(|line| {
            let rest = line.strip_prefix("    ")?;
            if rest.starts_with(' ') || !rest.starts_with(char::is_uppercase) {
                return None;
            }
            let name = rest
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or_default();
            (!name.is_empty()).then(|| Box::leak(name.to_owned().into_boxed_str()) as &'static str)
        })
        .collect()
}

/// The command table in every binding README lists exactly the commands that
/// exist.
///
/// These tables are the API reference for eight registries, and they had already
/// drifted once: P11.13 corrected all eight by hand, with nothing to stop the
/// next drift. A command added to the core and not to the tables is undocumented
/// in eight places at once; one removed leaves eight pages describing something
/// that now errors.
///
/// The variants are read out of the source because `Command` is private -- it is
/// an implementation detail of the JSON boundary, and making it public to be
/// testable would be the test dictating the API.
#[test]
fn every_binding_readme_documents_every_command() {
    let root = repo_root();
    let source = read(&root, "crates/wickra-terminal-core/src/terminal.rs");
    let start = source
        .find("enum Command {")
        .expect("the Command enum moved or was renamed");
    let body = &source[start..];
    let body = &body[..body
        .find(
            "
}",
        )
        .expect("an unterminated enum")];

    // One variant per line at four spaces of indent, which is how the enum is
    // written; deeper indents are a variant's fields.
    let variants: Vec<&str> = body
        .lines()
        .skip(1)
        .filter_map(|line| {
            let rest = line.strip_prefix("    ")?;
            if rest.starts_with(' ') || !rest.starts_with(char::is_uppercase) {
                return None;
            }
            Some(
                rest.split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or_default(),
            )
        })
        .filter(|name| !name.is_empty())
        .collect();
    assert_eq!(
        variants.len(),
        19,
        "expected nineteen commands, found {variants:?}"
    );

    for rel in BINDING_READMES {
        let text = read(&root, rel);
        // The command table specifically, found by its header: these READMEs
        // carry other tables whose first cell is also backticked, and reading
        // those made this reject a correct page.
        let after_header = text
            .split_once("| Command | Effect |")
            .unwrap_or_else(|| panic!("{rel} has no command table"))
            .1;
        let table: String = after_header
            .lines()
            .skip(1)
            .take_while(|line| line.starts_with('|'))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!table.is_empty(), "{rel}'s command table is empty");

        for variant in &variants {
            assert!(
                table.contains(&format!("`{variant}`")),
                "{rel} does not document the {variant} command"
            );
        }

        // And nothing the core does not have: a table row for a command that was
        // renamed or removed reads as an API that exists.
        //
        // Only the first cell, which is the command column. The description
        // beside it backticks other things -- `Manual` is a source kind, not a
        // command -- and reading those made this reject a correct table.
        for row in table.lines() {
            let command_cell = row.split('|').nth(1).unwrap_or_default();
            for name in command_cell.split('`').skip(1).step_by(2) {
                assert!(
                    variants.contains(&name),
                    "{rel} documents {name}, which is not a command"
                );
            }
        }
    }
}

const OPEN: &str = "<!--indicator-count-->";
const CLOSE: &str = "<!--/indicator-count-->";

#[test]
fn benchmarks_md_lists_the_benchmarks_that_exist() {
    // BENCHMARKS.md described four benchmarks while three existed, and
    // contradicted itself two sections apart: four bullet points above, three
    // table rows below. Nobody reads a document against its own source, so this
    // does.
    let root = repo_root();
    let bench = fs::read_to_string(root.join("crates/wickra-terminal-bench/benches/terminal.rs"))
        .expect("the bench source");
    let doc = read(&root, "BENCHMARKS.md");

    let defined = names_between(&bench, "bench_function(\"", "\"");
    assert!(
        !defined.is_empty(),
        "no benchmarks found in the bench source"
    );

    let in_prose = names_between(&doc, "- **`", "`**");
    let in_table = names_between(
        &doc, "
| `", "`",
    );

    assert_eq!(
        in_prose, defined,
        "BENCHMARKS.md's prose and the bench source disagree"
    );
    assert_eq!(
        in_table, defined,
        "BENCHMARKS.md's results table and the bench source disagree"
    );
}

/// Every substring bracketed by `open` and `close`, as a sorted set.
fn names_between(text: &str, open: &str, close: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(open) {
        let after = &rest[start + open.len()..];
        let Some(end) = after.find(close) else { break };
        found.push(after[..end].to_string());
        rest = &after[end + close.len()..];
    }
    found.sort_unstable();
    found.dedup();
    found
}

/// The data rows of the markdown table whose header line is `header`.
fn table_rows<'a>(text: &'a str, header: &str) -> Vec<&'a str> {
    let start = text
        .find(header)
        .unwrap_or_else(|| panic!("no table headed {header}"));
    text[start..]
        .lines()
        .skip(1)
        .take_while(|line| line.starts_with('|'))
        .filter(|line| !line.trim_start_matches('|').trim_start().starts_with('-'))
        .collect()
}

/// The cell at `index` of a markdown row, trimmed.
fn cell(row: &str, index: usize) -> &str {
    row.split('|')
        .nth(index + 1)
        .unwrap_or_else(|| panic!("row has no cell {index}: {row}"))
        .trim()
}

/// `docs/INDICATORS.md` lists every input family the registry actually feeds.
///
/// The prose said "all four families" over a table of five, and stayed saying it
/// while four more were wired. A count in prose is not checkable, so the table
/// is checked instead, against the families the generator emitted.
#[test]
fn the_documented_input_families_are_the_registered_ones() {
    let text = read(&repo_root(), "docs/INDICATORS.md");
    let rows = table_rows(&text, "| Input | Fed with | Advances |");
    let documented: Vec<String> = rows
        .iter()
        .map(|row| cell(row, 0).replace(['`', ' '], ""))
        .collect();

    for family in wickra_terminal_core::registry::INPUT_FAMILIES {
        let key = family.replace(' ', "");
        assert!(
            documented
                .iter()
                .any(|row| row.ends_with(&format!("({key})")) || row == &key),
            "docs/INDICATORS.md lists no input family for {family}; it has {documented:?}"
        );
    }
    assert_eq!(
        documented.len(),
        wickra_terminal_core::registry::INPUT_FAMILIES.len(),
        "docs/INDICATORS.md lists {} input families, the registry feeds {}",
        documented.len(),
        wickra_terminal_core::registry::INPUT_FAMILIES.len()
    );
}

/// The reach table adds up, and every document citing its total agrees.
///
/// Only the registry row carries a marker, so the other three and the total were
/// bare numbers that nothing moved when a surface grew. The total is the number
/// this repository is judged by, which is exactly why it should not be a number
/// someone remembered to update.
#[test]
fn the_reach_table_sums_to_the_documented_total() {
    // The one indicator no surface fits: `Footprint` answers with price levels,
    // and the terminal renders it from its own state as the `footprint` panel.
    const FOOTPRINT: usize = 1;
    let root = repo_root();
    let profiles = wickra_terminal_core::registry::PROFILES.len();
    let bars = wickra_terminal_core::registry::BAR_TYPES.len();
    let total = wickra_terminal_core::registry::DEFAULTS.len() + profiles + bars + FOOTPRINT;

    let text = read(&root, "docs/INDICATORS.md");
    let rows = table_rows(&text, "| Surface | Count | What it answers with |");
    let mut summed = 0;
    for row in &rows {
        let surface = cell(row, 0);
        let claimed = cell(row, 1)
            .replace(OPEN, "")
            .replace(CLOSE, "")
            .parse::<usize>()
            .unwrap_or_else(|_| panic!("the {surface} row states no count"));
        let actual = match surface {
            "registry" => wickra_terminal_core::registry::DEFAULTS.len(),
            "profiles" => profiles,
            "alternative bars" => bars,
            "the `footprint` panel" => FOOTPRINT,
            other => panic!("unknown surface row {other}; teach this test what counts it"),
        };
        assert_eq!(
            claimed, actual,
            "the {surface} row claims {claimed}, there are {actual}"
        );
        summed += actual;
    }
    assert_eq!(summed, total, "the reach table covers {summed} of {total}");

    // Every document that states the total, stating it as bold digits and
    // nothing else, so the guard cannot be satisfied by an unrelated number.
    for rel in ["docs/INDICATORS.md", "README.md", "CHANGELOG.md"] {
        let text = read(&root, rel);
        let cited: Vec<&str> = text
            .match_indices("**")
            .filter_map(|(idx, _)| {
                let rest = &text[idx + 2..];
                let end = rest.find("**")?;
                let inner = &rest[..end];
                inner.chars().all(|c| c.is_ascii_digit()).then_some(inner)
            })
            .collect();
        assert!(!cited.is_empty(), "{rel} states no reach total");
        for claim in cited {
            assert_eq!(claim, total.to_string(), "{rel} claims a reach of {claim}");
        }
    }
}

/// Every panel the core can emit is one the web renderer knows how to draw.
///
/// The core is where the list lives, so this is where the guard belongs. Two
/// panels — `Profile` and `Bars` — were added to `PanelKind`, given view-models,
/// given TUI widgets, and never taught to the web renderer. The core emitted
/// them in every frame; `findPanel` never asked for them; they vanished without
/// an error anywhere. ARCHITECTURE.md says adding a panel here makes it appear
/// in every renderer at once, and for those two it was not true.
///
/// Nothing else could have caught it. The golden corpus compares frames, which
/// were correct. The web suite tests the mappings it has. A panel a renderer has
/// simply never heard of has no test to fail.
///
/// Checked in three places, because each is a separate way to drop a panel: the
/// `PanelKind` union (the layout would place nothing), the `PanelView` union
/// (`findPanel` would not type-check against it), and a section in the template
/// (the placement exists and draws nothing).
/// Every action the shared keymap can bind reaches one renderer or the other on
/// purpose, and none of them is silently inert.
///
/// `layout.keybinds` sits in the config precisely so a rebinding moves both
/// front-ends, which makes a bound action that no renderer answers the worst
/// shape available: the key looks configured, the config validates, and nothing
/// happens. `remove_source` and `save_recording` were exactly that in the
/// browser -- bound in the default keymap, answered by the TUI, and dropped into
/// `runAction`'s catch-all.
///
/// Three are deliberately unanswered in the browser and are named here rather
/// than inferred, so adding a fourth is a decision someone writes down:
/// `quit`, because a tab is not the terminal's to close, and the panel-focus and
/// scroll pairs, because a web panel is a scrollable box the browser drives.
#[test]
fn every_bound_action_reaches_a_renderer() {
    /// Answered by the TUI alone, each for a reason a browser cannot argue with.
    ///
    /// `quit`, because a tab is not the terminal's to close. The scroll pair,
    /// because a web panel is a scrollable box the browser already drives. The
    /// panel-focus pair, because focus is a renderer's own idea and the browser
    /// does not have one -- and `remove_panel` and `move_panel` go with it for
    /// exactly that reason: with no focused panel there is nothing for them to
    /// act on, and a key that removed an arbitrary panel would be worse than a
    /// key that does nothing. The browser removes with the `x` on the panel
    /// itself, which names its target by sitting on it.
    const BROWSER_DECLINES: [&str; 7] = [
        "quit",
        "next_panel",
        "prev_panel",
        "scroll_up",
        "scroll_down",
        "remove_panel",
        "move_panel",
    ];

    let root = repo_root();
    let input_rs = read(&root, "crates/ui-tui/src/input.rs");
    let app_vue = read(&root, "web/src/App.vue");
    let binds = wickra_terminal_core::config::Keybinds::default();
    assert!(
        binds.bindings.len() >= 19,
        "the default keymap shrank to {} actions",
        binds.bindings.len()
    );

    for action in binds.bindings.keys() {
        assert!(
            input_rs.contains(&format!("Some(\"{action}\")")),
            "crates/ui-tui/src/input.rs: {action:?} is bound and the TUI keymap resolves it to nothing"
        );
        if BROWSER_DECLINES.contains(&action.as_str()) {
            continue;
        }
        assert!(
            app_vue.contains(&format!("case '{action}'")),
            "web/src/App.vue: {action:?} is bound and runAction drops it into the catch-all"
        );
    }
}

#[test]
fn every_panel_kind_reaches_the_web_renderer() {
    let root = repo_root();
    let panels_rs = read(&root, "crates/wickra-terminal-core/src/panels/mod.rs");
    let types_ts = read(&root, "web/src/types.ts");
    let app_vue = read(&root, "web/src/App.vue");

    // The variants of `enum PanelKind`, read from the enum itself rather than
    // restated here — a list this test carried its own copy of would go stale
    // in exactly the way the bug did.
    let body = panels_rs
        .split_once("pub enum PanelKind {")
        .and_then(|(_, rest)| rest.split_once("\n}"))
        .map(|(body, _)| body)
        .expect("panels/mod.rs declares enum PanelKind");
    let kinds: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_suffix(','))
        .filter(|name| {
            name.chars().next().is_some_and(char::is_uppercase)
                && name.chars().all(char::is_alphanumeric)
        })
        .collect();
    assert!(kinds.len() >= 7, "found only {kinds:?} in PanelKind");

    for kind in kinds {
        let tag = kind.to_lowercase();
        assert!(
            types_ts.contains(&format!("'{kind}'")),
            "web/src/types.ts: PanelKind has no {kind}, so the layout places nothing for it"
        );
        assert!(
            types_ts.contains(&format!("panel: '{tag}'")),
            "web/src/types.ts: PanelView has no {tag} variant, so the frame's is discarded"
        );
        assert!(
            app_vue.contains(&format!("placements.{kind}")),
            "web/src/App.vue: no section for {kind}, so it is placed and never drawn"
        );
    }
}
