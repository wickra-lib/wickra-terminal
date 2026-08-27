//! Tick-to-OHLCV aggregation.
//!
//! The terminal folds a stream of individual trades; more than half of the
//! Wickra indicator set consumes a [`Candle`] instead. This module is the bridge:
//! a [`CandleBuilder`] accumulates trades into bars of one [`Timeframe`] and
//! emits each bar once it is closed.
//!
//! Only *closed* bars reach the indicators. Feeding the bar in progress would
//! make every value repaint as the bar fills — the last print of a minute would
//! silently rewrite the reading the previous print produced — so the partial bar
//! is exposed separately, for renderers that want to draw a forming candle, and
//! is never fed to an indicator.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use wickra_core::Candle;

use crate::error::{Error, Result};

/// Milliseconds in one second, minute, hour and day.
const SECOND_MS: i64 = 1_000;
const MINUTE_MS: i64 = 60 * SECOND_MS;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const DAY_MS: i64 = 24 * HOUR_MS;

/// A bar duration, written the way venues write it: `30s`, `1m`, `5m`, `4h`, `1d`.
///
/// Serialises as that same string, so a config round-trips as
/// `"timeframe": "1m"` rather than as a millisecond count nobody would read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timeframe {
    millis: i64,
}

impl Timeframe {
    /// Parse the compact venue notation: an integer count and a unit suffix, one
    /// of `s`, `m`, `h`, `d`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if the string is not `<positive integer><unit>`,
    /// or if the resulting duration would overflow.
    pub fn parse(text: &str) -> Result<Self> {
        let trimmed = text.trim();
        let split = trimmed.len().checked_sub(1).filter(|_| trimmed.is_ascii());
        let Some(split) = split else {
            return Err(Error::Config(format!("invalid timeframe: {text:?}")));
        };
        let (count, unit) = trimmed.split_at(split);
        let unit_ms = match unit {
            "s" => SECOND_MS,
            "m" => MINUTE_MS,
            "h" => HOUR_MS,
            "d" => DAY_MS,
            _ => {
                return Err(Error::Config(format!(
                    "invalid timeframe unit {unit:?} in {text:?} (expected s, m, h or d)"
                )))
            }
        };
        let count: i64 = count.parse().map_err(|_| {
            Error::Config(format!(
                "invalid timeframe count {count:?} in {text:?} (expected a positive integer)"
            ))
        })?;
        if count <= 0 {
            return Err(Error::Config(format!(
                "invalid timeframe {text:?}: the count must be positive"
            )));
        }
        let millis = count.checked_mul(unit_ms).ok_or_else(|| {
            Error::Config(format!("invalid timeframe {text:?}: duration overflows"))
        })?;
        Ok(Self { millis })
    }

    /// The bar duration in milliseconds.
    #[must_use]
    pub const fn millis(self) -> i64 {
        self.millis
    }

    /// The opening timestamp of the bar `ts` falls into.
    ///
    /// Uses Euclidean division so pre-epoch timestamps floor downwards like every
    /// other one, rather than truncating towards zero and putting a negative
    /// timestamp in the *following* bar.
    #[must_use]
    pub const fn bucket(self, ts: i64) -> i64 {
        ts.div_euclid(self.millis) * self.millis
    }

    /// The compact notation this timeframe was parsed from, normalised to the
    /// largest unit that divides it exactly.
    #[must_use]
    pub fn label(self) -> String {
        for (unit_ms, suffix) in [(DAY_MS, 'd'), (HOUR_MS, 'h'), (MINUTE_MS, 'm')] {
            if self.millis % unit_ms == 0 {
                return format!("{}{suffix}", self.millis / unit_ms);
            }
        }
        format!("{}s", self.millis / SECOND_MS)
    }
}

impl Default for Timeframe {
    /// One minute — short enough to fill quickly on a live feed, long enough that
    /// a chart of a few hundred bars covers a session.
    fn default() -> Self {
        Self { millis: MINUTE_MS }
    }
}

impl Serialize for Timeframe {
    fn serialize<S: Serializer>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.label())
    }
}

