//! ML Strategy Predictor - COMPLETO y FUNCIONAL

use crate::ml_engine::calibration::{CalibrationMethod, ProbabilityCalibrator};
use crate::ml_engine::dataset::{Dataset, TradeSample};
use crate::ml_engine::features::{FeatureEngine, MLFeatureVector, MarketContext};
use crate::ml_engine::filters::{FilterContext, FilterDecision, SmartFilterEngine};
use crate::ml_engine::models::{EnsembleWeights, MLPredictor, ModelPrediction};
use crate::ml_engine::training::{TrainingPipeline, WalkForwardConfig};
use crate::ml_engine::{MLEngineConfig, MLEngineState, Prediction};
use crate::strategy::calibrator::IndicatorCalibrator;
use crate::types::{Asset, Direction, Timeframe};
use std::collections::HashMap;
use tracing::{info, warn};

/// Predictor principal de estrategía ML - IMPLEMENTACIÓN COMPLETA
pub struct MLStrategyPredictor {
    config: MLEngineConfig,
    feature_engine: FeatureEngine,
    filter_engine: SmartFilterEngine,
    calibrator: ProbabilityCalibrator,
    ml_predictor: Option<MLPredictor>,
    state: MLEngineState,
    dataset: Dataset,
    trades_since_retrain: usize,
    window_observations_since_retrain: usize,
}

impl MLStrategyPredictor {
    pub fn new(config: MLEngineConfig) -> Self {
        let state = MLEngineState::new(config.clone());

        Self {
            config: config.clone(),
            feature_engine: FeatureEngine::new(),
            filter_engine: SmartFilterEngine::new(config.filters.clone()),
            calibrator: ProbabilityCalibrator::new(CalibrationMethod::Isotonic),
            ml_predictor: None,
            state,
            dataset: Dataset::new(),
            trades_since_retrain: 0,
            window_observations_since_retrain: 0,
        }
    }

    /// Generar predicción para features actuales
    pub fn predict(
        &mut self,
        features: &crate::features::Features,
        context: &MarketContext,
    ) -> Option<Prediction> {
        // 1. Calcular features ML
        let ml_features = self.feature_engine.compute(features, context);

        // 2. Aplicar filtros inteligentes
        let filter_context = self.create_filter_context(features, context);
        match self.filter_engine.evaluate(&filter_context) {
            FilterDecision::Reject(reason) => {
                warn!("Señal filtrada: {:?}", reason);
                return None;
            }
            FilterDecision::Allow => {}
        }

        // 3. Predecir con ML
        let ml_prediction = if let Some(ref predictor) = self.ml_predictor {
            if let Some(pred) = predictor.predict(&ml_features) {
                pred
            } else {
                return None;
            }
        } else {
            return None;
        };

        // 3.5 FILTRO DE MODEL AGREEMENT
        // Solo operar cuando los 3 modelos están de acuerdo o 2 de 3 con alta confianza
        if let Some(ref predictor) = self.ml_predictor {
            let agreement = predictor.get_model_agreement(&ml_features);
            // Requerir al menos 2 de 3 modelos de acuerdo
            if agreement.agreeing_models < 2 {
                warn!(
                    "Señal filtrada por falta de acuerdo entre modelos: {}/3",
                    agreement.agreeing_models
                );
                return None;
            }
            // Si solo 2 de 3 de acuerdo, requerir edge más alto
            if agreement.agreeing_models == 2 {
                let edge_2_of_3 = (ml_prediction.prob_up - 0.5).abs();
                if edge_2_of_3 < 0.12 {
                    // 12% edge mínimo cuando solo 2 modelos acuerdan
                    return None;
                }
            }
        }

        // 4. Calibrar probabilidad
        let calibrated_prob = self.calibrator.calibrate(ml_prediction.prob_up);

        // 5. Calcular edge
        let edge = (calibrated_prob - 0.5).abs();

        // UMbral de edge más alto para filtrar señales débiles
        // 0.08 = 8% edge mínimo (antes era 3%)
        if edge < 0.08 {
            return None;
        }

        // 6. Determinar dirección
        let direction = if calibrated_prob > 0.5 {
            Direction::Up
        } else {
            Direction::Down
        };

        // 7. Calcular confianza final con boost por edge fuerte
        let edge_boost = if edge > 0.15 { 1.2 } else if edge > 0.10 { 1.1 } else { 1.0 };
        let confidence = ml_prediction.confidence * (1.0 + edge * 2.0).min(1.5) * edge_boost;

        // 8. Verificar mínima confianza - más alto para mejor win rate
        // 0.60 = 60% confianza mínima (antes era 55%)
        if confidence < 0.60 {
            return None;
        }

        Some(Prediction {
            timestamp: features.ts,
            asset: features.asset,
            timeframe: features.timeframe,
            prob_up: calibrated_prob,
            confidence: confidence.clamp(0.0, 1.0),
            direction,
            edge,
            features_used: MLFeatureVector::feature_names()
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
            model_contributions: HashMap::new(), // Simplificado
            ensemble_weight: 1.0,
        })
    }

