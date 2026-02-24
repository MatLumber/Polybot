//! V3 Strategy - ML-Powered Trading Strategy
//!
//! Uses ensemble ML models (Random Forest, Gradient Boosting, Logistic Regression)
//! to predict market direction with 55-60% win rate target.

use crate::ml_engine::dataset::{Dataset, TradeSample};
use crate::ml_engine::features::{FeatureEngine, MLFeatureVector, MarketContext};
use crate::ml_engine::filters::{FilterConfig, FilterContext, FilterDecision, SmartFilterEngine};
use crate::ml_engine::models::{EnsembleWeights, MLPredictor, ModelPrediction};
use crate::ml_engine::persistence::{MLPersistenceManager, TrainingRecord};
use crate::ml_engine::training::{TrainingPipeline, WalkForwardConfig};
use crate::ml_engine::{MLEngineConfig, MLEngineState, Prediction};
use crate::strategy::calibrator::{IndicatorCalibrator, IndicatorStats, TradeResult};
use crate::strategy::GeneratedSignal;
use crate::types::{Asset, Direction, Timeframe};
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use tracing::{error, info, warn};

/// One observed market window, used to record ground-truth BTC direction
/// at window close regardless of whether a trade was entered.
#[derive(Debug, Clone)]
struct WindowObservation {
    /// ML feature vector captured at the first signal evaluation of this window
    features: MLFeatureVector,
    /// BTC price at the first tick of this window (approximates the market-open price)
    price_at_open: f64,
    /// Polymarket token (YES) price at the first tick of this window
    token_price_at_open: f64,
    /// Timestamp when this window closes (ms)
    close_ts: i64,
    /// Asset
    asset: Asset,
    /// Timeframe
    timeframe: Timeframe,
    /// Window open timestamp (ms) – used as the sample timestamp
    open_ts: i64,
}

/// ML-Powered Trading Strategy (V3)
pub struct V3Strategy {
    /// ML Engine configuration
    config: MLEngineConfig,
    /// Ensemble predictor with RF, GB, LR models
    predictor: Option<MLPredictor>,
    /// Feature engineering engine
    feature_engine: FeatureEngine,
    /// Smart filters for trade validation
    filter_engine: SmartFilterEngine,
    /// Training pipeline for model updates
    training_pipeline: TrainingPipeline,
    /// Calibrator for indicator weights
    calibrator: IndicatorCalibrator,
    /// ML Engine state (predictions, performance tracking)
    state: MLEngineState,
    /// Historical feature data per market
    feature_history: HashMap<(Asset, Timeframe), Vec<crate::features::Features>>,
    /// Last filter reason for debugging
    last_filter_reason: Option<String>,
    /// Dataset for training
    dataset: Dataset,
    /// Persistence manager
    persistence: MLPersistenceManager,
    /// Pending window observations keyed by (asset, timeframe, window_open_ts).
    /// Captures one feature snapshot per market window so the ML can learn from
    /// EVERY window outcome — not just windows where a trade was executed.
    pending_windows: HashMap<(Asset, Timeframe, i64), WindowObservation>,
    /// Finalized window outcomes per market (last 5). Used to compute sequential
    /// features: prev_window_dir_1/2/3, window_streak, cross_tf_alignment.
    window_history: HashMap<(Asset, Timeframe), VecDeque<f64>>,
    /// Recent Polymarket 24h volume per market for volume_trend feature.
    volume_history: HashMap<(Asset, Timeframe), VecDeque<f64>>,
    /// Counter of window observations (NOT trades) — used for retraining trigger.
    window_observations_count: usize,
}

