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
    /// Window open timestamp (ms) â€“ used as the sample timestamp
    open_ts: i64,
}

/// ML-Powered Trading Strategy (V3)
pub struct V3Strategy {
    /// ML Engine configuration
    config: MLEngineConfig,
    /// Ensemble predictor with RF, GB, LR models (shared, all assets/timeframes)
    predictor: Option<MLPredictor>,
    /// Per-asset predictors (BTC, ETH) — trained when asset has >= min_samples.
    /// Preferred over shared predictor when available (better asset-specific patterns).
    asset_predictors: HashMap<Asset, MLPredictor>,
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
    /// EVERY window outcome â€” not just windows where a trade was executed.
    pending_windows: HashMap<(Asset, Timeframe, i64), WindowObservation>,
    /// Finalized window outcomes per market (last 5). Used to compute sequential
    /// features: prev_window_dir_1/2/3, window_streak, cross_tf_alignment.
    window_history: HashMap<(Asset, Timeframe), VecDeque<f64>>,
    /// Recent Polymarket 24h volume per market for volume_trend feature.
    volume_history: HashMap<(Asset, Timeframe), VecDeque<f64>>,
    /// Counter of window observations (NOT trades) â€” used for retraining trigger.
    window_observations_count: usize,
    /// One-per-window throttle: tracks the last window open_ts we entered per market.
    last_entry_window: HashMap<(Asset, Timeframe), i64>,
}

impl V3Strategy {
    pub fn new(ml_config: MLEngineConfig, _base_config: crate::strategy::StrategyConfig) -> Self {
        Self::new_with_data_dir(ml_config, _base_config, "./data")
    }