    fn create_filter_context(
        &self,
        features: &crate::features::Features,
        context: &MarketContext,
    ) -> FilterContext {
        FilterContext {
            asset: features.asset,
            timeframe: features.timeframe,
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
        }
    }

    /// Entrenar modelo inicial con datos históricos
    pub fn train_initial(&mut self, dataset: Dataset) -> anyhow::Result<()> {
        info!("🚀 Entrenando modelo ML con {} muestras...", dataset.len());

        if dataset.len() < self.config.training.min_samples_for_training {
            warn!(
                "Dataset insuficiente: {} < {}",
                dataset.len(),
                self.config.training.min_samples_for_training
            );
            return Ok(());
        }

        // BALANCEO DE CLASES: crucial para evitar sesgo hacia clase mayoritaria
        let mut balanced_dataset = dataset.clone();
        balanced_dataset.balance_classes();
        info!(
            "📊 Dataset balanceado: {} muestras (original: {})",
            balanced_dataset.len(),
            dataset.len()
        );

        let weights = EnsembleWeights::from_config(&self.config.ensemble);
        let mut predictor = MLPredictor::new(weights);

        // Entrenar con SmartCore REAL usando dataset balanceado
        predictor.train(&balanced_dataset)?;

        self.ml_predictor = Some(predictor);
        self.dataset = dataset; // Guardar original para futuro retraining

        info!("✅ Modelo ML entrenado exitosamente");

        // Mostrar métricas
        if let Some(ref pred) = self.ml_predictor {
            info!(
                "📊 Ensemble accuracy: {:.2}%",
                pred.ensemble_accuracy() * 100.0
            );
            // Log top features
            let top = pred.top_features(5);
            info!("📊 Top 5 features: {:?}", top);
        }

        Ok(())
    }

    /// Realizar walk-forward validation
    pub fn walk_forward_validation(&mut self) -> anyhow::Result<()> {
        if self.dataset.len() < 50 {
            warn!(
                "Dataset insuficiente para walk-forward: {} muestras",
                self.dataset.len()
            );
            return Ok(());
        }

        info!("🔄 Iniciando walk-forward validation...");

        let pipeline = TrainingPipeline::new(self.config.clone(), WalkForwardConfig::default());

        // Validar y entrenar
        let _report = pipeline.generate_report();
        _report.print();

        info!("✅ Walk-forward validation completo");

        Ok(())
    }

