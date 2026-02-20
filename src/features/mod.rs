//! Feature Engine - Technical indicators and market features
//!
//! Computes technical indicators from candle data for signal generation:
//! - RSI (Wilder's smoothing)
//! - MACD (with proper EMA signal line)
//! - Bollinger Bands
//! - VWAP (Volume Weighted Average Price)
//! - ATR (Average True Range)
//! - Heikin Ashi candles (with history-based trend)
//! - Momentum/Velocity features
//! - ADX (Average Directional Index)
//! - Stochastic RSI
//! - OBV (On Balance Volume)
//! - Relative Volume
//!
//! NEW FEATURES (v2):
//! - Window-relative price action: How much price moved since window start
//! - Short-term weighted momentum: Recent 2-3 min more predictive for 15m windows
//! - "Already moved too much" detection: Avoid late entries
//! - Intra-window volatility: Micro-volatility within the trading window
//! - Market timing score: Best moment to enter within the window
//!
//! NEW FEATURES (v3):
//! - Orderbook imbalance tracking via OrderbookImbalanceTracker
//! - Order flow delta (buy volume - sell volume)
//! - Pressure detection from microstructure

use anyhow::Result;
use std::collections::{HashMap, VecDeque};

use crate::oracle::NormalizedTick;
use crate::types::{Asset, Candle, Direction, FeatureSet, Timeframe};

pub mod orderbook_tracker;
pub use orderbook_tracker::{MicrostructureFeatures, OrderbookImbalanceTracker, PressureSignal};

// NEW v3.0: Temporal patterns, settlement prediction, cross-asset analysis
pub mod temporal_patterns;
pub use temporal_patterns::{HourlyStats, TemporalPatternAnalyzer, TemporalStatsSummary};

pub mod settlement_predictor;
pub use settlement_predictor::{SettlementEdge, SettlementPrediction, SettlementPricePredictor};

pub mod cross_asset;
pub use cross_asset::{CrossAssetAnalyzer, CrossAssetSignal, CrossAssetSignalType};

/// Feature engine for computing technical indicators
pub struct FeatureEngine {
    /// RSI period
    rsi_period: usize,
    /// MACD periods (fast, slow, signal)
    macd_periods: (usize, usize, usize),
    /// Bollinger Band period and multiplier
    bb_config: (usize, f64),
    /// ATR period
    atr_period: usize,
    /// Lookback for momentum
    momentum_lookback: usize,
    /// ADX period
    adx_period: usize,
    /// Stochastic RSI lookback
    stoch_rsi_period: usize,
    /// MACD line history for proper signal line computation
    macd_history: HashMap<(Asset, Timeframe), VecDeque<f64>>,
    /// Previous Heikin Ashi close for trend comparison
    prev_ha_close: HashMap<(Asset, Timeframe), f64>,
    /// RSI smoothing state (prev_avg_gain, prev_avg_loss) per asset/timeframe
    rsi_state: HashMap<(Asset, Timeframe), (f64, f64)>,
    /// RSI history for Stochastic RSI
    rsi_history: HashMap<(Asset, Timeframe), VecDeque<f64>>,
    /// OBV accumulator per asset/timeframe
    obv_state: HashMap<(Asset, Timeframe), f64>,
    /// Previous close for OBV calculation
    prev_close: HashMap<(Asset, Timeframe), f64>,

    // ============================================
    // NEW: Window tracking state
    // ============================================
    /// Window start price per (asset, timeframe)
    window_start_price: HashMap<(Asset, Timeframe), f64>,
    /// Window start timestamp per (asset, timeframe)
    window_start_ts: HashMap<(Asset, Timeframe), i64>,
    /// Window high/low for intra-window range
    window_high: HashMap<(Asset, Timeframe), f64>,
    window_low: HashMap<(Asset, Timeframe), f64>,
    /// Tick history for short-term momentum (stores recent price changes)
    tick_history: HashMap<(Asset, Timeframe), VecDeque<TickData>>,
    /// Orderbook data from Polymarket (if available)
    orderbook_data: HashMap<(Asset, Timeframe), InternalOrderbookSnapshot>,
    /// External orderbook tracker for advanced microstructure analysis
    orderbook_tracker: Option<std::sync::Arc<std::sync::Mutex<OrderbookImbalanceTracker>>>,

    // ============================================
    // NEW v3.0: Advanced pattern analysis
    // ============================================
    /// Temporal pattern analyzer for time-of-day performance
    temporal_analyzer: TemporalPatternAnalyzer,
    /// Settlement price predictor for Chainlink oracle
    settlement_predictor: SettlementPricePredictor,
    /// Cross-asset correlation analyzer (BTC vs ETH, 15m vs 1h)
    cross_asset_analyzer: CrossAssetAnalyzer,
}

/// Market regime classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketRegime {
    Trending,
    Ranging,
    Volatile,
}

impl Default for MarketRegime {
    fn default() -> Self {
        MarketRegime::Ranging
    }
}

/// Tick data for short-term momentum calculation
#[derive(Debug, Clone, Copy)]
struct TickData {
    ts: i64,
    price: f64,
    volume: f64,
}

/// Internal orderbook snapshot for backward compatibility
#[derive(Debug, Clone, Default)]
struct InternalOrderbookSnapshot {
    /// Best bid price
    best_bid: f64,
    /// Best ask price
    best_ask: f64,
    /// Total bid volume at top 5 levels
    bid_volume: f64,
    /// Total ask volume at top 5 levels
    ask_volume: f64,
    /// Timestamp of last update
    ts: i64,
    /// Recent buy volume (from trades)
    buy_volume: f64,
    /// Recent sell volume (from trades)
    sell_volume: f64,
}

/// Computed features for a single asset/timeframe
#[derive(Debug, Clone, Default)]
pub struct Features {
    pub asset: Asset,
    pub timeframe: Timeframe,
    pub ts: i64,

    // Price features
    pub close: f64,
    pub returns: f64,
    pub log_returns: f64,

    // RSI (Wilder's smoothing)
    pub rsi: Option<f64>,

    // MACD (proper signal line)
    pub macd: Option<f64>,
    pub macd_signal: Option<f64>,
    pub macd_hist: Option<f64>,