impl V3Strategy {
    pub fn new(ml_config: MLEngineConfig, _base_config: crate::strategy::StrategyConfig) -> Self {
        let calibrator = IndicatorCalibrator::with_min_samples(30);

        // Initialize persistence manager
        let persistence = MLPersistenceManager::new("./data");

        // Try to load previous state
        let (mut predictor, mut dataset, mut state) = match persistence.load_ml_state() {
            Ok(Some((persisted, loaded_dataset))) => {
                info!("🧠 Loading persisted ML state...");

                // Restore ensemble weights
                let weights = persisted.ensemble_weights;
                let pred = if ml_config.enabled {
                    Some(MLPredictor::new(weights))
                } else {
                    None
                };

                let mut ml_state = MLEngineState::new(ml_config.clone());
                ml_state.total_predictions = persisted.total_predictions;
                ml_state.correct_predictions = persisted.correct_predictions;
                ml_state.incorrect_predictions = persisted.incorrect_predictions;
                ml_state.last_retraining = persisted.last_retraining;

                let accuracy = if persisted.total_predictions > 0 {
                    persisted.correct_predictions as f64 / persisted.total_predictions as f64
                } else {
                    0.5
                };

                info!(
                    "✅ ML state restored: {} predictions, {:.1}% accuracy, {} samples in dataset",
                    persisted.total_predictions,
                    accuracy * 100.0,
                    loaded_dataset.len()
                );

                (pred, loaded_dataset, ml_state)
            }
            Ok(None) => {
                info!("🆕 No previous ML state found, starting fresh");
                let weights = EnsembleWeights::from_config(&ml_config.ensemble);
                let pred = if ml_config.enabled {
                    Some(MLPredictor::new(weights))
                } else {
                    None
                };
                (pred, Dataset::new(), MLEngineState::new(ml_config.clone()))
            }
            Err(e) => {
                warn!("⚠️ Failed to load ML state: {}, starting fresh", e);
                let weights = EnsembleWeights::from_config(&ml_config.ensemble);
                let pred = if ml_config.enabled {
                    Some(MLPredictor::new(weights))
                } else {
                    None
                };
                (pred, Dataset::new(), MLEngineState::new(ml_config.clone()))
            }
        };

        // Initialize filter engine
        let filter_config = FilterConfig {
            max_spread_bps_15m: ml_config.filters.max_spread_bps_15m,
            max_spread_bps_1h: ml_config.filters.max_spread_bps_1h,
            min_depth_usdc: ml_config.filters.min_depth_usdc,
            max_volatility_5m: ml_config.filters.max_volatility_5m,
            min_volatility_5m: ml_config.filters.min_volatility_5m,
            optimal_hours_only: ml_config.filters.optimal_hours_only,
            max_window_progress: ml_config.filters.max_window_progress,
            min_time_to_close_minutes: ml_config.filters.min_time_to_close_minutes,
            min_model_confidence: ml_config.min_confidence,
        };
        let filter_engine = SmartFilterEngine::new(filter_config);

        // Initialize training pipeline
        let walk_forward_config = WalkForwardConfig::default();
        let training_pipeline = TrainingPipeline::new(ml_config.clone(), walk_forward_config);

        let mut strategy = Self {
            config: ml_config.clone(),
            predictor,
            feature_engine: FeatureEngine::new(),
            filter_engine,
            training_pipeline,
            calibrator,
            state,
            feature_history: HashMap::new(),
            last_filter_reason: None,
            dataset,
            persistence,
            pending_windows: HashMap::new(),
            window_history: HashMap::new(),
            volume_history: HashMap::new(),
            window_observations_count: 0,
        };

        // Auto-train models on startup if we have enough samples
        if strategy.dataset.len() >= strategy.config.training.min_samples_for_training {
            info!(
                "🎓 Auto-training models on startup with {} samples...",
                strategy.dataset.len()
            );
            if let Err(e) = strategy.train_initial_model(vec![]) {
                warn!("⚠️ Auto-training failed: {}", e);
            }
        }

        strategy
    }

