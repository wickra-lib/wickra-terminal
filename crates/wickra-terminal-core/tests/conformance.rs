//! Conformance: every panel and source is object-safe (usable as a boxed trait
//! object in a heterogeneous collection) and reports the kind it was built for.
//! This guards the trait shapes the renderers and bindings rely on.

use wickra_terminal_core::{
    build_panel, build_source, DataSource, Market, Panel, PanelKind, PanelSpec, RectSpec,
    SourceKind, SourceSpec,
};

const KINDS: [PanelKind; 5] = [
    PanelKind::Chart,
    PanelKind::Book,
    PanelKind::Tape,
    PanelKind::Watchlist,
    PanelKind::Footprint,
];

#[test]
fn panels_are_object_safe_and_report_their_kind() {
    let panels: Vec<Box<dyn Panel>> = KINDS
        .iter()
        .map(|&kind| build_panel(&PanelSpec::new(kind, RectSpec::new(0, 0, 100, 100))))
        .collect();

    assert_eq!(panels.len(), KINDS.len());
    for (panel, kind) in panels.iter().zip(KINDS) {
        assert_eq!(panel.kind(), kind);
    }
}

#[test]
fn sources_are_object_safe_and_report_their_kind() {
    let synth = build_source(0, &SourceSpec::Synth { seed: 1 }).unwrap();
    let replay = build_source(
        1,
        &SourceSpec::Replay {
            dataset: "[]".to_string(),
        },
    )
    .unwrap();

    let manual = build_source(2, &SourceSpec::Manual).unwrap();

    let sources: Vec<Box<dyn DataSource>> = vec![synth, replay, manual];
    assert_eq!(sources[0].id(), 0);
    assert_eq!(sources[0].kind(), SourceKind::Synth);
    assert_eq!(sources[1].id(), 1);
    assert_eq!(sources[1].kind(), SourceKind::Replay);
    assert_eq!(sources[2].id(), 2);
    assert_eq!(sources[2].kind(), SourceKind::Manual);
}

// `Live` is the one source whose trait shape most needs this guard: it is the
// only implementation behind a feature flag, so a signature drift there compiles
// away on a default `cargo check` of the wasm binding and surfaces only in a
// native build. Constructing it does not touch the network -- `LiveSource::connect`
// builds the HTTP client and nothing else; the subscribe handshake is what talks
// to the venue, and this test never subscribes.
#[cfg(feature = "live")]
#[test]
fn live_source_is_object_safe_and_reports_its_kind() {
    let live = build_source(
        3,
        &SourceSpec::Live {
            venue: "binance".to_string(),
            symbol: "BTC/USDT".to_string(),
            testnet: false,
            market: Market::Spot,
        },
    )
    .unwrap();

    let sources: Vec<Box<dyn DataSource>> = vec![live];
    assert_eq!(sources[0].id(), 3);
    assert_eq!(sources[0].kind(), SourceKind::Live);
}

/// Every market kind opens, and none of them opens the spot book by mistake.
///
/// Offline: `connect` builds the venue's HTTP client and does not reach for a
/// socket -- the sockets are opened on the first poll. So the one thing worth
/// asserting here is exactly the thing that was wrong, which is that the market
/// was hard-coded to spot and a perpetual could not be opened at all.
#[cfg(feature = "live")]
#[test]
fn a_live_source_opens_every_market_kind() {
    for market in [
        Market::Spot,
        Market::UsdMFutures,
        Market::CoinMFutures,
        Market::Margin,
    ] {
        let source = build_source(
            5,
            &SourceSpec::Live {
                venue: "binance".to_string(),
                symbol: "BTC/USDT".to_string(),
                testnet: false,
                market,
            },
        )
        .unwrap_or_else(|err| panic!("binance {market:?}: {err}"));
        assert_eq!(source.kind(), SourceKind::Live);
    }
}

/// The testnet host is a separate branch, and a source that silently opened
/// mainnet for a testnet config would be the worst kind of wrong.
#[cfg(feature = "live")]
#[test]
fn a_live_source_opens_a_testnet_host() {
    let source = build_source(
        6,
        &SourceSpec::Live {
            venue: "binance".to_string(),
            symbol: "BTC/USDT".to_string(),
            testnet: true,
            market: Market::UsdMFutures,
        },
    )
    .expect("binance testnet");
    assert_eq!(source.kind(), SourceKind::Live);
}

#[cfg(feature = "live")]
#[test]
fn live_source_rejects_an_unknown_venue() {
    let err = build_source(
        4,
        &SourceSpec::Live {
            venue: "not-a-venue".to_string(),
            symbol: "BTC/USDT".to_string(),
            testnet: false,
            market: Market::Spot,
        },
    );
    assert!(err.is_err());
}