impl<'de> Deserialize<'de> for Timeframe {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> core::result::Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// Accumulates trades into OHLCV bars of one timeframe.
///
/// Each [`update`](CandleBuilder::update) folds one trade. When a trade lands in
/// a later bar than the one in progress, the finished bar is returned and the new
/// one opens with that trade.
#[derive(Debug, Clone)]
pub struct CandleBuilder {
    timeframe: Timeframe,
    /// The bar in progress: its opening timestamp and accumulating OHLCV.
    open_ts: Option<i64>,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

impl CandleBuilder {
    /// A builder for bars of `timeframe`, with no bar in progress.
    #[must_use]
    pub const fn new(timeframe: Timeframe) -> Self {
        Self {
            timeframe,
            open_ts: None,
            open: 0.0,
            high: 0.0,
            low: 0.0,
            close: 0.0,
            volume: 0.0,
        }
    }

    /// The timeframe this builder bars at.
    #[must_use]
    pub const fn timeframe(&self) -> Timeframe {
        self.timeframe
    }

    /// Fold one trade.
    ///
    /// Returns the bar that just closed, if this trade opened a new one. A trade
    /// that lands in the bar in progress extends it and returns `None`.
    ///
    /// Out-of-order trades — a print stamped inside a bar that has already closed
    /// — are folded into the current bar rather than reopening the old one, which
    /// would emit bars out of order and desynchronise every indicator behind it.
    pub fn update(&mut self, price: f64, quantity: f64, timestamp: i64) -> Option<Candle> {
        let bucket = self.timeframe.bucket(timestamp);
        match self.open_ts {
            None => {
                self.start(bucket, price, quantity);
                None
            }
            Some(current) if bucket > current => {
                let closed = self.finish(current);
                self.start(bucket, price, quantity);
                Some(closed)
            }
            Some(_) => {
                self.high = self.high.max(price);
                self.low = self.low.min(price);
                self.close = price;
                self.volume += quantity;
                None
            }
        }
    }

    /// The bar currently accumulating, or `None` before the first trade.
    ///
    /// This is the forming bar a renderer may want to draw. It is deliberately
    /// not what indicators are fed.
    #[must_use]
    pub fn partial(&self) -> Option<Candle> {
        self.open_ts.map(|ts| self.finish(ts))
    }

    fn start(&mut self, bucket: i64, price: f64, quantity: f64) {
        self.open_ts = Some(bucket);
        self.open = price;
        self.high = price;
        self.low = price;
        self.close = price;
        self.volume = quantity;
    }

