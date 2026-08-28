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

use terminal_core::{Config, SourceSpec, Terminal};

/// The documents whose examples must work.
const DOCS: [&str; 7] = [
    "README.md",
    "docs/INDICATORS.md",
    "docs/Cookbook.md",
    "docs/PANELS.md",
    "docs/RENDERERS.md",
    "docs/SOURCES.md",
    "docs/STREAMING.md",
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
    let actual = terminal_core::registry::DEFAULTS.len().to_string();

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
    let registered = terminal_core::registry::DEFAULTS.len();
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
                "{rel} says {claimed} unreachable and the registry holds {registered},                  which is not the {LIBRARY_TOTAL} the library ships"
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
    let actual = terminal_core::registry::DEFAULTS.len().to_string();

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

/// The citation must not date a release that has not happened.
///
/// `version` and `date-released` are what the citation widget and Zenodo present
/// as the thing being cited. This file carried `0.1.0` and a date against zero
/// tags and zero releases. Both keys are optional in CFF, and the wickra library
/// omits them even though it has released thirty times, so omitting them here is
/// the convention as well as the truth.
#[test]
fn the_citation_claims_no_release() {
    let root = repo_root();
    let text = read(&root, "CITATION.cff");
    // Line-anchored: `cff-version:` is the schema version and belongs here.
    for key in ["version:", "date-released:"] {
        assert!(
            !text.lines().any(|line| line.starts_with(key)),
            "CITATION.cff carries {key}, which cites a release that does not exist"
        );
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
    let bench = fs::read_to_string(root.join("crates/terminal-bench/benches/terminal.rs"))
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
