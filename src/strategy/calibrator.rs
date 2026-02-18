//! Indicator Calibrator
//!
//! Dynamically calibrates indicator weights based on historical performance.
//! Learning is market-aware: BTC_15M, BTC_1H, ETH_15M, ETH_1H.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::{Asset, Timeframe};

const GLOBAL_MARKET_KEY: &str = "GLOBAL";

/// Canonical market key used for per-market learning persistence.
pub type MarketKey = String;
const CALIBRATION_BINS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalibrationBin {
    pub count: usize,
    pub sum_p: f64,
    pub sum_y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCalibrationMetrics {
    pub sample_count: usize,
    pub brier_sum: f64,
    pub bins: Vec<CalibrationBin>,
}

impl Default for MarketCalibrationMetrics {
    fn default() -> Self {
        Self {
            sample_count: 0,
            brier_sum: 0.0,
            bins: vec![CalibrationBin::default(); CALIBRATION_BINS],
        }
    }
}

impl MarketCalibrationMetrics {
    pub fn record(&mut self, p_pred: f64, is_win: bool) {
        let p = p_pred.clamp(0.0, 1.0);
        let y = if is_win { 1.0 } else { 0.0 };

        self.sample_count += 1;
        self.brier_sum += (p - y).powi(2);

        let idx = ((p * CALIBRATION_BINS as f64).floor() as usize).min(CALIBRATION_BINS - 1);
        if let Some(bin) = self.bins.get_mut(idx) {
            bin.count += 1;
            bin.sum_p += p;
            bin.sum_y += y;
        }
    }

    pub fn brier_score(&self) -> Option<f64> {
        if self.sample_count == 0 {
            None
        } else {
            Some(self.brier_sum / self.sample_count as f64)
        }
    }

    pub fn ece(&self) -> Option<f64> {
        if self.sample_count == 0 {
            return None;
        }
        let total = self.sample_count as f64;
        let mut ece = 0.0;
        for bin in &self.bins {
            if bin.count == 0 {
                continue;
            }
            let n = bin.count as f64;
            let avg_p = bin.sum_p / n;
            let avg_y = bin.sum_y / n;
            ece += (n / total) * (avg_p - avg_y).abs();
        }
        Some(ece)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationQualitySnapshot {
    pub market_key: String,
    pub sample_count: usize,
    pub brier_score: Option<f64>,
    pub ece: Option<f64>,
}

/// Trade result for calibration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeResult {
    Win,
    Loss,
}

/// Statistics for a single indicator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorStats {
    /// Indicator name
    pub name: String,
    /// Total signals generated
    pub total_signals: usize,
    /// Winning signals
    pub wins: usize,
    /// Losing signals
    pub losses: usize,
    /// Win rate (0.0 to 1.0)
    pub win_rate: f64,
    /// Calibrated weight based on performance
    pub calibrated_weight: f64,
    /// Default weight (before calibration)
    pub default_weight: f64,
    /// Last updated timestamp
    pub last_updated: i64,
}

impl Default for IndicatorStats {
    fn default() -> Self {
        Self {
            name: String::new(),
            total_signals: 0,
            wins: 0,
            losses: 0,
            win_rate: 0.0,
            calibrated_weight: 1.0,
            default_weight: 1.0,
            last_updated: 0,
        }
    }
}

impl IndicatorStats {
    pub fn new(name: &str, default_weight: f64) -> Self {
        Self {
            name: name.to_string(),
            default_weight,
            calibrated_weight: default_weight,
            ..Default::default()
        }
    }

    pub fn record_signal(&mut self, result: TradeResult) {
        match result {
            TradeResult::Win => self.wins += 1,
            TradeResult::Loss => self.losses += 1,
        }
        self.total_signals += 1;
        self.last_updated = chrono::Utc::now().timestamp();
    }

    pub fn recalculate(&mut self, min_samples: usize) {
        if self.total_signals > 0 {
            self.win_rate = self.wins as f64 / self.total_signals as f64;
        }

        if self.total_signals < min_samples {
            return;
        }

        // Binary prediction market payout-aware weighting.
        let performance_factor = if self.win_rate >= 0.55 {
            1.0 + (self.win_rate - 0.55) * 4.0
        } else if self.win_rate >= 0.50 {
            0.8 + (self.win_rate - 0.50) * 4.0
        } else if self.win_rate >= 0.40 {
            0.5 + (self.win_rate - 0.40) * 3.0
        } else {
            0.2 + self.win_rate * 0.75
        };

        self.calibrated_weight = self.default_weight * performance_factor;
        self.calibrated_weight = self
            .calibrated_weight
            .clamp(self.default_weight * 0.1, self.default_weight * 3.0);
    }
}

/// Indicator Calibrator
pub struct IndicatorCalibrator {
    /// Statistics per market and indicator: market_key -> indicator_name -> stats
    stats_by_market: HashMap<MarketKey, HashMap<String, IndicatorStats>>,
    /// Probability calibration metrics per market.
    calibration_by_market: HashMap<MarketKey, MarketCalibrationMetrics>,
    /// Minimum samples needed before calibration kicks in (per market)
    min_samples: usize,
    /// Default weights for indicators
    default_weights: HashMap<String, f64>,
}

impl IndicatorCalibrator {
    pub fn new() -> Self {
        let mut default_weights = HashMap::new();

        // Tier 1: Core Trend Indicators
        default_weights.insert("adx_trend".to_string(), 2.5);
        default_weights.insert("ema_trend".to_string(), 1.5);

        // Tier 2: Momentum Confirmation
        default_weights.insert("macd_histogram".to_string(), 1.5);
        default_weights.insert("momentum_acceleration".to_string(), 2.0);
        default_weights.insert("heikin_ashi".to_string(), 1.0);
        default_weights.insert("short_term_momentum".to_string(), 1.5);

        // Tier 3: Mean-Reversion Signals
        default_weights.insert("rsi_extreme".to_string(), 1.5);
        default_weights.insert("bollinger_band".to_string(), 2.0);
        default_weights.insert("stoch_rsi".to_string(), 1.0);

        // Tier 4: Confirmation
        default_weights.insert("rsi_divergence".to_string(), 2.0);
        default_weights.insert("obv_slope".to_string(), 0.5);

        // Tier 5: Orderbook & Microstructure
        default_weights.insert("orderbook_imbalance".to_string(), 2.0);
        default_weights.insert("orderflow_delta".to_string(), 1.5);

        Self {
            stats_by_market: HashMap::new(),
            calibration_by_market: HashMap::new(),
            min_samples: 30,
            default_weights,
        }
    }

    pub fn with_min_samples(min_samples: usize) -> Self {
        let mut calibrator = Self::new();
        calibrator.min_samples = min_samples;
        calibrator
    }

    pub fn canonical_market_key(asset: Asset, timeframe: Timeframe) -> MarketKey {
        let tf = match timeframe {
            Timeframe::Min15 => "15M",
            Timeframe::Hour1 => "1H",
        };
        format!("{}_{}", asset, tf)
    }

    fn default_weight(&self, indicator_name: &str) -> f64 {
        self.default_weights
            .get(indicator_name)
            .copied()
            .unwrap_or(1.0)
    }

    fn market_map_mut(&mut self, market_key: &str) -> &mut HashMap<String, IndicatorStats> {
        self.stats_by_market
            .entry(market_key.to_string())
            .or_insert_with(HashMap::new)
    }

    fn market_map(&self, market_key: &str) -> Option<&HashMap<String, IndicatorStats>> {
        self.stats_by_market.get(market_key)
    }

    /// Legacy/global recording retained for backwards compatibility in tests/tools.
    pub fn record_trade(&mut self, indicators_used: &[String], result: TradeResult) {
        self.record_trade_for_market_key(GLOBAL_MARKET_KEY, indicators_used, result);
    }

    /// Record a trade result for a specific market key.
    pub fn record_trade_for_market_key(
        &mut self,
        market_key: &str,
        indicators_used: &[String],
        result: TradeResult,
    ) {
        for indicator in indicators_used {
            let default = self.default_weight(indicator);
            let stats = self
                .market_map_mut(market_key)
                .entry(indicator.clone())
                .or_insert_with(|| IndicatorStats::new(indicator, default));
            stats.record_signal(result);
        }
    }

    /// Record a trade result for a specific asset/timeframe market.
    pub fn record_trade_for_market(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        indicators_used: &[String],
        result: TradeResult,
    ) {
        let market_key = Self::canonical_market_key(asset, timeframe);
        self.record_trade_for_market_key(&market_key, indicators_used, result);
    }

    pub fn record_prediction_for_market_key(
        &mut self,
        market_key: &str,
        p_pred: f64,
        is_win: bool,
    ) {
        let metrics = self
            .calibration_by_market
            .entry(market_key.to_string())
            .or_insert_with(MarketCalibrationMetrics::default);
        metrics.record(p_pred, is_win);
    }

    pub fn record_prediction_for_market(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        p_pred: f64,
        is_win: bool,
    ) {
        let market_key = Self::canonical_market_key(asset, timeframe);
        self.record_prediction_for_market_key(&market_key, p_pred, is_win);
    }

    /// Recalculate all weights based on accumulated data.
    pub fn recalibrate(&mut self) {
        for market_map in self.stats_by_market.values_mut() {
            for stats in market_map.values_mut() {
                stats.recalculate(self.min_samples);
            }
        }
    }

    /// Legacy/global lookup retained for backwards compatibility.
    pub fn get_weight(&self, indicator_name: &str) -> f64 {
        self.get_weight_for_market_key(GLOBAL_MARKET_KEY, indicator_name)
    }

    /// Get calibrated weight for an indicator in a market key.
    pub fn get_weight_for_market_key(&self, market_key: &str, indicator_name: &str) -> f64 {
        self.market_map(market_key)
            .and_then(|m| m.get(indicator_name))
            .map(|s| s.calibrated_weight)
            .unwrap_or_else(|| self.default_weight(indicator_name))
    }

    /// Get calibrated weight for an indicator in a specific market.
    pub fn get_weight_for_market(
        &self,
        asset: Asset,
        timeframe: Timeframe,
        indicator_name: &str,
    ) -> f64 {
        let market_key = Self::canonical_market_key(asset, timeframe);
        self.get_weight_for_market_key(&market_key, indicator_name)
    }

    /// Legacy/global stats lookup retained for backwards compatibility.
    pub fn get_stats(&self, indicator_name: &str) -> Option<&IndicatorStats> {
        self.market_map(GLOBAL_MARKET_KEY)
            .and_then(|m| m.get(indicator_name))
    }

    /// Get statistics for an indicator in a market key.
    pub fn get_market_stats(
        &self,
        market_key: &str,
        indicator_name: &str,
    ) -> Option<&IndicatorStats> {
        self.market_map(market_key)
            .and_then(|m| m.get(indicator_name))
    }

    /// Export aggregated statistics (by indicator across markets) for dashboard/API.
    pub fn export_stats(&self) -> Vec<IndicatorStats> {
        let mut merged: HashMap<String, IndicatorStats> = HashMap::new();
        let mut weighted_sum: HashMap<String, (f64, usize)> = HashMap::new();

        for market_map in self.stats_by_market.values() {
            for stat in market_map.values() {
                let entry = merged
                    .entry(stat.name.clone())
                    .or_insert_with(|| IndicatorStats {
                        name: stat.name.clone(),
                        total_signals: 0,
                        wins: 0,
                        losses: 0,
                        win_rate: 0.0,
                        calibrated_weight: self.default_weight(&stat.name),
                        default_weight: self.default_weight(&stat.name),
                        last_updated: 0,
                    });

                entry.total_signals += stat.total_signals;
                entry.wins += stat.wins;
                entry.losses += stat.losses;
                entry.last_updated = entry.last_updated.max(stat.last_updated);

                let w = weighted_sum.entry(stat.name.clone()).or_insert((0.0, 0));
                w.0 += stat.calibrated_weight * stat.total_signals as f64;
                w.1 += stat.total_signals;
            }
        }

        for stat in merged.values_mut() {
            if stat.total_signals > 0 {
                stat.win_rate = stat.wins as f64 / stat.total_signals as f64;
            }
            if let Some((sum, n)) = weighted_sum.get(&stat.name) {
                if *n > 0 {
                    stat.calibrated_weight = *sum / *n as f64;
                }
            }
        }

        merged.into_values().collect()
    }

    /// Export state in v2 format: market_key -> indicator stats list.
    pub fn export_stats_by_market(&self) -> HashMap<MarketKey, Vec<IndicatorStats>> {
        let mut out = HashMap::new();
        for (market, stats) in &self.stats_by_market {
            out.insert(market.clone(), stats.values().cloned().collect());
        }
        out
    }

    pub fn export_calibration_quality_by_market(
        &self,
    ) -> HashMap<MarketKey, CalibrationQualitySnapshot> {
        let mut out = HashMap::new();
        for (market_key, metrics) in &self.calibration_by_market {
            out.insert(
                market_key.clone(),
                CalibrationQualitySnapshot {
                    market_key: market_key.clone(),
                    sample_count: metrics.sample_count,
                    brier_score: metrics.brier_score(),
                    ece: metrics.ece(),
                },
            );
        }
        out
    }

    /// Backward-compatible loader for legacy v1 format (single global stats vec).
    pub fn load_stats(&mut self, stats: Vec<IndicatorStats>) {
        let mut map = HashMap::new();
        for stat in stats {
            map.insert(stat.name.clone(), stat);
        }
        self.stats_by_market
            .insert(GLOBAL_MARKET_KEY.to_string(), map);
    }

    /// Load v2 market-aware state.
    pub fn load_stats_by_market(&mut self, stats: HashMap<MarketKey, Vec<IndicatorStats>>) {
        self.stats_by_market.clear();
        for (market, list) in stats {
            let mut map = HashMap::new();
            for stat in list {
                map.insert(stat.name.clone(), stat);
            }
            self.stats_by_market.insert(market, map);
        }
    }

    /// Reset all statistics.
    pub fn reset(&mut self) {
        self.stats_by_market.clear();
        self.calibration_by_market.clear();
    }

    /// Total trades tracked across markets (sum of market-level trade counts).
    pub fn total_trades(&self) -> usize {
        self.stats_by_market
            .values()
            .map(|market_map| {
                market_map
                    .values()
                    .map(|s| s.total_signals)
                    .max()
                    .unwrap_or(0)
            })
            .sum()
    }

    /// Check if at least one market is calibrated.
    pub fn is_calibrated(&self) -> bool {
        self.stats_by_market.values().any(|market_map| {
            market_map
                .values()
                .map(|s| s.total_signals)
                .max()
                .unwrap_or(0)
                >= self.min_samples
        })
    }

    /// Check if a specific market is calibrated.
    pub fn is_market_calibrated(&self, asset: Asset, timeframe: Timeframe) -> bool {
        let market_key = Self::canonical_market_key(asset, timeframe);
        self.market_map(&market_key)
            .map(|m| m.values().map(|s| s.total_signals).max().unwrap_or(0) >= self.min_samples)
            .unwrap_or(false)
    }

    /// Get overall win rate across all tracked indicators/markets.
    pub fn overall_win_rate(&self) -> f64 {
        let stats = self.export_stats();
        let wins: usize = stats.iter().map(|s| s.wins).sum();
        let total: usize = stats.iter().map(|s| s.total_signals).sum();
        if total > 0 {
            wins as f64 / total as f64
        } else {
            0.5
        }
    }

    /// Get indicators sorted by performance (aggregated).
    pub fn top_performers(&self, limit: usize) -> Vec<IndicatorStats> {
        let mut stats = self.export_stats();
        stats.sort_by(|a, b| {
            b.win_rate
                .partial_cmp(&a.win_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        stats.into_iter().take(limit).collect()
    }

    /// Get underperforming indicators (aggregated).
    pub fn underperformers(&self) -> Vec<IndicatorStats> {
        let avg = self.overall_win_rate();
        self.export_stats()
            .into_iter()
            .filter(|s| s.total_signals >= self.min_samples && s.win_rate < avg)
            .collect()
    }
}

impl Default for IndicatorCalibrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Calibration configuration
#[derive(Debug, Clone)]
pub struct CalibrationConfig {
    /// Minimum trades before calibration kicks in
    pub min_trades: usize,
    /// Weight adjustment factor (how much to adjust based on performance)
    pub adjustment_factor: f64,
    /// Maximum weight multiplier
    pub max_weight_multiplier: f64,
    /// Minimum weight multiplier
    pub min_weight_multiplier: f64,
}

impl Default for CalibrationConfig {
    fn default() -> Self {
        Self {
            min_trades: 30,
            adjustment_factor: 1.0,
            max_weight_multiplier: 3.0,
            min_weight_multiplier: 0.2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indicator_stats_creation() {
        let stats = IndicatorStats::new("rsi_extreme", 1.5);
        assert_eq!(stats.name, "rsi_extreme");
        assert_eq!(stats.default_weight, 1.5);
        assert_eq!(stats.calibrated_weight, 1.5);
    }

    #[test]
    fn test_record_signals() {
        let mut stats = IndicatorStats::new("test", 1.0);

        stats.record_signal(TradeResult::Win);
        stats.record_signal(TradeResult::Win);
        stats.record_signal(TradeResult::Loss);

        assert_eq!(stats.total_signals, 3);
        assert_eq!(stats.wins, 2);
        assert_eq!(stats.losses, 1);
    }

    #[test]
    fn test_recalculate_weight() {
        let mut stats = IndicatorStats::new("test", 1.0);
        for _ in 0..10 {
            stats.record_signal(TradeResult::Win);
        }
        stats.recalculate(5);
        assert!(stats.calibrated_weight > stats.default_weight);
        assert_eq!(stats.win_rate, 1.0);
    }

    #[test]
    fn test_calibrator_legacy_global() {
        let mut calibrator = IndicatorCalibrator::with_min_samples(5);

        for _ in 0..4 {
            calibrator.record_trade(&["rsi_extreme".to_string()], TradeResult::Win);
        }
        for _ in 0..1 {
            calibrator.record_trade(&["rsi_extreme".to_string()], TradeResult::Loss);
        }

        calibrator.recalibrate();
        let stats = calibrator.get_stats("rsi_extreme").unwrap();
        assert_eq!(stats.win_rate, 0.8);
    }

    #[test]
    fn test_market_calibrator_flow() {
        let mut calibrator = IndicatorCalibrator::with_min_samples(2);
        calibrator.record_trade_for_market(
            Asset::BTC,
            Timeframe::Min15,
            &["adx_trend".to_string()],
            TradeResult::Win,
        );
        calibrator.record_trade_for_market(
            Asset::BTC,
            Timeframe::Min15,
            &["adx_trend".to_string()],
            TradeResult::Loss,
        );
        calibrator.recalibrate();

        let weight = calibrator.get_weight_for_market(Asset::BTC, Timeframe::Min15, "adx_trend");
        assert!(weight > 0.0);
    }

    #[test]
    fn test_get_weight_before_calibration() {
        let calibrator = IndicatorCalibrator::new();
        let weight = calibrator.get_weight("adx_trend");
        assert_eq!(weight, 2.5);
    }

    #[test]
    fn test_is_calibrated() {
        let mut calibrator = IndicatorCalibrator::with_min_samples(5);
        assert!(!calibrator.is_calibrated());
        for _ in 0..5 {
            calibrator.record_trade_for_market(
                Asset::BTC,
                Timeframe::Min15,
                &["test".to_string()],
                TradeResult::Win,
            );
        }
        assert!(calibrator.is_calibrated());
    }

    #[test]
    fn test_calibration_quality_metrics_recorded() {
        let mut calibrator = IndicatorCalibrator::new();
        calibrator.record_prediction_for_market(Asset::BTC, Timeframe::Min15, 0.70, true);
        calibrator.record_prediction_for_market(Asset::BTC, Timeframe::Min15, 0.40, false);

        let exported = calibrator.export_calibration_quality_by_market();
        let row = exported.get("BTC_15M").expect("missing BTC_15M row");
        assert_eq!(row.sample_count, 2);
        assert!(row.brier_score.is_some());
        assert!(row.ece.is_some());
    }

    #[test]
    fn test_default_min_samples_is_30() {
        let mut calibrator = IndicatorCalibrator::new();
        for _ in 0..29 {
            calibrator.record_trade_for_market(
                Asset::BTC,
                Timeframe::Min15,
                &["adx_trend".to_string()],
                TradeResult::Win,
            );
        }
        assert!(!calibrator.is_market_calibrated(Asset::BTC, Timeframe::Min15));

        calibrator.record_trade_for_market(
            Asset::BTC,
            Timeframe::Min15,
            &["adx_trend".to_string()],
            TradeResult::Win,
        );
        assert!(calibrator.is_market_calibrated(Asset::BTC, Timeframe::Min15));
    }
}
