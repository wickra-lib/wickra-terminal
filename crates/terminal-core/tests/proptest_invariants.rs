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
use terminal_core::{AppState, CandleBuilder, Event, Symbol, Timeframe};
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
