//! Orderbook Imbalance Tracker
//!
//! Tracks orderbook changes and computes microstructure features:
//! - Weighted orderbook imbalance
//! - Order flow delta (buy volume - sell volume)
//! - Pressure detection (buying/selling pressure)
//! - Spread trends

use anyhow::Result;
use std::collections::{HashMap, VecDeque};

use crate::clob::types::{BookLevel, OrderBook, Side, Trade};
use crate::types::{Asset, Timeframe};

/// Maximum snapshots to keep in history
const MAX_HISTORY: usize = 100;

/// Book level with additional metadata
#[derive(Debug, Clone, Copy)]
pub struct WeightedLevel {
    pub price: f64,
    pub size: f64,
    pub distance_from_mid: f64,
}

/// Orderbook snapshot with computed features
#[derive(Debug, Clone)]
pub struct OrderbookSnapshot {
    pub token_id: String,
    pub asset: Asset,
    pub timeframe: Timeframe,
    pub best_bid: f64,
    pub best_ask: f64,
    pub mid_price: f64,
    pub spread: f64,
    pub spread_bps: f64,
    pub bid_levels: Vec<BookLevel>,
    pub ask_levels: Vec<BookLevel>,
    pub total_bid_volume: f64,
    pub total_ask_volume: f64,
    pub weighted_bid_pressure: f64,
    pub weighted_ask_pressure: f64,
    pub imbalance: f64,
    pub timestamp: i64,
}

impl Default for OrderbookSnapshot {
    fn default() -> Self {
        Self {
            token_id: String::new(),
            asset: Asset::BTC,
            timeframe: Timeframe::Min15,
            best_bid: 0.0,
            best_ask: 0.0,
            mid_price: 0.0,
            spread: 0.0,
            spread_bps: 0.0,
            bid_levels: Vec::new(),
            ask_levels: Vec::new(),
            total_bid_volume: 0.0,
            total_ask_volume: 0.0,
            weighted_bid_pressure: 0.0,
            weighted_ask_pressure: 0.0,
            imbalance: 0.0,
            timestamp: 0,
        }
    }
}

impl OrderbookSnapshot {
    pub fn from_orderbook(book: &OrderBook, asset: Asset, timeframe: Timeframe) -> Self {
        let best_bid = book.best_bid().map(|b| b.price).unwrap_or(0.0);
        let best_ask = book.best_ask().map(|b| b.price).unwrap_or(0.0);
        let mid_price = book.mid_price().unwrap_or(0.0);
        let spread = book.spread().unwrap_or(0.0);
        let spread_bps = if mid_price > 0.0 {
            (spread / mid_price) * 10000.0
        } else {
            0.0
        };

        let total_bid_volume: f64 = book.bids.iter().take(5).map(|b| b.size).sum();
        let total_ask_volume: f64 = book.asks.iter().take(5).map(|a| a.size).sum();

        let weighted_bid_pressure = book.weighted_bid_pressure(5);
        let weighted_ask_pressure = book.weighted_ask_pressure(5);

        let imbalance = book.imbalance(5);

        Self {
            token_id: book.token_id.clone(),
            asset,
            timeframe,
            best_bid,
            best_ask,
            mid_price,
            spread,
            spread_bps,
            bid_levels: book.bids.iter().take(5).cloned().collect(),
            ask_levels: book.asks.iter().take(5).cloned().collect(),
            total_bid_volume,
            total_ask_volume,
            weighted_bid_pressure,
            weighted_ask_pressure,
            imbalance,
            timestamp: book.timestamp,
        }
    }
}

/// Pressure signal from orderbook analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureSignal {
    StrongBuy,
    ModerateBuy,
    Neutral,
    ModerateSell,
    StrongSell,
}

impl Default for PressureSignal {
    fn default() -> Self {
        PressureSignal::Neutral
    }
}

/// Trade data for order flow tracking
#[derive(Debug, Clone, Copy)]
pub struct TradeData {
    pub price: f64,
    pub size: f64,
    pub side: Side,
    pub timestamp: i64,
}