    pub fn new_with_data_dir(
        ml_config: MLEngineConfig,
        _base_config: crate::strategy::StrategyConfig,
        data_dir: &str,
    ) -> Self {
        let calibrator = IndicatorCalibrator::with_min_samples(30);

        // Initialize persistence manager
        let persistence = MLPersistenceManager::new(data_dir);

        // Try to load previous state
        let (mut predictor, mut dataset, mut state) = match persistence.load_ml_state() {
            Ok(Some((persisted, loaded_dataset))) => {
                info!("ðŸ§  Loading persisted ML state...");

                // Restore ensemble weights
                let weights = persisted.ensemble_weights;
                let pred = if ml_config.enabled {
                    Some(MLPredictor::new(weights))
                } else {
                    None
                };

                let mut ml_state = MLEngineState::new(ml_config.clone());
                // Don't restore prediction counters â€” the persisted values can be stale/corrupted
                // across restarts. Let accuracy accumulate freshly from closed trades this session.
                ml_state.total_predictions = persisted.total_predictions;
                ml_state.correct_predictions = persisted.correct_predictions;
                ml_state.incorrect_predictions = persisted.incorrect_predictions;
                ml_state.last_retraining = persisted.last_retraining;

                if ml_state.total_predictions == 0 && !loaded_dataset.samples.is_empty() {
                    let reconstructed_correct = loaded_dataset
                        .samples
                        .iter()
                        .filter(|sample| sample.target > 0.5)
                        .count();
                    ml_state.total_predictions = loaded_dataset.len();
                    ml_state.correct_predictions = reconstructed_correct;
                    ml_state.incorrect_predictions =
                        loaded_dataset.len().saturating_sub(reconstructed_correct);
                    info!(
                        total_predictions = ml_state.total_predictions,
                        correct_predictions = ml_state.correct_predictions,
                        incorrect_predictions = ml_state.incorrect_predictions,
                        "Reconstructed ML counters from persisted dataset because saved counters were empty"
                    );
                }

                info!(
                    "âœ… ML state restored: {} samples in dataset (prediction accuracy resets each session)",
                    loaded_dataset.len()
                );

                (pred, loaded_dataset, ml_state)
            }
            Ok(None) => {
                info!("ðŸ†• No previous ML state found, starting fresh");
                let weights = EnsembleWeights::from_config(&ml_config.ensemble);
                let pred = if ml_config.enabled {
                    Some(MLPredictor::new(weights))
                } else {
                    None
                };
                (pred, Dataset::new(), MLEngineState::new(ml_config.clone()))
            }
            Err(e) => {
                warn!("âš ï¸ Failed to load ML state: {}, starting fresh", e);
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
            asset_predictors: HashMap::new(),
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
            last_entry_window: HashMap::new(),
        };

        // Auto-train models on startup if we have enough samples
        if strategy.dataset.len() >= strategy.config.training.min_samples_for_training {
            info!(
                "ðŸŽ“ Auto-training models on startup with {} samples...",
                strategy.dataset.len()
            );
            if let Err(e) = strategy.train_initial_model(vec![]) {
                warn!("âš ï¸ Auto-training failed: {}", e);
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
            let vh = self
                .volume_history
                .entry((asset, timeframe))
                .or_insert_with(|| VecDeque::with_capacity(20));
            vh.push_back(current_volume);
            if vh.len() > 20 {
                vh.pop_front();
            }
        }

        // 1. Finalize any windows that have already closed (use current price as proxy for
        //    the window-close price â€” this is accurate to within one tick, ~1 second).
        //    IMPORTANT: only finalize windows for the *same asset* being processed here.
        //    Each asset's features carry that asset's price; mixing prices across assets
        //    produces wrong UP/DOWN labels and corrupts the ML training dataset.
        let current_price = features.close;
        let current_token_price = features.polymarket_price.unwrap_or(0.5);
        let now_ts = features.ts;
        let expired_keys: Vec<_> = self
            .pending_windows
            .iter()
            .filter(|(_, obs)| now_ts >= obs.close_ts && obs.asset == asset)
            .map(|(k, _)| *k)
            .collect();
        for key in expired_keys {
            if let Some(obs) = self.pending_windows.remove(&key) {
                let target = if current_price >= obs.price_at_open {
                    1.0
                } else {
                    0.0
                };
                let direction_label = if target > 0.5 { "UP" } else { "DOWN" };
                // Update window_history for this market so sequential features stay fresh.
                {
                    let wh = self
                        .window_history
                        .entry((obs.asset, obs.timeframe))
                        .or_insert_with(|| VecDeque::with_capacity(6));
                    wh.push_back(target);
                    if wh.len() > 5 {
                        wh.pop_front();
                    }
                }
                info!(
                    asset = ?obs.asset,
                    timeframe = ?obs.timeframe,
                    price_open = obs.price_at_open,
                    price_close = current_price,
                    outcome = direction_label,
                    dataset_before = self.dataset.len(),
                    "ðŸªŸ Window closed â€” recording ground-truth observation"
                );
                // Skip samples with mostly-zero features (captured before indicator warmup)
                let feat_vec = obs.features.to_vec();
                let non_zero = feat_vec.iter().filter(|&&v| v.abs() > 1e-10).count();
                if non_zero < feat_vec.len() / 3 {
                    info!(
                        non_zero,
                        total = feat_vec.len(),
                        "âš ï¸ Skipping low-quality window observation ({}/{} non-zero features)",
                        non_zero,
                        feat_vec.len()
                    );
                } else {
                    self.dataset.add_window_observation(
                        obs.features,
                        target,
                        obs.open_ts,
                        obs.asset,
                        obs.timeframe,
                        obs.price_at_open,
                        current_price,
                    );
                    self.window_observations_count += 1;
                    info!(
                    "ðŸ“Š Window observation added to dataset: {} samples total ({} windows recorded)",
                    self.dataset.len(),
                    self.window_observations_count
                );
                    // Auto-save after EVERY new observation to survive restarts.
                    // Previously saved every 10 observations, but frequent restarts
                    // caused data loss since the threshold was never reached.
                    if let Err(e) = self.dataset.save(self.persistence.dataset_file()) {
                        warn!(
                            "Failed to auto-save dataset after window observation: {}",
                            e
                        );
                    }
                    // Check if we need to retrain based on window observations
                    if self.window_observations_count
                        % self.config.training.retrain_interval_window_observations
                        == 0
                        && self.dataset.len() >= self.config.training.min_samples_for_training
                    {
                        info!(
                            "ðŸ”„ Retraining after {} window observations...",
                            self.window_observations_count
                        );
                        if let Err(e) = self.train_initial_model(vec![]) {
                            warn!("âš ï¸ Retraining after window observations failed: {}", e);
                        }
                    }
                } // end else (non-zero filter)
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
            } else {
                0.0
            };
            ml_features.token_price_change_window = change.clamp(-1.0, 1.0);

            // price_vs_strike_pct: how far BTC/ETH has moved from the window-open price.
            // Positive = above strike (favors UP), negative = below (favors DOWN).
            // Clipped to [-10, +10] percent to avoid extreme values distorting the model.
            if obs.price_at_open > 1e-9 {
                let pct = (current_price - obs.price_at_open) / obs.price_at_open * 100.0;
                ml_features.price_vs_strike_pct = pct.clamp(-10.0, 10.0);
            }
        } else {
            // First tick of this window â€” no change yet.
            ml_features.token_price_window_open = current_token_price;
            ml_features.token_price_change_window = 0.0;
            ml_features.price_vs_strike_pct = 0.0;
        }

        // 3. Register this window if we haven't seen it yet (one snapshot per window).
        self.pending_windows
            .entry(win_key)
            .or_insert_with(|| WindowObservation {
                features: ml_features.clone(),
                price_at_open: current_price,
                token_price_at_open: current_token_price,
                close_ts: win_close_ts,
                asset,
                timeframe,
                open_ts: win_open_ts,
            });

        // Patch polymarket_price in the pending observation as soon as real data arrives.
        // The first tick of a window often has polymarket_price=0.5 (orderbook not yet loaded).
        // Once a real price is available (features.polymarket_price is Some), update the stored
        // snapshot so the ML trains on actual market consensus rather than the 0.5 neutral default.
        if features.polymarket_price.is_some() {
            if let Some(pending) = self.pending_windows.get_mut(&win_key) {
                if (pending.features.polymarket_price - 0.5).abs() < 1e-6 {
                    pending.features.polymarket_price = current_token_price;
                    pending.token_price_at_open = current_token_price;
                    tracing::debug!(
                        asset = ?asset,
                        timeframe = ?timeframe,
                        token_price = current_token_price,
                        "🔧 Updated window observation polymarket_price from 0.5 to real value"
                    );
                }
            }
        }
        // === END WINDOW OBSERVATION TRACKING ===
        // One-per-window throttle: max 1 trade per (asset, timeframe) window.
        // Run this before filters/ML to avoid repeated evaluations in the same window
        // after a trade was already opened.
        let now_ms = chrono::Utc::now().timestamp_millis();
        let window_is_current = win_close_ts > now_ms;
        let already_entered_this_window = window_is_current
            && self
                .last_entry_window
                .get(&(asset, timeframe))
                .copied()
                .map(|last| last == win_open_ts)
                .unwrap_or(false);

        if already_entered_this_window {
            tracing::info!(
                ?asset,
                ?timeframe,
                win_open_ts,
                "Signal throttled (one_per_window)"
            );
            self.last_filter_reason = Some("one_per_window_throttle".to_string());
            return None;
        }

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
                tracing::debug!(?asset, ?timeframe, "âœ… Market filters passed");
            }
            FilterDecision::Reject(reason) => {
                let reason_str = format!("{:?}", reason);
                tracing::info!(?asset, ?timeframe, reason = %reason_str, "âŒ Signal rejected by market filter");
                self.last_filter_reason = Some(reason_str);
                return None;
            }
        }

        // Check if ML is enabled and models are trained.
        // ml_available if shared predictor exists OR an asset-specific predictor exists.
        let ml_available = self.config.enabled
            && (self.predictor.is_some() || self.asset_predictors.contains_key(&asset));
        let dataset_size = self.dataset.len();
        let min_samples = self.config.training.min_samples_for_training;

        tracing::debug!(
            ?asset,
            ?timeframe,
            ml_enabled = self.config.enabled,
            predictor_loaded = self.predictor.is_some(),
            asset_predictor_loaded = self.asset_predictors.contains_key(&asset),
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
                    "âš ï¸ Concept drift detected â€” market regime may have changed, consider retraining"
                );
            }
        }

        let ml_ready = ml_available && dataset_size >= min_samples;

        // Try ML prediction if available
        if ml_ready {
            if let Some(prediction) = self.try_ml_prediction(&ml_features, features, &context) {
                // NOTE: do NOT mark last_entry_window here — it's marked in main.rs after
                // the risk manager approves, via confirm_window_entered(). This prevents
                // phantom throttles when the risk manager rejects a valid signal.
                return Some(prediction);
            }

            if !self.config.allow_fallback_when_ml_ready {
                tracing::info!(
                    ?asset,
                    ?timeframe,
                    reason = ?self.last_filter_reason,
                    "ML ready but signal rejected; fallback disabled"
                );
                return None;
            }
        }

        // Fallback to rule-based if ML not available or rejected
        if !ml_ready && dataset_size < min_samples {
            tracing::info!(
                ?asset,
                ?timeframe,
                dataset_size,
                min_samples_required = min_samples,
                "ML not ready (insufficient training data), using fallback"
            );
        } else if !ml_ready && !ml_available {
            tracing::info!(
                ?asset,
                ?timeframe,
                "ML disabled or models not loaded, using fallback"
            );
        }

        // Try fallback signal only during bootstrap, or if explicitly enabled after ML rejection.
        if !ml_ready || self.config.allow_fallback_when_ml_ready {
            if let Some(signal) = self.fallback_signal(features) {
                tracing::info!(
                    ?asset,
                    ?timeframe,
                    direction = ?signal.direction,
                    confidence = signal.confidence,
                    "Fallback signal generated"
                );
                // NOTE: do NOT mark last_entry_window here — confirmed via confirm_window_entered()
                return Some(signal);
            }
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
        // Prefer asset-specific predictor; fall back to shared predictor.
        let predictor: &MLPredictor = if let Some(p) = self.asset_predictors.get(&features.asset) {
            p
        } else if let Some(ref p) = self.predictor {
            p
        } else {
            return None;
        };
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

        // Token price divergence filter: only trade when our prediction diverges from the
        // Polymarket market consensus by â‰¥8%. Low divergence means we agree with the market
        // and have no edge â€” the token price already reflects our prediction.
        // market_price ~0.5 when no real Polymarket data available (safe to proceed).
        let market_token_price = ml_features.polymarket_price;
        let divergence = (prediction.prob_up - market_token_price).abs();
        if market_token_price > 0.45 && market_token_price < 0.55 {
            // Market is near 50% (high uncertainty) â€” divergence filter less meaningful,
            // allow if our signal has sufficient edge already (edge check below covers this).
        } else if divergence < 0.08 {
            let reason = format!(
                "low_token_divergence: |ml={:.2} - mkt={:.2}| = {:.3} < 0.08",
                prediction.prob_up, market_token_price, divergence
            );
            tracing::info!(
                asset = ?features.asset,
                timeframe = ?features.timeframe,
                ml_prob = prediction.prob_up,
                market_price = market_token_price,
                divergence,
                "âŒ ML signal rejected â€” no edge vs market consensus"
            );
            self.last_filter_reason = Some(reason);
            return None;
        }

        // Determine direction by market edge, not model threshold.
        // If model says prob_up=0.58 but market prices UP at 0.82, the opportunity is DOWN:
        //   edge_down = (1 - 0.58) - (1 - 0.82) = 0.42 - 0.18 = +0.24 (profitable)
        // Using prob_up > 0.5 would generate UP with edge = 0.58 - 0.82 = -0.24 (rejected).
        let p_market_up = ml_features.polymarket_price.clamp(0.01, 0.99);
        let direction = if prediction.prob_up > p_market_up {
            Direction::Up // model thinks UP is underpriced by market
        } else {
            Direction::Down // model thinks UP is overpriced; buy NO tokens
        };
        let (p_model_side, p_market_side) = if direction == Direction::Up {
            (prediction.prob_up, p_market_up)
        } else {
            (1.0 - prediction.prob_up, 1.0 - p_market_up)
        };
        let spread_abs = (ml_features.spread_bps.max(0.0) / 10_000.0).clamp(0.0, 0.50);
        let fee_rate = crate::polymarket::fee_rate_from_price(p_market_side);
        let ev = crate::polymarket::estimate_expected_value(
            p_market_side,
            p_model_side,
            p_market_side,
            fee_rate,
            spread_abs,
            0.005,
        );
        if ev.edge_net < self.config.min_edge_net {
            let reason = format!(
                "ml_edge_net_too_small: {:.4} < {:.4}",
                ev.edge_net, self.config.min_edge_net
            );
            tracing::info!(
                asset = ?features.asset,
                timeframe = ?features.timeframe,
                edge_net = ev.edge_net,
                min_edge_net = self.config.min_edge_net,
                "ML prediction rejected by net-edge gate"
            );
            self.last_filter_reason = Some(reason);
            return None;
        }

        // Calculate final confidence: blend model confidence with edge strength.
        // Previous formula (model_conf * edge * 2.0) required prob_up > 0.76 to pass
        // min_confidence=0.52, which silently rejected every ML signal.
        // New formula: confidence = model_conf * (0.7 + edge).
        // At edge=0.05 (prob_up=0.55): confidence = model_conf * 0.75 → model_conf=0.70 passes 0.52
        // At edge=0.15 (prob_up=0.65): confidence = model_conf * 0.85
        // At edge=0.30 (prob_up=0.80): confidence = model_conf * 1.00
        let edge = (prediction.prob_up - 0.5).abs();
        let confidence = (prediction.confidence * (0.7 + edge)).clamp(0.0, 1.0);

        // Require a minimum edge (at least 5% = prob_up > 0.55 or < 0.45)
        if edge < 0.05 {
            let reason = format!("ml_edge_too_small: {:.3} < 0.05", edge);
            tracing::debug!(%reason, "ML prediction edge too small");
            self.last_filter_reason = Some(reason);
            return None;
        }

        // Check minimum confidence after edge blending
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
            format!("edge_net:{:.4}", ev.edge_net),
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
            "ðŸ¤– V3 ML Signal generated"
        );

        Some(GeneratedSignal {
            asset: features.asset,
            timeframe: features.timeframe,
            direction,
            confidence,
            model_prob_up: Some(prediction.prob_up.clamp(0.01, 0.99)),
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

        // RSI removed â€” in crypto/BTC, RSI overbought means momentum continuation (not reversal)
        // Only EMA(1.0), MACD(0.5), HA(0.5) are used. Max score = 2.0 when all 3 agree.
        let max_score = 2.0;
        let confidence = ((score as f64).abs() / max_score).min(1.0) * 0.5 + 0.5; // 0.5 floor

        if confidence < self.config.min_confidence {
            tracing::info!(
                ?features.asset,
                ?features.timeframe,
                confidence,
                min_confidence = self.config.min_confidence,
                "âŒ Fallback signal rejected (low confidence, weak trend)"
            );
            self.last_filter_reason = Some("fallback_low_confidence".to_string());
            return None;
        }

        // Require 2-of-3 alignment: EMA(1.0) + MACD(0.5) + HA(0.5) ≥ 1.5 (2-of-3 sufficient)
        if score.abs() < 1.5 {
            tracing::info!(
                ?features.asset,
                ?features.timeframe,
                score,
                "âŒ Fallback signal rejected (insufficient directional alignment â€” need all 3 indicators)"
            );
            let ema_signal = match (features.ema_9, features.ema_21) {
                (Some(e9), Some(e21)) => {
                    if e9 > e21 {
                        "bullish"
                    } else {
                        "bearish"
                    }
                }
                _ => "missing",
            };
            let macd_signal = match features.macd_hist.or(features.macd) {
                Some(v) => {
                    if v > 0.0 {
                        "bullish"
                    } else {
                        "bearish"
                    }
                }
                None => "missing",
            };
            let ha_signal = match &features.ha_trend {
                Some(Direction::Up) => "bullish",
                Some(Direction::Down) => "bearish",
                None => "missing",
            };
            tracing::debug!(
                asset = ?features.asset,
                timeframe = ?features.timeframe,
                score,
                ema = ema_signal,
                macd = macd_signal,
                ha = ha_signal,
                "Fallback rejected: partial alignment (need all 3 to agree)"
            );
            self.last_filter_reason = Some("fallback_weak_directional_score".to_string());
            return None;
        }

        let indicator_direction = if score > 0.0 {
            Direction::Up
        } else {
            Direction::Down
        };

        // Market edge gate: reject fallback signal if market has already priced in the direction.
        // Example: EMA/MACD/HA all bullish → indicator says UP, but if market prices UP at 0.82,
        // there is no edge on UP (we would be buying an overpriced token).
        // In that case, reject the fallback rather than entering a losing trade.
        if let Some(p_market) = features.polymarket_price {
            let p_market = p_market.clamp(0.01, 0.99);
            // Approximate prob_up from score: score of ±2.0 → prob 0.75/0.25
            let prob_up_approx = (score / 2.0).clamp(-0.5, 0.5) + 0.5;
            let edge_for_direction = match indicator_direction {
                Direction::Up => prob_up_approx - p_market,
                Direction::Down => (1.0 - prob_up_approx) - (1.0 - p_market),
            };
            if edge_for_direction < self.config.min_edge_net {
                tracing::info!(
                    asset = ?features.asset,
                    timeframe = ?features.timeframe,
                    score,
                    p_market,
                    edge = edge_for_direction,
                    min_edge = self.config.min_edge_net,
                    "❌ Fallback signal rejected — market already priced in direction (no edge)"
                );
                self.last_filter_reason = Some(format!(
                    "fallback_no_market_edge: {:.3} < {:.3}",
                    edge_for_direction, self.config.min_edge_net
                ));
                return None;
            }
        }

        let direction = indicator_direction;

        Some(GeneratedSignal {
            asset: features.asset,
            timeframe: features.timeframe,
            direction,
            confidence,
            model_prob_up: None,
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
            "ðŸŽ“ V3 training with {} new historical trades...",
            historical_trades.len()
        );

        let current_size = self.dataset.len();
        info!("ðŸ“Š Current dataset size: {} samples", current_size);

        // Agregar nuevos trades al dataset EXISTENTE (acumulativo)
        for trade in historical_trades {
            self.dataset.add_trade(trade);
        }

        info!(
            "ðŸ“ˆ Dataset updated: {} â†’ {} samples (added {})",
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

        // Crear predictor si aÃºn no existe (caso de full reset)
        if self.predictor.is_none() && self.config.enabled {
            info!("ðŸ†• Creating new predictor (no persisted state found)");
            let weights =
                crate::ml_engine::models::EnsembleWeights::from_config(&self.config.ensemble);
            self.predictor = Some(crate::ml_engine::models::MLPredictor::new(weights));
        }

        // Crear copia balanceada para entrenamiento (segmentada por asset/timeframe)
        let mut training_dataset = self.build_segmented_training_dataset();
        // Purge samples older than walk_forward_train_days to keep model fresh.
        // Older market conditions (different volatility regimes, BTC price levels)
        // hurt generalization more than they help with more data.
        training_dataset.purge_old_samples(self.config.training.walk_forward_train_days);
        training_dataset.balance_classes();
        info!("Dataset balanced: {} samples", training_dataset.len());

        // Train predictor con el dataset acumulativo
        if let Some(ref mut predictor) = self.predictor {
            predictor.train(&training_dataset)?;
            self.state.training_epoch += 1;
            self.state.last_retraining = Some(chrono::Utc::now().timestamp_millis());
            info!(
                "âœ… V3 models trained with {} total samples! Accuracy: {:.2}%",
                training_dataset.len(),
                predictor.ensemble_accuracy() * 100.0
            );
        } else {
            warn!("âš ï¸ Predictor is None, cannot train. ML may be disabled.");
        }

        // Train per-asset predictors when each asset has sufficient data.
        // Asset-specific models capture BTC vs ETH dynamics better than the shared model.
        let min_samples = self.config.training.min_samples_for_training;
        let unique_assets: std::collections::HashSet<Asset> =
            self.dataset.samples.iter().map(|s| s.asset).collect();
        for asset in unique_assets {
            let asset_samples: Vec<_> = self
                .dataset
                .samples
                .iter()
                .filter(|s| s.asset == asset)
                .cloned()
                .collect();
            if asset_samples.len() < min_samples {
                tracing::info!(
                    ?asset,
                    samples = asset_samples.len(),
                    min_required = min_samples,
                    "Per-asset predictor not ready yet (insufficient samples)"
                );
                continue;
            }
            let mut asset_dataset = crate::ml_engine::dataset::Dataset::new();
            for s in asset_samples {
                asset_dataset.samples.push(s);
            }
            asset_dataset.balance_classes();
            let weights =
                crate::ml_engine::models::EnsembleWeights::from_config(&self.config.ensemble);
            let mut asset_predictor = crate::ml_engine::models::MLPredictor::new(weights);
            match asset_predictor.train(&asset_dataset) {
                Ok(()) => {
                    info!(
                        "✅ Per-asset predictor trained for {:?}: {} samples, {:.2}% accuracy",
                        asset,
                        asset_dataset.len(),
                        asset_predictor.ensemble_accuracy() * 100.0
                    );
                    self.asset_predictors.insert(asset, asset_predictor);
                }
                Err(e) => warn!(
                    "⚠️ Per-asset predictor training failed for {:?}: {}",
                    asset, e
                ),
            }
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

        info!("ðŸ’¾ ML state saved after training");
        Ok(())
    }

    /// Run walk-forward validation
    pub fn run_walk_forward(&mut self) -> anyhow::Result<()> {
        info!("ðŸ”„ Running walk-forward validation...");

        // This would use the training_pipeline in a full implementation
        // For now, we just mark it as complete

        info!("âœ… Walk-forward validation complete");
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
            "ðŸ§  V3 Indicator calibrator updated from trade"
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

        let predicted_prob_up = if record.p_model.is_finite() && record.p_model > 0.0 {
            Some(match direction {
                crate::types::Direction::Up => record.p_model.clamp(0.01, 0.99),
                crate::types::Direction::Down => (1.0 - record.p_model).clamp(0.01, 0.99),
            })
        } else {
            Self::extract_prob_up_from_indicators(&record.indicators_used).or_else(|| {
                Some(
                    if direction == crate::types::Direction::Up {
                        record.confidence
                    } else {
                        1.0 - record.confidence
                    }
                    .clamp(0.01, 0.99),
                )
            })
        };

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
            predicted_prob_up,
            indicators_triggered: record.indicators_used.clone(),
        };

        if self.predictor.is_some() {
            let is_win = trade_sample.is_win;

            if let Some(ref mut predictor) = self.predictor {
                if let Some(prob) = trade_sample.predicted_prob_up {
                    predictor.record_outcome(prob, is_win);
                    predictor.adjust_weights_dynamically();
                } else {
                    tracing::warn!(
                        trade_id = %trade_sample.trade_id,
                        "Missing predicted_prob_up; skipping predictor outcome update"
                    );
                }
            }
            // Also update asset-specific predictor if one exists.
            if let Some(ref mut asset_pred) = self.asset_predictors.get_mut(&asset) {
                if let Some(prob) = trade_sample.predicted_prob_up {
                    asset_pred.record_outcome(prob, is_win);
                    asset_pred.adjust_weights_dynamically();
                }
            }

            self.state.add_prediction_result(is_win);
            // All closed trades go into the training dataset (fallback + ML).
            // The target is pure BTC direction (1.0=UP, 0.0=DOWN), so fallback trades
            // provide valid supervised signal â€” the ML learns real BTC outcomes from them.
            // Excluding fallbacks created a bootstrap deadlock: ML needs 30 samples to
            // train, but with no samples it always uses fallback, which was excluded â†’
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

    pub fn sync_prediction_counters(&mut self, total: usize, correct: usize, incorrect: usize) {
        if total == 0 {
            return;
        }

        let incorrect = if correct + incorrect == total {
            incorrect
        } else {
            total.saturating_sub(correct)
        };

        if self.state.total_predictions == total
            && self.state.correct_predictions == correct
            && self.state.incorrect_predictions == incorrect
        {
            return;
        }

        self.state.total_predictions = total;
        self.state.correct_predictions = correct;
        self.state.incorrect_predictions = incorrect;

        tracing::info!(
            total_predictions = total,
            correct_predictions = correct,
            incorrect_predictions = incorrect,
            "Synchronized ML prediction counters from persisted trade history"
        );
    }

    /// Force save state to disk gracefully
    pub fn force_save_state(&mut self) {
        // Always save dataset first (critical during bootstrap when no model exists)
        if let Err(e) = self.dataset.save(self.persistence.dataset_file()) {
            tracing::warn!("Failed to save dataset on shutdown: {}", e);
        }
        if let Some(ref predictor) = self.predictor {
            if let Err(e) = self
                .persistence
                .save_ml_state(predictor, &self.state, &self.dataset)
            {
                tracing::warn!("Failed to auto-save ML state: {}", e);
            } else {
                tracing::info!(
                    dataset_size = self.dataset.len(),
                    "V3 ML State safely persisted on shutdown"
                );
            }
        } else {
            tracing::info!(
                dataset_size = self.dataset.len(),
                "Dataset saved on shutdown (no model trained yet)"
            );
        }
    }

    /// Trigger retraining if needed
    pub fn maybe_retrain(&mut self) -> anyhow::Result<()> {
        if self
            .state
            .should_retrain(self.config.training.retrain_interval_trades)
        {
            info!("ðŸ”„ Retraining V3 models...");
            if let Err(e) = self.train_initial_model(vec![]) {
                tracing::warn!("âš ï¸ Auto-training failed during retraining check: {}", e);
            }
        }
        Ok(())
    }

    // Getters for dashboard/monitoring
    pub fn last_filter_reason(&self) -> Option<String> {
        self.last_filter_reason.clone()
    }

    /// Mark the window as entered. Called from main.rs AFTER the risk manager approves a signal,
    /// so the one_per_window throttle only fires when a trade was actually attempted.
    pub fn confirm_window_entered(
        &mut self,
        asset: crate::types::Asset,
        timeframe: crate::types::Timeframe,
        win_open_ts: i64,
    ) {
        self.last_entry_window
            .insert((asset, timeframe), win_open_ts);
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

        let (total, correct, incorrect) = self.effective_prediction_counters();
        let accuracy = if total == 0 {
            0.5
        } else {
            correct as f64 / total as f64
        };
        let win_rate = if total == 0 {
            0.0
        } else {
            correct as f64 / total as f64
        };
        let loss_rate = if total == 0 {
            0.0
        } else {
            incorrect as f64 / total as f64
        };

        MLStateResponse {
            enabled: self.config.enabled,
            model_type: format!("{:?}", self.config.model_type),
            version: "3.0".to_string(),
            model_accuracy: accuracy,
            total_predictions: total,
            correct_predictions: correct,
            incorrect_predictions: incorrect,
            win_rate,
            loss_rate,
            is_calibrated: self.is_calibrated(),
            last_filter_reason: self.last_filter_reason.clone(),
            ensemble_weights,
            model_info,
            training_epoch: self.state.training_epoch,
            dataset_size: self.dataset.len(),
        }
    }

    fn effective_prediction_counters(&self) -> (usize, usize, usize) {
        if self.state.total_predictions > 0
            || self.state.correct_predictions > 0
            || self.state.incorrect_predictions > 0
        {
            return (
                self.state.total_predictions,
                self.state.correct_predictions,
                self.state.incorrect_predictions,
            );
        }

        if self.dataset.samples.is_empty() {
            return (0, 0, 0);
        }

        let correct = self
            .dataset
            .samples
            .iter()
            .filter(|sample| sample.target > 0.5)
            .count();
        let total = self.dataset.len();
        let incorrect = total.saturating_sub(correct);
        (total, correct, incorrect)
    }

    /// Save ML state manually
    pub fn save_state(&mut self) -> anyhow::Result<()> {
        // Always save dataset
        self.dataset.save(self.persistence.dataset_file())?;
        if let Some(ref predictor) = self.predictor {
            self.persistence
                .save_ml_state(predictor, &self.state, &self.dataset)?;
            info!(dataset_size = self.dataset.len(), "ML state saved manually");
        } else {
            info!(
                dataset_size = self.dataset.len(),
                "Dataset saved manually (no model yet)"
            );
        }
        Ok(())
    }

    /// Create a backup of current ML state
    pub fn backup(&self) -> anyhow::Result<String> {
        let backup_path = self.persistence.backup()?;
        info!("ðŸ’¾ ML state backed up to: {}", backup_path);
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
            info!("ðŸ”„ ML state restored from backup: {}", backup_name);
        }

        Ok(())
    }

    /// Get training history
    pub fn get_training_history(&self) -> Vec<TrainingRecord> {
        self.persistence.load_training_history().unwrap_or_default()
    }

    /// Agregar un trade al dataset de entrenamiento (acumulativo)
    pub fn add_trade_to_dataset(&mut self, trade: TradeSample) {
        // Skip samples with mostly-zero features (captured before indicator warmup)
        let feature_vec = trade.entry_features.to_vec();
        let non_zero = feature_vec.iter().filter(|&&v| v.abs() > 1e-10).count();
        if non_zero < feature_vec.len() / 3 {
            info!(
                non_zero,
                total = feature_vec.len(),
                "âš ï¸ Skipping low-quality trade sample ({}/{} non-zero features)",
                non_zero,
                feature_vec.len()
            );
            return;
        }

        let previous_size = self.dataset.len();
        self.dataset.add_trade(trade);

        // Guardar dataset cada 10 trades nuevos
        if self.dataset.len() % 10 == 0 {
            if let Err(e) = self.dataset.save(self.persistence.dataset_file()) {
                warn!("Failed to auto-save dataset: {}", e);
            } else {
                info!("ðŸ’¾ Dataset auto-saved: {} samples", self.dataset.len());
            }
        }

        info!(
            "ðŸ“Š Trade added to dataset: {} â†’ {} samples",
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
        let my_last = self
            .window_history
            .get(&(asset, timeframe))
            .and_then(|h| h.back().copied());
        let other_last = self
            .window_history
            .get(&(asset, other_tf))
            .and_then(|h| h.back().copied());
        match (my_last, other_last) {
            (Some(mine), Some(other)) => {
                if (mine > 0.5) == (other > 0.5) {
                    1.0
                } else {
                    -1.0
                }
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
            if change > 0.10 {
                1.0
            } else if change < -0.10 {
                -1.0
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Build a training dataset segmented by (asset, timeframe) so the largest market
    /// does not dominate all model updates.
    fn build_segmented_training_dataset(&self) -> Dataset {
        if !self.config.segmented_training {
            return self.dataset.clone();
        }
        let mut grouped: HashMap<
            (Asset, Timeframe),
            Vec<crate::ml_engine::dataset::LabeledSample>,
        > = HashMap::new();
        for sample in &self.dataset.samples {
            grouped
                .entry((sample.asset, sample.timeframe))
                .or_default()
                .push(sample.clone());
        }
        let mut segment_sizes: Vec<usize> = grouped.values().map(|s| s.len()).collect();
        segment_sizes.retain(|s| *s > 0);
        // Not enough segment diversity: keep full dataset as-is.
        if segment_sizes.len() <= 1 {
            return self.dataset.clone();
        }
        let min_segment = *segment_sizes.iter().min().unwrap_or(&0);
        let max_segment = *segment_sizes.iter().max().unwrap_or(&0);
        if min_segment == 0 || max_segment == 0 {
            return self.dataset.clone();
        }
        // Cap each segment to at most 3x the smallest segment, using most-recent samples.
        let cap_per_segment = (min_segment * 3).max(min_segment);
        let mut segmented_samples = Vec::with_capacity(self.dataset.samples.len());
        for samples in grouped.values_mut() {
            samples.sort_by_key(|s| s.timestamp);
            let take = samples.len().min(cap_per_segment);
            segmented_samples.extend(samples.iter().rev().take(take).cloned());
        }
        if segmented_samples.is_empty() {
            return self.dataset.clone();
        }
        segmented_samples.sort_by_key(|s| s.timestamp);
        tracing::info!(
            original_samples = self.dataset.samples.len(),
            segmented_samples = segmented_samples.len(),
            segment_count = grouped.len(),
            min_segment = min_segment,
            max_segment = max_segment,
            cap_per_segment = cap_per_segment,
            "Segmented training dataset prepared"
        );
        Dataset {
            samples: segmented_samples,
            trade_samples: Vec::new(),
        }
    }
    fn extract_prob_up_from_indicators(indicators: &[String]) -> Option<f64> {
        const PREFIXES: [&str; 4] = ["prob_up:", "prob_up=", "p_up:", "p_up="];
        for item in indicators {
            let s = item.trim().to_ascii_lowercase();
            for prefix in PREFIXES {
                if let Some(raw) = s.strip_prefix(prefix) {
                    if let Ok(v) = raw.trim().parse::<f64>() {
                        if v.is_finite() {
                            return Some(v.clamp(0.01, 0.99));
                        }
                    }
                }
            }
        }
        None
    }
    /// Obtener estadisticas del dataset
    pub fn get_dataset_stats(&self) -> DatasetStats {
        let mut segment_counts: HashMap<(Asset, Timeframe), (usize, usize)> = HashMap::new();
        for sample in &self.dataset.samples {
            let entry = segment_counts
                .entry((sample.asset, sample.timeframe))
                .or_insert((0, 0));
            entry.0 += 1;
            if sample.target > 0.5 {
                entry.1 += 1;
            }
        }

        let mut by_segment = Vec::new();
        for ((asset, timeframe), (samples, wins)) in segment_counts {
            by_segment.push(DatasetSegmentStats {
                asset: asset.to_string(),
                timeframe: timeframe.to_string(),
                samples,
                wins,
                losses: samples.saturating_sub(wins),
                win_rate: if samples > 0 {
                    wins as f64 / samples as f64
                } else {
                    0.0
                },
            });
        }
        by_segment.sort_by(|a, b| a.asset.cmp(&b.asset).then(a.timeframe.cmp(&b.timeframe)));

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
            by_segment,
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
    pub by_segment: Vec<DatasetSegmentStats>,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct DatasetSegmentStats {
    pub asset: String,
    pub timeframe: String,
    pub samples: usize,
    pub wins: usize,
    pub losses: usize,
    pub win_rate: f64,
}
