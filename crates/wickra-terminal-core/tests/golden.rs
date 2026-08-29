//! Golden-fixture parity: committed configs and command sequences drive the
//! terminal and must produce byte-identical frames, so the deterministic
//! feed-to-frame pipeline can never drift silently — and every language binding
//! is checked against the same files.
//!
//! Regenerate with `WICKRA_REGEN=1 cargo test -p wickra-terminal-core --test golden`.
//!
//! # The corpus
//!
//! `golden/manifest.json` lists every scenario, and that is what makes the corpus
//! extensible: a binding reads the manifest, replays each scenario's commands
//! against its config, and compares. Adding a scenario is one entry in
//! `SCENARIOS` below plus a regeneration — no binding test changes, in any of the
//! nine languages.
//!
//! Per scenario:
//! - `configs/<name>.json` — the complete `Terminal::new` config, so every binding
//!   constructs the identical terminal from one committed file with no JSON
//!   assembly.
//! - `commands/<name>.txt` — the command sequence, one per line. A file rather
//!   than an array in the manifest, so every manifest value stays a plain path:
//!   a command is a JSON string full of quotes, and embedding it would leave the
//!   manifest full of escapes for the two bindings that carry no JSON dependency
//!   to unpick by hand.
//! - `expected/<name>.min.json` — the frame exactly as `command_json` emits it
//!   (compact `serde_json::to_string`). Every binding returns that string
//!   verbatim, so a binding is at parity iff its frame matches byte-for-byte,
//!   with no per-language JSON deep-equal needed.
//! - `expected/<name>.json` — the same frame pretty-printed, for a human reading
//!   a diff.
//!
//! `config.json`, `replay/basic.json` and `expected/basic*.json` are kept at
//! their original paths: they are what the first corpus shipped, and moving them
//! would break every binding at once for no gain.

use std::fs;

use rust_decimal::Decimal;
use wickra_exchange_core::{BookDelta, BookLevel, Event, OrderBookSnapshot, OrderSide, TradePrint};
use wickra_terminal_core::config::{Keybinds, PanelSpec, RectSpec};
use wickra_terminal_core::panels::PanelKind;
use wickra_terminal_core::{Config, IndicatorSpec, SourceSpec, Symbol, Terminal, Timeframe};

fn golden_dir() -> String {
    format!("{}/../../golden", env!("CARGO_MANIFEST_DIR"))
}

const SYMBOL: &str = "BTC/USDT";

fn sym() -> Symbol {
    Symbol::new("BTC", "USDT")
}

fn trade(price: i64, qty: i64, buy: bool, ts: i64) -> Event {
    Event::Trade(TradePrint {
        symbol: sym(),
        price: Decimal::new(price, 0),
        quantity: Decimal::new(qty, 2),
        aggressor: if buy { OrderSide::Buy } else { OrderSide::Sell },
        timestamp: ts,
    })
}

fn level(price: i64, qty: i64) -> BookLevel {
    BookLevel::new(Decimal::new(price, 0), Decimal::new(qty, 1))
}

/// The canonical recorded feed: a handful of prints plus a book snapshot.
fn canonical_feed() -> Vec<Event> {
    vec![
        trade(20_000, 50, true, 1),
        trade(20_001, 30, true, 2),
        trade(19_999, 40, false, 3),
        Event::BookSnapshot(OrderBookSnapshot {
            symbol: sym(),
            last_update_id: 10,
            bids: vec![level(19_999, 15), level(19_998, 25)],
            asks: vec![level(20_001, 12), level(20_002, 30)],
        }),
        trade(20_002, 20, true, 4),
        trade(20_000, 10, false, 5),
    ]
}

/// A snapshot followed by diffs, including removals — the highest-rate message
/// on a live feed and the one the basic scenario never exercises.
fn book_delta_feed() -> Vec<Event> {
    vec![
        Event::BookSnapshot(OrderBookSnapshot {
            symbol: sym(),
            last_update_id: 1,
            bids: vec![level(19_999, 10), level(19_998, 20), level(19_997, 30)],
            asks: vec![level(20_001, 10), level(20_002, 20), level(20_003, 30)],
        }),
        // Re-price the top of book and remove a level a side.
        Event::BookDelta(BookDelta {
            symbol: sym(),
            first_update_id: 2,
            final_update_id: 2,
            bids: vec![level(19_999, 55), level(19_998, 0)],
            asks: vec![level(20_001, 45), level(20_002, 0)],
        }),
        // Add a new level outside the previous range on each side.
        Event::BookDelta(BookDelta {
            symbol: sym(),
            first_update_id: 3,
            final_update_id: 3,
            bids: vec![level(19_996, 5)],
            asks: vec![level(20_004, 5)],
        }),
        trade(20_000, 25, true, 4),
    ]
}