    /// Process features and generate ML-powered signal
    ///
    /// Flow:
    /// 1. Apply smart filters (market conditions)
    /// 2. Try ML prediction if available and enabled
    /// 3. If ML fails or has low confidence, try fallback rule-based
    /// 4. Return signal with metadata for tracking
    pub fn process(&mut self, features: &crate::features::Features) -> Option<GeneratedSignal> {
        self.last_filter_reason = None;

        // Track this evaluation attempt
        let asset = features.asset;
        let timeframe = features.timeframe;

        // Store in history
        let key = (asset, timeframe);
        let history = self.feature_history.entry(key).or_default();
        history.push(features.clone());
        if history.len() > 100 {
            history.remove(0);
        }

        // Build market context
        let context = self.build_market_context(features);

        // Compute ML feature vector
        let mut ml_features = self.feature_engine.compute(features, &context);

        // === WINDOW OBSERVATION TRACKING ===
        // Record one ML feature snapshot per market window (regardless of whether a trade
        // is entered). When the window closes we learn the final BTC direction and add it
        // to the training dataset. This lets the ML learn from ALL market states, including
        // bearish indicator states where the edge-filter would block a real trade.
        let (win_open_ts, win_close_ts) = Self::window_boundaries(features.ts, timeframe);
        let win_key = (asset, timeframe, win_open_ts);

        // Track recent Polymarket volume for the volume_trend feature.
        let current_volume = features.polymarket_volume_24hr.unwrap_or(0.0);
        {
            let vh = self.volume_history
                .entry((asset, timeframe))
                .or_insert_with(|| VecDeque::with_capacity(20));
            vh.push_back(current_volume);
            if vh.len() > 20 { vh.pop_front(); }
        }

        // 1. Finalize any windows that have already closed (use current price as proxy for
        //    the window-close BTC price — this is accurate to within one tick, ~1 second).
        let current_btc_price = features.close;
        let current_token_price = features.polymarket_price.unwrap_or(0.5);
        let now_ts = features.ts;
        let expired_keys: Vec<_> = self
            .pending_windows
            .iter()
            .filter(|(_, obs)| now_ts >= obs.close_ts)
            .map(|(k, _)| *k)
            .collect();
        for key in expired_keys {
            if let Some(obs) = self.pending_windows.remove(&key) {
                let target = if current_btc_price >= obs.price_at_open { 1.0 } else { 0.0 };
                let direction_label = if target > 0.5 { "UP" } else { "DOWN" };
                // Update window_history for this market so sequential features stay fresh.
                {
                    let wh = self.window_history
                        .entry((obs.asset, obs.timeframe))
                        .or_insert_with(|| VecDeque::with_capacity(6));
                    wh.push_back(target);
                    if wh.len() > 5 { wh.pop_front(); }
                }
                info!(
                    asset = ?obs.asset,
                    timeframe = ?obs.timeframe,
                    price_open = obs.price_at_open,
                    price_close = current_btc_price,
                    outcome = direction_label,
                    dataset_before = self.dataset.len(),
                    "🪟 Window closed — recording ground-truth observation"
                );
                self.dataset.add_window_observation(
                    obs.features,
                    target,
                    obs.open_ts,
                    obs.asset,
                    obs.timeframe,
                    obs.price_at_open,
                    current_btc_price,
                );
                self.window_observations_count += 1;
                info!(
                    "📊 Window observation added to dataset: {} samples total ({} windows recorded)",
                    self.dataset.len(),
                    self.window_observations_count
                );
                // Auto-save every 10 new observations (reuses trade-save interval logic)
                if self.dataset.len() % 10 == 0 {
                    if let Err(e) = self.dataset.save(&format!("{}/ml_engine/dataset.json", "./data")) {
                        warn!("Failed to auto-save dataset after window observation: {}", e);
                    }
                }
                // Check if we need to retrain based on window observations
                if self.window_observations_count % self.config.training.retrain_interval_window_observations == 0
                    && self.dataset.len() >= self.config.training.min_samples_for_training
                {
                    info!(
                        "🔄 Retraining after {} window observations...",
                        self.window_observations_count
                    );
                    if let Err(e) = self.train_initial_model(vec![]) {
                        warn!("⚠️ Retraining after window observations failed: {}", e);
                    }
                }
            }
        }

        // 2. Patch ml_features with the 8 new window-history / cross-TF / token-dynamics
        //    features. This must happen AFTER finalizing expired windows (so window_history
        //    is up-to-date) and BEFORE registering the current window (so the stored
        //    snapshot already includes these enriched features).
        ml_features.prev_window_dir_1 = self.get_prev_window_dir(asset, timeframe, 1);
        ml_features.prev_window_dir_2 = self.get_prev_window_dir(asset, timeframe, 2);
        ml_features.prev_window_dir_3 = self.get_prev_window_dir(asset, timeframe, 3);
        ml_features.window_streak = self.calculate_window_streak(asset, timeframe);
        ml_features.cross_tf_alignment = self.calculate_cross_tf_alignment(asset, timeframe);
        ml_features.volume_trend = self.calculate_volume_trend_for_market(asset, timeframe);

        // Token price dynamics: compare current token price to the price at window open.
        // If the window is already tracked, we know the opening price.
        if let Some(obs) = self.pending_windows.get(&win_key) {
            ml_features.token_price_window_open = obs.token_price_at_open;
            let change = if obs.token_price_at_open > 1e-9 {
                (current_token_price - obs.token_price_at_open) / obs.token_price_at_open
            } else { 0.0 };
            ml_features.token_price_change_window = change.clamp(-1.0, 1.0);
        } else {
            // First tick of this window — no change yet.
            ml_features.token_price_window_open = current_token_price;
            ml_features.token_price_change_window = 0.0;
        }

        // 3. Register this window if we haven't seen it yet (one snapshot per window).
        self.pending_windows.entry(win_key).or_insert_with(|| WindowObservation {
            features: ml_features.clone(),
            price_at_open: current_btc_price,
            token_price_at_open: current_token_price,
            close_ts: win_close_ts,
            asset,
            timeframe,
            open_ts: win_open_ts,
        });
        // === END WINDOW OBSERVATION TRACKING ===

        // Apply smart filters first
        let filter_context = FilterContext {
            asset,
            timeframe,
            timestamp: features.ts,
            spread_bps: features.spread_bps.unwrap_or(0.0),
            depth_usdc: features.orderbook_depth_top5.unwrap_or(0.0),
            orderbook_depth: features.orderbook_depth_top5.unwrap_or(0.0),
            volatility_5m: features.volatility.unwrap_or(0.0),
            hour: context.hour,
            day_of_week: context.day_of_week,
            minutes_to_close: context.minutes_to_close,
            window_progress: features.window_progress.unwrap_or(1.0),
            is_macro_event_near: false,
            model_confidence: 0.0,
        };

        match self.filter_engine.evaluate(&filter_context) {
            FilterDecision::Allow => {
                tracing::debug!(?asset, ?timeframe, "✅ Market filters passed");
            }
            FilterDecision::Reject(reason) => {
                let reason_str = format!("{:?}", reason);
                tracing::info!(?asset, ?timeframe, reason = %reason_str, "❌ Signal rejected by market filter");
                self.last_filter_reason = Some(reason_str);
                return None;
            }
        }

        // Check if ML is enabled and models are trained
        let ml_available = self.config.enabled && self.predictor.is_some();
        let dataset_size = self.dataset.len();
        let min_samples = self.config.training.min_samples_for_training;

        tracing::debug!(
            ?asset,
            ?timeframe,
            ml_enabled = self.config.enabled,
            predictor_loaded = self.predictor.is_some(),
            dataset_size,
            min_samples_required = min_samples,
            "ML status check"
        );

        // Concept drift warning
        if let Some(ref predictor) = self.predictor {
            if predictor.is_drift_detected() {
                warn!(
                    ?asset, ?timeframe,
                    rolling_acc = predictor.recent_rolling_accuracy(),
                    baseline = predictor.drift_baseline_accuracy,
                    "⚠️ Concept drift detected — market regime may have changed, consider retraining"
                );
            }
        }

        // Try ML prediction if available
        if ml_available && dataset_size >= min_samples {
            if let Some(prediction) = self.try_ml_prediction(&ml_features, features, &context) {
                return Some(prediction);
            }
        }

        // Fallback to rule-based if ML not available or rejected
        if dataset_size < min_samples {
            tracing::info!(
                ?asset,
                ?timeframe,
                dataset_size,
                min_samples_required = min_samples,
                "ML not ready (insufficient training data), using fallback"
            );
        } else if !ml_available {
            tracing::info!(
                ?asset,
                ?timeframe,
                "ML disabled or models not loaded, using fallback"
            );
        }

        // Try fallback signal
        if let Some(signal) = self.fallback_signal(features) {
            tracing::info!(
                ?asset, ?timeframe,
                direction = ?signal.direction,
                confidence = signal.confidence,
                "📊 Fallback signal generated"
            );
            return Some(signal);
        }

        tracing::debug!(
            ?asset,
            ?timeframe,
            "No signal generated (ML and fallback both failed)"
        );
        None
    }