/// Order flow accumulator
#[derive(Debug, Clone, Default)]
pub struct OrderFlowAccumulator {
    /// Cumulative buy volume in current window
    pub buy_volume: f64,
    /// Cumulative sell volume in current window
    pub sell_volume: f64,
    /// Number of buy trades
    pub buy_count: usize,
    /// Number of sell trades
    pub sell_count: usize,
    /// Window start timestamp
    pub window_start: i64,
    /// Trade history for the current window
    pub trades: VecDeque<TradeData>,
}

impl OrderFlowAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self, window_start: i64) {
        self.buy_volume = 0.0;
        self.sell_volume = 0.0;
        self.buy_count = 0;
        self.sell_count = 0;
        self.window_start = window_start;
        self.trades.clear();
    }

    pub fn add_trade(&mut self, trade: TradeData) {
        match trade.side {
            Side::Buy => {
                self.buy_volume += trade.size;
                self.buy_count += 1;
            }
            Side::Sell => {
                self.sell_volume += trade.size;
                self.sell_count += 1;
            }
        }
        self.trades.push_back(trade);
        if self.trades.len() > MAX_HISTORY {
            self.trades.pop_front();
        }
    }

    pub fn delta(&self) -> f64 {
        self.buy_volume - self.sell_volume
    }

    pub fn delta_ratio(&self) -> f64 {
        let total = self.buy_volume + self.sell_volume;
        if total > 0.0 {
            (self.buy_volume - self.sell_volume) / total
        } else {
            0.0
        }
    }

    pub fn trade_imbalance(&self) -> f64 {
        let total = self.buy_count + self.sell_count;
        if total > 0 {
            (self.buy_count as f64 - self.sell_count as f64) / total as f64
        } else {
            0.0
        }
    }
}

/// Main orderbook imbalance tracker
pub struct OrderbookImbalanceTracker {
    /// History of orderbook snapshots per (asset, timeframe)
    history: HashMap<(Asset, Timeframe), VecDeque<OrderbookSnapshot>>,
    /// Order flow accumulator per (asset, timeframe)
    orderflow: HashMap<(Asset, Timeframe), OrderFlowAccumulator>,
    /// Last trade timestamp per token_id
    last_trade_ts: HashMap<String, i64>,
    /// Last computed pressure signal
    last_pressure: HashMap<(Asset, Timeframe), PressureSignal>,
    /// Imbalance change rate (how fast imbalance is changing)
    imbalance_velocity: HashMap<(Asset, Timeframe), f64>,
}

impl OrderbookImbalanceTracker {
    pub fn new() -> Self {
        Self {
            history: HashMap::new(),
            orderflow: HashMap::new(),
            last_trade_ts: HashMap::new(),
            last_pressure: HashMap::new(),
            imbalance_velocity: HashMap::new(),
        }
    }

    /// Update orderbook data
    pub fn update_orderbook(&mut self, book: &OrderBook, asset: Asset, timeframe: Timeframe) {
        let key = (asset, timeframe);
        let snapshot = OrderbookSnapshot::from_orderbook(book, asset, timeframe);

        let history = self.history.entry(key).or_insert_with(VecDeque::new);

        if let Some(last) = history.back() {
            let imbalance_change = snapshot.imbalance - last.imbalance;
            let time_diff = (snapshot.timestamp - last.timestamp) as f64 / 1000.0;
            if time_diff > 0.0 {
                self.imbalance_velocity
                    .insert(key, imbalance_change / time_diff);
            }
        }

        history.push_back(snapshot);
        if history.len() > MAX_HISTORY {
            history.pop_front();
        }
    }

    /// Process a trade for order flow tracking
    pub fn process_trade(&mut self, trade: &Trade, asset: Asset, timeframe: Timeframe) {
        let key = (asset, timeframe);
        let orderflow = self
            .orderflow
            .entry(key)
            .or_insert_with(OrderFlowAccumulator::new);

        let trade_data = TradeData {
            price: trade.price,
            size: trade.size,
            side: trade.side,
            timestamp: trade.timestamp,
        };

        orderflow.add_trade(trade_data);
        self.last_trade_ts
            .insert(trade.token_id.clone(), trade.timestamp);
    }

    /// Reset order flow for a new window
    pub fn reset_orderflow(&mut self, asset: Asset, timeframe: Timeframe, window_start: i64) {
        let key = (asset, timeframe);
        let orderflow = self
            .orderflow
            .entry(key)
            .or_insert_with(OrderFlowAccumulator::new);
        orderflow.reset(window_start);
    }