    // Bollinger Bands
    pub bb_upper: Option<f64>,
    pub bb_middle: Option<f64>,
    pub bb_lower: Option<f64>,
    pub bb_width: Option<f64>,
    pub bb_position: Option<f64>, // 0 = lower band, 1 = upper band

    // ATR (volatility)
    pub atr: Option<f64>,
    pub atr_pct: Option<f64>, // ATR as % of price

    // VWAP deviation
    pub vwap: Option<f64>,
    pub vwap_deviation: Option<f64>,

    // Momentum
    pub momentum: Option<f64>,
    pub velocity: Option<f64>,

    // Trend features
    pub ema_9: Option<f64>,
    pub ema_21: Option<f64>,
    pub trend_strength: Option<f64>,

    // Volatility
    pub volatility: Option<f64>,

    // Heikin Ashi (history-based trend)
    pub ha_close: Option<f64>,
    pub ha_trend: Option<Direction>,

    // ADX (Average Directional Index)
    pub adx: Option<f64>,
    pub plus_di: Option<f64>,
    pub minus_di: Option<f64>,

    // Stochastic RSI
    pub stoch_rsi: Option<f64>,
    pub stoch_rsi_k: Option<f64>,
    pub stoch_rsi_d: Option<f64>,

    // OBV (On Balance Volume)
    pub obv: Option<f64>,
    pub obv_slope: Option<f64>,

    // Relative Volume
    pub relative_volume: Option<f64>,

    // Market Regime
    pub regime: MarketRegime,

    // ============================================
    // NEW FEATURES (v2) - Window-Relative Analysis
    // ============================================
    /// Price at the start of the current window
    pub window_start_price: Option<f64>,
    /// Price movement since window start (as %)
    /// Positive = price went up, Negative = price went down
    pub window_price_change_pct: Option<f64>,
    /// Absolute price movement since window start (regardless of direction)
    pub window_price_moved_abs: Option<f64>,
    /// How far into the window are we? (0.0 = start, 1.0 = end)
    pub window_progress: Option<f64>,

    /// Short-term momentum (last 2-3 candles, more predictive for short windows)
    pub short_term_momentum: Option<f64>,
    /// Short-term velocity (acceleration in last 2-3 candles)
    pub short_term_velocity: Option<f64>,
    /// Weighted momentum: recent candles weighted higher
    pub weighted_momentum: Option<f64>,

    /// Intra-window volatility (micro-volatility within the window)
    pub intra_window_volatility: Option<f64>,
    /// Price range within current window (high - low) / open
    pub intra_window_range: Option<f64>,

    /// Market timing score (-1.0 to 1.0)
    /// Positive = good time to enter UP, Negative = good time to enter DOWN
    /// Considers: window progress, price movement, momentum alignment
    pub market_timing_score: Option<f64>,

    /// "Already moved too much" flag
    /// True if price already moved >0.15% in a direction (late entry risk)
    pub late_entry_up: bool,
    pub late_entry_down: bool,

    /// Orderbook imbalance (from Polymarket CLOB)
    /// Positive = more buying pressure, Negative = more selling pressure
    pub orderbook_imbalance: Option<f64>,
    /// Top-5 bid depth in shares/contracts.
    pub orderbook_bid_depth: Option<f64>,
    /// Top-5 ask depth in shares/contracts.
    pub orderbook_ask_depth: Option<f64>,
    /// Top-5 total depth (bid + ask).
    pub orderbook_depth_top5: Option<f64>,
    /// Bid-ask spread in basis points
    pub spread_bps: Option<f64>,
    /// Order flow delta (buy volume - sell volume)
    pub orderflow_delta: Option<f64>,
}

impl FeatureEngine {
    pub fn new() -> Self {
        Self {
            rsi_period: 14,
            macd_periods: (12, 26, 9),
            bb_config: (20, 2.0),
            atr_period: 14,
            momentum_lookback: 10,
            adx_period: 14,
            stoch_rsi_period: 14,
            macd_history: HashMap::new(),
            prev_ha_close: HashMap::new(),
            rsi_state: HashMap::new(),
            rsi_history: HashMap::new(),
            obv_state: HashMap::new(),
            prev_close: HashMap::new(),
            // NEW: Window tracking
            window_start_price: HashMap::new(),
            window_start_ts: HashMap::new(),
            window_high: HashMap::new(),
            window_low: HashMap::new(),
            tick_history: HashMap::new(),
            orderbook_data: HashMap::new(),
            orderbook_tracker: None,
            // NEW v3.0: Initialize advanced analyzers
            temporal_analyzer: TemporalPatternAnalyzer::new(5),
            settlement_predictor: SettlementPricePredictor::new(1000),
            cross_asset_analyzer: CrossAssetAnalyzer::new(50),
        }
    }

    /// Get mutable reference to temporal analyzer for recording trades
    pub fn temporal_analyzer_mut(&mut self) -> &mut TemporalPatternAnalyzer {
        &mut self.temporal_analyzer
    }

    /// Get reference to settlement predictor
    pub fn settlement_predictor(&self) -> &SettlementPricePredictor {
        &self.settlement_predictor
    }

    /// Get mutable reference to settlement predictor
    pub fn settlement_predictor_mut(&mut self) -> &mut SettlementPricePredictor {
        &mut self.settlement_predictor
    }

    /// Get reference to cross-asset analyzer
    pub fn cross_asset_analyzer(&self) -> &CrossAssetAnalyzer {
        &self.cross_asset_analyzer
    }

    /// Get mutable reference to cross-asset analyzer
    pub fn cross_asset_analyzer_mut(&mut self) -> &mut CrossAssetAnalyzer {
        &mut self.cross_asset_analyzer
    }

    /// Set the external orderbook tracker
    pub fn set_orderbook_tracker(
        &mut self,
        tracker: std::sync::Arc<std::sync::Mutex<OrderbookImbalanceTracker>>,
    ) {
        self.orderbook_tracker = Some(tracker);
    }

    /// Update orderbook data from Polymarket
    pub fn update_orderbook(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        best_bid: f64,
        best_ask: f64,
        bid_volume: f64,
        ask_volume: f64,
        buy_vol: f64,
        sell_vol: f64,
    ) {
        let snapshot = InternalOrderbookSnapshot {
            best_bid,
            best_ask,
            bid_volume,
            ask_volume,
            ts: chrono::Utc::now().timestamp_millis(),
            buy_volume: buy_vol,
            sell_volume: sell_vol,
        };
        self.orderbook_data.insert((asset, timeframe), snapshot);
    }

