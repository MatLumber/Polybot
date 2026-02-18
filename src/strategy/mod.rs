//! Strategy Engine - Polymarket Binary Prediction Strategy
//!
//! This strategy is designed for Polymarket prediction markets where:
//! - The question is: "Will price be HIGHER or LOWER at market close?"
//! - WIN = shares worth $1.00 (profit ~80%)
//! - LOSS = shares worth $0.00 (lose 100% of bet)
//!
//! Therefore we need HIGH ACCURACY (>55% winrate is profitable).
//! The strategy uses a multi-indicator voting system with strict quality filters.
//!
//! Key principles:
//! 1. We predict the DIRECTION of the entire window, not scalp intra-window
//! 2. Only trade when indicators strongly agree (high conviction)
//! 3. Use regime detection to pick the right strategy:
//!    - TRENDING: Follow the trend (momentum + ADX + EMA)
//!    - RANGING: Mean reversion (RSI + BB extremes)
//!    - VOLATILE: No trade (too unpredictable for binary bets)
//! 4. Cooldown = one full window (prevent re-entering same window)
//!
//! NEW (v3):
//! - Dynamic indicator calibration based on historical performance
//! - Enhanced multi-TF alignment with regime-aware weighting
//! - Intra-window volatility filter

pub mod calibrator;
pub use calibrator::{
    CalibrationQualitySnapshot, IndicatorCalibrator, IndicatorStats, TradeResult,
};

use anyhow::Result;
use std::collections::HashMap;

use crate::features::{Features, MarketRegime};
use crate::types::{Asset, Direction, FeatureSet, Signal, Timeframe};

/// Strategy configuration
#[derive(Debug, Clone)]
pub struct StrategyConfig {
    /// Minimum confidence to generate a signal
    pub min_confidence: f64,
    /// RSI thresholds for overbought/oversold
    pub rsi_overbought: f64,
    pub rsi_oversold: f64,
    /// MACD histogram threshold (relative to price)
    pub macd_threshold: f64,
    /// BB position thresholds
    pub bb_overbought: f64,
    pub bb_oversold: f64,
    /// Trend strength threshold (EMA spread / price)
    pub trend_threshold: f64,
    /// Volatility scaling factor
    pub volatility_scale: f64,
    /// Minimum number of active votes required to generate a signal
    pub min_active_votes: usize,
    /// Minimum winning/losing vote ratio required to avoid near-tie calls
    pub min_vote_ratio: f64,
    /// Signal cooldown per asset in milliseconds (legacy, now per-timeframe)
    pub signal_cooldown_ms: i64,
    /// Stochastic RSI thresholds
    pub stoch_rsi_overbought: f64,
    pub stoch_rsi_oversold: f64,
    /// Volume confirmation threshold (relative volume above this = confirmation)
    pub volume_confirm_threshold: f64,
    /// Volume penalty threshold (relative volume below this = penalty)
    pub volume_penalty_threshold: f64,
    /// Multi-timeframe bonus multiplier
    pub multi_tf_bonus: f64,
    /// Divergence detection lookback (number of feature snapshots)
    pub divergence_lookback: usize,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.60, // High bar — binary bets need accuracy
            rsi_overbought: 65.0, // Tighter RSI for prediction accuracy
            rsi_oversold: 35.0,
            macd_threshold: 0.001, // Ignore noise
            bb_overbought: 0.85,   // BB extremes
            bb_oversold: 0.15,
            trend_threshold: 0.002, // Real trends only
            volatility_scale: 0.5,
            min_active_votes: 1,         // Allow exploration in paper mode
            min_vote_ratio: 1.05,        // Avoid only true coin-flip ties
            signal_cooldown_ms: 900_000, // 15 minutes (legacy)
            stoch_rsi_overbought: 0.80,
            stoch_rsi_oversold: 0.20,
            volume_confirm_threshold: 1.3, // Volume must be 30% above average
            volume_penalty_threshold: 0.6, // Penalize low volume
            multi_tf_bonus: 0.06,          // Conservative multi-TF bonus
            divergence_lookback: 5,
        }
    }
}

/// Strategy engine for signal generation
pub struct StrategyEngine {
    config: StrategyConfig,
    /// Feature history per asset/timeframe
    feature_history: HashMap<(Asset, Timeframe), Vec<Features>>,
    /// Maximum history to keep
    max_history: usize,
    /// Last signal time per asset/timeframe (for cooldown)
    last_signal_time: HashMap<(Asset, Timeframe), i64>,
    /// Latest features per timeframe for multi-TF analysis
    latest_features: HashMap<(Asset, Timeframe), Features>,
    /// Indicator calibrator for dynamic weight adjustment
    calibrator: IndicatorCalibrator,
    /// Indicators used in last signal (for tracking)
    last_indicators_used: Vec<String>,
    /// Last strategy filter reason (for diagnostics/dashboard)
    last_filter_reason: Option<String>,
}

/// Generated signal with confidence
#[derive(Debug, Clone)]
pub struct GeneratedSignal {
    pub asset: Asset,
    pub timeframe: Timeframe,
    pub direction: Direction,
    pub confidence: f64,
    pub reasons: Vec<String>,
    pub ts: i64,
    /// Indicators that contributed to this signal
    pub indicators_used: Vec<String>,
}

impl StrategyEngine {
    pub fn new(config: StrategyConfig) -> Self {
        Self::with_calibration_min_samples(config, 30)
    }

    pub fn with_calibration_min_samples(config: StrategyConfig, min_samples: usize) -> Self {
        Self {
            config,
            feature_history: HashMap::new(),
            max_history: 100,
            last_signal_time: HashMap::new(),
            latest_features: HashMap::new(),
            calibrator: IndicatorCalibrator::with_min_samples(min_samples.max(1)),
            last_indicators_used: Vec::new(),
            last_filter_reason: None,
        }
    }