    /// Get weighted imbalance (more recent snapshots weighted higher)
    pub fn weighted_imbalance(&self, key: (Asset, Timeframe)) -> Option<f64> {
        let history = self.history.get(&key)?;
        if history.is_empty() {
            return None;
        }

        let total_weight: f64 = (1..=history.len()).map(|i| i as f64).sum();
        let weighted_sum: f64 = history
            .iter()
            .enumerate()
            .map(|(i, snap)| snap.imbalance * ((i + 1) as f64))
            .sum();

        Some(weighted_sum / total_weight)
    }

    /// Get order flow delta (buy volume - sell volume)
    pub fn orderflow_delta(&self, key: (Asset, Timeframe)) -> f64 {
        self.orderflow.get(&key).map(|of| of.delta()).unwrap_or(0.0)
    }

    /// Get order flow delta ratio (-1.0 to 1.0)
    pub fn orderflow_delta_ratio(&self, key: (Asset, Timeframe)) -> f64 {
        self.orderflow
            .get(&key)
            .map(|of| of.delta_ratio())
            .unwrap_or(0.0)
    }

    /// Get trade imbalance (ratio of buy trades to sell trades)
    pub fn trade_imbalance(&self, key: (Asset, Timeframe)) -> f64 {
        self.orderflow
            .get(&key)
            .map(|of| of.trade_imbalance())
            .unwrap_or(0.0)
    }

    /// Detect buying/selling pressure
    pub fn detect_pressure(&self, key: (Asset, Timeframe)) -> PressureSignal {
        let history = match self.history.get(&key) {
            Some(h) if !h.is_empty() => h,
            _ => return PressureSignal::Neutral,
        };

        let orderflow = self.orderflow.get(&key);
        let current = history.back().unwrap();

        let mut buy_score = 0.0;
        let mut sell_score = 0.0;

        if current.imbalance > 0.2 {
            buy_score += 2.0;
        } else if current.imbalance > 0.1 {
            buy_score += 1.0;
        } else if current.imbalance < -0.2 {
            sell_score += 2.0;
        } else if current.imbalance < -0.1 {
            sell_score += 1.0;
        }

        if let Some(of) = orderflow {
            let delta_ratio = of.delta_ratio();
            if delta_ratio > 0.3 {
                buy_score += 1.5;
            } else if delta_ratio > 0.15 {
                buy_score += 0.75;
            } else if delta_ratio < -0.3 {
                sell_score += 1.5;
            } else if delta_ratio < -0.15 {
                sell_score += 0.75;
            }
        }

        if let Some(&velocity) = self.imbalance_velocity.get(&key) {
            if velocity > 0.05 {
                buy_score += 1.0;
            } else if velocity < -0.05 {
                sell_score += 1.0;
            }
        }

        if history.len() >= 3 {
            let recent_imbalances: Vec<f64> =
                history.iter().rev().take(3).map(|s| s.imbalance).collect();

            let trend = (recent_imbalances[0] - recent_imbalances[2]) / 2.0;
            if trend > 0.05 {
                buy_score += 0.5;
            } else if trend < -0.05 {
                sell_score += 0.5;
            }
        }

        let net_score = buy_score - sell_score;

        if net_score >= 3.0 {
            PressureSignal::StrongBuy
        } else if net_score >= 1.5 {
            PressureSignal::ModerateBuy
        } else if net_score <= -3.0 {
            PressureSignal::StrongSell
        } else if net_score <= -1.5 {
            PressureSignal::ModerateSell
        } else {
            PressureSignal::Neutral
        }
    }

    /// Get latest orderbook snapshot
    pub fn latest_snapshot(&self, key: (Asset, Timeframe)) -> Option<&OrderbookSnapshot> {
        self.history.get(&key)?.back()
    }

    /// Get imbalance velocity (rate of change)
    pub fn imbalance_velocity(&self, key: (Asset, Timeframe)) -> f64 {
        self.imbalance_velocity.get(&key).copied().unwrap_or(0.0)
    }

    /// Get spread trend (positive = widening, negative = tightening)
    pub fn spread_trend(&self, key: (Asset, Timeframe)) -> Option<f64> {
        let history = self.history.get(&key)?;
        if history.len() < 3 {
            return None;
        }

        let recent: Vec<_> = history.iter().rev().take(3).collect();
        let old_spread = recent[2].spread;
        let new_spread = recent[0].spread;

        if old_spread > 0.0 {
            Some((new_spread - old_spread) / old_spread)
        } else {
            None
        }
    }