    /// Update orderbook from external tracker
    pub fn update_from_tracker(&mut self, asset: Asset, timeframe: Timeframe) {
        if let Some(ref tracker) = self.orderbook_tracker {
            if let Ok(tracker) = tracker.lock() {
                let key = (asset, timeframe);
                let micro_features = tracker.get_features(key);

                let snapshot = InternalOrderbookSnapshot {
                    best_bid: 0.0,
                    best_ask: 0.0,
                    bid_volume: micro_features.bid_volume,
                    ask_volume: micro_features.ask_volume,
                    ts: chrono::Utc::now().timestamp_millis(),
                    buy_volume: micro_features.orderflow_delta.max(0.0),
                    sell_volume: micro_features.orderflow_delta.min(0.0).abs(),
                };
                self.orderbook_data.insert(key, snapshot);
            }
        }
    }

    /// Compute all features from a series of candles
    pub fn compute(&mut self, candles: &[Candle]) -> Option<Features> {
        if candles.is_empty() {
            tracing::debug!("FeatureEngine::compute: No candles provided");
            return None;
        }

        let last = candles.last()?;
        let key = (last.asset, last.timeframe);

        tracing::debug!(
            asset = ?last.asset,
            timeframe = ?last.timeframe,
            candle_count = candles.len(),
            first_close = candles.first().map(|c| c.close),
            last_close = last.close,
            "FeatureEngine::compute starting"
        );
        let key = (last.asset, last.timeframe);
        let mut features = Features {
            asset: last.asset,
            timeframe: last.timeframe,
            ts: last.open_time,
            close: last.close,
            ..Default::default()
        };

        // Need at least 2 candles for returns
        if candles.len() >= 2 {
            let prev = &candles[candles.len() - 2];
            features.returns = (last.close - prev.close) / prev.close;
            features.log_returns = (last.close / prev.close).ln();
        }

        // Compute indicators
        features.rsi = self.compute_rsi_wilders(candles, key);

        // Store RSI in history for Stochastic RSI
        if let Some(rsi) = features.rsi {
            let rsi_hist = self.rsi_history.entry(key).or_insert_with(VecDeque::new);
            rsi_hist.push_back(rsi);
            if rsi_hist.len() > 50 {
                rsi_hist.pop_front();
            }
        }

        if let Some((macd, signal, hist)) = self.compute_macd_proper(candles, key) {
            features.macd = Some(macd);
            features.macd_signal = Some(signal);
            features.macd_hist = Some(hist);
        }

        if let Some((upper, middle, lower)) = self.compute_bollinger(candles) {
            features.bb_upper = Some(upper);
            features.bb_middle = Some(middle);
            features.bb_lower = Some(lower);
            features.bb_width = Some((upper - lower) / middle);
            features.bb_position = Some((last.close - lower) / (upper - lower));
        }

        features.atr = self.compute_atr(candles);
        if let (Some(atr), true) = (features.atr, last.close > 0.0) {
            features.atr_pct = Some(atr / last.close);
        }

        features.vwap = self.compute_vwap(candles);
        if let Some(vwap) = features.vwap {
            if vwap > 0.0 {
                features.vwap_deviation = Some((last.close - vwap) / vwap);
            }
        }

        // Momentum features
        if candles.len() >= self.momentum_lookback {
            let prev = &candles[candles.len() - self.momentum_lookback];
            features.momentum = Some((last.close - prev.close) / prev.close);

            // Velocity = rate of change of momentum
            if candles.len() >= self.momentum_lookback + 1 {
                let prev_prev = &candles[candles.len() - self.momentum_lookback - 1];
                let prev_momentum = (prev.close - prev_prev.close) / prev_prev.close;
                features.velocity = Some(features.momentum.unwrap() - prev_momentum);
            }
        }

        // EMAs
        features.ema_9 = self.compute_ema(candles, 9);
        features.ema_21 = self.compute_ema(candles, 21);

        // Trend strength based on EMA crossover
        if let (Some(ema9), Some(ema21)) = (features.ema_9, features.ema_21) {
            features.trend_strength = Some((ema9 - ema21) / ema21);
        }

        // Volatility (standard deviation of returns)
        features.volatility = self.compute_volatility(candles, 20);

        // Heikin Ashi (with history-based trend)
        if let Some(ha) = self.compute_heikin_ashi(candles) {
            features.ha_close = Some(ha);
            // Compare current HA close to previous HA close for trend
            if let Some(prev_ha) = self.prev_ha_close.get(&key) {
                features.ha_trend = if ha > *prev_ha {
                    Some(Direction::Up)
                } else {
                    Some(Direction::Down)
                };
            }
            self.prev_ha_close.insert(key, ha);
        }

        // ADX
        if let Some((adx, plus_di, minus_di)) = self.compute_adx(candles) {
            features.adx = Some(adx);
            features.plus_di = Some(plus_di);
            features.minus_di = Some(minus_di);
        }

        // Stochastic RSI
        if let Some((stoch_rsi, k, d)) = self.compute_stoch_rsi(key) {
            features.stoch_rsi = Some(stoch_rsi);
            features.stoch_rsi_k = Some(k);
            features.stoch_rsi_d = Some(d);
        }

        // OBV
        if let Some((obv, slope)) = self.compute_obv(candles, key) {
            features.obv = Some(obv);
            features.obv_slope = Some(slope);
        }

        // Relative Volume
        features.relative_volume = self.compute_relative_volume(candles, 20);

        // Market Regime Detection
        features.regime = self.detect_regime(&features);

        // ============================================
        // NEW: Window-Relative Price Analysis
        // ============================================
        self.compute_window_features(candles, &mut features);

        // ============================================
        // NEW: Orderbook/Microstructure Features
        // ============================================
        self.compute_orderbook_features(&key, &mut features);

        // Log feature computation results for debugging
        tracing::debug!(
            asset = ?key.0,
            timeframe = ?key.1,
            rsi = ?features.rsi,
            macd = ?features.macd,
            has_rsi = features.rsi.is_some(),
            has_macd = features.macd.is_some(),
            has_bb = features.bb_position.is_some(),
            has_atr = features.atr.is_some(),
            has_vwap = features.vwap.is_some(),
            "FeatureEngine::compute completed with {} indicators",
            [
                features.rsi.is_some(),
                features.macd.is_some(),
                features.bb_position.is_some(),
                features.atr.is_some(),
                features.vwap.is_some(),
            ].iter().filter(|&&x| x).count()
        );

        Some(features)
    }

