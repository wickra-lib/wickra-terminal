//! Property tests: fold arbitrary event streams into `AppState` and assert the
//! invariants the renderers rely on — the tape ring never exceeds its cap, the
//! book stays consistent (best bid below best ask), and nothing panics.

use proptest::prelude::*;
use rust_decimal::Decimal;
use terminal_core::{AppState, Event, Symbol};
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
    fn book_best_bid_is_below_best_ask(
        bids in prop::collection::vec(1u32..500, 1..30),
        asks in prop::collection::vec(501u32..1000, 1..30),
    ) {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        state.fold(0, &sym, &snapshot(&sym, &bids, &asks));
        let st = state.get(&(0, sym)).unwrap();
        if let (Some((best_bid, _)), Some((best_ask, _))) =
            (st.book.best_bid(), st.book.best_ask())
        {
            // Bids are all < 500 and asks all > 500, so the book is crossed-free
            // and the top-of-book ordering holds.
            prop_assert!(best_bid < best_ask);
        }
    }
}