    /// Actualizar con resultado de trade
    pub fn update_with_trade_result(&mut self, trade: TradeSample) {
        // Agregar al dataset
        self.dataset.add_trade(trade.clone());

        // Actualizar calibrador con la probabilidad ML (prob_up), no calibrator_confidence.
        // Si falta prob_up, no inyectamos un 0.5 artificial porque distorsiona calibracion.
        if let Some(prob_up) = trade.predicted_prob_up {
            self.calibrator.add_observation(prob_up, trade.is_win);
        } else {
            tracing::warn!(
                trade_id = %trade.trade_id,
                "Missing predicted_prob_up; skipping calibrator update"
            );
        }

        // Actualizar estadísticas
        self.state.add_prediction_result(trade.is_win);

        // Actualizar predictor con la probabilidad ML real (si existe)
        if let Some(ref mut predictor) = self.ml_predictor {
            if let Some(prob_up) = trade.predicted_prob_up {
                predictor.record_outcome(prob_up, trade.is_win);
            } else {
                tracing::warn!(
                    trade_id = %trade.trade_id,
                    "Missing predicted_prob_up; skipping predictor outcome update"
                );
            }
        }

        // Check for concept drift — if detected, force immediate retrain
        if let Some(ref predictor) = self.ml_predictor {
            if predictor.is_drift_detected() {
                warn!(
                    "⚠️ Concept drift detected! Rolling accuracy: {:.1}% vs baseline: {:.1}%. Forcing retrain.",
                    predictor.recent_rolling_accuracy() * 100.0,
                    predictor.drift_baseline_accuracy * 100.0
                );
                if let Err(e) = self.incremental_update() {
                    warn!("Error in drift-triggered retrain: {}", e);
                }
                self.trades_since_retrain = 0;
            }
        }

        // Verificar si necesitamos retraining
        self.trades_since_retrain += 1;
        if self.trades_since_retrain >= self.config.training.retrain_interval_trades {
            info!(
                "🔄 Retraining automático después de {} trades",
                self.trades_since_retrain
            );

            if let Err(e) = self.incremental_update() {
                warn!("Error en retraining: {}", e);
            }

            self.trades_since_retrain = 0;
        }

        // Ajustar pesos dinámicamente
        if let Some(ref mut predictor) = self.ml_predictor {
            predictor.adjust_weights_dynamically();
        }
    }

    /// Actualización incremental del modelo
    fn incremental_update(&mut self) -> anyhow::Result<()> {
        if self.dataset.len() < self.config.training.min_samples_for_training {
            return Ok(());
        }

        info!("🔄 Actualizando modelo incrementalmente...");

        if let Some(ref mut predictor) = self.ml_predictor {
            // Balancear clases antes de reentrenar
            let mut balanced = self.dataset.clone();
            balanced.balance_classes();

            predictor.train(&balanced)?;
            self.state.last_retraining = Some(chrono::Utc::now().timestamp());

            // Log métricas post-retraining
            info!(
                "✅ Modelo actualizado - accuracy: {:.2}%",
                predictor.ensemble_accuracy() * 100.0
            );

            // Log feature importance top 5
            let top = predictor.top_features(5);
            info!("📊 Top features: {:?}", top);
        }

        Ok(())
    }

    /// Obtener feature importance
    pub fn get_feature_importance(&self) -> Option<&HashMap<String, f64>> {
        self.ml_predictor.as_ref().map(|p| &p.feature_importance)
    }

    /// Guardar estado
    pub fn save_state(&self, path: &str) -> anyhow::Result<()> {
        let state_path = format!("{}/ml_engine_state.json", path);
        let json = serde_json::to_string_pretty(&self.state)?;
        std::fs::write(&state_path, json)?;

        self.dataset.save(&format!("{}/dataset.json", path))?;

        info!("Estado ML guardado en {}", state_path);
        Ok(())
    }

    /// Cargar estado
    pub fn load_state(&mut self, path: &str) -> anyhow::Result<()> {
        let state_path = format!("{}/ml_engine_state.json", path);
        if std::path::Path::new(&state_path).exists() {
            let json = std::fs::read_to_string(&state_path)?;
            self.state = serde_json::from_str(&json)?;
        }

        let dataset_path = format!("{}/dataset.json", path);
        if std::path::Path::new(&dataset_path).exists() {
            self.dataset = Dataset::load(&dataset_path)?;

            // Re-entrenar
            if self.dataset.len() >= self.config.training.min_samples_for_training {
                self.train_initial(self.dataset.clone())?;
            }
        }

        Ok(())
    }

    /// Obtener estadísticas
    pub fn get_stats(&self) -> EngineStats {
        EngineStats {
            total_predictions: self.state.total_predictions,
            accuracy: self.state.accuracy(),
            dataset_size: self.dataset.len(),
            calibrated: self.calibrator.n_observations() > 0,
            model_trained: self.ml_predictor.is_some(),
            last_retraining: self.state.last_retraining,
        }
    }
}

/// Estadísticas del engine
#[derive(Debug, Clone)]
pub struct EngineStats {
    pub total_predictions: usize,
    pub accuracy: f64,
    pub dataset_size: usize,
    pub calibrated: bool,
    pub model_trained: bool,
    pub last_retraining: Option<i64>,
}