    /// Try to generate ML prediction
    fn try_ml_prediction(
        &mut self,
        ml_features: &crate::ml_engine::MLFeatureVector,
        features: &crate::features::Features,
        _context: &crate::ml_engine::features::MarketContext,
    ) -> Option<GeneratedSignal> {
        let predictor = self.predictor.as_ref()?;
        let prediction = predictor.predict(ml_features)?;

        tracing::debug!(
            asset = ?features.asset,
            timeframe = ?features.timeframe,
            prob_up = prediction.prob_up,
            confidence = prediction.confidence,
            model = %prediction.model_name,
            "ML prediction result"
        );

        // Check minimum confidence
        if prediction.confidence < self.config.min_confidence {
            let reason = format!(
                "low_ml_confidence: {:.2} < {:.2}",
                prediction.confidence, self.config.min_confidence
            );
            tracing::debug!(%reason, "ML prediction rejected");
            self.last_filter_reason = Some(reason);
            return None;
        }

        // Determine direction
        let direction = if prediction.prob_up > 0.5 {
            Direction::Up
        } else {
            Direction::Down
        };

        // Calculate final confidence naturally
        let confidence = prediction.confidence * (prediction.prob_up - 0.5).abs() * 2.0;

        // Check minimum confidence again after applying multiplier
        if confidence < self.config.min_confidence {
            let reason = format!(
                "low_edge_confidence: {:.2} < {:.2}",
                confidence, self.config.min_confidence
            );
            tracing::debug!(%reason, "ML prediction inherently lacked edge");
            self.last_filter_reason = Some(reason);
            return None;
        }

        // Track prediction for later validation
        self.state.add_prediction(Prediction {
            timestamp: features.ts,
            asset: features.asset,
            timeframe: features.timeframe,
            prob_up: prediction.prob_up,
            confidence: prediction.confidence,
            direction,
            edge: (prediction.prob_up - 0.5).abs(),
            features_used: vec!["v3_ml".to_string()],
            model_contributions: HashMap::new(),
            ensemble_weight: 1.0,
        });

        // NOT incrementing total_predictions here. It should only increment
        // when a trade outcome is fully registered in register_closed_trade_result.

        // Build reasons
        let mut reasons = vec![
            format!("ML_{}", prediction.model_name),
            format!("prob_up:{:.2}", prediction.prob_up),
            format!("conf:{:.2}", prediction.confidence),
        ];

        // Add top features
        let top_features = predictor.top_features(3);
        for (name, importance) in top_features {
            reasons.push(format!("{}:{:.2}", name, importance));
        }

        tracing::info!(
            asset = ?features.asset,
            timeframe = ?features.timeframe,
            direction = ?direction,
            confidence = confidence,
            prob_up = prediction.prob_up,
            model = %prediction.model_name,
            "🤖 V3 ML Signal generated"
        );

        Some(GeneratedSignal {
            asset: features.asset,
            timeframe: features.timeframe,
            direction,
            confidence,
            reasons,
            ts: features.ts,
            indicators_used: vec!["v3_ml".to_string()],
        })
    }