    /// Compute window-relative features
    fn compute_window_features(&mut self, candles: &[Candle], features: &mut Features) {
        let key = (features.asset, features.timeframe);
        let last = match candles.last() {
            Some(c) => c,
            None => return,
        };

        // Window duration in milliseconds
        let window_ms = features.timeframe.duration_secs() as i64 * 1000;

        // Check if we're in a new window
        let current_window_start = (last.open_time / window_ms) * window_ms;
        let prev_window_start = self.window_start_ts.get(&key).copied().unwrap_or(0);

        if current_window_start != prev_window_start {
            // New window - reset tracking
            self.window_start_price.insert(key, last.open);
            self.window_start_ts.insert(key, current_window_start);
            self.window_high.insert(key, last.high);
            self.window_low.insert(key, last.low);
        } else {
            // Same window - update high/low
            if let Some(high) = self.window_high.get_mut(&key) {
                *high = high.max(last.high);
            }
            if let Some(low) = self.window_low.get_mut(&key) {
                *low = low.min(last.low);
            }
        }

        // Calculate window-relative features
        if let (Some(start_price), Some(window_high), Some(window_low)) = (
            self.window_start_price.get(&key),
            self.window_high.get(&key),
            self.window_low.get(&key),
        ) {
            if *start_price > 0.0 {
                // Price change since window start
                let change_pct = (last.close - start_price) / start_price;
                features.window_start_price = Some(*start_price);
                features.window_price_change_pct = Some(change_pct);
                features.window_price_moved_abs = Some(change_pct.abs());

                // Intra-window range
                let range = (window_high - window_low) / start_price;
                features.intra_window_range = Some(range);

                // Window progress (how far into the window are we)
                // FIX: Use current time, not candle open time
                let now = chrono::Utc::now().timestamp_millis();
                let elapsed = now - current_window_start;
                let progress = elapsed as f64 / window_ms as f64;
                features.window_progress = Some(progress.clamp(0.0, 1.0));

                // Late entry detection
                // FIX: Dynamic threshold based on timeframe - 0.15% was too tight
                // Normal BTC volatility: ~0.5% in 15m, ~1.5% in 1h
                let late_threshold = match features.timeframe {
                    Timeframe::Min15 => 0.005, // 0.5%
                    Timeframe::Hour1 => 0.015, // 1.5%
                };
                features.late_entry_up = change_pct > late_threshold;
                features.late_entry_down = change_pct < -late_threshold;
            }
        }

        // ============================================
        // Short-term momentum (more predictive for 15m windows)
        // ============================================
        self.compute_short_term_momentum(candles, features);

        // ============================================
        // Market timing score
        // ============================================
        self.compute_market_timing_score(features);

        // ============================================
        // Intra-window volatility
        // ============================================
        if candles.len() >= 3 {
            // Use last 3 candles for micro-volatility
            let recent: Vec<f64> = candles.iter().rev().take(3).map(|c| c.close).collect();
            let mean = recent.iter().sum::<f64>() / recent.len() as f64;
            let variance: f64 =
                recent.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / recent.len() as f64;
            features.intra_window_volatility = Some(variance.sqrt() / mean);
        }
    }

    /// Compute short-term momentum (last 2-3 candles, weighted higher for recent)
    fn compute_short_term_momentum(&mut self, candles: &[Candle], features: &mut Features) {
        if candles.len() < 3 {
            return;
        }

        let last = candles.last().unwrap();
        let key = (features.asset, features.timeframe);

        // Update tick history
        let tick_hist = self.tick_history.entry(key).or_insert_with(VecDeque::new);
        tick_hist.push_back(TickData {
            ts: last.open_time,
            price: last.close,
            volume: last.volume,
        });
        if tick_hist.len() > 20 {
            tick_hist.pop_front();
        }

        // Short-term momentum: last 2-3 candles
        let c1 = candles[candles.len() - 1].close;
        let c2 = candles[candles.len() - 2].close;
        let c3 = candles[candles.len() - 3].close;

        // 1-candle momentum (most recent)
        let mom_1 = (c1 - c2) / c2;
        // 2-candle momentum
        let mom_2 = (c1 - c3) / c3;

        features.short_term_momentum = Some(mom_1);

        // Short-term velocity (change in momentum)
        if candles.len() >= 4 {
            let c4 = candles[candles.len() - 4].close;
            let prev_mom_1 = (c2 - c3) / c3;
            features.short_term_velocity = Some(mom_1 - prev_mom_1);
        }

        // Weighted momentum: recent candles weighted higher
        // Weight: 0.5 for most recent, 0.3 for second, 0.2 for third
        let weighted = mom_1 * 0.5
            + ((c2 - c3) / c3) * 0.3
            + ((c3
                - if candles.len() >= 4 {
                    candles[candles.len() - 4].close
                } else {
                    c3
                })
                / if candles.len() >= 4 {
                    candles[candles.len() - 4].close
                } else {
                    c3
                })
                * 0.2;
        features.weighted_momentum = Some(weighted);
    }

    /// Compute market timing score
    fn compute_market_timing_score(&self, features: &mut Features) {
        let mut score = 0.0;
        let mut factors = 0;

        // Factor 1: Window progress
        // Best entries are early in the window (0-40% progress)
        if let Some(progress) = features.window_progress {
            if progress < 0.3 {
                score += 0.3; // Early window bonus
            } else if progress > 0.7 {
                score -= 0.2; // Late window penalty
            }
            factors += 1;
        }

        // Factor 2: Price movement within window
        // If price hasn't moved much, it's a good time to enter (trend may be starting)
        if let Some(moved) = features.window_price_moved_abs {
            if moved < 0.001 {
                score += 0.2; // Very little movement = good entry
            } else if moved > 0.002 {
                score -= 0.15; // Significant movement = late entry risk
            }
            factors += 1;
        }

        // Factor 3: Short-term momentum alignment
        if let (Some(st_mom), Some(weighted)) =
            (features.short_term_momentum, features.weighted_momentum)
        {
            // If short-term momentum and weighted momentum align, boost score
            if st_mom.signum() == weighted.signum() && st_mom.abs() > 0.0005 {
                score += 0.2 * st_mom.signum();
            }
            factors += 1;
        }

        // Factor 4: Intra-window volatility
        if let Some(vol) = features.intra_window_volatility {
            if vol < 0.005 {
                score += 0.1; // Low volatility = more predictable
            } else if vol > 0.015 {
                score -= 0.15; // High volatility = unpredictable
            }
            factors += 1;
        }

        // Factor 5: Late entry penalty
        if features.late_entry_up {
            score -= 0.3; // Already moved up, don't buy UP
        }
        if features.late_entry_down {
            score += 0.3; // Already moved down, don't buy DOWN (score positive = UP)
        }

        if factors > 0 {
            // Normalize score to -1.0 to 1.0 range
            features.market_timing_score = Some((score / factors as f64).clamp(-1.0, 1.0));
        }
    }

