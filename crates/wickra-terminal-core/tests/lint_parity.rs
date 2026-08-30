//! The clippy lists that Cargo cannot share.
//!
//! Every crate here inherits the workspace lints with `[lints] workspace = true`
//! except `bindings/c` and `bindings/node`, which need their own `unsafe_code`
//! level -- the C ABI writes unsafe deliberately, and napi's derive macros expand
//! to unsafe inside the Node crate. Cargo has no way to merge an inherited table
//! with a per-crate override: a crate that writes `[lints.rust]` gets no
//! workspace lints at all, so those two restate the whole clippy list verbatim.
//!
//! P3.6 removed that duplication once and P11.25 reintroduced it, because the
//! override is genuinely required. What was missing is anything to notice when a
//! copy drifts: a lint relaxed at the root and not in the copies would be
//! enforced in two crates and nowhere else, and one relaxed only in a copy would
//! be quietly weaker there. This compares them.

use std::fs;
use std::path::PathBuf;

/// The crates that cannot inherit, and why they cannot.
const OVERRIDING: [&str; 2] = ["bindings/c/Cargo.toml", "bindings/node/Cargo.toml"];

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

/// The entries of a `[…lints.clippy]` table, in file order.
///
/// Read as text rather than parsed: a TOML dependency for one table would be a
/// build cost every crate in the workspace pays, and the tables are flat.
fn clippy_table(manifest: &str, header: &str) -> Vec<String> {
    let body = manifest
        .split_once(header)
        .unwrap_or_else(|| panic!("no {header} table"))
        .1;
    body.lines()
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToString::to_string)
        .collect()
}

#[test]
fn the_crates_that_cannot_inherit_carry_the_same_clippy_list() {
    let root = repo_root();
    let workspace = fs::read_to_string(root.join("Cargo.toml")).expect("the workspace manifest");
    let expected = clippy_table(&workspace, "[workspace.lints.clippy]");
    assert!(
        expected.len() > 5,
        "the workspace clippy table looks empty: {expected:?}"
    );

    for rel in OVERRIDING {
        let manifest = fs::read_to_string(root.join(rel)).unwrap_or_else(|_| panic!("{rel}"));
        let copy = clippy_table(&manifest, "[lints.clippy]");
        assert_eq!(
            copy, expected,
            "{rel} has drifted from the workspace clippy list; Cargo cannot merge an           inherited table with a per-crate override, so this copy has to be kept           in step by hand"
        );
    }
}

#[test]
fn every_other_crate_inherits_rather_than_copying() {
    // The list above is exhaustive only while nothing else overrides. A third
    // crate writing its own table would be a third copy nothing compares.
    let root = repo_root();
    let mut found = Vec::new();
    for manifest in [
        "bindings/c/Cargo.toml",
        "bindings/node/Cargo.toml",
        "bindings/python/Cargo.toml",
        "bindings/wasm/Cargo.toml",
        "crates/wickra-terminal-bench/Cargo.toml",
        "crates/wickra-terminal-core/Cargo.toml",
        "crates/ui-tui/Cargo.toml",
        "examples/rust/Cargo.toml",
    ] {
        let text = fs::read_to_string(root.join(manifest)).unwrap_or_else(|_| panic!("{manifest}"));
        if text.contains("[lints.clippy]") {
            found.push(manifest);
        }
    }
    assert_eq!(
        found, OVERRIDING,
        "a crate carries its own clippy table without being listed here"
    );
}

/// No two workspace targets normalise to the same output stem.
///
/// Cargo turns a target name into an object-file stem by replacing `-` with `_`,
/// so a bin called `wickra-terminal` and the C ABI's `wickra_terminal` cdylib
/// both write `wickra_terminal.pdb`. A parallel `cargo build --workspace` on
/// Windows then races for that file and fails with `LNK1201`, which reads as a
/// disk or permissions fault and is neither.
///
/// Cargo notices, but only warns -- `output filename collision at ...` in a build
/// that otherwise succeeds, which is exactly the kind of line that scrolls past.
/// So it is asserted here instead. wickra-backtest hit the same constraint and
/// answered it the same way, by naming its CLI `wkbt`.
#[test]
fn no_two_targets_share_an_output_stem() {
    let root = repo_root();
    let workspace = fs::read_to_string(root.join("Cargo.toml")).expect("the workspace manifest");
    let members = between(&workspace, "members = [", "]");

    let mut stems: Vec<(String, String)> = Vec::new();
    for member in members {
        let manifest_path = root.join(&member).join("Cargo.toml");
        let manifest = fs::read_to_string(&manifest_path)
            .unwrap_or_else(|_| panic!("{member} is a workspace member with no manifest"));
        for (section, target) in target_names(&manifest) {
            stems.push((target.replace('-', "_"), format!("{member} [{section}]")));
        }
    }

    let mut clashes = Vec::new();
    for (index, (stem, owner)) in stems.iter().enumerate() {
        for (other_stem, other_owner) in &stems[index + 1..] {
            if stem == other_stem {
                clashes.push(format!("{stem}: {owner} and {other_owner}"));
            }
        }
    }
    assert!(
        clashes.is_empty(),
        "{} pair(s) of targets write the same output stem:\n  {}",
        clashes.len(),
        clashes.join("\n  ")
    );
}

/// The slice between two markers, split into quoted entries.
fn between(text: &str, open: &str, close: &str) -> Vec<String> {
    let start = text.find(open).map_or(0, |at| at + open.len());
    let end = text[start..]
        .find(close)
        .map_or(text.len(), |at| start + at);
    text[start..end]
        .split(',')
        .filter_map(|entry| {
            let entry = entry.trim().trim_matches('"');
            (!entry.is_empty()).then(|| entry.to_owned())
        })
        .collect()
}

/// Every explicitly named `[lib]` and `[[bin]]` target in one manifest.
///
/// A target with no `name` takes the package name, and two packages cannot share
/// one, so only the explicit names can collide.
fn target_names(manifest: &str) -> Vec<(&'static str, String)> {
    let mut found = Vec::new();
    for (section, header) in [("lib", "[lib]"), ("bin", "[[bin]]")] {
        let mut rest = manifest;
        while let Some(at) = rest.find(header) {
            rest = &rest[at + header.len()..];
            let block = rest.split("\n[").next().unwrap_or(rest);
            if let Some(line) = block
                .lines()
                .find(|line| line.trim_start().starts_with("name"))
            {
                if let Some(name) = line.split('=').nth(1) {
                    found.push((section, name.trim().trim_matches('"').to_owned()));
                }
            }
        }
    }
    found
}
