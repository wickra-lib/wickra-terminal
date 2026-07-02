//! The order-book panel.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use super::{Panel, PanelKind, DEFAULT_DEPTH};
use crate::source::{SourceId, Symbol};
use crate::state::AppState;
use crate::view::{BookView, Level, PanelView};

/// An L2 order book for the focused market.
pub struct BookPanel;

fn levels(side: Vec<(Decimal, Decimal)>) -> Vec<Level> {
    side.into_iter()
        .map(|(price, quantity)| Level {
            price: price.to_f64().unwrap_or(0.0),
            quantity: quantity.to_f64().unwrap_or(0.0),
        })
        .collect()
}

impl Panel for BookPanel {
    fn kind(&self) -> PanelKind {
        PanelKind::Book
    }

    fn view(&self, state: &AppState, focus: (SourceId, &Symbol)) -> PanelView {
        let symbol = focus.1.to_string();
        let book = match state.get(&(focus.0, focus.1.clone())) {
            Some(st) => BookView {
                symbol,
                bids: levels(st.book.top_bids(DEFAULT_DEPTH)),
                asks: levels(st.book.top_asks(DEFAULT_DEPTH)),
                spread: st.book.spread().and_then(|s| s.to_f64()),
            },
            None => BookView {
                symbol,
                bids: Vec::new(),
                asks: Vec::new(),
                spread: None,
            },
        };
        PanelView::Book(book)
    }
}