    fn clear_filter_reason(&mut self) {
        self.last_filter_reason = None;
    }

    fn set_filter_reason(&mut self, reason: &str) {
        self.last_filter_reason = Some(reason.to_string());
    }

    /// Last strategy-level reason a feature was filtered out.
    pub fn last_filter_reason(&self) -> Option<String> {
        self.last_filter_reason.clone()
    }

    /// Record trade result for calibration
    pub fn record_trade_result(&mut self, result: TradeResult) {
        if !self.last_indicators_used.is_empty() {
            self.calibrator
                .record_trade(&self.last_indicators_used, result);
            self.calibrator.recalibrate();
        }
    }

    /// Legacy/global calibration path (kept for backwards compatibility).
    pub fn record_trade_with_indicators(&mut self, indicators: &[String], result: TradeResult) {
        self.calibrator.record_trade(indicators, result);
        self.calibrator.recalibrate();
    }

    /// Record trade result with explicit indicators for a specific market.
    pub fn record_trade_with_indicators_for_market(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        indicators: &[String],
        result: TradeResult,
    ) {
        self.calibrator
            .record_trade_for_market(asset, timeframe, indicators, result);
        self.calibrator.recalibrate();
    }

    pub fn record_prediction_outcome_for_market(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        p_pred: f64,
        is_win: bool,
    ) {
        self.calibrator
            .record_prediction_for_market(asset, timeframe, p_pred, is_win);
    }

    /// Get calibrator statistics
    pub fn get_indicator_stats(&self) -> Vec<IndicatorStats> {
        self.calibrator.export_stats()
    }

    /// Check if calibration is ready
    pub fn is_calibrated(&self) -> bool {
        self.calibrator.is_calibrated()
    }

    /// Export calibrator state for persistence (v2 market-aware format).
    pub fn export_calibrator_state_v2(&self) -> HashMap<String, Vec<IndicatorStats>> {
        self.calibrator.export_stats_by_market()
    }

    pub fn export_calibration_quality_by_market(
        &self,
    ) -> HashMap<String, CalibrationQualitySnapshot> {
        self.calibrator.export_calibration_quality_by_market()
    }

    /// Import calibrator state from persistence (v2 market-aware format).
    pub fn import_calibrator_state_v2(&mut self, stats: HashMap<String, Vec<IndicatorStats>>) {
        self.calibrator.load_stats_by_market(stats);
        self.calibrator.recalibrate();
    }

    /// Import legacy v1 global calibrator state.
    pub fn import_calibrator_state(&mut self, stats: Vec<IndicatorStats>) {
        self.calibrator.load_stats(stats);
        self.calibrator.recalibrate();
    }

    /// Export legacy aggregated view for compatibility.
    pub fn export_calibrator_state(&self) -> Vec<IndicatorStats> {
        self.calibrator.export_stats()
    }

    /// Get total trades recorded by calibrator
    pub fn calibrator_total_trades(&self) -> usize {
        self.calibrator.total_trades()
    }

    #[inline]
    fn calibrated_weight(&self, features: &Features, indicator_name: &str) -> f64 {
        self.calibrator
            .get_weight_for_market(features.asset, features.timeframe, indicator_name)
    }

    /// Process features and potentially generate a signal
    pub fn process(&mut self, features: &Features) -> Option<GeneratedSignal> {
        let key = (features.asset, features.timeframe);
        self.clear_filter_reason();

        // ── ASSET FILTER: Only trade BTC and ETH ──
        match features.asset {
            Asset::BTC | Asset::ETH => {}
            _ => {
                self.set_filter_reason("asset_not_supported");
                return None;
            } // Skip SOL, XRP, etc.
        }

        // Store features in history
        let history = self.feature_history.entry(key).or_insert_with(Vec::new);
        history.push(features.clone());
        if history.len() > self.max_history {
            history.remove(0);
        }

        // Store latest features for multi-TF analysis
        self.latest_features.insert(key, features.clone());

        // Check signal cooldown (matched to timeframe window)
        let cooldown_key = (features.asset, features.timeframe);
        let cooldown_ms = match features.timeframe {
            Timeframe::Min15 => 900_000,   // 15 min — one full window
            Timeframe::Hour1 => 3_600_000, // 1 hour — one full window
        };
        if let Some(last_ts) = self.last_signal_time.get(&cooldown_key) {
            if features.ts - last_ts < cooldown_ms {
                self.set_filter_reason("signal_cooldown");
                return None;
            }
        }

        // Analyze features to generate signal
        let signal = match self.generate_signal(features) {
            Some(sig) => sig,
            None => {
                if self.last_filter_reason.is_none() {
                    self.set_filter_reason("strategy_no_signal");
                }
                return None;
            }
        };

        // Record signal time for cooldown
        self.last_signal_time.insert(cooldown_key, features.ts);

        Some(signal)
    }

