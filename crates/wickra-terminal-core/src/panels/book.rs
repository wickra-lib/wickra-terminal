//! The order-book panel.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use super::{Panel, PanelKind};
use crate::source::{SourceId, Symbol};
use crate::state::AppState;
use crate::view::{BookView, Level, PanelView};

/// An L2 order book for the focused market.
#[derive(Debug)]
pub struct BookPanel {
    /// How many levels a side this panel carries. A renderer draws what fits
    /// and scrolls through the rest.
    pub(crate) depth: usize,
}

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
                bids: levels(st.book.top_bids(self.depth)),
                asks: levels(st.book.top_asks(self.depth)),
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
