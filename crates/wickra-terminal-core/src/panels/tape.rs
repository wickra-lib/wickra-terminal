//! The time-and-sales (tape) panel.

use rust_decimal::prelude::ToPrimitive;

use super::{Panel, PanelKind};
use crate::source::{SourceId, Symbol};
use crate::state::AppState;
use crate::view::{PanelView, TapePrint, TapeView};
use wickra_exchange_core::OrderSide;

/// A rolling time-and-sales tape for the focused market.
#[derive(Debug)]
pub struct TapePanel {
    /// How many prints this panel carries. A renderer draws what fits and
    /// scrolls through the rest.
    pub(crate) rows: usize,
}

impl Panel for TapePanel {
    fn kind(&self) -> PanelKind {
        PanelKind::Tape
    }

    fn view(&self, state: &AppState, focus: (SourceId, &Symbol)) -> PanelView {
        let symbol = focus.1.to_string();
        let prints = state
            .get(&(focus.0, focus.1.clone()))
            .map(|st| {
                st.tape
                    .recent(self.rows)
                    .into_iter()
                    .map(|print| TapePrint {
                        price: print.price.to_f64().unwrap_or(0.0),
                        quantity: print.quantity.to_f64().unwrap_or(0.0),
                        side: match print.aggressor {
                            OrderSide::Buy => "buy".to_string(),
                            OrderSide::Sell => "sell".to_string(),
                        },
                        timestamp: print.timestamp,
                    })
                    .collect()
            })
            .unwrap_or_default();
        PanelView::Tape(TapeView { symbol, prints })
    }
}