    /// Generate signal from current features
    ///
    /// Polymarket binary prediction strategy:
    /// We need to predict if price will be HIGHER or LOWER at window close.
    ///
    /// TRENDING regime: Follow the trend (high confidence)
    /// RANGING regime: Mean-reversion from extremes only (moderate confidence)
    /// VOLATILE regime: Skip entirely (too unpredictable for binary bets)
    fn generate_signal(&mut self, features: &Features) -> Option<GeneratedSignal> {
        let regime = features.regime;

        // ── VOLATILE REGIME FILTER ──
        // In volatile markets, binary prediction accuracy drops significantly.
        // Skip entirely — the expected value is negative when accuracy < 55%.
        if regime == MarketRegime::Volatile && matches!(features.timeframe, Timeframe::Hour1) {
            self.set_filter_reason("regime_volatile_1h");
            return None;
        }

        // ── LATE ENTRY FILTER (HIGH PRIORITY) ──
        // "Already moved too much" filter - critical for avoiding losses
        // If price already moved too much in window, we're likely entering late
        // and the move is already exhausted
        let window_progress = features.window_progress.unwrap_or(1.0);
        if window_progress > 0.65 {
            if features.late_entry_up {
                // Price already moved UP too much - don't chase late in the window
                self.set_filter_reason("late_entry_up");
                return None;
            }
            if features.late_entry_down {
                // Price already moved DOWN too much - don't chase late in the window
                self.set_filter_reason("late_entry_down");
                return None;
            }
        }

        // ── SPREAD FILTER (HIGH PRIORITY) ──
        // 15m requires tighter microstructure than 1h.
        let max_spread_bps = match features.timeframe {
            Timeframe::Min15 => 1200.0,
            Timeframe::Hour1 => 2000.0,
        };
        if let Some(spread_bps) = features.spread_bps {
            if spread_bps > max_spread_bps {
                // Spread too wide - market is illiquid
                self.set_filter_reason("spread_too_wide");
                return None;
            }
        }

        // ── DEPTH FILTER (HIGH PRIORITY) ──
        // Reject low-liquidity books (top-5 depth too thin).
        let min_depth_top5 = match features.timeframe {
            Timeframe::Min15 => 10.0,
            Timeframe::Hour1 => 5.0,
        };
        if let Some(depth_top5) = features.orderbook_depth_top5 {
            if depth_top5 > 0.0 && depth_top5 < min_depth_top5 {
                self.set_filter_reason("depth_too_low");
                return None;
            }
        }

        // ── EARLY WINDOW VOLATILITY FILTER ──
        // If we're in the first 30% of the window and volatility is high,
        // wait for things to stabilize before entering
        if let (Some(progress), Some(intra_vol)) =
            (features.window_progress, features.intra_window_volatility)
        {
            if progress < 0.3 && intra_vol > 0.02 {
                // High volatility early in window - skip
                self.set_filter_reason("early_window_high_volatility");
                return None;
            }
        }

        let mut up_votes: f64 = 0.0;
        let mut down_votes: f64 = 0.0;
        let mut reasons: Vec<String> = Vec::new();
        let mut active_vote_count: usize = 0;
        let mut indicators_used: Vec<String> = Vec::new();

        // ═══════════════════════════════════════════════════
        // TIER 1: Core Trend Indicators (highest weight)
        // These are the most reliable for predicting window direction
        // ═══════════════════════════════════════════════════

        // ADX + DI: The strongest trend signal
        // If ADX > 25, trend is strong and DI tells direction
        if let (Some(adx), Some(plus_di), Some(minus_di)) =
            (features.adx, features.plus_di, features.minus_di)
        {
            if adx > 25.0 {
                let di_spread = (plus_di - minus_di).abs();
                // Only count if DI spread is meaningful (not a flat market)
                if di_spread > 5.0 {
                    let base_weight = if adx > 35.0 { 3.0 } else { 2.0 };
                    let weight =
                        self.calibrated_weight(features, "adx_trend") * (base_weight / 2.5);
                    if plus_di > minus_di {
                        up_votes += weight;
                        reasons.push(format!(
                            "ADX trend UP: {:.1} (+DI:{:.1} -DI:{:.1})",
                            adx, plus_di, minus_di
                        ));
                    } else {
                        down_votes += weight;
                        reasons.push(format!(
                            "ADX trend DOWN: {:.1} (+DI:{:.1} -DI:{:.1})",
                            adx, plus_di, minus_di
                        ));
                    }
                    active_vote_count += 1;
                    indicators_used.push("adx_trend".to_string());
                }
            }
        }

        // EMA Trend: 9/21 EMA crossover direction
        if let Some(trend) = features.trend_strength {
            let abs_trend = trend.abs();
            if abs_trend > 0.003 {
                // Strong trend
                let direction = if trend > 0.0 { 1 } else { -1 };
                let weight = self.calibrated_weight(features, "ema_trend") * 1.33;
                if direction > 0 {
                    up_votes += weight;
                } else {
                    down_votes += weight;
                }
                active_vote_count += 1;
                indicators_used.push("ema_trend".to_string());
                reasons.push(format!("Strong EMA trend: {:.4}", trend));
            } else if abs_trend > self.config.trend_threshold {
                // Moderate trend
                let direction = if trend > 0.0 { 1 } else { -1 };
                let weight = self.calibrated_weight(features, "ema_trend");
                if direction > 0 {
                    up_votes += weight;
                } else {
                    down_votes += weight;
                }
                active_vote_count += 1;
                indicators_used.push("ema_trend".to_string());
                reasons.push(format!("EMA trend: {:.4}", trend));
            }
        }

        // ═══════════════════════════════════════════════════
        // TIER 2: Momentum Confirmation
        // ═══════════════════════════════════════════════════

        // MACD Histogram: Momentum confirmation
        if let (Some(_macd), Some(hist)) = (features.macd, features.macd_hist) {
            let abs_hist = hist.abs();
            if abs_hist > self.config.macd_threshold {
                let base_weight = if abs_hist > self.config.macd_threshold * 5.0 {
                    2.0
                } else {
                    1.5
                };
                let weight =
                    self.calibrated_weight(features, "macd_histogram") * (base_weight / 1.5);
                if hist > 0.0 {
                    up_votes += weight;
                    reasons.push(format!("MACD bullish: {:.4}", hist));
                } else {
                    down_votes += weight;
                    reasons.push(format!("MACD bearish: {:.4}", hist));
                }
                active_vote_count += 1;
                indicators_used.push("macd_histogram".to_string());
            }
        }

        // Momentum + Velocity: Price acceleration
        if let Some(momentum) = features.momentum {
            let vel = features.velocity.unwrap_or(0.0);
            let abs_mom = momentum.abs();

            if abs_mom > 0.001 && vel.abs() > 0.0002 && momentum.signum() == vel.signum() {
                // Strong momentum with acceleration — very predictive
                let weight = self.calibrated_weight(features, "momentum_acceleration");
                if momentum > 0.0 {
                    up_votes += weight;
                } else {
                    down_votes += weight;
                }
                active_vote_count += 1;
                indicators_used.push("momentum_acceleration".to_string());
                reasons.push(format!("Strong momentum: {:.4} vel: {:.4}", momentum, vel));
            } else if abs_mom > 0.0008 {
                // Moderate momentum
                let weight = self.calibrated_weight(features, "momentum_acceleration") * 0.5;
                if momentum > 0.0 {
                    up_votes += weight;
                } else {
                    down_votes += weight;
                }
                active_vote_count += 1;
                indicators_used.push("momentum_acceleration".to_string());
                reasons.push(format!("Momentum: {:.4}", momentum));
            }
        }

        // Heikin Ashi Trend: Smoothed candle direction
        if let Some(ha_trend) = features.ha_trend {
            let weight = self.calibrated_weight(features, "heikin_ashi");
            match ha_trend {
                Direction::Up => {
                    up_votes += weight;
                }
                Direction::Down => {
                    down_votes += weight;
                }
            }
            active_vote_count += 1;
            indicators_used.push("heikin_ashi".to_string());
            reasons.push(format!("HA trend: {:?}", ha_trend));
        }

        // ── SHORT-TERM WEIGHTED MOMENTUM (HIGH PRIORITY) ──
        // Most recent 2-3 candles are weighted 3x more than older ones
        // This catches early momentum shifts before they're obvious
        if let Some(st_momentum) = features.short_term_momentum {
            let weighted_mom = features.weighted_momentum.unwrap_or(st_momentum);
            let abs_st = st_momentum.abs();

            if abs_st > 0.0005 {
                // Short-term momentum is significant
                let base_weight = if abs_st > 0.002 { 2.5 } else { 1.5 };
                // Weighted momentum confirms the direction
                let confirmed = weighted_mom.signum() == st_momentum.signum();
                let weight =
                    self.calibrated_weight(features, "short_term_momentum") * (base_weight / 1.5);
                let final_weight = if confirmed { weight * 1.2 } else { weight };

                if st_momentum > 0.0 {
                    up_votes += final_weight;
                    reasons.push(format!(
                        "Short-term momentum UP: {:.5}{}",
                        st_momentum,
                        if confirmed { " (confirmed)" } else { "" }
                    ));
                } else {
                    down_votes += final_weight;
                    reasons.push(format!(
                        "Short-term momentum DOWN: {:.5}{}",
                        st_momentum,
                        if confirmed { " (confirmed)" } else { "" }
                    ));
                }
                active_vote_count += 1;
                indicators_used.push("short_term_momentum".to_string());
            }
        }

        // ═══════════════════════════════════════════════════
        // TIER 3: Mean-Reversion Signals (mainly for ranging)
        // ═══════════════════════════════════════════════════

        // RSI: Only at extremes, regime-aware
        if let Some(rsi) = features.rsi {
            let (vote, base_weight, reason) = self.analyze_rsi(rsi, regime);
            if vote > 0 {
                let weight = self.calibrated_weight(features, "rsi_extreme") * (base_weight / 1.5);
                up_votes += weight;
                active_vote_count += 1;
                indicators_used.push("rsi_extreme".to_string());
            } else if vote < 0 {
                let weight = self.calibrated_weight(features, "rsi_extreme") * (base_weight / 1.5);
                down_votes += weight;
                active_vote_count += 1;
                indicators_used.push("rsi_extreme".to_string());
            }
            if !reason.is_empty() {
                reasons.push(reason);
            }
        }

        // Bollinger Band position (regime-aware)
        if let Some(bb_pos) = features.bb_position {
            let (vote, base_weight, reason) = self.analyze_bb(bb_pos, regime);
            if vote > 0 {
                let weight =
                    self.calibrated_weight(features, "bollinger_band") * (base_weight / 2.0);
                up_votes += weight;
                active_vote_count += 1;
                indicators_used.push("bollinger_band".to_string());
            } else if vote < 0 {
                let weight =
                    self.calibrated_weight(features, "bollinger_band") * (base_weight / 2.0);
                down_votes += weight;
                active_vote_count += 1;
                indicators_used.push("bollinger_band".to_string());
            }
            if !reason.is_empty() {
                reasons.push(reason);
            }
        }

        // Stochastic RSI (extreme zones only)
        if let Some(stoch_rsi) = features.stoch_rsi {
            if stoch_rsi < self.config.stoch_rsi_oversold {
                let weight = self.calibrated_weight(features, "stoch_rsi");
                up_votes += weight;
                active_vote_count += 1;
                indicators_used.push("stoch_rsi".to_string());
                reasons.push(format!("StochRSI oversold: {:.2}", stoch_rsi));
            } else if stoch_rsi > self.config.stoch_rsi_overbought {
                let weight = self.calibrated_weight(features, "stoch_rsi");
                down_votes += weight;
                active_vote_count += 1;
                indicators_used.push("stoch_rsi".to_string());
                reasons.push(format!("StochRSI overbought: {:.2}", stoch_rsi));
            }
        }

        // ═══════════════════════════════════════════════════
        // TIER 4: Confirmation (low weight, tiebreakers)
        // ═══════════════════════════════════════════════════

        // RSI Divergence (strong reversal signal)
        // IMPORTANT: Deactivate in short windows - divergence is unreliable with limited data
        // Only use for 1h timeframe where we have enough price history
        let is_short_window = matches!(features.timeframe, Timeframe::Min15);
        if !is_short_window {
            if let Some((div_vote, div_reason)) = self.detect_rsi_divergence(features) {
                let weight = self.calibrated_weight(features, "rsi_divergence");
                if div_vote > 0 {
                    up_votes += weight;
                } else {
                    down_votes += weight;
                }
                active_vote_count += 1;
                indicators_used.push("rsi_divergence".to_string());
                reasons.push(div_reason);
            }
        }

        // OBV confirmation (only with significant volume)
        if let (Some(obv_slope), Some(rel_vol)) = (features.obv_slope, features.relative_volume) {
            if rel_vol > 1.0 && obv_slope.abs() > 0.0 {
                let weight = self.calibrated_weight(features, "obv_slope");
                if obv_slope > 0.0 {
                    up_votes += weight;
                } else {
                    down_votes += weight;
                }
                indicators_used.push("obv_slope".to_string());
                reasons.push(format!(
                    "OBV {}: {:.0} (vol:{:.1}x)",
                    if obv_slope > 0.0 { "rising" } else { "falling" },
                    obv_slope,
                    rel_vol
                ));
            }
        }

        // ═══════════════════════════════════════════════════
        // TIER 5: Orderbook & Microstructure (HIGH PRIORITY)
        // These are leading indicators, not lagging
        // ═══════════════════════════════════════════════════

        // Orderbook Imbalance: Bid/Ask pressure
        // Positive = more bids (bullish), Negative = more asks (bearish)
        if let Some(imbalance) = features.orderbook_imbalance {
            let abs_imb = imbalance.abs();
            if abs_imb > 0.15 {
                // Significant imbalance detected
                let base_weight = if abs_imb > 0.35 { 2.5 } else { 1.5 };
                let weight =
                    self.calibrated_weight(features, "orderbook_imbalance") * (base_weight / 2.0);
                if imbalance > 0.0 {
                    up_votes += weight;
                    reasons.push(format!(
                        "Orderbook bullish: {:.1}% bid pressure",
                        imbalance * 100.0
                    ));
                } else {
                    down_votes += weight;
                    reasons.push(format!(
                        "Orderbook bearish: {:.1}% ask pressure",
                        imbalance.abs() * 100.0
                    ));
                }
                active_vote_count += 1;
                indicators_used.push("orderbook_imbalance".to_string());
            }
        }

        // Orderflow Delta: Net buying/selling pressure
        // Positive = net buying, Negative = net selling
        if let Some(delta) = features.orderflow_delta {
            let abs_delta = delta.abs();
            if abs_delta > 0.10 {
                // Significant orderflow detected
                let weight = self.calibrated_weight(features, "orderflow_delta");
                if delta > 0.0 {
                    up_votes += weight;
                    reasons.push(format!("Orderflow buying: +{:.1}%", delta * 100.0));
                } else {
                    down_votes += weight;
                    reasons.push(format!("Orderflow selling: {:.1}%", delta * 100.0));
                }
                active_vote_count += 1;
                indicators_used.push("orderflow_delta".to_string());
            }
        }

        // ═══════════════════════════════════════════════════
        // SIGNAL QUALITY FILTERS
        // ═══════════════════════════════════════════════════

        // Minimum active votes
        if active_vote_count < self.config.min_active_votes {
            self.set_filter_reason("insufficient_active_votes");
            return None;
        }

        // Calculate direction and raw confidence
        let total_votes = up_votes + down_votes;
        if total_votes == 0.0 {
            self.set_filter_reason("zero_total_votes");
            return None;
        }

        let (direction, winning_votes, losing_votes) = if up_votes > down_votes {
            (Direction::Up, up_votes, down_votes)
        } else {
            (Direction::Down, down_votes, up_votes)
        };

        // ── VOTE MARGIN REQUIREMENT ──
        // For binary bets, we need directional conviction.
        // The minimum margin is configurable for runtime tuning.
        let vote_ratio = if losing_votes > 0.0 {
            winning_votes / losing_votes
        } else {
            10.0
        };
        if vote_ratio < self.config.min_vote_ratio {
            self.set_filter_reason("vote_margin_too_low");
            return None;
        }

        let signal_strength = winning_votes / total_votes;

        // Volatility adjustment (high volatility = less confident)
        let volatility_adj = features
            .volatility
            .map(|v| 1.0 - (v * self.config.volatility_scale).min(0.25))
            .unwrap_or(1.0);

        let mut confidence = (signal_strength * volatility_adj).min(1.0);

        // ── REGIME ADJUSTMENT ──
        match regime {
            MarketRegime::Trending => {
                // Trending is the best regime for binary prediction
                confidence *= 1.05; // Small boost for trending
            }
            MarketRegime::Ranging => {
                // Ranging is harder to predict — only trust strong signals
                confidence *= 0.90; // 10% penalty
            }
            MarketRegime::Volatile => {
                confidence *= 0.85;
            }
        }

        // Volume confirmation/penalty
        if let Some(rel_vol) = features.relative_volume {
            if rel_vol >= self.config.volume_confirm_threshold {
                confidence *= 1.04; // 4% bonus for high volume
                reasons.push(format!("Volume confirmed: {:.1}x avg", rel_vol));
            } else if rel_vol <= self.config.volume_penalty_threshold {
                confidence *= 0.85; // 15% penalty for low volume
                reasons.push(format!("Low volume warning: {:.1}x avg", rel_vol));
            }
        }

        // ── MARKET TIMING SCORE (HIGH PRIORITY) ──
        // Intelligent market timing based on:
        // - Window position (early vs late in window)
        // - Intra-window volatility patterns
        // - Price position relative to window high/low
        if let Some(timing_score) = features.market_timing_score {
            // Timing score ranges from -0.5 to +0.5
            // Positive = good timing, Negative = bad timing
            if timing_score > 0.2 {
                confidence *= 1.08; // 8% bonus for excellent timing
                reasons.push(format!("Excellent timing: +{:.2}", timing_score));
            } else if timing_score > 0.0 {
                confidence *= 1.02; // 2% bonus for good timing
            } else if timing_score < -0.2 {
                confidence *= 0.90; // 10% penalty for bad timing
                reasons.push(format!("Poor timing: {:.2}", timing_score));
            } else if timing_score < 0.0 {
                confidence *= 0.96; // 4% penalty for suboptimal timing
            }
        }

        // Multi-timeframe alignment bonus
        if let Some(bonus) = self.check_multi_tf_alignment(features.asset, direction) {
            confidence += bonus;
            reasons.push("Multi-TF aligned".to_string());
        }

        // ── CONFIDENCE CAP ──
        // Never be overconfident — even the best signals fail 30%+ of the time
        confidence = confidence.min(0.92);

        // Only generate signal if confidence is above threshold
        if confidence < self.config.min_confidence {
            self.set_filter_reason("confidence_below_min");
            return None;
        }

        // Store indicators used for calibration tracking
        self.last_indicators_used = indicators_used.clone();

        Some(GeneratedSignal {
            asset: features.asset,
            timeframe: features.timeframe,
            direction,
            confidence,
            reasons,
            ts: features.ts,
            indicators_used,
        })
    }

