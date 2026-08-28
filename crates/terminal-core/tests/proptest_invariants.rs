//! Property tests: fold arbitrary event streams into `AppState` and assert the
//! invariants the renderers and the indicators rely on.
//!
//! The bar to clear here is that a property can fail. One of these used to
//! generate bids below 500 and asks above it, so a crossed book was impossible
//! by construction and the assertion could only ever pass — it proved the
//! accessors returned something, not that the fold maintained anything. Its
//! replacement generates overlapping ranges, so crossed books actually occur and
//! the properties have to say what is true when they do.

use proptest::prelude::*;
use rust_decimal::Decimal;
use terminal_core::{
    AppState, CandleBuilder, Config, Event, IndicatorSpec, SourceSpec, Symbol, Terminal, Timeframe,
};
use wickra_exchange_core::{BookLevel, OrderBookSnapshot, OrderSide, TradePrint};

/// The default tape-ring cap in `SymbolState` (mirrored from the core).
const TAPE_CAP: usize = 256;

fn trade(sym: &Symbol, price: u32, buy: bool) -> Event {
    Event::Trade(TradePrint {
        symbol: sym.clone(),
        price: Decimal::from(price),
        quantity: Decimal::from(1),
        aggressor: if buy { OrderSide::Buy } else { OrderSide::Sell },
        timestamp: 0,
    })
}

fn snapshot(sym: &Symbol, bids: &[u32], asks: &[u32]) -> Event {
    Event::BookSnapshot(OrderBookSnapshot {
        symbol: sym.clone(),
        last_update_id: 1,
        bids: bids
            .iter()
            .map(|&p| BookLevel::new(Decimal::from(p), Decimal::from(1)))
            .collect(),
        asks: asks
            .iter()
            .map(|&p| BookLevel::new(Decimal::from(p), Decimal::from(1)))
            .collect(),
    })
}

proptest! {
    #[test]
    fn tape_never_exceeds_its_cap(prices in prop::collection::vec(1u32..10_000, 0..600)) {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        for (i, price) in prices.iter().enumerate() {
            state.fold(0, &sym, &trade(&sym, *price, i % 2 == 0));
        }
        if let Some(st) = state.get(&(0, sym)) {
            prop_assert!(st.tape.len() <= TAPE_CAP);
        }
    }

    #[test]
    fn last_price_tracks_the_final_trade(prices in prop::collection::vec(1u32..10_000, 1..200)) {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        for price in &prices {
            state.fold(0, &sym, &trade(&sym, *price, true));
        }
        let st = state.get(&(0, sym)).unwrap();
        prop_assert_eq!(st.last, Decimal::from(*prices.last().unwrap()));
    }

    #[test]
    fn the_book_sides_stay_ordered_even_when_they_cross(
        bids in prop::collection::vec(1u32..1000, 1..30),
        asks in prop::collection::vec(1u32..1000, 1..30),
    ) {
        // Overlapping ranges on purpose: a crossed book is an ordinary thing to
        // see between a snapshot and the diffs that follow it, and the ordering
        // of each side is what holds regardless.
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        state.fold(0, &sym, &snapshot(&sym, &bids, &asks));
        let st = state.get(&(0, sym)).unwrap();

        let top_bids = st.book.top_bids(50);
        let top_asks = st.book.top_asks(50);
        prop_assert!(top_bids.windows(2).all(|w| w[0].0 > w[1].0), "bids not descending");
        prop_assert!(top_asks.windows(2).all(|w| w[0].0 < w[1].0), "asks not ascending");

        // `best_*` must agree with the ordered view rather than being a second,
        // independently-derived answer.
        prop_assert_eq!(st.book.best_bid().map(|(p, _)| p), top_bids.first().map(|(p, _)| *p));
        prop_assert_eq!(st.book.best_ask().map(|(p, _)| p), top_asks.first().map(|(p, _)| *p));
    }

    #[test]
    fn the_core_conversion_accepts_exactly_the_books_the_core_would(
        // Overlapping but skewed, so both answers are common: drawn from one
        // range, valid books were 6 cases in 257 and the accepting branch was
        // barely exercised.
        bids in prop::collection::vec(1u32..600, 0..20),
        asks in prop::collection::vec(400u32..1000, 0..20),
    ) {
        // The indicators read `to_core()`, and wickra-core rejects a book that is
        // one-sided or crossed. This pins the terminal's answer to the core's
        // rule rather than letting the two drift into disagreeing.
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        state.fold(0, &sym, &snapshot(&sym, &bids, &asks));
        let st = state.get(&(0, sym)).unwrap();

        let expected = match (st.book.best_bid(), st.book.best_ask()) {
            (Some((bid, _)), Some((ask, _))) => bid < ask,
            _ => false,
        };
        prop_assert_eq!(st.book.to_core().is_some(), expected);
    }

    #[test]
    fn the_footprint_conserves_traded_volume(
        prices in prop::collection::vec(1u32..50, 1..200),
    ) {
        // A conservation law: every print's quantity lands in exactly one price
        // bucket, so the buckets must sum to what was fed. Repeated prices at the
        // same price are the interesting case, which the narrow price range makes
        // frequent.
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        for (i, price) in prices.iter().enumerate() {
            state.fold(0, &sym, &trade(&sym, *price, i % 3 != 0));
        }
        let st = state.get(&(0, sym)).unwrap();
        let booked: Decimal = st
            .footprint
            .top(st.footprint.len())
            .into_iter()
            .map(|(_, buy, sell)| buy + sell)
            .sum();
        // Each synthetic print carries a quantity of one.
        prop_assert_eq!(booked, Decimal::from(prices.len() as u32));
    }
}

