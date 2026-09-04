//! The watchlist panel — every tracked market at a glance.

use rust_decimal::prelude::ToPrimitive;

use super::{Panel, PanelKind};
use crate::source::{SourceId, Symbol};
use crate::state::AppState;
use crate::view::{PanelView, WatchRow, WatchlistView};

/// A multi-market watchlist. Unlike the other panels it spans every tracked
/// market, not just the focused one, so it ignores `focus`.
#[derive(Debug)]
pub struct WatchlistPanel;

impl Panel for WatchlistPanel {
    fn kind(&self) -> PanelKind {
        PanelKind::Watchlist
    }

    fn view(&self, state: &AppState, _focus: (SourceId, &Symbol)) -> PanelView {
        let rows = state
            .watchlist
            .iter()
            .map(|key| {
                let Some(market) = state.get(key) else {
                    // Subscribed, nothing folded yet. The row stays so the
                    // layout does not jump when the first print arrives.
                    return WatchRow {
                        source: key.0,
                        symbol: key.1.to_string(),
                        last: 0.0,
                        bid: 0.0,
                        ask: 0.0,
                        volume: 0.0,
                        change: 0.0,
                    };
                };
                let last = market.last.to_f64().unwrap_or(0.0);
                // Tested on the decimal rather than on the converted float: a
                // zero open means nothing has been folded yet, and dividing by
                // it would report an infinite move on the first print.
                let change = if market.open.is_zero() {
                    0.0
                } else {
                    let open = market.open.to_f64().unwrap_or(0.0);
                    (last - open) / open * 100.0
                };
                WatchRow {
                    source: key.0,
                    symbol: key.1.to_string(),
                    last,
                    bid: market.bid.to_f64().unwrap_or(0.0),
                    ask: market.ask.to_f64().unwrap_or(0.0),
                    volume: market.volume.to_f64().unwrap_or(0.0),
                    change,
                }
            })
            .collect();
        PanelView::Watchlist(WatchlistView { rows })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use wickra_exchange_core::{OrderSide, Ticker, TradePrint};

    fn trade(sym: &Symbol, price: rust_decimal::Decimal) -> wickra_exchange_core::Event {
        wickra_exchange_core::Event::Trade(TradePrint {
            symbol: sym.clone(),
            price,
            quantity: dec!(1),
            aggressor: OrderSide::Buy,
            timestamp: 0,
        })
    }

    fn rows(state: &AppState) -> Vec<WatchRow> {
        let sym = Symbol::new("BTC", "USDT");
        let PanelView::Watchlist(view) = WatchlistPanel.view(state, (0, &sym)) else {
            panic!("the watchlist panel answers with a watchlist")
        };
        view.rows
    }

    /// The row carries what the ticker brought, and a change measured from the
    /// open rather than from the previous frame.
    #[test]
    fn a_row_reports_the_quote_the_turnover_and_the_move() {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        state.watchlist.push((0, sym.clone()));
        state.fold(0, &sym, &trade(&sym, dec!(100)));
        state.fold(
            0,
            &sym,
            &wickra_exchange_core::Event::Ticker(Ticker {
                symbol: sym.clone(),
                last: dec!(110),
                bid: dec!(109),
                ask: dec!(111),
                volume: dec!(5000),
                timestamp: 0,
            }),
        );

        let rows = rows(&state);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert!((row.last - 110.0).abs() < 1e-9);
        assert!((row.bid - 109.0).abs() < 1e-9);
        assert!((row.ask - 111.0).abs() < 1e-9);
        assert!((row.volume - 5000.0).abs() < 1e-9);
        // 100 -> 110 is ten percent, and it is signed.
        assert!((row.change - 10.0).abs() < 1e-9, "change: {}", row.change);
    }

    /// A market that is subscribed and has folded nothing keeps its row.
    ///
    /// Rows that appear and disappear as a session warms up make a renderer
    /// relayout for no reason, and a change computed against an open of zero
    /// would be an infinity rather than a number.
    #[test]
    fn a_subscribed_market_with_no_events_is_a_zeroed_row() {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        state.watchlist.push((0, sym));

        let rows = rows(&state);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert!(row.last.abs() < f64::EPSILON);
        assert!(row.bid.abs() < f64::EPSILON);
        assert!(row.ask.abs() < f64::EPSILON);
        assert!(row.volume.abs() < f64::EPSILON);
        assert!(row.change.abs() < f64::EPSILON);
    }

    /// A market that has traded but never tickered reports no quote.
    ///
    /// The change is still real: it comes from the prints, which is the whole
    /// reason the open is folded from any price rather than from the ticker.
    #[test]
    fn trades_without_a_ticker_still_report_a_change() {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        state.watchlist.push((0, sym.clone()));
        state.fold(0, &sym, &trade(&sym, dec!(200)));
        state.fold(0, &sym, &trade(&sym, dec!(150)));

        let row = rows(&state).remove(0);
        assert!(
            row.bid.abs() < f64::EPSILON,
            "a quote appeared from nowhere"
        );
        assert!(
            row.ask.abs() < f64::EPSILON,
            "a quote appeared from nowhere"
        );
        assert!((row.change + 25.0).abs() < 1e-9, "change: {}", row.change);
    }
}