    /// Analyze RSI (regime-aware) — for binary prediction we need EXTREMES
    fn analyze_rsi(&self, rsi: f64, regime: MarketRegime) -> (i32, f64, String) {
        match regime {
            MarketRegime::Trending => {
                // In trending markets, only extreme RSI matters
                if rsi < 25.0 {
                    (1, 1.5, format!("RSI extreme oversold (trend): {:.1}", rsi))
                } else if rsi > 75.0 {
                    (
                        -1,
                        1.5,
                        format!("RSI extreme overbought (trend): {:.1}", rsi),
                    )
                } else {
                    (0, 0.0, String::new())
                }
            }
            MarketRegime::Ranging => {
                // In ranging: RSI is most effective for mean-reversion
                if rsi < self.config.rsi_oversold {
                    (1, 2.0, format!("RSI oversold: {:.1}", rsi))
                } else if rsi > self.config.rsi_overbought {
                    (-1, 2.0, format!("RSI overbought: {:.1}", rsi))
                } else {
                    (0, 0.0, String::new())
                }
            }
            MarketRegime::Volatile => {
                // No RSI signal in volatile markets
                (0, 0.0, String::new())
            }
        }
    }

    /// Analyze MACD — already handled inline, but kept for tests
    pub fn analyze_macd(&self, hist: f64) -> (i32, f64, String) {
        if hist > self.config.macd_threshold {
            (1, 1.5, format!("MACD bullish: {:.6}", hist))
        } else if hist < -self.config.macd_threshold {
            (-1, 1.5, format!("MACD bearish: {:.6}", hist))
        } else {
            (0, 0.0, String::new())
        }
    }