    /// Compute orderbook/microstructure features from Polymarket
    fn compute_orderbook_features(&self, key: &(Asset, Timeframe), features: &mut Features) {
        // First try external tracker for advanced features
        if let Some(ref tracker) = self.orderbook_tracker {
            if let Ok(tracker) = tracker.lock() {
                let micro_features = tracker.get_features(*key);

                features.orderbook_imbalance = Some(micro_features.orderbook_imbalance);
                features.spread_bps = Some(micro_features.spread_bps);
                features.orderflow_delta = Some(micro_features.orderflow_delta);
                features.orderbook_bid_depth = Some(micro_features.bid_volume);
                features.orderbook_ask_depth = Some(micro_features.ask_volume);
                features.orderbook_depth_top5 =
                    Some((micro_features.bid_volume + micro_features.ask_volume).max(0.0));

                return;
            }
        }

        // Fallback to internal orderbook data
        if let Some(ob) = self.orderbook_data.get(key) {
            // Orderbook imbalance: (bid_vol - ask_vol) / (bid_vol + ask_vol)
            let total_vol = ob.bid_volume + ob.ask_volume;
            if total_vol > 0.0 {
                features.orderbook_imbalance = Some((ob.bid_volume - ob.ask_volume) / total_vol);
            }
            features.orderbook_bid_depth = Some(ob.bid_volume.max(0.0));
            features.orderbook_ask_depth = Some(ob.ask_volume.max(0.0));
            features.orderbook_depth_top5 = Some(total_vol.max(0.0));

            // Spread in basis points
            if ob.best_bid > 0.0 && ob.best_ask > 0.0 {
                let spread = (ob.best_ask - ob.best_bid) / ((ob.best_ask + ob.best_bid) / 2.0);
                features.spread_bps = Some(spread * 10000.0);
            }

            // Order flow delta (buy volume - sell volume)
            features.orderflow_delta = Some(ob.buy_volume - ob.sell_volume);
        }
    }

    /// Compute RSI using Wilder's smoothing method
    fn compute_rsi_wilders(&mut self, candles: &[Candle], key: (Asset, Timeframe)) -> Option<f64> {
        if candles.len() < self.rsi_period + 1 {
            tracing::debug!(
                asset = ?key.0,
                timeframe = ?key.1,
                candle_count = candles.len(),
                required = self.rsi_period + 1,
                "RSI: Not enough candles"
            );
            return None;
        }

        let has_prev_state = self.rsi_state.contains_key(&key);

        tracing::debug!(
            asset = ?key.0,
            timeframe = ?key.1,
            has_prev_state = has_prev_state,
            candle_count = candles.len(),
            "RSI: Computing with {} candles", candles.len()
        );

        if !has_prev_state {
            // First computation: use simple average to seed
            let mut gains = 0.0;
            let mut losses = 0.0;
            for i in (candles.len() - self.rsi_period)..candles.len() {
                let change = candles[i].close - candles[i - 1].close;
                if change > 0.0 {
                    gains += change;
                } else {
                    losses += change.abs();
                }
            }
            let avg_gain = gains / self.rsi_period as f64;
            let avg_loss = losses / self.rsi_period as f64;
            self.rsi_state.insert(key, (avg_gain, avg_loss));

            if avg_loss == 0.0 && avg_gain == 0.0 {
                return Some(50.0); // No movement = neutral
            }
            if avg_loss == 0.0 {
                return Some(100.0);
            }
            if avg_gain == 0.0 {
                return Some(0.0);
            }
            let rs = avg_gain / avg_loss;
            let rsi = 100.0 - (100.0 / (1.0 + rs));
            Some(rsi.clamp(1.0, 99.0)) // Clamp to avoid exact 0/100
        } else {
            // Wilder's smoothing: use previous avg_gain/avg_loss
            let (prev_avg_gain, prev_avg_loss) = *self.rsi_state.get(&key).unwrap();
            let period = self.rsi_period as f64;

            let last_change = candles[candles.len() - 1].close - candles[candles.len() - 2].close;
            let current_gain = if last_change > 0.0 { last_change } else { 0.0 };
            let current_loss = if last_change < 0.0 {
                last_change.abs()
            } else {
                0.0
            };

            let avg_gain = (prev_avg_gain * (period - 1.0) + current_gain) / period;
            let avg_loss = (prev_avg_loss * (period - 1.0) + current_loss) / period;

            self.rsi_state.insert(key, (avg_gain, avg_loss));

            if avg_loss < 1e-12 && avg_gain < 1e-12 {
                return Some(50.0); // No movement
            }
            if avg_loss < 1e-12 {
                return Some(99.0);
            }
            if avg_gain < 1e-12 {
                return Some(1.0);
            }
            let rs = avg_gain / avg_loss;
            let rsi = 100.0 - (100.0 / (1.0 + rs));
            Some(rsi.clamp(1.0, 99.0))
        }
    }

