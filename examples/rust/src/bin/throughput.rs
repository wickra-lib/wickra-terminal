//! Throughput baseline for the per-binding benchmarks: the same work with no
//! language boundary in the way.
//!
//! Each of the nine bindings ships a `throughput` benchmark that drives the same
//! config through the same two commands and reports commands per second. This is
//! the row they are measured against, and it has to be measured the same way to
//! be comparable -- a tight loop of N commands, median of three runs, one warmup
//! -- rather than by criterion, whose per-iteration harness costs enough to make
//! the comparison misleading. `crates/wickra-terminal-bench` measures the same
//! call under criterion for tracking regressions over time; that is a different
//! question from what a boundary costs.
//!
//! ```bash
//! cargo run -p wickra-terminal-example --bin throughput --release
//! cargo run -p wickra-terminal-example --bin throughput --release -- 100000
//! ```

use std::hint::black_box;
use std::time::Instant;

use wickra_terminal_core::Terminal;

/// Shared by all nine binding benchmarks, so the numbers compare.
const CONFIG: &str = concat!(
    "{\"sources\":[{\"Synth\":{\"seed\":1}}],",
    "\"layout\":{\"panels\":[",
    "{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":40}},",
    "{\"kind\":\"Book\",\"rect\":{\"x\":0,\"y\":40,\"w\":50,\"h\":30}},",
    "{\"kind\":\"Tape\",\"rect\":{\"x\":50,\"y\":40,\"w\":50,\"h\":30}}]}}",
);
const SUBSCRIBE: &str = "{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}";
const TICK: &str = "{\"type\":\"Tick\"}";
const LIST: &str = "{\"type\":\"ListIndicators\"}";

/// The catalogue response is ~30 kB, so a hundred of them is a noisy sample --
/// noisy enough that this baseline once looked slower than a binding.
const CATALOGUE_REPS: u32 = 1000;

/// Median elapsed nanoseconds over three runs, after one warmup.
fn median_ns(terminal: &mut Terminal, command: &str, count: u32) -> f64 {
    let drive = |terminal: &mut Terminal| {
        for _ in 0..count {
            black_box(terminal.command_json(black_box(command)).unwrap());
        }
    };
    drive(terminal);
    let mut samples = [0.0_f64; 3];
    for sample in &mut samples {
        let start = Instant::now();
        drive(terminal);
        *sample = start.elapsed().as_nanos() as f64;
    }
    samples.sort_by(f64::total_cmp);
    samples[1]
}

fn main() {
    let ticks: u32 = std::env::args()
        .nth(1)
        .and_then(|raw| raw.parse().ok())
        .filter(|&parsed| parsed >= 100)
        .unwrap_or(20_000);

    let mut terminal = Terminal::from_json(CONFIG).expect("the benchmark config is valid");
    terminal.command_json(SUBSCRIBE).expect("subscribe");
    let frame_bytes = terminal.command_json(TICK).expect("tick").len();
    let catalogue_bytes = terminal.command_json(LIST).expect("list").len();

    let tick_ns = median_ns(&mut terminal, TICK, ticks);
    let list_ns = median_ns(&mut terminal, LIST, CATALOGUE_REPS);

    println!("wickra-terminal Rust throughput - {ticks} commands (median of 3), no boundary\n");
    println!(
        "{:<18}{:>14}{:>14}{:>12}",
        "Command", "per second", "us/command", "payload"
    );
    println!("{}", "-".repeat(58));
    let row = |name: &str, count: f64, ns: f64, bytes: usize| {
        println!(
            "{name:<18}{:>14.0}{:>14.2}{:>11}B",
            count / (ns / 1e9),
            ns / count / 1e3,
            bytes
        );
    };
    row("Tick", f64::from(ticks), tick_ns, frame_bytes);
    row(
        "ListIndicators",
        f64::from(CATALOGUE_REPS),
        list_ns,
        catalogue_bytes,
    );
    println!(
        "\nOne command, no boundary. This is the floor the nine bindings are measured\n\
         against; the numbers are machine-dependent, so compare on one machine only."
    );
}
