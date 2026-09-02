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

#[cfg(test)]
mod tests {
    use super::super::DEFAULT_DEPTH;
    use super::*;

    /// A market the state has never folded still answers with a book.
    ///
    /// Focus moves to a market the moment it is subscribed, which is before its
    /// first depth message arrives -- so this is the view every live market is
    /// drawn from for its first frame. A renderer handed nothing there would
    /// have to guess, and an empty spread is not the same as a spread of zero:
    /// one says no quote has arrived, the other says the book is locked.
    #[test]
    fn a_market_with_no_state_yet_has_an_empty_book_and_no_spread() {
        let sym = Symbol::new("BTC", "USDT");
        let state = AppState::default();

        // `find_map` rather than a refutable `let`: the `else` arm of a `let`
        // that destructures the one variant this panel returns is a line no run
        // can reach, and an unreachable arm inside a test reads like tested
        // code.
        let view = [(BookPanel {
            depth: DEFAULT_DEPTH,
        })
        .view(&state, (0, &sym))]
        .into_iter()
        .find_map(|panel| match panel {
            PanelView::Book(view) => Some(view),
            _ => None,
        })
        .expect("the book panel answers with a book");

        assert_eq!(view.symbol, "BTC/USDT");
        assert!(view.bids.is_empty());
        assert!(view.asks.is_empty());
        assert!(view.spread.is_none(), "a spread appeared from nowhere");
    }
}