    /// Compute MACD with proper EMA-based signal line
    fn compute_macd_proper(
        &mut self,
        candles: &[Candle],
        key: (Asset, Timeframe),
    ) -> Option<(f64, f64, f64)> {
        let (fast, slow, signal_period) = self.macd_periods;

        let ema_fast = self.compute_ema(candles, fast)?;
        let ema_slow = self.compute_ema(candles, slow)?;
        let macd = ema_fast - ema_slow;

        // Store MACD value in history
        let history = self.macd_history.entry(key).or_insert_with(VecDeque::new);
        history.push_back(macd);
        if history.len() > 100 {
            history.pop_front();
        }

        // Compute signal line as EMA(signal_period) of MACD history
        let signal = if history.len() >= signal_period {
            let multiplier = 2.0 / (signal_period as f64 + 1.0);
            let slice: Vec<f64> = history.iter().copied().collect();
            let mut ema = slice[0];
            for val in slice.iter().skip(1) {
                ema = (val - ema) * multiplier + ema;
            }
            ema
        } else {
            // Not enough history yet, use simple average
            let sum: f64 = history.iter().sum();
            sum / history.len() as f64
        };

        let hist = macd - signal;
        Some((macd, signal, hist))
    }

    /// Compute Bollinger Bands (returns upper, middle, lower)
    fn compute_bollinger(&self, candles: &[Candle]) -> Option<(f64, f64, f64)> {
        let (period, multiplier) = self.bb_config;

        if candles.len() < period {
            return None;
        }

        let recent: Vec<f64> = candles.iter().rev().take(period).map(|c| c.close).collect();
        let sma = recent.iter().sum::<f64>() / period as f64;

        let variance: f64 = recent.iter().map(|p| (p - sma).powi(2)).sum::<f64>() / period as f64;
        let std = variance.sqrt();

        let upper = sma + multiplier * std;
        let lower = sma - multiplier * std;

        Some((upper, sma, lower))
    }

    /// Compute ATR (Average True Range)
    fn compute_atr(&self, candles: &[Candle]) -> Option<f64> {
        if candles.len() < self.atr_period + 1 {
            return None;
        }

        let mut tr_values = Vec::new();
        for i in 1..=self.atr_period {
            let idx = candles.len() - i;
            let curr = &candles[idx];
            let prev = &candles[idx - 1];

            let tr = (curr.high - curr.low)
                .max((curr.high - prev.close).abs())
                .max((curr.low - prev.close).abs());
            tr_values.push(tr);
        }

        Some(tr_values.iter().sum::<f64>() / tr_values.len() as f64)
    }

    /// Compute VWAP
    fn compute_vwap(&self, candles: &[Candle]) -> Option<f64> {
        if candles.is_empty() {
            return None;
        }

        let mut sum_pv = 0.0;
        let mut sum_volume = 0.0;

        for c in candles.iter().rev().take(50) {
            let typical = (c.high + c.low + c.close) / 3.0;
            sum_pv += typical * c.volume;
            sum_volume += c.volume;
        }

        if sum_volume > 0.0 {
            Some(sum_pv / sum_volume)
        } else {
            None
        }
    }

    /// Compute EMA
    fn compute_ema(&self, candles: &[Candle], period: usize) -> Option<f64> {
        if candles.len() < period {
            return None;
        }

        let multiplier = 2.0 / (period as f64 + 1.0);
        let mut ema = candles[0].close;

        for c in candles.iter().skip(1) {
            ema = (c.close - ema) * multiplier + ema;
        }

        Some(ema)
    }

    /// Compute volatility (std dev of returns)
    fn compute_volatility(&self, candles: &[Candle], period: usize) -> Option<f64> {
        if candles.len() < period + 1 {
            return None;
        }

        let mut returns = Vec::new();
        for i in (candles.len() - period)..candles.len() {
            let ret = (candles[i].close - candles[i - 1].close) / candles[i - 1].close;
            returns.push(ret);
        }

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance: f64 =
            returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;

        Some(variance.sqrt())
    }

    /// Compute Heikin Ashi close
    fn compute_heikin_ashi(&self, candles: &[Candle]) -> Option<f64> {
        let c = candles.last()?;
        Some((c.open + c.high + c.low + c.close) / 4.0)
    }

    /// Compute ADX (Average Directional Index)
    /// Returns (ADX, +DI, -DI)
    fn compute_adx(&self, candles: &[Candle]) -> Option<(f64, f64, f64)> {
        let period = self.adx_period;
        if candles.len() < period * 2 + 1 {
            return None;
        }

        let mut plus_dm_values = Vec::new();
        let mut minus_dm_values = Vec::new();
        let mut tr_values = Vec::new();

        let start = candles.len() - period * 2;
        for i in (start + 1)..candles.len() {
            let curr = &candles[i];
            let prev = &candles[i - 1];

            let up_move = curr.high - prev.high;
            let down_move = prev.low - curr.low;

            let plus_dm = if up_move > down_move && up_move > 0.0 {
                up_move
            } else {
                0.0
            };
            let minus_dm = if down_move > up_move && down_move > 0.0 {
                down_move
            } else {
                0.0
            };

            let tr = (curr.high - curr.low)
                .max((curr.high - prev.close).abs())
                .max((curr.low - prev.close).abs());

            plus_dm_values.push(plus_dm);
            minus_dm_values.push(minus_dm);
            tr_values.push(tr);
        }

        if tr_values.is_empty() {
            return None;
        }

        // Smoothed averages using Wilder's method
        let mut smoothed_plus_dm = plus_dm_values.iter().take(period).sum::<f64>();
        let mut smoothed_minus_dm = minus_dm_values.iter().take(period).sum::<f64>();
        let mut smoothed_tr = tr_values.iter().take(period).sum::<f64>();

        let mut dx_values = Vec::new();

        for i in period..tr_values.len() {
            smoothed_plus_dm =
                smoothed_plus_dm - (smoothed_plus_dm / period as f64) + plus_dm_values[i];
            smoothed_minus_dm =
                smoothed_minus_dm - (smoothed_minus_dm / period as f64) + minus_dm_values[i];
            smoothed_tr = smoothed_tr - (smoothed_tr / period as f64) + tr_values[i];

            if smoothed_tr == 0.0 {
                continue;
            }

            let plus_di = 100.0 * smoothed_plus_dm / smoothed_tr;
            let minus_di = 100.0 * smoothed_minus_dm / smoothed_tr;

            let di_sum = plus_di + minus_di;
            if di_sum > 0.0 {
                let dx = 100.0 * (plus_di - minus_di).abs() / di_sum;
                dx_values.push((dx, plus_di, minus_di));
            }
        }

        if dx_values.is_empty() {
            return None;
        }

        // ADX is smoothed average of DX
        let adx: f64 = dx_values.iter().map(|(dx, _, _)| dx).sum::<f64>() / dx_values.len() as f64;
        let (_, last_plus_di, last_minus_di) = dx_values.last().unwrap();

        Some((adx, *last_plus_di, *last_minus_di))
    }

