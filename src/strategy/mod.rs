//! Strategy Engine v2.0 - PolyBot Mejorado
//!
//! Mejoras implementadas:
//! - Sistema de votación por clusters (indicadores agrupados por tipo)
//! - Filtros de entrada más estrictos (edge mínimo 5%)
//! - Parámetros adaptativos por asset/timeframe
//! - Calibración agresiva por régimen de mercado
//! - Desactivación temporal de ETH_1H por bajo rendimiento
//!
//! This strategy is designed for Polymarket prediction markets where:
//! - The question is: "Will price be HIGHER or LOWER at market close?"
//! - WIN = shares worth $1.00 (profit ~80%)
//! - LOSS = shares worth $0.00 (lose 100% of bet)
//!
//! Therefore we need HIGH ACCURACY (>55% winrate is profitable).

pub mod calibrator;
pub use calibrator::{
    CalibrationQualitySnapshot, IndicatorCalibrator, IndicatorStats, TradeResult,
};

use anyhow::Result;
use std::collections::HashMap;

use crate::features::{
    CrossAssetAnalyzer, CrossAssetSignal, Features, MarketRegime, SettlementEdge,
    SettlementPrediction, SettlementPricePredictor, TemporalPatternAnalyzer,
};
use crate::types::{Asset, Direction, FeatureSet, Signal, Timeframe};
use chrono::Utc;

/// Cluster types for indicator grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndicatorCluster {
    Trend,          // ADX, EMA
    Momentum,       // MACD, Momentum, Heikin Ashi
    Reversion,      // RSI, Bollinger, StochRSI
    Microstructure, // Orderbook, Orderflow
    Confirmation,   // RSI Divergence, OBV
}

/// Vote result from a cluster
#[derive(Debug, Clone)]
pub struct ClusterVote {
    pub cluster: IndicatorCluster,
    pub up_votes: f64,
    pub down_votes: f64,
    pub confidence: f64,  // 0.0 to 1.0, how strong is the cluster signal
    pub is_aligned: bool, // Are all indicators in cluster aligned?
}