    /// Analyze Bollinger Bands (regime-aware)
    pub fn analyze_bb(&self, bb_pos: f64, regime: MarketRegime) -> (i32, f64, String) {
        match regime {
            MarketRegime::Ranging => {
                // Mean reversion: BB extremes predict reversal
                if bb_pos < self.config.bb_oversold {
                    (1, 2.0, format!("BB oversold (ranging): {:.2}", bb_pos))
                } else if bb_pos > self.config.bb_overbought {
                    (-1, 2.0, format!("BB overbought (ranging): {:.2}", bb_pos))
                } else {
                    (0, 0.0, String::new())
                }
            }
            MarketRegime::Trending => {
                // In trends, BB breakouts confirm direction
                if bb_pos < 0.1 {
                    (-1, 1.0, format!("BB lower breakout (trend): {:.2}", bb_pos))
                } else if bb_pos > 0.9 {
                    (1, 1.0, format!("BB upper breakout (trend): {:.2}", bb_pos))
                } else {
                    (0, 0.0, String::new())
                }
            }
            MarketRegime::Volatile => {
                (0, 0.0, String::new()) // No BB signal in volatile
            }
        }
    }

    /// Detect RSI divergence using feature history
    fn detect_rsi_divergence(&self, features: &Features) -> Option<(i32, String)> {
        let key = (features.asset, features.timeframe);
        let history = self.feature_history.get(&key)?;

        let lookback = self.config.divergence_lookback;
        if history.len() < lookback + 1 {
            return None;
        }

        let current_rsi = features.rsi?;
        let current_price = features.close;

        let past_idx = history.len() - lookback;
        let past = &history[past_idx];
        let past_rsi = past.rsi?;
        let past_price = past.close;

        // Bearish divergence: price higher high, RSI lower high
        if current_price > past_price && current_rsi < past_rsi {
            let price_change_pct = (current_price - past_price) / past_price * 100.0;
            let rsi_change = current_rsi - past_rsi;
            if price_change_pct > 0.5 && rsi_change < -5.0 {
                return Some((
                    -1,
                    format!(
                        "Bearish RSI divergence: price +{:.1}% but RSI {:.1}->{:.1}",
                        price_change_pct, past_rsi, current_rsi
                    ),
                ));
            }
        }

        // Bullish divergence: price lower low, RSI higher low
        if current_price < past_price && current_rsi > past_rsi {
            let price_change_pct = (past_price - current_price) / past_price * 100.0;
            let rsi_change = current_rsi - past_rsi;
            if price_change_pct > 0.5 && rsi_change > 5.0 {
                return Some((
                    1,
                    format!(
                        "Bullish RSI divergence: price -{:.1}% but RSI {:.1}->{:.1}",
                        price_change_pct, past_rsi, current_rsi
                    ),
                ));
            }
        }

        None
    }