/// Repeated prices on both sides, so the footprint accumulates rather than just
/// recording one entry per price.
fn footprint_feed() -> Vec<Event> {
    vec![
        trade(20_000, 10, true, 1),
        trade(20_000, 15, true, 2),
        trade(20_000, 5, false, 3),
        trade(20_001, 20, true, 4),
        trade(20_001, 30, false, 5),
        trade(19_999, 40, false, 6),
        trade(19_999, 10, false, 7),
        trade(20_000, 25, true, 8),
    ]
}

/// A price path long enough to warm up a short moving average and an RSI, so the
/// scenario pins real indicator values rather than a row of nulls.
fn indicator_feed() -> Vec<Event> {
    (0..40)
        .map(|step| {
            // A rising then falling path: a flat one drives RSI to a degenerate
            // value and pins nothing useful.
            let wave = if step < 20 { step } else { 40 - step };
            trade(20_000 + wave * 3, 10, step % 3 != 0, step + 1)
        })
        .collect()
}

/// Two markets moving together, for the pairwise family.
///
/// The reference prints before this market on every step, which is the order
/// that gives a defined answer: `fold` reads the reference markets as they stand
/// before folding the current event, so a reference that printed after would be
/// one step stale.
///
/// The paths are sine waves rather than ramps. A straight line has constant
/// first differences, which drives the variance of the differences to zero and
/// makes correlation undefined -- reported as `0.0`, which looks exactly like a
/// dead wiring. Both move on the same wave, so the correlation is a clean +1 and
/// a regression in the pairing shows up as a number that is not 1.
fn pairwise_feed() -> Vec<Event> {
    let mut feed = Vec::new();
    for step in 0..40_i64 {
        let wave = if step < 20 { step } else { 40 - step };
        feed.push(Event::Trade(TradePrint {
            symbol: Symbol::new("ETH", "USDT"),
            price: Decimal::new(1_500 + wave * 7, 0),
            quantity: Decimal::new(10, 2),
            aggressor: OrderSide::Buy,
            timestamp: step * 2 + 1,
        }));
        feed.push(trade(20_000 + wave * 3, 10, true, step * 2 + 2));
    }
    feed
}

/// A feed whose timestamps span several seconds, so bars actually close.
///
/// `indicator_feed` stamps one millisecond per event, which keeps forty events
/// inside a single second: at a `1s` timeframe no bar ever closes and a
/// candle-input indicator stays silent, which would pin nothing.
fn multi_second_feed() -> Vec<Event> {
    (0..48_i64)
        .map(|step| {
            let wave = if step < 24 { step } else { 48 - step };
            // Three trades a second, so each bar has a real open, high and close,
            // and long enough that the indicator warms up again after the switch.
            trade(20_000 + wave * 5, 10, step % 3 != 0, step * 334)
        })
        .collect()
}

/// One scenario: a config, the commands to drive it, and the name its fixtures
/// carry.
struct Scenario {
    name: &'static str,
    config: Config,
    commands: Vec<String>,
    /// Where the recorded feed lives, when the scenario has one.
    replay_path: Option<&'static str>,
    feed: Option<Vec<Event>>,
}