    /// Compute Stochastic RSI
    /// Returns (StochRSI, %K, %D)
    fn compute_stoch_rsi(&self, key: (Asset, Timeframe)) -> Option<(f64, f64, f64)> {
        let history = self.rsi_history.get(&key)?;
        let period = self.stoch_rsi_period;

        if history.len() < period {
            return None;
        }

        let recent: Vec<f64> = history.iter().rev().take(period).copied().collect();
        let current_rsi = *history.back()?;

        let min_rsi = recent.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_rsi = recent.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        let range = max_rsi - min_rsi;
        if range == 0.0 {
            return Some((0.5, 0.5, 0.5));
        }

        let stoch_rsi = (current_rsi - min_rsi) / range;

        // %K = 3-period SMA of StochRSI (simplified: use recent values)
        let k = if history.len() >= period + 2 {
            let mut k_values = Vec::new();
            for i in 0..3 {
                let idx = history.len() - 1 - i;
                let rsi_val = history[idx];
                let k_val = (rsi_val - min_rsi) / range;
                k_values.push(k_val);
            }
            k_values.iter().sum::<f64>() / 3.0
        } else {
            stoch_rsi
        };

        // %D = 3-period SMA of %K (simplified)
        let d = k; // Simplified: would need %K history for proper computation

        Some((stoch_rsi, k, d))
    }

    /// Compute OBV (On Balance Volume)
    /// Returns (OBV, OBV slope)
    fn compute_obv(&mut self, candles: &[Candle], key: (Asset, Timeframe)) -> Option<(f64, f64)> {
        if candles.len() < 2 {
            return None;
        }

        let last = candles.last()?;
        let prev_close = self.prev_close.get(&key).copied().unwrap_or(last.close);
        let current_obv = self.obv_state.get(&key).copied().unwrap_or(0.0);

        let new_obv = if last.close > prev_close {
            current_obv + last.volume
        } else if last.close < prev_close {
            current_obv - last.volume
        } else {
            current_obv
        };

        self.obv_state.insert(key, new_obv);
        self.prev_close.insert(key, last.close);

        // OBV slope: compare current OBV to OBV from ~5 candles ago
        // FIX: Actually look at the OBV change over last 5 candles worth of data
        // Instead of just the last tick, compute the real slope
        let obv_history = self.obv_state.entry(key).or_insert(0.0);

        // For slope, we look at the cumulative volume direction over recent candles
        // Calculate OBV change over last 5 candles directly from candle data
        let slope = if candles.len() >= 5 {
            let obv_5_ago: f64 = candles.iter().rev().skip(1).take(4).map(|c| c.volume).sum();
            let recent_direction: f64 = candles
                .iter()
                .rev()
                .take(5)
                .map(|c| {
                    if c.close > c.open {
                        c.volume
                    } else if c.close < c.open {
                        -c.volume
                    } else {
                        0.0
                    }
                })
                .sum();
            recent_direction
        } else {
            new_obv - current_obv
        };

        Some((new_obv, slope))
    }

    /// Compute relative volume (current volume / average volume)
    fn compute_relative_volume(&self, candles: &[Candle], period: usize) -> Option<f64> {
        if candles.len() < period + 1 {
            return None;
        }

        let current_volume = candles.last()?.volume;
        let avg_volume: f64 = candles
            .iter()
            .rev()
            .skip(1)
            .take(period)
            .map(|c| c.volume)
            .sum::<f64>()
            / period as f64;

        if avg_volume > 0.0 {
            Some(current_volume / avg_volume)
        } else {
            None
        }
    }

    /// Detect market regime based on computed features
    fn detect_regime(&self, features: &Features) -> MarketRegime {
        // Use ADX for trend strength
        if let Some(adx) = features.adx {
            if adx > 25.0 {
                return MarketRegime::Trending;
            }
        }

        // Use ATR spike detection for volatile regime
        if let (Some(atr_pct), Some(volatility)) = (features.atr_pct, features.volatility) {
            // If volatility is more than 2x typical, it's volatile
            if volatility > 0.02 || atr_pct > 0.03 {
                return MarketRegime::Volatile;
            }
        }

        // Default: ranging
        MarketRegime::Ranging
    }

    /// Convert features to FeatureSet for strategy
    pub fn to_feature_set(&self, features: &Features) -> FeatureSet {
        FeatureSet {
            ts: features.ts,
            asset: features.asset,
            timeframe: features.timeframe,
            rsi: features.rsi.unwrap_or(50.0),
            macd_line: features.macd.unwrap_or(0.0),
            macd_signal: features.macd_signal.unwrap_or(0.0),
            macd_hist: features.macd_hist.unwrap_or(0.0),
            vwap: features.vwap.unwrap_or(0.0),
            bb_upper: features.bb_upper.unwrap_or(0.0),
            bb_lower: features.bb_lower.unwrap_or(0.0),
            atr: features.atr.unwrap_or(0.0),
            momentum: features.momentum.unwrap_or(0.0),
            momentum_accel: features.velocity.unwrap_or(0.0),
            book_imbalance: 0.0,
            spread_bps: 0.0,
            trade_intensity: 0.0,
            ha_close: features.ha_close.unwrap_or(features.close),
            ha_trend: features
                .ha_trend
                .map(|d| if d == Direction::Up { 1 } else { -1 })
                .unwrap_or(0),
            oracle_confidence: 1.0,
            adx: features.adx.unwrap_or(0.0),
            stoch_rsi: features.stoch_rsi.unwrap_or(0.5),
            obv: features.obv.unwrap_or(0.0),
            relative_volume: features.relative_volume.unwrap_or(1.0),
            regime: match features.regime {
                MarketRegime::Trending => 1,
                MarketRegime::Ranging => 0,
                MarketRegime::Volatile => -1,
            },
        }
    }
}

