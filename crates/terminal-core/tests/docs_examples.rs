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
const BINDING_READMES: [&str; 8] = [
    "bindings/c/README.md",
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
    let source = read(&root, "crates/terminal-core/src/terminal.rs");
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
        12,
        "expected twelve commands, found {variants:?}"
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
