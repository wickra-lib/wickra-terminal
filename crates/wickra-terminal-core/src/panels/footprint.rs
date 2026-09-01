//! The footprint (volume-profile) panel: volume traded at each price, split by
//! aggressor side.

use rust_decimal::prelude::ToPrimitive;

use super::{Panel, PanelKind};
use crate::source::{SourceId, Symbol};
use crate::state::AppState;
use crate::view::{FootprintLevel, FootprintView, PanelView};

/// A per-price buy/sell volume profile for the focused market.
#[derive(Debug)]
pub struct FootprintPanel {
    /// How many price levels this panel carries around the last trade.
    pub(crate) depth: usize,
}

impl Panel for FootprintPanel {
    fn kind(&self) -> PanelKind {
        PanelKind::Footprint
    }

    fn view(&self, state: &AppState, focus: (SourceId, &Symbol)) -> PanelView {
        let symbol = focus.1.to_string();
        let levels = state
            .get(&(focus.0, focus.1.clone()))
            .map(|st| {
                st.footprint
                    .around(st.last, self.depth)
                    .into_iter()
                    .map(|(price, buy, sell)| FootprintLevel {
                        price: price.to_f64().unwrap_or(0.0),
                        buy: buy.to_f64().unwrap_or(0.0),
                        sell: sell.to_f64().unwrap_or(0.0),
                    })
                    .collect()
            })
            .unwrap_or_default();
        PanelView::Footprint(FootprintView { symbol, levels })
    }
}

#[cfg(test)]
mod tests {
    use super::super::DEFAULT_DEPTH;
    use super::*;
    use rust_decimal_macros::dec;
    use wickra_exchange_core::{Event, OrderSide, TradePrint};

    fn trade(sym: &Symbol, price: rust_decimal::Decimal, side: OrderSide) -> Event {
        Event::Trade(TradePrint {
            symbol: sym.clone(),
            price,
            quantity: dec!(2),
            aggressor: side,
            timestamp: 0,
        })
    }

    #[test]
    fn the_panel_follows_the_market_rather_than_the_high() {
        // The symptom this fixes: the panel showed the highest prices ever
        // traded, so after a move it displayed a ladder the market had left. On a
        // synthetic 200k-print walk it read 513.03 down to 512.81 against a last
        // trade of 495.19.
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        for cents in 0..400 {
            let price = rust_decimal::Decimal::from(50_000 - cents) / dec!(100);
            state.fold(0, &sym, &trade_at(&sym, price));
        }

        let PanelView::Footprint(view) = FootprintPanel {
            depth: DEFAULT_DEPTH,
        }
        .view(&state, (0, &sym)) else {
            panic!("expected a footprint view");
        };
        let last = 496.01;
        assert_eq!(view.levels.len(), DEFAULT_DEPTH);
        for level in &view.levels {
            assert!(
                (level.price - last).abs() < 1.0,
                "the panel shows {} against a last of {last}",
                level.price
            );
        }
    }

    fn trade_at(sym: &Symbol, price: rust_decimal::Decimal) -> Event {
        Event::Trade(TradePrint {
            symbol: sym.clone(),
            price,
            quantity: dec!(1),
            aggressor: OrderSide::Buy,
            timestamp: 0,
        })
    }

    #[test]
    fn footprint_panel_splits_buy_and_sell_volume_by_price() {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        state.fold(0, &sym, &trade(&sym, dec!(100), OrderSide::Buy));
        state.fold(0, &sym, &trade(&sym, dec!(100), OrderSide::Sell));
        state.fold(0, &sym, &trade(&sym, dec!(101), OrderSide::Buy));

        let PanelView::Footprint(view) = FootprintPanel {
            depth: DEFAULT_DEPTH,
        }
        .view(&state, (0, &sym)) else {
            panic!("expected a footprint view");
        };
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        // Highest price first.
        assert!(close(view.levels[0].price, 101.0));
        assert!(close(view.levels[0].buy, 2.0));
        assert!(close(view.levels[0].sell, 0.0));
        assert!(close(view.levels[1].price, 100.0));
        assert!(close(view.levels[1].buy, 2.0));
        assert!(close(view.levels[1].sell, 2.0));
    }
}
