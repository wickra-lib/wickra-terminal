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