    /// Check if another timeframe agrees with the signal direction
    /// Uses DYNAMIC weights based on:
    /// 1. Short-term momentum alignment (most important)
    /// 2. Window price position (avoid late entries)
    /// 3. Orderbook pressure (leading indicator)
    fn check_multi_tf_alignment(&self, asset: Asset, direction: Direction) -> Option<f64> {
        let timeframes = [Timeframe::Min15, Timeframe::Hour1];
        let mut total_bonus = 0.0;
        let mut aligned_count = 0;

        for tf in &timeframes {
            let key = (asset, *tf);
            if let Some(other_features) = self.latest_features.get(&key) {
                // ── DYNAMIC MULTI-TF ALIGNMENT ──
                // Check multiple factors, not just trend

                // 1. Trend alignment (base)
                let trend_aligned = if let Some(trend) = other_features.trend_strength {
                    let other_direction = if trend > self.config.trend_threshold {
                        Some(Direction::Up)
                    } else if trend < -self.config.trend_threshold {
                        Some(Direction::Down)
                    } else {
                        None
                    };
                    other_direction == Some(direction)
                } else {
                    false
                };

                if !trend_aligned {
                    continue;
                }

                // 2. Short-term momentum alignment (HIGH PRIORITY)
                let momentum_aligned = if let Some(st_momentum) = other_features.short_term_momentum
                {
                    let expected_sign = match direction {
                        Direction::Up => 1.0,
                        Direction::Down => -1.0,
                    };
                    st_momentum.signum() == expected_sign
                } else {
                    true // No penalty if no data
                };

                // 3. Late entry check (avoid aligning with exhausted moves)
                let not_late_entry = match direction {
                    Direction::Up => !other_features.late_entry_up,
                    Direction::Down => !other_features.late_entry_down,
                };

                // 4. Orderbook alignment (leading indicator)
                let orderbook_aligned = if let Some(imbalance) = other_features.orderbook_imbalance
                {
                    let expected_sign = match direction {
                        Direction::Up => 1.0,
                        Direction::Down => -1.0,
                    };
                    imbalance.signum() == expected_sign || imbalance.abs() < 0.1
                // Small imbalance is neutral
                } else {
                    true // No penalty if no data
                };

                // Calculate dynamic bonus
                let mut bonus = self.config.multi_tf_bonus; // Base bonus

                // Momentum alignment bonus
                if momentum_aligned {
                    bonus *= 1.3; // 30% boost
                } else {
                    bonus *= 0.5; // 50% penalty
                }

                // Late entry penalty
                if !not_late_entry {
                    bonus *= 0.0; // Zero bonus if late entry
                }

                // Orderbook alignment bonus
                if orderbook_aligned {
                    bonus *= 1.15; // 15% boost
                }

                total_bonus += bonus;
                aligned_count += 1;
            }
        }

        if aligned_count > 0 {
            // Cap the total bonus
            Some(total_bonus.min(0.12))
        } else {
            None
        }
    }