fn tick(n: usize) -> Vec<String> {
    (0..n).map(|_| r#"{"type":"Tick"}"#.to_string()).collect()
}

fn subscribe(source: u32) -> String {
    format!(r#"{{"type":"Subscribe","source":{source},"symbol":"{SYMBOL}"}}"#)
}

fn replay_config(feed: &[Event]) -> Config {
    let mut config = Config::default_layout();
    config.sources = vec![SourceSpec::Replay {
        dataset: serde_json::to_string(feed).unwrap(),
    }];
    config
}

fn scenarios() -> Vec<Scenario> {
    let basic = canonical_feed();
    let deltas = book_delta_feed();
    let footprint = footprint_feed();
    let indicators = indicator_feed();

    let mut with_indicators = replay_config(&indicators);
    with_indicators.indicators = vec![
        IndicatorSpec::new("Sma", vec![5.0]),
        IndicatorSpec::new("Rsi", vec![14.0]),
        IndicatorSpec::new("MacdIndicator", vec![12.0, 26.0, 9.0]),
    ];
    with_indicators.timeframe = Timeframe::parse("1s").unwrap();

    let pairwise = pairwise_feed();
    let mut with_pair = replay_config(&pairwise);
    with_pair.indicators = vec![IndicatorSpec::paired(
        "RollingCorrelation",
        vec![20.0],
        "ETH/USDT",
    )];
    with_pair.timeframe = Timeframe::parse("1s").unwrap();

    let timeframe_feed = multi_second_feed();
    let mut with_timeframe = replay_config(&timeframe_feed);
    // A candle-input indicator, so the bar size is observable in the frame: Atr
    // reads bars and nothing else, and changing the timeframe restarts it.
    with_timeframe.indicators = vec![IndicatorSpec::new("Atr", vec![2.0])];
    with_timeframe.timeframe = Timeframe::parse("1s").unwrap();

    let lifecycle_feed = canonical_feed();
    let lifecycle = replay_config(&lifecycle_feed);

    // The three surfaces that are not the registry. Each gets a scenario, so the
    // nine language suites hold them to byte parity the same way they hold the
    // readings -- a profile that serialised its bins differently in one binding
    // would otherwise pass everywhere.
    // A feed that spans SECONDS, not milliseconds. `indicator_feed` stamps its
    // forty trades one millisecond apart, so at a one-second bar they all fall
    // in the same bar and none ever closes -- which a price indicator does not
    // notice and a bar-input one does. Both surfaces below read closed bars, so
    // recorded against that feed they would have captured an empty histogram
    // and an empty bar list, and the scenario would have proved nothing while
    // passing.
    let profile_feed = multi_second_feed();
    let mut with_profile = replay_config(&profile_feed);
    with_profile.profiles = vec![IndicatorSpec::new("VolumeProfile", vec![4.0, 8.0])];
    with_profile.layout.panels = vec![PanelSpec {
        kind: PanelKind::Profile,
        rect: RectSpec {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        },
    }];
    with_profile.timeframe = Timeframe::parse("1s").unwrap();

    let bar_feed = multi_second_feed();
    let mut with_bars = replay_config(&bar_feed);
    // A three-unit brick on a feed that walks in threes, so bricks complete
    // within the scenario rather than after it.
    with_bars.bars = vec![IndicatorSpec::new("RenkoBars", vec![3.0])];
    with_bars.layout.panels = vec![PanelSpec {
        kind: PanelKind::Bars,
        rect: RectSpec {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        },
    }];
    with_bars.timeframe = Timeframe::parse("1s").unwrap();

    let derivatives_feed = indicator_feed();
    let mut with_derivatives = replay_config(&derivatives_feed);
    with_derivatives.indicators = vec![IndicatorSpec::new("FundingRate", vec![])];
    with_derivatives.timeframe = Timeframe::parse("1s").unwrap();

    let mut multi = Config::default_layout();
    multi.sources = vec![
        SourceSpec::Replay {
            dataset: serde_json::to_string(&basic).unwrap(),
        },
        SourceSpec::Synth { seed: 7 },
    ];

    vec![
        Scenario {
            name: "basic",
            config: replay_config(&basic),
            commands: [vec![subscribe(0)], tick(basic.len())].concat(),
            replay_path: Some("replay/basic.json"),
            feed: Some(basic),
        },
        Scenario {
            name: "book_deltas",
            config: replay_config(&deltas),
            commands: [vec![subscribe(0)], tick(deltas.len())].concat(),
            replay_path: Some("replay/book_deltas.json"),
            feed: Some(deltas),
        },
        Scenario {
            name: "footprint",
            config: replay_config(&footprint),
            commands: [vec![subscribe(0)], tick(footprint.len())].concat(),
            replay_path: Some("replay/footprint.json"),
            feed: Some(footprint),
        },
        Scenario {
            name: "indicators",
            config: with_indicators,
            commands: [vec![subscribe(0)], tick(indicators.len())].concat(),
            replay_path: Some("replay/indicators.json"),
            feed: Some(indicators),
        },
        Scenario {
            // A pairwise indicator across two markets: the reference has to reach
            // it through the tick, and the label has to carry which market it is
            // against, because the same indicator against another one is a
            // different reading.
            name: "pairwise",
            config: with_pair,
            commands: [
                vec![
                    subscribe(0),
                    r#"{"type":"Subscribe","source":0,"symbol":"ETH/USDT"}"#.to_string(),
                    format!(r#"{{"type":"SetFocus","source":0,"symbol":"{SYMBOL}"}}"#),
                ],
                tick(pairwise.len()),
            ]
            .concat(),
            replay_path: Some("replay/pairwise.json"),
            feed: Some(pairwise),
        },
        Scenario {
            // Drive to the end, rewind, and drive forward again: the frame after
            // a seek must equal the frame at that point the first time through,
            // which is the whole promise of the time machine.
            name: "seek",
            config: replay_config(&canonical_feed()),
            commands: [
                vec![subscribe(0)],
                tick(6),
                vec![r#"{"type":"Seek","source":0,"index":2}"#.to_string()],
                tick(2),
            ]
            .concat(),
            replay_path: None,
            feed: None,
        },
        Scenario {
            // `SetTimeframe` mid-run. It is the candle work's public entry point
            // and was reachable from no binding test and no scenario, so nothing
            // outside the Rust unit tests held the eight other languages to it.
            name: "timeframe",
            config: with_timeframe,
            commands: [
                vec![subscribe(0)],
                tick(8),
                vec![r#"{"type":"SetTimeframe","timeframe":"2s"}"#.to_string()],
                tick(42),
            ]
            .concat(),
            replay_path: Some("replay/timeframe.json"),
            feed: Some(timeframe_feed),
        },
        Scenario {
            // The runtime-source API: `AddSource`, `Unsubscribe` and
            // `RemoveSource`, none of which any binding exercised. A source added
            // and then removed must leave the watchlist and the panels as if it
            // had never been opened.
            name: "source_lifecycle",
            config: lifecycle,
            commands: [
                vec![subscribe(0)],
                tick(3),
                vec![
                    r#"{"type":"AddSource","spec":{"Synth":{"seed":11}}}"#.to_string(),
                    format!(r#"{{"type":"Subscribe","source":1,"symbol":"{SYMBOL}"}}"#),
                ],
                tick(3),
                // Drop the source the terminal started with, keeping the one
                // added at run time. The final frame then shows a watchlist of
                // source 1 alone, which no sequence without both AddSource and
                // RemoveSource can produce -- a fixture that ended back at the
                // starting state would pin nothing.
                vec![
                    format!(r#"{{"type":"Unsubscribe","source":0,"symbol":"{SYMBOL}"}}"#),
                    r#"{"type":"RemoveSource","id":0}"#.to_string(),
                    format!(r#"{{"type":"SetFocus","source":1,"symbol":"{SYMBOL}"}}"#),
                ],
                tick(2),
            ]
            .concat(),
            replay_path: None,
            feed: None,
        },
        Scenario {
            // A profile: the panel answers with a histogram rather than a
            // reading, and its bins have to serialise identically everywhere.
            name: "profiles",
            config: with_profile,
            commands: [vec![subscribe(0)], tick(profile_feed.len())].concat(),
            replay_path: Some("replay/profiles.json"),
            feed: Some(profile_feed),
        },
        Scenario {
            // An alternative chart: zero, one or several bars complete per
            // candle, so the count in the frame is itself the assertion.
            name: "alt_bars",
            config: with_bars,
            commands: [vec![subscribe(0)], tick(bar_feed.len())].concat(),
            replay_path: Some("replay/alt_bars.json"),
            feed: Some(bar_feed),
        },
        Scenario {
            // The derivatives command: the only market input a host pushes in
            // rather than a source producing, so the corpus has to drive it.
            name: "derivatives",
            config: with_derivatives,
            commands: [
                vec![
                    subscribe(0),
                    format!(
                        r#"{{"type":"FeedDerivatives","source":0,"symbol":"{SYMBOL}","update":{{"funding_rate":0.0001,"mark_price":20010.0,"index_price":20000.0,"futures_price":20020.0,"open_interest":1000000.0,"timestamp":1}}}}"#
                    ),
                ],
                tick(derivatives_feed.len() / 2),
                vec![format!(
                    r#"{{"type":"FeedDerivatives","source":0,"symbol":"{SYMBOL}","update":{{"funding_rate":0.0003,"mark_price":20040.0,"timestamp":2}}}}"#
                )],
                tick(derivatives_feed.len() / 2),
            ]
            .concat(),
            replay_path: Some("replay/derivatives.json"),
            feed: Some(derivatives_feed),
        },
        Scenario {
            // Two sources at once, with the second subscribed and focused, so the
            // watchlist carries both and the panels follow the focused one.
            name: "multi_source",
            config: multi,
            commands: [
                vec![
                    subscribe(0),
                    subscribe(1),
                    format!(r#"{{"type":"SetFocus","source":1,"symbol":"{SYMBOL}"}}"#),
                ],
                tick(6),
            ]
            .concat(),
            replay_path: None,
            feed: None,
        },
    ]
}

/// Drive one scenario and return its compact frame.
fn run(scenario: &Scenario) -> String {
    let mut terminal = Terminal::new(&scenario.config)
        .unwrap_or_else(|err| panic!("{}: config rejected: {err}", scenario.name));
    let mut frame = String::new();
    for command in &scenario.commands {
        frame = terminal
            .command_json(command)
            .unwrap_or_else(|err| panic!("{}: {command} rejected: {err}", scenario.name));
    }
    frame
}

/// The config as a binding reads it: everything but the keybinds.
///
/// Serialised from the `Config` itself and then stripped, rather than built
/// key by key. The hand-built version listed four fields, and a config field
/// added after it was written simply never reached the committed file: the
/// profile and bar scenarios recorded a Rust frame full of data while every
/// binding, reading the same file, built a terminal with neither configured and
/// answered with empty lists. Serialising the whole thing cannot forget a field.
///
/// Keybinds are the one omission, and stay one: they carry a non-deterministic
/// map order, never affect a frame, and `Terminal::new` fills the defaults.
fn config_json(config: &Config) -> serde_json::Value {
    let mut value = serde_json::to_value(config).expect("a config serialises");
    if let Some(layout) = value.get_mut("layout").and_then(|l| l.as_object_mut()) {
        layout.remove("keybinds");
    }
    value
}

fn write_or_compare(path: &str, content: &str, regen: bool) {
    if regen {
        if let Some(parent) = std::path::Path::new(path).parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
        return;
    }
    let committed = fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("missing {path}; regenerate with WICKRA_REGEN=1"));
    assert_eq!(
        content.trim_end(),
        committed.trim_end(),
        "{path} drifted — regenerate with WICKRA_REGEN=1"
    );
}

/// What a scenario must NOT record: the empty answer for the thing it exists
/// to show.
///
/// A recorded frame is compared byte-for-byte against what the terminal emits,
/// which makes a scenario that records emptiness pass forever while proving
/// nothing. Two did exactly that on their first regeneration here: the profile
/// and alternative-bar scenarios were driven by a feed whose forty trades are
/// one MILLISECOND apart, so at a one-second bar none ever closed. Both
/// surfaces read closed bars, both recorded an empty list, and both passed.
///
/// Byte parity across nine languages is worth nothing if the bytes say nothing.
const MUST_NOT_RECORD: [(&str, &str); 4] = [
    ("indicators", r#""indicators":[]"#),
    ("profiles", r#""bins":[]"#),
    ("alt_bars", r#""bars":[]"#),
    // The derivatives scenario tracks one indicator and feeds it twice; a
    // null reading means the update never reached it.
    ("derivatives", r#""value":null"#),
];

#[test]
fn golden_corpus_is_byte_exact() {
    let dir = golden_dir();
    let regen = std::env::var("WICKRA_REGEN").is_ok();
    let scenarios = scenarios();

    let mut manifest = Vec::new();
    for scenario in &scenarios {
        let frame_min = run(scenario);
        // Parsed back into the typed `Frame`, not into a `Value`: a Value is a
        // map and pretty-printing it sorts the keys, so the human-readable copy
        // would lose the field order the wire form has.
        let frame: wickra_terminal_core::Frame = serde_json::from_str(&frame_min)
            .unwrap_or_else(|err| panic!("{}: frame does not round-trip: {err}", scenario.name));

        for (name, empty) in MUST_NOT_RECORD {
            if name == scenario.name {
                assert!(
                    !frame_min.contains(empty),
                    "{name} recorded {empty}: the scenario passes byte parity while showing nothing"
                );
            }
        }

        // The file a binding reads must rebuild the config this scenario ran,
        // and that is checked here rather than trusted. It was not trusted
        // idly: `config_json` used to list its fields by hand, so a config
        // field added afterwards never reached the file. Every binding then
        // built a terminal missing that field and answered with empty lists,
        // while Rust -- which never reads the file -- stayed green. Only the
        // scenarios that happened to use the new field caught it, and only in
        // the foreign suites. This catches any dropped field, in Rust, at once.
        let written: Config = serde_json::from_value(config_json(&scenario.config))
            .unwrap_or_else(|err| panic!("{}: config does not round-trip: {err}", scenario.name));
        let mut expected_config = scenario.config.clone();
        expected_config.layout.keybinds = Keybinds::default();
        assert_eq!(
            written, expected_config,
            "{}: the config a binding reads differs from the config Rust ran",
            scenario.name
        );

        let config_rel = format!("configs/{}.json", scenario.name);
        let expected_rel = format!("expected/{}.min.json", scenario.name);
        let commands_rel = format!("commands/{}.txt", scenario.name);

        write_or_compare(
            &format!("{dir}/{config_rel}"),
            &format!(
                "{}\n",
                serde_json::to_string_pretty(&config_json(&scenario.config)).unwrap()
            ),
            regen,
        );
        write_or_compare(&format!("{dir}/{expected_rel}"), &frame_min, regen);
        write_or_compare(
            &format!("{dir}/expected/{}.json", scenario.name),
            &format!("{}\n", serde_json::to_string_pretty(&frame).unwrap()),
            regen,
        );
        if let (Some(rel), Some(feed)) = (scenario.replay_path, scenario.feed.as_ref()) {
            write_or_compare(
                &format!("{dir}/{rel}"),
                &format!("{}\n", serde_json::to_string_pretty(feed).unwrap()),
                regen,
            );
        }

        // The commands live in a file of their own, one per line, rather than as
        // an array in the manifest. A command is a JSON string full of quotes,
        // and embedding it would fill the manifest with escapes -- which the two
        // bindings that deliberately carry no JSON dependency would then have to
        // unescape by hand. With the sequence in a text file, every value in the
        // manifest is a plain path and reading it needs no parser at all.
        write_or_compare(
            &format!("{dir}/{commands_rel}"),
            &format!(
                "{}
",
                scenario.commands.join(
                    "
"
                )
            ),
            regen,
        );

        manifest.push(serde_json::json!({
            "name": scenario.name,
            "config": config_rel,
            "expected": expected_rel,
            "commands": commands_rel,
        }));
    }

    write_or_compare(
        &format!("{dir}/manifest.json"),
        &format!(
            "{}\n",
            serde_json::to_string_pretty(&serde_json::json!({ "scenarios": manifest })).unwrap()
        ),
        regen,
    );

    // `config.json` at the root is what the first corpus shipped and what several
    // bindings still open by name. It stays a copy of the basic scenario's config
    // rather than a second source of truth.
    write_or_compare(
        &format!("{dir}/config.json"),
        &format!(
            "{}\n",
            serde_json::to_string_pretty(&config_json(&scenarios[0].config)).unwrap()
        ),
        regen,
    );
}

#[test]
fn a_drained_replay_holds_its_frame() {
    // The bindings tick a fixed count past the feed length and rely on the frame
    // being stable once the replay is drained; without that, every binding's
    // expected file would depend on exactly how far it over-ticked.
    let feed = canonical_feed();
    let config = replay_config(&feed);
    let mut terminal = Terminal::new(&config).unwrap();
    terminal.subscribe(0, &sym()).unwrap();
    for _ in 0..feed.len() {
        terminal.tick();
    }
    let drained = serde_json::to_string(&terminal.frame()).unwrap();
    for _ in 0..16 {
        terminal.tick();
    }
    assert_eq!(
        drained,
        serde_json::to_string(&terminal.frame()).unwrap(),
        "frame changed after the replay was exhausted"
    );
}

#[test]
fn seeking_back_reproduces_the_earlier_frame() {
    // What the `seek` scenario pins, asserted directly rather than only through
    // its fixture: rewinding and re-folding must land on the same state the
    // forward pass had at that point.
    let feed = canonical_feed();
    let config = replay_config(&feed);

    let mut forward = Terminal::new(&config).unwrap();
    forward.subscribe(0, &sym()).unwrap();
    for _ in 0..2 {
        forward.tick();
    }
    let at_two = serde_json::to_string(&forward.frame()).unwrap();

    let mut rewound = Terminal::new(&config).unwrap();
    rewound.subscribe(0, &sym()).unwrap();
    for _ in 0..feed.len() {
        rewound.tick();
    }
    rewound.seek(0, 2).unwrap();
    assert_eq!(
        at_two,
        serde_json::to_string(&rewound.frame()).unwrap(),
        "a seek to index 2 did not reproduce the frame after two ticks"
    );
}