impl Default for FeatureEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candle(asset: Asset, ts: i64, close: f64) -> Candle {
        Candle {
            open_time: ts,
            close_time: ts + 900000,
            asset,
            timeframe: Timeframe::Min15,
            open: close - 10.0,
            high: close + 20.0,
            low: close - 20.0,
            close,
            volume: 1000.0,
            trades: 100,
        }
    }

    fn make_candle_with_volume(asset: Asset, ts: i64, close: f64, volume: f64) -> Candle {
        Candle {
            open_time: ts,
            close_time: ts + 900000,
            asset,
            timeframe: Timeframe::Min15,
            open: close - 10.0,
            high: close + 20.0,
            low: close - 20.0,
            close,
            volume,
            trades: 100,
        }
    }

    #[test]
    fn test_rsi_computation_wilders() {
        let mut engine = FeatureEngine::new();
        let mut candles = Vec::new();

        // Create 20 candles with varying prices (uptrend)
        for i in 0..20 {
            let close = 50000.0 + (i as f64 * 10.0);
            candles.push(make_candle(Asset::BTC, 1700000000000 + i * 900000, close));
        }

        let features = engine.compute(&candles).unwrap();
        assert!(features.rsi.is_some());
        // RSI should be high (uptrend)
        assert!(features.rsi.unwrap() > 50.0);
    }

    #[test]
    fn test_rsi_wilders_smoothing() {
        let mut engine = FeatureEngine::new();
        let mut candles = Vec::new();

        // Create initial candles
        for i in 0..20 {
            let close = 50000.0 + (i as f64 * 10.0);
            candles.push(make_candle(Asset::BTC, 1700000000000 + i * 900000, close));
        }

        // First computation seeds the state
        let features1 = engine.compute(&candles).unwrap();
        assert!(features1.rsi.is_some());

        // Add another candle and compute again (should use Wilder's smoothing)
        candles.push(make_candle(
            Asset::BTC,
            1700000000000 + 20 * 900000,
            50210.0,
        ));
        let features2 = engine.compute(&candles).unwrap();
        assert!(features2.rsi.is_some());
    }

    #[test]
    fn test_macd_proper_signal_line() {
        let mut engine = FeatureEngine::new();
        let mut candles = Vec::new();

        // Create 30 candles for MACD computation
        for i in 0..30 {
            let close = 50000.0 + (i as f64 * 5.0);
            candles.push(make_candle(Asset::BTC, 1700000000000 + i * 900000, close));
        }

        // Compute multiple times to build MACD history
        for i in 0..10 {
            candles.push(make_candle(
                Asset::BTC,
                1700000000000 + (30 + i) * 900000,
                50150.0 + i as f64 * 3.0,
            ));
            engine.compute(&candles);
        }

        let features = engine.compute(&candles).unwrap();
        assert!(features.macd.is_some());
        assert!(features.macd_signal.is_some());
        assert!(features.macd_hist.is_some());
        // Signal should NOT just be macd * 0.8
        let macd = features.macd.unwrap();
        let signal = features.macd_signal.unwrap();
        assert!((signal - macd * 0.8).abs() > 0.0001 || macd.abs() < 0.001);
    }

    #[test]
    fn test_heikin_ashi_trend() {
        let mut engine = FeatureEngine::new();
        let mut candles = Vec::new();

        // Create uptrending candles
        for i in 0..25 {
            let close = 50000.0 + (i as f64 * 50.0);
            candles.push(make_candle(Asset::BTC, 1700000000000 + i * 900000, close));
        }

        // First compute to set prev_ha_close
        engine.compute(&candles);

        // Add another higher candle
        candles.push(make_candle(
            Asset::BTC,
            1700000000000 + 25 * 900000,
            51300.0,
        ));
        let features = engine.compute(&candles).unwrap();

        // In an uptrend, HA close should be trending up
        assert!(features.ha_close.is_some());
        if let Some(trend) = features.ha_trend {
            assert_eq!(trend, Direction::Up);
        }
    }

    #[test]
    fn test_bb_computation() {
        let mut engine = FeatureEngine::new();
        let mut candles = Vec::new();

        for i in 0..25 {
            let close = 50000.0 + ((i % 5) as f64 * 100.0 - 200.0);
            candles.push(make_candle(Asset::ETH, 1700000000000 + i * 900000, close));
        }

        let features = engine.compute(&candles).unwrap();
        assert!(features.bb_upper.is_some());
        assert!(features.bb_middle.is_some());
        assert!(features.bb_lower.is_some());
        assert!(features.bb_upper.unwrap() > features.bb_lower.unwrap());
    }

    #[test]
    fn test_adx_computation() {
        let mut engine = FeatureEngine::new();
        let mut candles = Vec::new();

        // Create 50 candles with a strong trend for ADX
        for i in 0..50 {
            let close = 50000.0 + (i as f64 * 100.0);
            candles.push(make_candle(Asset::BTC, 1700000000000 + i * 900000, close));
        }

        let features = engine.compute(&candles).unwrap();
        assert!(features.adx.is_some());
        assert!(features.plus_di.is_some());
        assert!(features.minus_di.is_some());
    }

    #[test]
    fn test_relative_volume() {
        let mut engine = FeatureEngine::new();
        let mut candles = Vec::new();

        // Create 25 candles with normal volume, last one with high volume
        for i in 0..24 {
            candles.push(make_candle_with_volume(
                Asset::BTC,
                1700000000000 + i * 900000,
                50000.0,
                1000.0,
            ));
        }
        // Last candle with 3x volume
        candles.push(make_candle_with_volume(
            Asset::BTC,
            1700000000000 + 24 * 900000,
            50000.0,
            3000.0,
        ));

        let features = engine.compute(&candles).unwrap();
        assert!(features.relative_volume.is_some());
        // Should be approximately 3.0 (3x average)
        assert!(features.relative_volume.unwrap() > 2.5);
    }

    #[test]
    fn test_regime_detection() {
        let mut features = Features::default();
        let engine = FeatureEngine::new();

        // Trending regime: high ADX
        features.adx = Some(30.0);
        assert_eq!(engine.detect_regime(&features), MarketRegime::Trending);

        // Volatile regime: high volatility
        features.adx = Some(15.0);
        features.atr_pct = Some(0.05);
        features.volatility = Some(0.03);
        assert_eq!(engine.detect_regime(&features), MarketRegime::Volatile);

        // Ranging regime: low ADX, low volatility
        features.adx = Some(15.0);
        features.atr_pct = Some(0.01);
        features.volatility = Some(0.01);
        assert_eq!(engine.detect_regime(&features), MarketRegime::Ranging);
    }
}