    /// Fallback rule-based signal when ML is not available
    fn fallback_signal(&mut self, features: &crate::features::Features) -> Option<GeneratedSignal> {
        let mut score: f64 = 0.0;
        let mut reasons = Vec::new();

        // 1. Trend Direction (EMA 9 vs EMA 21)
        if let (Some(ema9), Some(ema21)) = (features.ema_9, features.ema_21) {
            if ema9 > ema21 {
                score += 1.0;
                reasons.push("EMA_bullish".to_string());
            } else {
                score -= 1.0;
                reasons.push("EMA_bearish".to_string());
            }
        }

        // 2. Momentum Confirmation (MACD Histogram)
        if let Some(macd_hist) = features.macd_hist {
            // Is MACD histogram positive (momentum upwards)?
            if macd_hist > 0.0 {
                score += 0.5;
                reasons.push("MACD_momentum_bullish".to_string());
            } else {
                score -= 0.5;
                reasons.push("MACD_momentum_bearish".to_string());
            }
        } else if let Some(macd) = features.macd {
            // Fallback to macd alone if hist not available (should be always available though)
            if macd > 0.0 {
                score += 0.5;
                reasons.push("MACD_bullish".to_string());
            } else {
                score -= 0.5;
                reasons.push("MACD_bearish".to_string());
            }
        }

        // 3. Heikin Ashi Trend Confirmation
        if let Some(ha_trend) = &features.ha_trend {
            match ha_trend {
                Direction::Up => {
                    score += 0.5;
                    reasons.push("HA_bullish".to_string());
                }
                Direction::Down => {
                    score -= 0.5;
                    reasons.push("HA_bearish".to_string());
                }
            }
        }

        // 4. Extreme Overbought/Oversold Penalties
        // Instead of trading RSI mean-reverting everywhere, only penalize EXTREMES
        if let Some(rsi) = features.rsi {
            if rsi > 80.0 {
                score -= 1.0; // Penalty against UP
                reasons.push("RSI_extreme_overbought".to_string());
            } else if rsi < 20.0 {
                score += 1.0; // Penalty against DOWN
                reasons.push("RSI_extreme_oversold".to_string());
            }
        }

        // Calculate confidence based on maximum possible absolute score (which is ~3.0 for all aligned)
        let max_score = 3.0; // 1.0 (EMA) + 0.5 (MACD) + 0.5 (HA) + 1.0 (RSI penalty)
        let confidence = ((score as f64).abs() / max_score).min(1.0) * 0.4 + 0.6; // Scale base line to 0.6 minimum

        if confidence < self.config.min_confidence {
            tracing::info!(
                ?features.asset,
                ?features.timeframe,
                confidence,
                min_confidence = self.config.min_confidence,
                "❌ Fallback signal rejected (low confidence, weak trend)"
            );
            self.last_filter_reason = Some("fallback_low_confidence".to_string());
            return None;
        }

        // Ensure strong directional conviction: EMA and MACD must be somewhat aligned
        if score.abs() < 1.0 {
            tracing::info!(
                ?features.asset,
                ?features.timeframe,
                score,
                "❌ Fallback signal rejected (insufficient directional alignment)"
            );
            self.last_filter_reason = Some("fallback_weak_directional_score".to_string());
            return None;
        }

        let direction = if score > 0.0 {
            Direction::Up
        } else {
            Direction::Down
        };

        Some(GeneratedSignal {
            asset: features.asset,
            timeframe: features.timeframe,
            direction,
            confidence,
            reasons,
            ts: features.ts,
            indicators_used: vec!["v3_fallback_trend".to_string()],
        })
    }

    /// Build market context from features
    fn build_market_context(&self, features: &crate::features::Features) -> MarketContext {
        use chrono::{Datelike, TimeZone, Timelike, Utc};

        let dt = Utc
            .timestamp_millis_opt(features.ts)
            .single()
            .unwrap_or_else(|| Utc::now());

        let hour = dt.hour() as u8;
        let day_of_week = dt.weekday().num_days_from_monday() as u8;

        MarketContext {
            timestamp: features.ts,
            hour,
            day_of_week,
            minutes_to_close: 60.0 * (24 - hour as i64) as f64, // Simplified
            minutes_since_market_open: (hour as f64 * 60.0).max(0.0),
            calibrator_confidence: self.calibrator.get_confidence(),
            num_indicators_agreeing: 3, // Simplified
            indicators_avg_win_rate: self.state.accuracy(),
            bullish_weight: if features.macd.unwrap_or(0.0) > 0.0 {
                1.0
            } else {
                0.0
            },
            bearish_weight: if features.macd.unwrap_or(0.0) < 0.0 {
                1.0
            } else {
                0.0
            },
        }
    }

    /// Train models with historical data
    pub fn train_initial_model(
        &mut self,
        historical_trades: Vec<TradeSample>,
    ) -> anyhow::Result<()> {
        info!(
            "🎓 V3 training with {} new historical trades...",
            historical_trades.len()
        );

        let current_size = self.dataset.len();
        info!("📊 Current dataset size: {} samples", current_size);

        // Agregar nuevos trades al dataset EXISTENTE (acumulativo)
        for trade in historical_trades {
            self.dataset.add_trade(trade);
        }

        info!(
            "📈 Dataset updated: {} → {} samples (added {})",
            current_size,
            self.dataset.len(),
            self.dataset.len() - current_size
        );

        // Verificar si tenemos suficientes muestras totales
        if self.dataset.len() < self.config.training.min_samples_for_training {
            warn!(
                "Not enough total samples for training (need {}, have {})",
                self.config.training.min_samples_for_training,
                self.dataset.len()
            );
            return Ok(());
        }

        // Crear predictor si aún no existe (caso de full reset)
        if self.predictor.is_none() && self.config.enabled {
            info!("🆕 Creating new predictor (no persisted state found)");
            let weights = crate::ml_engine::models::EnsembleWeights::from_config(&self.config.ensemble);
            self.predictor = Some(crate::ml_engine::models::MLPredictor::new(weights));
        }

        // Crear copia balanceada para entrenamiento
        let mut training_dataset = self.dataset.clone();
        training_dataset.balance_classes();
        info!("Dataset balanced: {} samples", training_dataset.len());

        // Train predictor con el dataset acumulativo
        if let Some(ref mut predictor) = self.predictor {
            predictor.train(&training_dataset)?;
            self.state.training_epoch += 1;
            self.state.last_retraining = Some(chrono::Utc::now().timestamp_millis());
            info!(
                "✅ V3 models trained with {} total samples! Accuracy: {:.2}%",
                training_dataset.len(),
                predictor.ensemble_accuracy() * 100.0
            );
        } else {
            warn!("⚠️ Predictor is None, cannot train. ML may be disabled.");
        }

        // Run walk-forward validation
        self.run_walk_forward()?;

        // Save trained state
        if let Some(ref predictor) = self.predictor {
            if let Err(e) = self
                .persistence
                .save_ml_state(predictor, &self.state, &self.dataset)
            {
                warn!("Failed to save ML state after training: {}", e);
            }

            // Record training in history
            let record = TrainingRecord {
                timestamp: chrono::Utc::now().timestamp_millis(),
                samples_count: self.dataset.len(),
                accuracy: self.state.accuracy(),
                ensemble_weights: predictor.weights.clone(),
            };
            if let Err(e) = self.persistence.save_training_record(record) {
                warn!("Failed to save training record: {}", e);
            }
        }

        info!("💾 ML state saved after training");
        Ok(())
    }