proptest! {
    #[test]
    fn closed_bars_are_ordered_and_aligned_to_the_timeframe(
        steps in prop::collection::vec(0i64..5_000, 1..300),
    ) {
        // The bars feed every candle indicator, so two things have to hold: they
        // arrive in order, and each one starts on a timeframe boundary. An
        // out-of-order or unaligned bar desynchronises every indicator behind it.
        let timeframe = Timeframe::parse("1s").expect("1s is a valid timeframe");
        let mut builder = CandleBuilder::new(timeframe);
        let mut closed = Vec::new();
        let mut ts = 0i64;
        for (i, step) in steps.iter().enumerate() {
            ts = ts.saturating_add(*step);
            let price = 100.0 + (i % 17) as f64;
            if let Some(bar) = builder.update(price, 1.0, ts) {
                closed.push(bar);
            }
        }
        for bar in &closed {
            prop_assert_eq!(bar.timestamp, timeframe.bucket(bar.timestamp));
            prop_assert!(bar.high >= bar.low);
            prop_assert!(bar.high >= bar.open && bar.high >= bar.close);
            prop_assert!(bar.low <= bar.open && bar.low <= bar.close);
        }
        prop_assert!(
            closed.windows(2).all(|w| w[0].timestamp < w[1].timestamp),
            "closed bars are not strictly increasing in time"
        );
    }
}