    /// Get order flow statistics
    pub fn orderflow_stats(&self, key: (Asset, Timeframe)) -> Option<OrderFlowStats> {
        let of = self.orderflow.get(&key)?;
        Some(OrderFlowStats {
            buy_volume: of.buy_volume,
            sell_volume: of.sell_volume,
            buy_count: of.buy_count,
            sell_count: of.sell_count,
            delta: of.delta(),
            delta_ratio: of.delta_ratio(),
            trade_imbalance: of.trade_imbalance(),
        })
    }

    /// Get combined microstructure features for strategy
    pub fn get_features(&self, key: (Asset, Timeframe)) -> MicrostructureFeatures {
        let snapshot = self.latest_snapshot(key);
        let orderflow = self.orderflow.get(&key);
        let pressure = self.detect_pressure(key);

        MicrostructureFeatures {
            orderbook_imbalance: snapshot.map(|s| s.imbalance).unwrap_or(0.0),
            spread_bps: snapshot.map(|s| s.spread_bps).unwrap_or(0.0),
            mid_price: snapshot.map(|s| s.mid_price).unwrap_or(0.0),
            best_bid: snapshot.map(|s| s.best_bid).unwrap_or(0.0),
            best_ask: snapshot.map(|s| s.best_ask).unwrap_or(0.0),
            orderflow_delta: orderflow.map(|of| of.delta()).unwrap_or(0.0),
            orderflow_delta_ratio: orderflow.map(|of| of.delta_ratio()).unwrap_or(0.0),
            trade_imbalance: orderflow.map(|of| of.trade_imbalance()).unwrap_or(0.0),
            imbalance_velocity: self.imbalance_velocity(key),
            spread_trend: self.spread_trend(key).unwrap_or(0.0),
            pressure,
            bid_volume: snapshot.map(|s| s.total_bid_volume).unwrap_or(0.0),
            ask_volume: snapshot.map(|s| s.total_ask_volume).unwrap_or(0.0),
            weighted_bid_pressure: snapshot.map(|s| s.weighted_bid_pressure).unwrap_or(0.0),
            weighted_ask_pressure: snapshot.map(|s| s.weighted_ask_pressure).unwrap_or(0.0),
        }
    }
}

impl Default for OrderbookImbalanceTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Order flow statistics
#[derive(Debug, Clone, Copy)]
pub struct OrderFlowStats {
    pub buy_volume: f64,
    pub sell_volume: f64,
    pub buy_count: usize,
    pub sell_count: usize,
    pub delta: f64,
    pub delta_ratio: f64,
    pub trade_imbalance: f64,
}

/// Combined microstructure features
#[derive(Debug, Clone, Copy)]
pub struct MicrostructureFeatures {
    pub orderbook_imbalance: f64,
    pub spread_bps: f64,
    pub mid_price: f64,
    pub best_bid: f64,
    pub best_ask: f64,
    pub orderflow_delta: f64,
    pub orderflow_delta_ratio: f64,
    pub trade_imbalance: f64,
    pub imbalance_velocity: f64,
    pub spread_trend: f64,
    pub pressure: PressureSignal,
    pub bid_volume: f64,
    pub ask_volume: f64,
    pub weighted_bid_pressure: f64,
    pub weighted_ask_pressure: f64,
}