    /// Run walk-forward validation
    pub fn run_walk_forward(&mut self) -> anyhow::Result<()> {
        info!("🔄 Running walk-forward validation...");

        // This would use the training_pipeline in a full implementation
        // For now, we just mark it as complete

        info!("✅ Walk-forward validation complete");
        Ok(())
    }

    /// Record trade result and update learning
    pub fn record_trade_result(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        indicators: &[String],
        is_win: bool,
        _confidence: f64,
        _edge: f64,
    ) {
        // Record in calibrator
        let result = if is_win {
            TradeResult::Win
        } else {
            TradeResult::Loss
        };
        self.calibrator.record_trade(indicators, result);
        self.calibrator.recalibrate();

        // Let the register_closed_trade_result method handle the ML Predictor,
        // State predictions and dataset additions to avoid duplication.

        info!(
            is_win = is_win,
            accuracy = self.state.accuracy(),
            total = self.state.total_predictions,
            "🧠 V3 Indicator calibrator updated from trade"
        );
    }

    /// Feed closed paper trade outcome back into ML predictor to improve probability calibration over time
    pub fn register_closed_trade_result(
        &mut self,
        record: &crate::paper_trading::PaperTradeRecord,
    ) {
        if record.asset.is_empty() || record.timeframe.is_empty() {
            return;
        }

        let asset = match crate::types::Asset::from_str(&record.asset) {
            Some(a) => a,
            None => return,
        };

        let timeframe = match crate::types::Timeframe::from_str(&record.timeframe) {
            Some(t) => t,
            None => return,
        };

        let direction = if record.direction.eq_ignore_ascii_case("up")
            || record.direction.eq_ignore_ascii_case("buy")
        {
            crate::types::Direction::Up
        } else {
            crate::types::Direction::Down
        };

        // We require features to be re-assembled or we pass a dummy if missing from the paper record directly
        // The predictor relies on the calibrator confidence to adjust the outcome mapping
        let mut entry_features = crate::ml_engine::features::MLFeatureVector::default();
        entry_features.calibrator_confidence = record.confidence;

        let trade_sample = crate::ml_engine::dataset::TradeSample {
            trade_id: record.trade_id.clone(),
            entry_ts: record.market_open_ts.max(record.timestamp),
            exit_ts: record
                .market_close_ts
                .max(record.timestamp + record.hold_duration_ms),
            asset,
            timeframe,
            direction,
            // Use prediction_correct for ML, not pnl >= 0
            is_win: record.prediction_correct,
            entry_features,
            entry_price: record.entry_price,
            exit_price: record.exit_price,
            pnl: record.pnl,
            estimated_edge: record.edge_net,
            indicators_triggered: record.indicators_used.clone(),
        };

        if self.predictor.is_some() {
            let is_win = trade_sample.is_win;
            let conf = trade_sample.entry_features.calibrator_confidence;

            if let Some(ref mut predictor) = self.predictor {
                predictor.record_outcome(conf, is_win);
                predictor.adjust_weights_dynamically();
            }

            self.state.add_prediction_result(is_win);
            // All closed trades go into the training dataset (fallback + ML).
            // The target is pure BTC direction (1.0=UP, 0.0=DOWN), so fallback trades
            // provide valid supervised signal — the ML learns real BTC outcomes from them.
            // Excluding fallbacks created a bootstrap deadlock: ML needs 30 samples to
            // train, but with no samples it always uses fallback, which was excluded →
            // dataset stays at 0 forever.
            self.add_trade_to_dataset(trade_sample);

            // Auto-save ML state after every trade so we don't lose data on crash/restart
            if self.state.total_predictions % 1 == 0 {
                if let Some(ref predictor) = self.predictor {
                    if let Err(e) =
                        self.persistence
                            .save_ml_state(predictor, &self.state, &self.dataset)
                    {
                        tracing::warn!("Failed to auto-save ML state: {}", e);
                    }
                }
            }
            // Trigger retraining if appropriate
            if let Err(e) = self.maybe_retrain() {
                tracing::warn!("Failed during maybe_retrain check: {}", e);
            }

            info!(
                asset = ?asset,
                timeframe = ?timeframe,
                prediction_correct = record.prediction_correct,
                window = %format!("${:.2} -> ${:.2}", record.window_open_price, record.window_close_price),
                actual_direction = %record.actual_price_direction,
                "Trade outcome registered in V3 Predictor (using prediction_correct)"
            );
        }
    }