/// Strategy configuration v2.0
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
    // NUEVO: Filtros estrictos v2.0
    pub min_edge_net: f64,
    pub max_late_entry_15m: f64,
    pub max_late_entry_1h: f64,
    // NUEVO: Umbral adaptativo por asset
    pub late_entry_threshold_btc_15m: f64,
    pub late_entry_threshold_eth_15m: f64,
    pub late_entry_threshold_btc_1h: f64,
    pub late_entry_threshold_eth_1h: f64,
    // NUEVO: Sistema de clusters
    pub cluster_min_alignment: f64, // Mínimo 0.7 (70% de indicadores en cluster deben estar alineados)
    pub cluster_require_trend_momentum_agreement: bool,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.65, // Aumentado de 0.60 a 0.65
            rsi_overbought: 65.0, // Tighter RSI for prediction accuracy
            rsi_oversold: 35.0,
            macd_threshold: 0.001, // Ignore noise
            bb_overbought: 0.85,   // BB extremes
            bb_oversold: 0.15,
            trend_threshold: 0.002, // Real trends only
            volatility_scale: 0.5,
            min_active_votes: 1,         // Allow exploration in paper mode
            min_vote_ratio: 1.15,        // Aumentado de 1.05 a 1.15 (más estricto)
            signal_cooldown_ms: 900_000, // 15 minutes (legacy)
            stoch_rsi_overbought: 0.80,
            stoch_rsi_oversold: 0.20,
            volume_confirm_threshold: 1.3, // Volume must be 30% above average
            volume_penalty_threshold: 0.6, // Penalize low volume
            multi_tf_bonus: 0.06,          // Conservative multi-TF bonus
            divergence_lookback: 5,
            // NUEVO: Filtros estrictos v2.0
            min_edge_net: 0.05, // 5% edge mínimo
            max_late_entry_15m: 0.60,
            max_late_entry_1h: 0.70,
            late_entry_threshold_btc_15m: 0.005,
            late_entry_threshold_eth_15m: 0.008,
            late_entry_threshold_btc_1h: 0.015,
            late_entry_threshold_eth_1h: 0.025,
            cluster_min_alignment: 0.70,
            cluster_require_trend_momentum_agreement: true,
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
    /// Temporal pattern analyzer for time-of-day optimization
    temporal_analyzer: TemporalPatternAnalyzer,
    /// Settlement price predictor for Chainlink oracle
    settlement_predictor: SettlementPricePredictor,
    /// Cross-asset analyzer for BTC/ETH correlation
    cross_asset_analyzer: CrossAssetAnalyzer,
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
            temporal_analyzer: TemporalPatternAnalyzer::new(5),
            settlement_predictor: SettlementPricePredictor::new(1000),
            cross_asset_analyzer: CrossAssetAnalyzer::new(50),
        }
    }

    /// Get mutable reference to temporal analyzer
    pub fn temporal_analyzer_mut(&mut self) -> &mut TemporalPatternAnalyzer {
        &mut self.temporal_analyzer
    }

    /// Get mutable reference to settlement predictor
    pub fn settlement_predictor_mut(&mut self) -> &mut SettlementPricePredictor {
        &mut self.settlement_predictor
    }

    /// Get mutable reference to cross-asset analyzer
    pub fn cross_asset_analyzer_mut(&mut self) -> &mut CrossAssetAnalyzer {
        &mut self.cross_asset_analyzer
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

    /// Record complete trade result with all v3.0 analyzers
    pub fn record_complete_trade(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        indicators: &[String],
        win: bool,
        confidence: f64,
        direction: Direction,
        edge: f64,
    ) {
        let result = if win {
            TradeResult::Win
        } else {
            TradeResult::Loss
        };

        // Update calibrator
        self.calibrator
            .record_trade_for_market(asset, timeframe, indicators, result);
        self.calibrator.recalibrate();

        // Update temporal analyzer
        self.temporal_analyzer.record_trade(
            asset,
            timeframe,
            Utc::now(),
            win,
            confidence,
            direction,
            edge,
        );
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

    /// Generate signal from current features - Sistema de Clusters v2.0
    ///
    /// Los indicadores se agrupan en clusters por tipo:
    /// - Cluster Trend: ADX, EMA
    /// - Cluster Momentum: MACD, Momentum, Heikin Ashi
    /// - Cluster Reversion: RSI, Bollinger, StochRSI
    /// - Cluster Microstructure: Orderbook, Orderflow
    /// - Cluster Confirmation: RSI Divergence, OBV
    ///
    /// Señal válida solo si:
    /// 1. Cluster Trend y Momentum están alineados (misma dirección)
    /// 2. Al menos 3 clusters activos
    /// 3. Confianza >= min_confidence
    fn generate_signal(&mut self, features: &Features) -> Option<GeneratedSignal> {
        let regime = features.regime;

        // ── DESACTIVAR ETH_1H TEMPORALMENTE ──
        if features.asset == Asset::ETH && matches!(features.timeframe, Timeframe::Hour1) {
            self.set_filter_reason("eth_1h_disabled_pending_fix");
            return None;
        }

        // ── VOLATILE REGIME FILTER ──
        // In volatile markets, binary prediction accuracy drops significantly.
        // Skip entirely — the expected value is negative when accuracy < 55%.
        if regime == MarketRegime::Volatile && matches!(features.timeframe, Timeframe::Hour1) {
            self.set_filter_reason("regime_volatile_1h");
            return None;
        }

        // ── LATE ENTRY FILTER (HIGH PRIORITY) - v2.0 ──
        // Umbral adaptativo por asset/timeframe
        let max_late_entry = match features.timeframe {
            Timeframe::Min15 => self.config.max_late_entry_15m,
            Timeframe::Hour1 => self.config.max_late_entry_1h,
        };

        let window_progress = features.window_progress.unwrap_or(1.0);
        if window_progress > max_late_entry {
            if features.late_entry_up {
                self.set_filter_reason("late_entry_up");
                return None;
            }
            if features.late_entry_down {
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

        // ═══════════════════════════════════════════════════
        // SISTEMA DE CLUSTERS v2.0
        // Agrupar indicadores por tipo y evaluar cada cluster
        // ═══════════════════════════════════════════════════

        let mut cluster_votes: Vec<ClusterVote> = Vec::new();
        let mut reasons: Vec<String> = Vec::new();
        let mut indicators_used: Vec<String> = Vec::new();

        // ── CLUSTER 1: TREND (ADX + EMA) ──
        let mut trend_up = 0.0;
        let mut trend_down = 0.0;
        let mut trend_active = 0;

        if let (Some(adx), Some(plus_di), Some(minus_di)) =
            (features.adx, features.plus_di, features.minus_di)
        {
            if adx > 25.0 {
                let di_spread = (plus_di - minus_di).abs();
                if di_spread > 5.0 {
                    let base_weight = if adx > 35.0 { 3.0 } else { 2.0 };
                    let weight =
                        self.calibrated_weight(features, "adx_trend") * (base_weight / 2.5);
                    if plus_di > minus_di {
                        trend_up += weight;
                        reasons.push(format!("ADX UP: {:.1}", adx));
                    } else {
                        trend_down += weight;
                        reasons.push(format!("ADX DOWN: {:.1}", adx));
                    }
                    trend_active += 1;
                    indicators_used.push("adx_trend".to_string());
                }
            }
        }

        if let Some(trend) = features.trend_strength {
            let abs_trend = trend.abs();
            if abs_trend > self.config.trend_threshold {
                let weight = self.calibrated_weight(features, "ema_trend")
                    * if abs_trend > 0.003 { 1.33 } else { 1.0 };
                if trend > 0.0 {
                    trend_up += weight;
                } else {
                    trend_down += weight;
                }
                trend_active += 1;
                indicators_used.push("ema_trend".to_string());
                reasons.push(format!("EMA: {:.4}", trend));
            }
        }

        if trend_active > 0 {
            let trend_total = trend_up + trend_down;
            let trend_confidence = if trend_total > 0.0 {
                (trend_up.max(trend_down) / trend_total).max(0.5)
            } else {
                0.5
            };
            let is_aligned = (trend_up == 0.0 || trend_down == 0.0)
                || (trend_up.max(trend_down) / trend_total) >= self.config.cluster_min_alignment;
            cluster_votes.push(ClusterVote {
                cluster: IndicatorCluster::Trend,
                up_votes: trend_up,
                down_votes: trend_down,
                confidence: trend_confidence,
                is_aligned,
            });
        }

        // ── CLUSTER 2: MOMENTUM (MACD + Momentum + Heikin Ashi + ST Momentum) ──
        let mut mom_up = 0.0;
        let mut mom_down = 0.0;
        let mut mom_active = 0;

        if let (Some(_), Some(hist)) = (features.macd, features.macd_hist) {
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
                    mom_up += weight;
                    reasons.push(format!("MACD: {:.4}", hist));
                } else {
                    mom_down += weight;
                }
                mom_active += 1;
                indicators_used.push("macd_histogram".to_string());
            }
        }

        if let Some(momentum) = features.momentum {
            let vel = features.velocity.unwrap_or(0.0);
            let abs_mom = momentum.abs();
            if abs_mom > 0.001 && vel.abs() > 0.0002 && momentum.signum() == vel.signum() {
                let weight = self.calibrated_weight(features, "momentum_acceleration");
                if momentum > 0.0 {
                    mom_up += weight;
                } else {
                    mom_down += weight;
                }
                mom_active += 1;
                indicators_used.push("momentum_acceleration".to_string());
                reasons.push(format!("Mom+: {:.4}", momentum));
            }
        }

        if let Some(ha_trend) = features.ha_trend {
            let weight = self.calibrated_weight(features, "heikin_ashi");
            match ha_trend {
                Direction::Up => mom_up += weight,
                Direction::Down => mom_down += weight,
            }
            mom_active += 1;
            indicators_used.push("heikin_ashi".to_string());
        }

        if let Some(st_momentum) = features.short_term_momentum {
            let abs_st = st_momentum.abs();
            if abs_st > 0.0005 {
                let weighted_mom = features.weighted_momentum.unwrap_or(st_momentum);
                let base_weight = if abs_st > 0.002 { 2.5 } else { 1.5 };
                let confirmed = weighted_mom.signum() == st_momentum.signum();
                let weight = self.calibrated_weight(features, "short_term_momentum")
                    * (base_weight / 1.5)
                    * if confirmed { 1.2 } else { 1.0 };
                if st_momentum > 0.0 {
                    mom_up += weight;
                } else {
                    mom_down += weight;
                }
                mom_active += 1;
                indicators_used.push("short_term_momentum".to_string());
            }
        }

        if mom_active > 0 {
            let mom_total = mom_up + mom_down;
            let mom_confidence = if mom_total > 0.0 {
                (mom_up.max(mom_down) / mom_total).max(0.5)
            } else {
                0.5
            };
            let is_aligned = (mom_up == 0.0 || mom_down == 0.0)
                || (mom_up.max(mom_down) / mom_total) >= self.config.cluster_min_alignment;
            cluster_votes.push(ClusterVote {
                cluster: IndicatorCluster::Momentum,
                up_votes: mom_up,
                down_votes: mom_down,
                confidence: mom_confidence,
                is_aligned,
            });
        }

        // ── CLUSTER 3: REVERSION (RSI + Bollinger + StochRSI) ──
        let mut rev_up = 0.0;
        let mut rev_down = 0.0;
        let mut rev_active = 0;

        if let Some(rsi) = features.rsi {
            let (vote, base_weight, reason) = self.analyze_rsi(rsi, regime);
            if vote != 0 {
                let weight = self.calibrated_weight(features, "rsi_extreme") * (base_weight / 1.5);
                if vote > 0 {
                    rev_up += weight;
                } else {
                    rev_down += weight;
                }
                rev_active += 1;
                indicators_used.push("rsi_extreme".to_string());
                if !reason.is_empty() {
                    reasons.push(reason);
                }
            }
        }

        if let Some(bb_pos) = features.bb_position {
            let (vote, base_weight, reason) = self.analyze_bb(bb_pos, regime);
            if vote != 0 {
                let weight =
                    self.calibrated_weight(features, "bollinger_band") * (base_weight / 2.0);
                if vote > 0 {
                    rev_up += weight;
                } else {
                    rev_down += weight;
                }
                rev_active += 1;
                indicators_used.push("bollinger_band".to_string());
                if !reason.is_empty() {
                    reasons.push(reason);
                }
            }
        }

        if let Some(stoch_rsi) = features.stoch_rsi {
            if stoch_rsi < self.config.stoch_rsi_oversold {
                rev_up += self.calibrated_weight(features, "stoch_rsi");
                rev_active += 1;
                indicators_used.push("stoch_rsi".to_string());
            } else if stoch_rsi > self.config.stoch_rsi_overbought {
                rev_down += self.calibrated_weight(features, "stoch_rsi");
                rev_active += 1;
                indicators_used.push("stoch_rsi".to_string());
            }
        }

        if rev_active > 0 {
            let rev_total = rev_up + rev_down;
            let rev_confidence = if rev_total > 0.0 {
                (rev_up.max(rev_down) / rev_total).max(0.5)
            } else {
                0.5
            };
            let is_aligned = (rev_up == 0.0 || rev_down == 0.0)
                || (rev_up.max(rev_down) / rev_total) >= self.config.cluster_min_alignment;
            cluster_votes.push(ClusterVote {
                cluster: IndicatorCluster::Reversion,
                up_votes: rev_up,
                down_votes: rev_down,
                confidence: rev_confidence,
                is_aligned,
            });
        }

        // ── CLUSTER 4: MICROSTRUCTURE (Orderbook + Orderflow) ──
        let mut micro_up = 0.0;
        let mut micro_down = 0.0;
        let mut micro_active = 0;

        if let Some(imbalance) = features.orderbook_imbalance {
            let abs_imb = imbalance.abs();
            if abs_imb > 0.15 {
                let base_weight = if abs_imb > 0.35 { 2.5 } else { 1.5 };
                let weight =
                    self.calibrated_weight(features, "orderbook_imbalance") * (base_weight / 2.0);
                if imbalance > 0.0 {
                    micro_up += weight;
                    reasons.push(format!("OB: {:.0}%", imbalance * 100.0));
                } else {
                    micro_down += weight;
                }
                micro_active += 1;
                indicators_used.push("orderbook_imbalance".to_string());
            }
        }

        if let Some(delta) = features.orderflow_delta {
            let abs_delta = delta.abs();
            if abs_delta > 0.10 {
                let weight = self.calibrated_weight(features, "orderflow_delta");
                if delta > 0.0 {
                    micro_up += weight;
                } else {
                    micro_down += weight;
                }
                micro_active += 1;
                indicators_used.push("orderflow_delta".to_string());
            }
        }

        if micro_active > 0 {
            let micro_total = micro_up + micro_down;
            let micro_confidence = if micro_total > 0.0 {
                (micro_up.max(micro_down) / micro_total).max(0.5)
            } else {
                0.5
            };
            let is_aligned = (micro_up == 0.0 || micro_down == 0.0)
                || (micro_up.max(micro_down) / micro_total) >= self.config.cluster_min_alignment;
            cluster_votes.push(ClusterVote {
                cluster: IndicatorCluster::Microstructure,
                up_votes: micro_up,
                down_votes: micro_down,
                confidence: micro_confidence,
                is_aligned,
            });
        }

        // ── CLUSTER 5: CONFIRMATION (RSI Divergence + OBV) ──
        let mut conf_up = 0.0;
        let mut conf_down = 0.0;
        let mut conf_active = 0;

        let is_short_window = matches!(features.timeframe, Timeframe::Min15);
        if !is_short_window {
            if let Some((div_vote, div_reason)) = self.detect_rsi_divergence(features) {
                let weight = self.calibrated_weight(features, "rsi_divergence");
                if div_vote > 0 {
                    conf_up += weight;
                } else {
                    conf_down += weight;
                }
                conf_active += 1;
                indicators_used.push("rsi_divergence".to_string());
                reasons.push(div_reason);
            }
        }

        if let (Some(obv_slope), Some(rel_vol)) = (features.obv_slope, features.relative_volume) {
            if rel_vol > 1.0 && obv_slope.abs() > 0.0 {
                let weight = self.calibrated_weight(features, "obv_slope");
                if obv_slope > 0.0 {
                    conf_up += weight;
                } else {
                    conf_down += weight;
                }
                conf_active += 1;
                indicators_used.push("obv_slope".to_string());
            }
        }

        if conf_active > 0 {
            let conf_total = conf_up + conf_down;
            let conf_confidence = if conf_total > 0.0 {
                (conf_up.max(conf_down) / conf_total).max(0.5)
            } else {
                0.5
            };
            let is_aligned = (conf_up == 0.0 || conf_down == 0.0)
                || (conf_up.max(conf_down) / conf_total) >= self.config.cluster_min_alignment;
            cluster_votes.push(ClusterVote {
                cluster: IndicatorCluster::Confirmation,
                up_votes: conf_up,
                down_votes: conf_down,
                confidence: conf_confidence,
                is_aligned,
            });
        }

        // ═══════════════════════════════════════════════════
        // EVALUACIÓN DE CLUSTERS v2.0
        // ═══════════════════════════════════════════════════

        // Requerir mínimo 2 clusters activos
        if cluster_votes.len() < 2 {
            self.set_filter_reason("insufficient_clusters");
            return None;
        }

        // Cluster Trend y Momentum DEBEN estar alineados
        let trend_cluster = cluster_votes
            .iter()
            .find(|c| c.cluster == IndicatorCluster::Trend);
        let mom_cluster = cluster_votes
            .iter()
            .find(|c| c.cluster == IndicatorCluster::Momentum);

        if self.config.cluster_require_trend_momentum_agreement {
            if let (Some(trend), Some(mom)) = (trend_cluster, mom_cluster) {
                let trend_dir = if trend.up_votes > trend.down_votes {
                    1
                } else {
                    -1
                };
                let mom_dir = if mom.up_votes > mom.down_votes { 1 } else { -1 };
                if trend_dir != mom_dir {
                    self.set_filter_reason("trend_momentum_misalignment");
                    return None;
                }
            }
        }

        // Calcular votos totales ponderados por confianza del cluster
        let mut total_up = 0.0;
        let mut total_down = 0.0;
        let mut active_indicators = 0;

        for cluster in &cluster_votes {
            let cluster_weight = cluster.confidence; // Ponderar por qué tan alineado está el cluster
            total_up += cluster.up_votes * cluster_weight;
            total_down += cluster.down_votes * cluster_weight;
            active_indicators += 1;
        }

        let total_votes = total_up + total_down;
        if total_votes == 0.0 {
            self.set_filter_reason("zero_total_votes");
            return None;
        }

        let (direction, winning_votes, losing_votes) = if total_up > total_down {
            (Direction::Up, total_up, total_down)
        } else {
            (Direction::Down, total_down, total_up)
        };

        // VOTE MARGIN REQUIREMENT
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

        // Volatility adjustment
        let volatility_adj = features
            .volatility
            .map(|v| 1.0 - (v * self.config.volatility_scale).min(0.25))
            .unwrap_or(1.0);

        let mut confidence = (signal_strength * volatility_adj).min(1.0);

        // Regime adjustment
        match regime {
            MarketRegime::Trending => confidence *= 1.05,
            MarketRegime::Ranging => confidence *= 0.90,
            MarketRegime::Volatile => confidence *= 0.85,
        }

        // Volume confirmation
        if let Some(rel_vol) = features.relative_volume {
            if rel_vol >= self.config.volume_confirm_threshold {
                confidence *= 1.04;
                reasons.push(format!("Vol: {:.1}x", rel_vol));
            } else if rel_vol <= self.config.volume_penalty_threshold {
                confidence *= 0.85;
            }
        }

        // Market timing score
        if let Some(timing_score) = features.market_timing_score {
            if timing_score > 0.2 {
                confidence *= 1.08;
            } else if timing_score > 0.0 {
                confidence *= 1.02;
            } else if timing_score < -0.2 {
                confidence *= 0.90;
            } else if timing_score < 0.0 {
                confidence *= 0.96;
            }
        }

        // Multi-timeframe alignment
        if let Some(bonus) = self.check_multi_tf_alignment(features.asset, direction) {
            confidence += bonus;
            reasons.push("Multi-TF".to_string());
        }

        // ============================================
        // NEW v3.0: Temporal Pattern Adjustment
        // ============================================
        let now = Utc::now();
        let (temporal_adj, temporal_reason) = self.temporal_analyzer.get_temporal_adjustment(
            features.asset,
            features.timeframe,
            direction,
            now,
        );

        // Check if we should block trading at this time
        let (should_block, block_reason) =
            self.temporal_analyzer
                .should_block_trading(features.asset, features.timeframe, now);

        if should_block {
            self.set_filter_reason(&format!("temporal_block: {}", block_reason));
            return None;
        }

        if temporal_adj != 1.0 {
            confidence *= temporal_adj;
            if !temporal_reason.is_empty() {
                reasons.push(format!("Temporal: {}", temporal_reason));
            }
        }

        // ============================================
        // NEW v3.0: Cross-Asset Correlation Signal
        // ============================================
        if let Some(cross_signal) = self
            .cross_asset_analyzer
            .detect_divergence(features.timeframe)
        {
            match cross_signal.signal_type {
                crate::features::CrossAssetSignalType::CorrelatedMovement => {
                    if cross_signal.primary_direction == direction {
                        confidence *= 1.0 + (cross_signal.correlation.abs() * 0.08);
                        reasons.push(format!(
                            "BTC-ETH aligned: {:.0}%",
                            cross_signal.correlation * 100.0
                        ));
                    }
                }
                crate::features::CrossAssetSignalType::Divergence => {
                    if cross_signal.confidence > 0.8 {
                        // Strong divergence - our signal might be catching the reversal
                        confidence *= 1.12;
                        reasons.push("Divergence opportunity".to_string());
                    }
                }
                crate::features::CrossAssetSignalType::BTCDominant => {
                    if features.asset == Asset::BTC {
                        confidence *= 1.06;
                        reasons.push("BTC leading".to_string());
                    }
                }
                crate::features::CrossAssetSignalType::ETHDominant => {
                    if features.asset == Asset::ETH {
                        confidence *= 1.06;
                        reasons.push("ETH leading".to_string());
                    }
                }
                _ => {}
            }
        }

        // ============================================
        // NEW v3.0: Settlement Price Prediction Edge
        // ============================================
        // This requires market metadata - we'll use current price as proxy for strike
        // In production, you'd fetch the actual strike price from market conditions
        let current_price = features.close;
        if current_price > 0.0 {
            // Note: In production, you'd get the actual expiry timestamp and strike price
            // from the market metadata. Here we'll estimate based on timeframe.
            let time_to_expiry_secs = match features.timeframe {
                Timeframe::Min15 => 15 * 60,
                Timeframe::Hour1 => 60 * 60,
            };
            let expiry_ts = (now.timestamp_millis() + time_to_expiry_secs * 1000) as i64;

            let settlement_pred = self.settlement_predictor.predict_settlement(
                features.asset,
                current_price,
                features.timeframe,
                expiry_ts,
                features.orderbook_imbalance.unwrap_or(0.0),
            );

            // Get market-implied probability (using close price as proxy)
            // In production, this would come from orderbook mid price
            let market_mid = features.close / 100000.0; // Normalized to 0-1 range

            // Calculate edge
            let strike_price = current_price; // Simplified - actual strike would come from market
            let settlement_edge = self.settlement_predictor.calculate_settlement_edge(
                &settlement_pred,
                market_mid,
                strike_price,
            );

            // Boost confidence if our settlement prediction aligns with signal
            if settlement_edge.predicted_direction == direction {
                let edge_boost = (settlement_edge.edge.abs() * 0.5).min(0.10);
                confidence *= 1.0 + edge_boost;
                if edge_boost > 0.02 {
                    reasons.push(format!(
                        "Settlement edge: {:.1}%",
                        settlement_edge.edge * 100.0
                    ));
                }
            } else if settlement_edge.confidence > 0.7 {
                // Settlement prediction contradicts our signal - reduce confidence
                confidence *= 0.9;
                reasons.push("Settlement mismatch".to_string());
            }
        }

        // Confidence cap
        confidence = confidence.min(0.95); // Increased cap to 0.95 for v3.0

        // Final check
        if confidence < self.config.min_confidence {
            self.set_filter_reason("confidence_below_min");
            return None;
        }

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
        features.timeframe = Timeframe::Hour1; // El filtro de volatilidad aplica solo a 1h

        let signal = engine.process(&features);
        assert!(
            signal.is_none(),
            "Should NOT generate signal in volatile 1h regime"
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
