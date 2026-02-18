//! Candle Builder - Builds OHLCV candles from trade/tick stream
//!
//! Supports multiple timeframes (15m, 1h) and maintains rolling windows
//! of candles for technical analysis.

use anyhow::Result;
use chrono::{DateTime, TimeZone, Timelike, Utc};
use std::collections::{HashMap, VecDeque};

use crate::oracle::NormalizedTick;
use crate::types::{Asset, Candle, PriceSource as Source, Timeframe};

/// Candle builder that aggregates ticks into OHLCV candles
pub struct CandleBuilder {
    /// Current incomplete candles per (asset, timeframe)
    current: HashMap<(Asset, Timeframe), BuildingCandle>,
    /// Completed candles per (asset, timeframe)
    history: HashMap<(Asset, Timeframe), VecDeque<Candle>>,
    /// Maximum candles to keep in history
    max_history: usize,
}

#[derive(Debug, Clone)]
struct BuildingCandle {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    trade_count: u64,
    start_ts: i64,
    end_ts: i64,
    source: Source,
    sources: Vec<Source>,
}

impl BuildingCandle {
    fn new(ts: i64, price: f64, source: Source) -> Self {
        Self {
            open: price,
            high: price,
            low: price,
            close: price,
            volume: 0.0,
            trade_count: 0,
            start_ts: ts,
            end_ts: ts,
            source,
            sources: vec![source],
        }
    }

    fn update(&mut self, ts: i64, price: f64, size: f64, source: Source) {
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
        self.volume += size;
        self.trade_count += 1;
        self.end_ts = ts;
        if !self.sources.contains(&source) {
            self.sources.push(source);
        }
    }

    fn finalize(&self, asset: Asset, timeframe: Timeframe) -> Candle {
        Candle {
            open_time: self.start_ts,
            close_time: self.end_ts,
            asset,
            timeframe,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: self.volume,
            trades: self.trade_count,
        }
    }
}

impl CandleBuilder {
    pub fn new(max_history: usize) -> Self {
        Self {
            current: HashMap::new(),
            history: HashMap::new(),
            max_history,
        }
    }

    /// Add a tick and potentially complete a candle
    pub fn add_tick(&mut self, tick: &NormalizedTick, timeframe: Timeframe) -> Option<Candle> {
        let candle_ts = Self::candle_start(tick.ts, timeframe);
        let key = (tick.asset, timeframe);

        // Check if we need to finalize current candle
        let completed = if let Some(current) = self.current.get(&key) {
            if current.start_ts != candle_ts {
                // Time to finalize - clone the data we need
                Some(current.finalize(tick.asset, timeframe))
            } else {
                None
            }
        } else {
            None
        };

        // Add completed candle to history (outside the borrow)
        if let Some(ref candle) = completed {
            self.add_to_history(key, candle.clone());
        }

        // Update or create current candle
        self.current
            .entry(key)
            .and_modify(|c| {
                if c.start_ts == candle_ts {
                    c.update(tick.ts, tick.mid, 0.0, tick.source); // Volume from tick not available
                } else {
                    // Use candle_ts (aligned) for start_ts so comparison works correctly
                    let mut candle = BuildingCandle::new(candle_ts, tick.mid, tick.source);
                    candle.end_ts = tick.ts;
                    *c = candle;
                }
            })
            .or_insert_with(|| {
                // Use candle_ts (aligned) for start_ts so comparison works correctly
                let mut candle = BuildingCandle::new(candle_ts, tick.mid, tick.source);
                candle.end_ts = tick.ts;
                candle
            });

        completed
    }

    /// Get candle start timestamp for a given timestamp and timeframe
    fn candle_start(ts: i64, timeframe: Timeframe) -> i64 {
        let dt = Utc.timestamp_millis(ts);
        match timeframe {
            Timeframe::Min15 => {
                let mins = dt.minute();
                let candle_mins = (mins / 15) * 15;
                dt.with_minute(candle_mins)
                    .and_then(|d| d.with_second(0))
                    .and_then(|d| d.with_nanosecond(0))
                    .map(|d| d.timestamp_millis())
                    .unwrap_or(ts)
            }
            Timeframe::Hour1 => dt
                .with_minute(0)
                .and_then(|d| d.with_second(0))
                .and_then(|d| d.with_nanosecond(0))
                .map(|d| d.timestamp_millis())
                .unwrap_or(ts),
        }
    }