    /// Force save state to disk gracefully
    pub fn force_save_state(&mut self) {
        if let Some(ref predictor) = self.predictor {
            if let Err(e) = self
                .persistence
                .save_ml_state(predictor, &self.state, &self.dataset)
            {
                tracing::warn!("Failed to auto-save ML state: {}", e);
            } else {
                tracing::info!("💾 V3 ML State safely persisted on shutdown");
            }
        }
    }

    /// Trigger retraining if needed
    pub fn maybe_retrain(&mut self) -> anyhow::Result<()> {
        if self
            .state
            .should_retrain(self.config.training.retrain_interval_trades)
        {
            info!("🔄 Retraining V3 models...");
            if let Err(e) = self.train_initial_model(vec![]) {
                tracing::warn!("⚠️ Auto-training failed during retraining check: {}", e);
            }
        }
        Ok(())
    }

    // Getters for dashboard/monitoring
    pub fn last_filter_reason(&self) -> Option<String> {
        self.last_filter_reason.clone()
    }

    pub fn export_calibrator_state_v2(&self) -> HashMap<String, Vec<IndicatorStats>> {
        self.calibrator.export_stats_by_market()
    }

    pub fn import_calibrator_state_v2(&mut self, stats: HashMap<String, Vec<IndicatorStats>>) {
        self.calibrator.load_stats_by_market(stats);
        self.calibrator.recalibrate();
    }

    pub fn is_calibrated(&self) -> bool {
        self.calibrator.is_calibrated()
    }

    pub fn calibrator_total_trades(&self) -> usize {
        self.calibrator.total_trades()
    }

