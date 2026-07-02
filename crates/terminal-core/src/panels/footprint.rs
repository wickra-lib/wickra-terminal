//! The footprint (volume-profile) panel: volume traded at each price, split by
//! aggressor side.

use rust_decimal::prelude::ToPrimitive;

use super::{Panel, PanelKind, DEFAULT_DEPTH};
use crate::source::{SourceId, Symbol};
use crate::state::AppState;
use crate::view::{FootprintLevel, FootprintView, PanelView};

/// A per-price buy/sell volume profile for the focused market.
pub struct FootprintPanel;

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
                    .top(DEFAULT_DEPTH)
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
    fn footprint_panel_splits_buy_and_sell_volume_by_price() {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        state.fold(0, &sym, &trade(&sym, dec!(100), OrderSide::Buy));
        state.fold(0, &sym, &trade(&sym, dec!(100), OrderSide::Sell));
        state.fold(0, &sym, &trade(&sym, dec!(101), OrderSide::Buy));

        let PanelView::Footprint(view) = FootprintPanel.view(&state, (0, &sym)) else {
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