    /// Build the candle for the bar opened at `ts`.
    ///
    /// `new_unchecked` rather than `new`: the invariants `Candle::new` validates
    /// (finite prices, `low <= open/close <= high`, non-negative volume) hold by
    /// construction here — `high` and `low` are running max/min over the same
    /// prices `open` and `close` are drawn from, and volume only ever accumulates
    /// non-negative quantities. Going through the checked constructor would add an
    /// error arm no input can reach.
    fn finish(&self, ts: i64) -> Candle {
        Candle::new_unchecked(self.open, self.high, self.low, self.close, self.volume, ts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assert two prices/volumes are equal.
    ///
    /// Every value under test is either stored verbatim from the input or a sum
    /// of small integers, so exact equality would be correct -- but
    /// `clippy::float_cmp` is on, and dotting `#[allow]` through the suite to say
    /// so would be worse than one helper that says it once. `#[track_caller]`
    /// keeps the failure pointing at the assertion, not at this line.
    #[track_caller]
    fn eq(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < f64::EPSILON,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn parses_every_unit() {
        assert_eq!(Timeframe::parse("30s").unwrap().millis(), 30_000);
        assert_eq!(Timeframe::parse("1m").unwrap().millis(), 60_000);
        assert_eq!(Timeframe::parse("4h").unwrap().millis(), 14_400_000);
        assert_eq!(Timeframe::parse("1d").unwrap().millis(), 86_400_000);
    }

    #[test]
    fn parse_rejects_malformed_input() {
        for bad in ["", "m", "0m", "-5m", "1w", "1", "abc", "1.5m"] {
            assert!(Timeframe::parse(bad).is_err(), "{bad:?} should not parse");
        }
    }

    #[test]
    fn label_round_trips_through_parse() {
        for text in ["30s", "1m", "15m", "4h", "1d"] {
            let tf = Timeframe::parse(text).unwrap();
            assert_eq!(tf.label(), text);
            assert_eq!(Timeframe::parse(&tf.label()).unwrap(), tf);
        }
    }

    #[test]
    fn serialises_as_its_label() {
        let tf = Timeframe::parse("5m").unwrap();
        let json = serde_json::to_string(&tf).unwrap();
        assert_eq!(json, "\"5m\"");
        assert_eq!(serde_json::from_str::<Timeframe>(&json).unwrap(), tf);
    }

    #[test]
    fn default_is_one_minute() {
        assert_eq!(Timeframe::default(), Timeframe::parse("1m").unwrap());
    }

    #[test]
    fn bucket_floors_towards_negative_infinity() {
        let tf = Timeframe::parse("1m").unwrap();
        assert_eq!(tf.bucket(0), 0);
        assert_eq!(tf.bucket(59_999), 0);
        assert_eq!(tf.bucket(60_000), 60_000);
        // A pre-epoch stamp must land in the bar that contains it, not the next.
        assert_eq!(tf.bucket(-1), -60_000);
        assert_eq!(tf.bucket(-60_000), -60_000);
    }

    #[test]
    fn first_trade_opens_a_bar_and_emits_nothing() {
        let mut builder = CandleBuilder::new(Timeframe::parse("1m").unwrap());
        assert!(builder.update(100.0, 1.0, 0).is_none());
        let partial = builder.partial().unwrap();
        eq(partial.open, 100.0);
        eq(partial.close, 100.0);
        eq(partial.volume, 1.0);
        assert_eq!(partial.timestamp, 0);
    }

    #[test]
    fn trades_within_one_bar_accumulate() {
        let mut builder = CandleBuilder::new(Timeframe::parse("1m").unwrap());
        builder.update(100.0, 1.0, 0);
        builder.update(105.0, 2.0, 10_000);
        builder.update(95.0, 3.0, 20_000);
        builder.update(101.0, 4.0, 30_000);
        let bar = builder.partial().unwrap();
        eq(bar.open, 100.0);
        eq(bar.high, 105.0);
        eq(bar.low, 95.0);
        eq(bar.close, 101.0);
        eq(bar.volume, 10.0);
    }

    #[test]
    fn crossing_a_boundary_emits_the_closed_bar() {
        let mut builder = CandleBuilder::new(Timeframe::parse("1m").unwrap());
        builder.update(100.0, 1.0, 0);
        builder.update(110.0, 1.0, 30_000);
        let closed = builder
            .update(120.0, 5.0, 60_000)
            .expect("bar should close");
        eq(closed.open, 100.0);
        eq(closed.high, 110.0);
        eq(closed.low, 100.0);
        eq(closed.close, 110.0);
        eq(closed.volume, 2.0);
        assert_eq!(closed.timestamp, 0);

        // The new bar opened with the trade that closed the old one.
        let partial = builder.partial().unwrap();
        assert_eq!(partial.timestamp, 60_000);
        eq(partial.open, 120.0);
        eq(partial.volume, 5.0);
    }

    #[test]
    fn skipping_empty_bars_does_not_emit_them() {
        // A quiet market produces no trades, so no bars: the next print jumps
        // straight to its own bucket rather than back-filling the gap.
        let mut builder = CandleBuilder::new(Timeframe::parse("1m").unwrap());
        builder.update(100.0, 1.0, 0);
        let closed = builder
            .update(100.0, 1.0, 600_000)
            .expect("bar should close");
        assert_eq!(closed.timestamp, 0);
        assert_eq!(builder.partial().unwrap().timestamp, 600_000);
    }

    #[test]
    fn an_out_of_order_trade_extends_the_current_bar() {
        let mut builder = CandleBuilder::new(Timeframe::parse("1m").unwrap());
        builder.update(100.0, 1.0, 0);
        builder.update(110.0, 1.0, 60_000);
        // A late print stamped inside the bar that already closed.
        assert!(builder.update(90.0, 1.0, 30_000).is_none());
        let partial = builder.partial().unwrap();
        assert_eq!(partial.timestamp, 60_000, "must not reopen the closed bar");
        eq(partial.low, 90.0);
        eq(partial.close, 90.0);
    }

    #[test]
    fn no_partial_before_the_first_trade() {
        let builder = CandleBuilder::new(Timeframe::default());
        assert!(builder.partial().is_none());
    }
}