proptest! {
    #[test]
    fn seeking_back_to_a_point_rebuilds_the_state_it_had_there(
        // Two markets on ONE source, so the whole feed is inside what a seek
        // resets and replays. A reference on another source is a different case:
        // that source is neither reset nor replayed, so its price cannot be
        // reconstructed and the indicator restarts rather than reporting a
        // number built from a present-day price. See `Terminal::seek`.
        steps in prop::collection::vec(1u32..400, 60..120),
        cursor in 20usize..40,
    ) {
        let sym_a = Symbol::new("BTC", "USDT");
        let sym_b = Symbol::new("ETH", "USDT");
        let mut feed = Vec::new();
        for (i, step) in (0_i64..).zip(steps.iter()) {
            let ts = i * 2;
            feed.push(Event::Trade(TradePrint {
                symbol: sym_b.clone(),
                price: Decimal::from(500 + *step),
                quantity: Decimal::from(1),
                aggressor: OrderSide::Buy,
                timestamp: ts,
            }));
            feed.push(Event::Trade(TradePrint {
                symbol: sym_a.clone(),
                price: Decimal::from(100 + *step),
                quantity: Decimal::from(1),
                aggressor: OrderSide::Buy,
                timestamp: ts + 1,
            }));
        }

        let mut config = Config::default_layout();
        config.sources = vec![SourceSpec::Replay {
            dataset: serde_json::to_string(&feed).expect("the feed serialises"),
        }];
        // A pairwise indicator, because that is the family a seek used to get
        // wrong: it reads another market's price, which a re-fold has to
        // reconstruct rather than read from the present.
        config.indicators = vec![IndicatorSpec::paired(
            "RollingCorrelation",
            vec![14.0],
            "ETH/USDT",
        )];

        let mut terminal = Terminal::new(&config).expect("the config is accepted");
        terminal.subscribe(0, &sym_a).expect("BTC subscribes");
        terminal.subscribe(0, &sym_b).expect("ETH subscribes");

        let mut at_cursor = String::new();
        for _ in 0..cursor {
            at_cursor = terminal
                .command_json(r#"{"type":"Tick"}"#)
                .expect("a tick is accepted");
        }
        // Drive well past it, then come back.
        for _ in 0..40 {
            terminal
                .command_json(r#"{"type":"Tick"}"#)
                .expect("a tick is accepted");
        }
        // The frame the seek itself returns, not the one after another tick:
        // seeking to `cursor` leaves exactly `cursor` events folded, so ticking
        // again would fold one more and compare two different points.
        let rebuilt = terminal
            .command_json(&format!(r#"{{"type":"Seek","source":0,"index":{cursor}}}"#))
            .expect("the replay source seeks");

        prop_assert_eq!(
            at_cursor,
            rebuilt,
            "seeking back to {} did not rebuild the frame it had there",
            cursor
        );
    }
}

#[test]
fn a_cross_source_reference_is_absent_after_a_seek_rather_than_stale() {
    // The case the scope exists for. `seek` resets and replays only the seeked
    // source, so a reference market living on ANOTHER source is neither reset
    // nor replayed. Reading it unscoped paired every historical tick with its
    // present-day price: a correlation of 0.88 became 0.0 after seeking to the
    // position it was already at, while the whole justification for re-folding
    // rather than snapshotting is that it rebuilds identical state.
    //
    // It cannot be rebuilt, so it must be absent rather than wrong.
    let btc = Symbol::new("BTC", "USDT");
    let eth = Symbol::new("ETH", "USDT");
    let trades = |symbol: &Symbol, base: u32, offset: i64| -> Vec<Event> {
        (0..60_i64)
            .map(|i| {
                Event::Trade(TradePrint {
                    symbol: symbol.clone(),
                    price: Decimal::from(base + u32::try_from(i % 17).unwrap_or(0)),
                    quantity: Decimal::from(1),
                    aggressor: OrderSide::Buy,
                    timestamp: i * 2 + offset,
                })
            })
            .collect()
    };

    let mut config = Config::default_layout();
    config.sources = vec![
        SourceSpec::Replay {
            dataset: serde_json::to_string(&trades(&btc, 100, 2)).expect("serialises"),
        },
        SourceSpec::Replay {
            dataset: serde_json::to_string(&trades(&eth, 500, 1)).expect("serialises"),
        },
    ];
    config.indicators = vec![IndicatorSpec::paired(
        "RollingCorrelation",
        vec![14.0],
        "ETH/USDT",
    )];

    let mut terminal = Terminal::new(&config).expect("the config is accepted");
    terminal.subscribe(0, &btc).expect("BTC subscribes");
    terminal.subscribe(1, &eth).expect("ETH subscribes");
    for _ in 0..60 {
        terminal
            .command_json(r#"{"type":"Tick"}"#)
            .expect("a tick is accepted");
    }

    let rebuilt = terminal
        .command_json(r#"{"type":"Seek","source":0,"index":40}"#)
        .expect("the replay source seeks");
    let value = rebuilt
        .split(r#""name":"RollingCorrelation(14) vs ETH/USDT","value":"#)
        .nth(1)
        .and_then(|rest| rest.split([',', '}']).next())
        .expect("the indicator is in the frame");
    assert_eq!(
        value, "null",
        "a cross-source reference survived a seek, so the reading was rebuilt from a          present-day price rather than a historical one"
    );
}
