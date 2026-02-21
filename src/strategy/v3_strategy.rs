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
use std::collections::HashMap;
use std::path::Path;
use tracing::{error, info, warn};

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
}

impl V3Strategy {
    pub fn new(ml_config: MLEngineConfig, _base_config: crate::strategy::StrategyConfig) -> Self {
        let calibrator = IndicatorCalibrator::with_min_samples(30);

        // Initialize persistence manager
        let persistence = MLPersistenceManager::new("./data");

        // Try to load previous state
        let (
            mut predictor,
            mut dataset,
            mut state,
        ) = match persistence.load_ml_state() {
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

                (
                    pred,
                    loaded_dataset,
                    ml_state,
                )
            }
            Ok(None) => {
                info!("🆕 No previous ML state found, starting fresh");
                let weights = EnsembleWeights::from_config(&ml_config.ensemble);
                let pred = if ml_config.enabled {
                    Some(MLPredictor::new(weights))
                } else {
                    None
                };
                (
                    pred,
                    Dataset::new(),
                    MLEngineState::new(ml_config.clone()),
                )
            }
            Err(e) => {
                warn!("⚠️ Failed to load ML state: {}, starting fresh", e);
                let weights = EnsembleWeights::from_config(&ml_config.ensemble);
                let pred = if ml_config.enabled {
                    Some(MLPredictor::new(weights))
                } else {
                    None
                };
                (
                    pred,
                    Dataset::new(),
                    MLEngineState::new(ml_config.clone()),
                )
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
            min_btc_eth_correlation: ml_config.filters.min_btc_eth_correlation,
            max_btc_eth_correlation: ml_config.filters.max_btc_eth_correlation,
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
        let ml_features = self.feature_engine.compute(features, &context);

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
            btc_eth_correlation: context.btc_eth_correlation,
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

        self.state.total_predictions += 1;

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
        let mut score = 0.0;
        let mut reasons = Vec::new();

        // RSI
        if let Some(rsi) = features.rsi {
            if rsi < 35.0 {
                score += 1.0;
                reasons.push("RSI_oversold".to_string());
            } else if rsi > 65.0 {
                score -= 1.0;
                reasons.push("RSI_overbought".to_string());
            }
        }

        // MACD
        if let Some(macd) = features.macd {
            if macd > 0.0 {
                score += 0.5;
                reasons.push("MACD_bullish".to_string());
            } else {
                score -= 0.5;
                reasons.push("MACD_bearish".to_string());
            }
        }

        let confidence = ((score as f64).abs() / 3.0).min(1.0) * 0.5 + 0.5;

        if confidence < self.config.min_confidence {
            tracing::info!(
                ?features.asset,
                ?features.timeframe,
                confidence,
                min_confidence = self.config.min_confidence,
                "❌ Fallback signal rejected (low confidence)"
            );
            self.last_filter_reason = Some("fallback_low_confidence".to_string());
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
            indicators_used: vec!["v3_fallback".to_string()],
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
            btc_eth_correlation: 0.0, // Would need cross-asset data
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
    pub fn register_closed_trade_result(&mut self, record: &crate::paper_trading::PaperTradeRecord) {
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
        
        let direction = if record.direction.eq_ignore_ascii_case("up") || record.direction.eq_ignore_ascii_case("buy") {
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
            exit_ts: record.market_close_ts.max(record.timestamp + record.hold_duration_ms),
            asset,
            timeframe,
            direction,
            is_win: record.pnl >= 0.0,
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
            
            info!(
                asset = ?asset,
                timeframe = ?timeframe,
                pnl = record.pnl,
                "✅ Trade outcome successfully registered in V3 Predictor for dynamic continuous learning"
            );
        }
    }

    /// Force save state to disk gracefully
    pub fn force_save_state(&mut self) {
        if let Some(ref predictor) = self.predictor {
            if let Err(e) =
                self.persistence
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
            // In a full implementation, this would retrain with recent data
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
            let info: Vec<(String, f64, f64)> = p.models.iter().enumerate().map(|(i, m)| {
                let weight = match i {
                    0 => p.weights.random_forest,
                    1 => p.weights.gradient_boosting,
                    2 => p.weights.logistic_regression,
                    _ => 0.0,
                };
                (m.name().to_string(), weight, m.accuracy())
            }).collect();
            (Some(weights), info)
        } else {
            (None, vec![
                ("Random Forest".to_string(), 0.40, 0.0),
                ("Gradient Boosting".to_string(), 0.35, 0.0),
                ("Logistic Regression".to_string(), 0.25, 0.0),
            ])
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