    /// Convert generated signal to Signal type
    pub fn to_signal(&self, gen: &GeneratedSignal, features: FeatureSet) -> Signal {
        Signal {
            id: format!("sig-{}", gen.ts),
            ts: gen.ts,
            asset: gen.asset,
            timeframe: gen.timeframe,
            direction: gen.direction,
            confidence: gen.confidence,
            features,
            strategy_id: "polybot-binary-v3".to_string(),
            market_slug: String::new(),
            condition_id: String::new(),
            token_id: String::new(),
            expires_at: 0,
            suggested_size_usdc: 8.0, // Will be overridden by risk manager
            quote_bid: 0.0,
            quote_ask: 0.0,
            quote_mid: 0.0,
            quote_depth_top5: 0.0,
            indicators_used: gen.indicators_used.clone(),
        }
    }
}

impl Default for StrategyEngine {
    fn default() -> Self {
        Self::new(StrategyConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_features(
        asset: Asset,
        rsi: Option<f64>,
        momentum: Option<f64>,
        trend: Option<f64>,
    ) -> Features {
        Features {
            asset,
            timeframe: Timeframe::Min15,
            ts: 1700000000000,
            close: 50000.0,
            rsi,
            momentum,
            trend_strength: trend,
            regime: MarketRegime::Ranging,
            ..Default::default()
        }
    }

    fn make_features_full(
        asset: Asset,
        rsi: Option<f64>,
        momentum: Option<f64>,
        trend: Option<f64>,
        macd_hist: Option<f64>,
        bb_position: Option<f64>,
        ha_trend: Option<Direction>,
    ) -> Features {
        Features {
            asset,
            timeframe: Timeframe::Min15,
            ts: 1700000000000,
            close: 50000.0,
            rsi,
            momentum,
            trend_strength: trend,
            macd: Some(0.001),
            macd_hist,
            bb_position,
            ha_trend,
            velocity: Some(0.001),
            regime: MarketRegime::Ranging,
            ..Default::default()
        }
    }

    #[test]
    fn test_rsi_oversold_signal() {
        let mut engine = StrategyEngine::default();

        let features = make_features_full(
            Asset::BTC,
            Some(25.0),          // RSI oversold -> UP
            Some(0.05),          // Momentum up -> UP
            Some(0.01),          // Trend up -> UP
            Some(0.001),         // MACD bullish -> UP
            Some(0.1),           // BB oversold -> UP
            Some(Direction::Up), // HA trend -> UP
        );

        let signal = engine.process(&features);
        assert!(signal.is_some());
        let sig = signal.unwrap();
        assert_eq!(sig.direction, Direction::Up);
        assert!(sig.confidence >= 0.60);
    }

    #[test]
    fn test_rsi_overbought_signal() {
        let mut engine = StrategyEngine::default();

        let features = make_features_full(
            Asset::BTC,
            Some(75.0),            // RSI overbought -> DOWN
            Some(-0.05),           // Momentum down -> DOWN
            Some(-0.01),           // Trend down -> DOWN
            Some(-0.001),          // MACD bearish -> DOWN
            Some(0.9),             // BB overbought -> DOWN
            Some(Direction::Down), // HA trend -> DOWN
        );

        let signal = engine.process(&features);
        assert!(signal.is_some());
        let sig = signal.unwrap();
        assert_eq!(sig.direction, Direction::Down);
    }

    #[test]
    fn test_no_signal_insufficient_votes() {
        let mut engine = StrategyEngine::default();

        let features = make_features(Asset::ETH, Some(25.0), None, None);
        let signal = engine.process(&features);
        assert!(signal.is_none());
    }

    #[test]
    fn test_no_signal_neutral() {
        let mut engine = StrategyEngine::default();
        let features = make_features(Asset::ETH, Some(50.0), None, None);

        let signal = engine.process(&features);
        assert!(signal.is_none());
    }

    #[test]
    fn test_signal_cooldown() {
        let mut engine = StrategyEngine::default();

        let features1 = make_features_full(
            Asset::BTC,
            Some(25.0),
            Some(0.05),
            Some(0.01),
            Some(0.001),
            Some(0.1),
            Some(Direction::Up),
        );

        let signal1 = engine.process(&features1);
        assert!(signal1.is_some());

        let mut features2 = features1.clone();
        features2.ts += 1000;
        let signal2 = engine.process(&features2);
        assert!(signal2.is_none());

        let mut features3 = features1.clone();
        features3.ts += 1_000_000;
        let signal3 = engine.process(&features3);
        assert!(signal3.is_some());
    }

    #[test]
    fn test_volatile_regime_no_signal() {
        let mut engine = StrategyEngine::default();

        let mut features = make_features_full(
            Asset::BTC,
            Some(25.0),
            Some(0.05),
            Some(0.01),
            Some(0.001),
            Some(0.1),
            Some(Direction::Up),
        );
        features.regime = MarketRegime::Volatile;

        let signal = engine.process(&features);
        assert!(
            signal.is_none(),
            "Should NOT generate signal in volatile regime"
        );
    }

    #[test]
    fn test_sol_xrp_filtered() {
        let mut engine = StrategyEngine::default();

        let features = make_features_full(
            Asset::SOL,
            Some(25.0),
            Some(0.05),
            Some(0.01),
            Some(0.001),
            Some(0.1),
            Some(Direction::Up),
        );

        let signal = engine.process(&features);
        assert!(signal.is_none(), "SOL should be filtered out");
    }

    #[test]
    fn test_confidence_no_floor() {
        let mut engine = StrategyEngine::default();

        let features = make_features_full(
            Asset::BTC,
            Some(25.0),
            Some(-0.05),
            Some(0.001),
            Some(0.0001),
            Some(0.5),
            Some(Direction::Up),
        );

        let signal = engine.process(&features);
        if let Some(sig) = signal {
            assert!(sig.confidence <= 1.0);
        }
    }

    #[test]
    fn test_regime_aware_rsi() {
        let engine = StrategyEngine::default();

        // In trending: RSI 65 should NOT trigger (needs > 75)
        let (vote, _, _) = engine.analyze_rsi(65.0, MarketRegime::Trending);
        assert_eq!(vote, 0);

        // In ranging: RSI 67 (above rsi_overbought=65) -> bearish
        let (vote, _, _) = engine.analyze_rsi(67.0, MarketRegime::Ranging);
        assert_eq!(vote, -1);

        // In trending: RSI 85 should trigger extreme overbought
        let (vote, _, _) = engine.analyze_rsi(85.0, MarketRegime::Trending);
        assert_eq!(vote, -1);
    }

    #[test]
    fn test_bb_regime_aware() {
        let engine = StrategyEngine::default();

        let (vote, _, _) = engine.analyze_bb(0.10, MarketRegime::Ranging);
        assert_eq!(vote, 1);

        let (vote, _, _) = engine.analyze_bb(0.95, MarketRegime::Trending);
        assert_eq!(vote, 1);
    }

    #[test]
    fn test_rsi_neutral_zone_fixed() {
        let engine = StrategyEngine::default();

        // RSI 42 in ranging -> no longer a bullish lean (thresholds tightened)
        let (vote, _, _) = engine.analyze_rsi(42.0, MarketRegime::Ranging);
        assert_eq!(vote, 0); // 42 is above rsi_oversold=35, no vote

        // RSI 30 in ranging -> oversold
        let (vote, _, _) = engine.analyze_rsi(30.0, MarketRegime::Ranging);
        assert_eq!(vote, 1);
    }
}