    /// Add completed candle to history
    fn add_to_history(&mut self, key: (Asset, Timeframe), candle: Candle) {
        let history = self.history.entry(key).or_insert_with(VecDeque::new);
        history.push_back(candle);
        while history.len() > self.max_history {
            history.pop_front();
        }
    }

    /// Get historical candles for an asset/timeframe
    pub fn get_history(&self, asset: Asset, timeframe: Timeframe) -> Vec<Candle> {
        self.history
            .get(&(asset, timeframe))
            .map(|h| h.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get current incomplete candle
    pub fn get_current(&self, asset: Asset, timeframe: Timeframe) -> Option<&BuildingCandle> {
        self.current.get(&(asset, timeframe))
    }

    /// Get last N completed candles (including current incomplete candle)
    pub fn get_last_n(&self, asset: Asset, timeframe: Timeframe, n: usize) -> Vec<Candle> {
        let mut result: Vec<Candle> = Vec::new();

        // Include current incomplete candle if available
        if let Some(current) = self.current.get(&(asset, timeframe)) {
            result.push(current.finalize(asset, timeframe));
        }

        // Add completed candles from history
        if let Some(history) = self.history.get(&(asset, timeframe)) {
            let from_history: Vec<Candle> = history
                .iter()
                .rev()
                .take(n.saturating_sub(result.len()))
                .cloned()
                .collect();
            result.extend(from_history);
        }

        // Take only n candles and reverse to get chronological order
        result.into_iter().take(n).rev().collect()
    }

    /// Force finalize current candle (e.g., on shutdown)
    pub fn finalize_all(&mut self) -> Vec<Candle> {
        // First drain current candles into a separate collection
        let current: Vec<((Asset, Timeframe), BuildingCandle)> = self.current.drain().collect();
        let mut completed = Vec::new();

        for ((asset, timeframe), candle) in current {
            let candle = candle.finalize(asset, timeframe);
            completed.push(candle.clone());
            self.add_to_history((asset, timeframe), candle);
        }
        completed
    }

    /// Seed the builder with historical candles (e.g., from REST API)
    /// Candles are added to history for their (asset, timeframe)
    pub fn seed_history(&mut self, candles: Vec<Candle>) {
        for candle in candles {
            let key = (candle.asset, candle.timeframe);
            self.add_to_history(key, candle);
        }
    }
}

impl Default for CandleBuilder {
    fn default() -> Self {
        Self::new(1000) // Keep 1000 candles per timeframe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tick(asset: Asset, ts: i64, price: f64) -> NormalizedTick {
        NormalizedTick {
            ts,
            asset,
            bid: price - 0.5,
            ask: price + 0.5,
            mid: price,
            source: Source::Binance,
            latency_ms: 10,
        }
    }

    #[test]
    fn test_candle_building() {
        let mut builder = CandleBuilder::new(100);
        // Use a base timestamp at the start of a 15-min period to avoid boundary issues
        // 1700000050000 = 2023-11-14 22:14:10 UTC (within 22:00-22:15 period)
        let base_ts = 1700000050000i64;

        // Add ticks within same candle (use 10-second intervals to stay in same period)
        builder.add_tick(&make_tick(Asset::BTC, base_ts, 50000.0), Timeframe::Min15);
        builder.add_tick(
            &make_tick(Asset::BTC, base_ts + 10000, 50100.0),
            Timeframe::Min15,
        );
        builder.add_tick(
            &make_tick(Asset::BTC, base_ts + 20000, 49900.0),
            Timeframe::Min15,
        );

        let current = builder.get_current(Asset::BTC, Timeframe::Min15).unwrap();
        assert_eq!(current.open, 50000.0);
        assert_eq!(current.high, 50100.0);
        assert_eq!(current.low, 49900.0);
        assert_eq!(current.close, 49900.0);
    }

    #[test]
    fn test_candle_completion() {
        let mut builder = CandleBuilder::new(100);
        let base_ts = 1700000000000i64;

        // Add tick in first candle
        builder.add_tick(&make_tick(Asset::ETH, base_ts, 3000.0), Timeframe::Hour1);

        // Add tick in next candle (1 hour later)
        let completed = builder.add_tick(
            &make_tick(Asset::ETH, base_ts + 3600000, 3100.0),
            Timeframe::Hour1,
        );

        assert!(completed.is_some());
        let candle = completed.unwrap();
        assert_eq!(candle.open, 3000.0);
        assert_eq!(candle.close, 3000.0);
    }
}