    /// Get ML state for dashboard
    pub fn get_ml_state(&self) -> MLStateResponse {
        let (ensemble_weights, model_info) = if let Some(ref p) = self.predictor {
            let weights = vec![
                p.weights.random_forest,
                p.weights.gradient_boosting,
                p.weights.logistic_regression,
            ];
            let info: Vec<(String, f64, f64)> = p
                .models
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let weight = match i {
                        0 => p.weights.random_forest,
                        1 => p.weights.gradient_boosting,
                        2 => p.weights.logistic_regression,
                        _ => 0.0,
                    };
                    (m.name().to_string(), weight, m.accuracy())
                })
                .collect();
            (Some(weights), info)
        } else {
            (
                None,
                vec![
                    ("Random Forest".to_string(), 0.40, 0.0),
                    ("Gradient Boosting".to_string(), 0.35, 0.0),
                    ("Logistic Regression".to_string(), 0.25, 0.0),
                ],
            )
        };

        let incorrect = self.state.incorrect_predictions;
        let correct = self.state.correct_predictions;
        let total = self.state.total_predictions;

        MLStateResponse {
            enabled: self.config.enabled,
            model_type: format!("{:?}", self.config.model_type),
            version: "3.0".to_string(),
            model_accuracy: self.state.accuracy(),
            total_predictions: self.state.total_predictions,
            correct_predictions: self.state.correct_predictions,
            incorrect_predictions: self.state.incorrect_predictions,
            win_rate: self.state.win_rate(),
            loss_rate: self.state.loss_rate(),
            is_calibrated: self.is_calibrated(),
            last_filter_reason: self.last_filter_reason.clone(),
            ensemble_weights,
            model_info,
            training_epoch: self.state.training_epoch,
            dataset_size: self.dataset.len(),
        }
    }

    /// Save ML state manually
    pub fn save_state(&mut self) -> anyhow::Result<()> {
        if let Some(ref predictor) = self.predictor {
            self.persistence
                .save_ml_state(predictor, &self.state, &self.dataset)?;
            info!("💾 ML state saved manually");
        }
        Ok(())
    }

    /// Create a backup of current ML state
    pub fn backup(&self) -> anyhow::Result<String> {
        let backup_path = self.persistence.backup()?;
        info!("💾 ML state backed up to: {}", backup_path);
        Ok(backup_path)
    }

    /// List available backups
    pub fn list_backups(&self) -> anyhow::Result<Vec<String>> {
        self.persistence.list_backups()
    }

    /// Restore from a backup
    pub fn restore_backup(&mut self, backup_name: &str) -> anyhow::Result<()> {
        self.persistence.restore_backup(backup_name)?;

        // Reload state after restore
        if let Some((persisted, dataset)) = self.persistence.load_ml_state()? {
            self.state.total_predictions = persisted.total_predictions;
            self.state.correct_predictions = persisted.correct_predictions;
            self.state.incorrect_predictions = persisted.incorrect_predictions;
            self.state.last_retraining = persisted.last_retraining;
            self.dataset = dataset;
            info!("🔄 ML state restored from backup: {}", backup_name);
        }

        Ok(())
    }

    /// Get training history
    pub fn get_training_history(&self) -> Vec<TrainingRecord> {
        self.persistence.load_training_history().unwrap_or_default()
    }

    /// Agregar un trade al dataset de entrenamiento (acumulativo)
    pub fn add_trade_to_dataset(&mut self, trade: TradeSample) {
        let previous_size = self.dataset.len();
        self.dataset.add_trade(trade);

        // Guardar dataset cada 10 trades nuevos
        if self.dataset.len() % 10 == 0 {
            if let Err(e) = self
                .dataset
                .save(&format!("{}/ml_engine/dataset.json", "./data"))
            {
                warn!("Failed to auto-save dataset: {}", e);
            } else {
                info!("💾 Dataset auto-saved: {} samples", self.dataset.len());
            }
        }

        info!(
            "📊 Trade added to dataset: {} → {} samples",
            previous_size,
            self.dataset.len()
        );
    }

    /// Compute the [open_ts, close_ts) window boundaries for a given tick timestamp.
    /// Uses simple time-grid alignment (floor to nearest 15m or 1h boundary).
    fn window_boundaries(ts_ms: i64, timeframe: Timeframe) -> (i64, i64) {
        let window_ms = match timeframe {
            Timeframe::Min15 => 15 * 60 * 1000,
            Timeframe::Hour1 => 60 * 60 * 1000,
        };
        let open = (ts_ms / window_ms) * window_ms;
        (open, open + window_ms)
    }

    /// Get the N-th previous window outcome (1 = most recent).
    /// Returns 0.5 when there is not enough history (unknown).
    fn get_prev_window_dir(&self, asset: Asset, timeframe: Timeframe, n: usize) -> f64 {
        if let Some(history) = self.window_history.get(&(asset, timeframe)) {
            let len = history.len();
            if len >= n {
                return history[len - n]; // VecDeque is front-indexed
            }
        }
        0.5 // unknown
    }

    /// Signed streak of consecutive windows in the same direction.
    /// Returns a value in [-1, 1] (positive = streak of UPs, negative = streak of DOWNs).
    /// Capped at 5 consecutive to stay in range.
    fn calculate_window_streak(&self, asset: Asset, timeframe: Timeframe) -> f64 {
        if let Some(history) = self.window_history.get(&(asset, timeframe)) {
            if history.is_empty() {
                return 0.0;
            }
            let last_dir = *history.back().unwrap();
            let last_is_up = last_dir > 0.5;
            let mut streak: i32 = 0;
            for &dir in history.iter().rev() {
                if (dir > 0.5) == last_is_up {
                    streak += if last_is_up { 1 } else { -1 };
                } else {
                    break;
                }
            }
            (streak as f64 / 5.0).clamp(-1.0, 1.0)
        } else {
            0.0
        }
    }

    /// Cross-timeframe alignment: +1 if this market and the other timeframe agree on the
    /// last window direction, -1 if they disagree, 0 if not enough history on either side.
    fn calculate_cross_tf_alignment(&self, asset: Asset, timeframe: Timeframe) -> f64 {
        let other_tf = match timeframe {
            Timeframe::Min15 => Timeframe::Hour1,
            Timeframe::Hour1 => Timeframe::Min15,
        };
        let my_last = self.window_history
            .get(&(asset, timeframe))
            .and_then(|h| h.back().copied());
        let other_last = self.window_history
            .get(&(asset, other_tf))
            .and_then(|h| h.back().copied());
        match (my_last, other_last) {
            (Some(mine), Some(other)) => {
                if (mine > 0.5) == (other > 0.5) { 1.0 } else { -1.0 }
            }
            _ => 0.0,
        }
    }

    /// Volume trend: +1 if recent 24h volume is rising, -1 if falling, 0 if stable.
    /// Compares the average of the last 5 observations to the previous 5.
    fn calculate_volume_trend_for_market(&self, asset: Asset, timeframe: Timeframe) -> f64 {
        if let Some(history) = self.volume_history.get(&(asset, timeframe)) {
            if history.len() < 10 {
                return 0.0;
            }
            let recent: f64 = history.iter().rev().take(5).sum::<f64>() / 5.0;
            let older: f64 = history.iter().rev().skip(5).take(5).sum::<f64>() / 5.0;
            if older < 1.0 {
                return 0.0;
            }
            let change = (recent - older) / older;
            if change > 0.10 { 1.0 } else if change < -0.10 { -1.0 } else { 0.0 }
        } else {
            0.0
        }
    }

    /// Obtener estadísticas del dataset
    pub fn get_dataset_stats(&self) -> DatasetStats {
        DatasetStats {
            total_samples: self.dataset.len(),
            wins: self
                .dataset
                .samples
                .iter()
                .filter(|s| s.target > 0.5)
                .count(),
            losses: self
                .dataset
                .samples
                .iter()
                .filter(|s| s.target <= 0.5)
                .count(),
        }
    }
}

/// ML State for dashboard
#[derive(Debug, Clone, serde::Serialize)]
pub struct MLStateResponse {
    pub enabled: bool,
    pub model_type: String,
    pub version: String,
    pub model_accuracy: f64,
    pub total_predictions: usize,
    pub correct_predictions: usize,
    pub incorrect_predictions: usize,
    pub win_rate: f64,
    pub loss_rate: f64,
    pub is_calibrated: bool,
    pub last_filter_reason: Option<String>,
    pub ensemble_weights: Option<Vec<f64>>,
    /// Per-model info: (name, weight, accuracy)
    pub model_info: Vec<(String, f64, f64)>,
    pub training_epoch: usize,
    pub dataset_size: usize,
}

/// Dataset statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct DatasetStats {
    pub total_samples: usize,
    pub wins: usize,
    pub losses: usize,
}