impl Default for MicrostructureFeatures {
    fn default() -> Self {
        Self {
            orderbook_imbalance: 0.0,
            spread_bps: 0.0,
            mid_price: 0.0,
            best_bid: 0.0,
            best_ask: 0.0,
            orderflow_delta: 0.0,
            orderflow_delta_ratio: 0.0,
            trade_imbalance: 0.0,
            imbalance_velocity: 0.0,
            spread_trend: 0.0,
            pressure: PressureSignal::Neutral,
            bid_volume: 0.0,
            ask_volume: 0.0,
            weighted_bid_pressure: 0.0,
            weighted_ask_pressure: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_orderbook(
        token_id: &str,
        bid_price: f64,
        ask_price: f64,
        bid_size: f64,
        ask_size: f64,
    ) -> OrderBook {
        OrderBook {
            token_id: token_id.to_string(),
            bids: vec![
                BookLevel {
                    price: bid_price,
                    size: bid_size,
                },
                BookLevel {
                    price: bid_price - 0.01,
                    size: bid_size * 0.8,
                },
                BookLevel {
                    price: bid_price - 0.02,
                    size: bid_size * 0.6,
                },
            ],
            asks: vec![
                BookLevel {
                    price: ask_price,
                    size: ask_size,
                },
                BookLevel {
                    price: ask_price + 0.01,
                    size: ask_size * 0.8,
                },
                BookLevel {
                    price: ask_price + 0.02,
                    size: ask_size * 0.6,
                },
            ],
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    #[test]
    fn test_orderbook_snapshot_creation() {
        let book = make_orderbook("test-token", 0.55, 0.56, 100.0, 80.0);
        let snapshot = OrderbookSnapshot::from_orderbook(&book, Asset::BTC, Timeframe::Min15);

        assert_eq!(snapshot.best_bid, 0.55);
        assert_eq!(snapshot.best_ask, 0.56);
        assert!(snapshot.mid_price > 0.55);
        assert!(snapshot.spread > 0.0);
        assert!(snapshot.spread_bps > 0.0);
    }

    #[test]
    fn test_imbalance_calculation() {
        let book = make_orderbook("test-token", 0.55, 0.56, 100.0, 80.0);
        let snapshot = OrderbookSnapshot::from_orderbook(&book, Asset::BTC, Timeframe::Min15);

        assert!(snapshot.imbalance > 0.0);
    }

    #[test]
    fn test_tracker_update() {
        let mut tracker = OrderbookImbalanceTracker::new();
        let book = make_orderbook("test-token", 0.55, 0.56, 100.0, 80.0);

        tracker.update_orderbook(&book, Asset::BTC, Timeframe::Min15);

        let snapshot = tracker.latest_snapshot((Asset::BTC, Timeframe::Min15));
        assert!(snapshot.is_some());
    }

    #[test]
    fn test_orderflow_accumulator() {
        let mut acc = OrderFlowAccumulator::new();

        acc.add_trade(TradeData {
            price: 0.55,
            size: 10.0,
            side: Side::Buy,
            timestamp: 1000,
        });

        acc.add_trade(TradeData {
            price: 0.56,
            size: 5.0,
            side: Side::Sell,
            timestamp: 1001,
        });

        assert_eq!(acc.buy_volume, 10.0);
        assert_eq!(acc.sell_volume, 5.0);
        assert_eq!(acc.delta(), 5.0);
        assert!(acc.delta_ratio() > 0.0);
    }

    #[test]
    fn test_pressure_detection() {
        let mut tracker = OrderbookImbalanceTracker::new();

        let bullish_book = make_orderbook("test-token", 0.55, 0.56, 150.0, 50.0);
        tracker.update_orderbook(&bullish_book, Asset::BTC, Timeframe::Min15);

        let pressure = tracker.detect_pressure((Asset::BTC, Timeframe::Min15));
        assert!(matches!(
            pressure,
            PressureSignal::ModerateBuy | PressureSignal::StrongBuy
        ));
    }

    #[test]
    fn test_weighted_imbalance() {
        let mut tracker = OrderbookImbalanceTracker::new();

        for i in 0..5 {
            let book = make_orderbook(
                "test-token",
                0.55 + (i as f64 * 0.01),
                0.56 + (i as f64 * 0.01),
                100.0,
                80.0,
            );
            tracker.update_orderbook(&book, Asset::BTC, Timeframe::Min15);
        }

        let weighted = tracker.weighted_imbalance((Asset::BTC, Timeframe::Min15));
        assert!(weighted.is_some());
    }

    #[test]
    fn test_microstructure_features() {
        let mut tracker = OrderbookImbalanceTracker::new();
        let book = make_orderbook("test-token", 0.55, 0.56, 100.0, 80.0);

        tracker.update_orderbook(&book, Asset::BTC, Timeframe::Min15);

        let features = tracker.get_features((Asset::BTC, Timeframe::Min15));
        assert!(features.orderbook_imbalance.abs() <= 1.0);
        assert!(features.spread_bps >= 0.0);
    }
}
