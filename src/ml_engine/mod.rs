//! ML Engine - Sistema de Machine Learning para PolyBot v3.0
//!
//! Este módulo implementa:
//! - Feature engineering avanzado
//! - Ensemble de modelos (Random Forest, Gradient Boosting, Logistic Regression)
//! - Calibración de probabilidades
//! - Filtros inteligentes
//! - Sistema de aprendizaje continuo

pub mod calibration;
pub mod config_bridge;
pub mod data_client;
pub mod dataset;
pub mod features;
pub mod filters;
pub mod integration;
pub mod models;
pub mod persistence;
pub mod predictor;
pub mod training;

pub use config_bridge::MLConfigConvertible;

pub use calibration::{CalibrationCurve, ProbabilityCalibrator};
pub use dataset::{Dataset, LabeledSample, TradeSample};
pub use features::{FeatureEngine, MLFeatureVector};
pub use filters::{FilterConfig, FilterDecision, SmartFilterEngine};
pub use models::{EnsembleWeights, MLPredictor, ModelPrediction};
pub use persistence::{MLPersistenceManager, MLPersistenceState, ModelPerformance, TrainingRecord};
pub use predictor::MLStrategyPredictor;
pub use training::{TrainingPipeline, WalkForwardConfig};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuración global del ML Engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLEngineConfig {
    /// Habilitar ML
    pub enabled: bool,
    /// Tipo de modelo
    pub model_type: ModelType,
    /// Features a usar
    pub features: FeatureConfig,
    /// Configuración de ensemble
    pub ensemble: EnsembleConfig,
    /// Configuración de filtros
    pub filters: FilterConfig,
    /// Configuración de training
    pub training: TrainingConfig,
    /// Confianza mínima para señales
    pub min_confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelType {
    RandomForest,
    GradientBoosting,
    LogisticRegression,
    Ensemble,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    pub use_microstructure: bool,
    pub use_temporal_patterns: bool,
    pub use_cross_asset: bool,
    pub use_calibrator_features: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleConfig {
    pub random_forest_weight: f64,
    pub gradient_boosting_weight: f64,
    pub logistic_regression_weight: f64,
    pub dynamic_weight_adjustment: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    pub retrain_interval_trades: usize,
    pub min_samples_for_training: usize,
    pub walk_forward_train_days: i64,
    pub walk_forward_test_days: i64,
    pub early_stopping_patience: usize,
}

impl Default for MLEngineConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            model_type: ModelType::Ensemble,
            features: FeatureConfig {
                use_microstructure: true,
                use_temporal_patterns: true,
                use_cross_asset: true,
                use_calibrator_features: true,
            },
            ensemble: EnsembleConfig {
                random_forest_weight: 0.4,
                gradient_boosting_weight: 0.35,
                logistic_regression_weight: 0.25,
                dynamic_weight_adjustment: true,
            },
            // FILTERS: More permissive defaults for paper trading
            // These can be overridden in config/local.yaml
            filters: FilterConfig {
                max_spread_bps_15m: 200.0,      // Was 100 - too restrictive
                max_spread_bps_1h: 300.0,       // Was 150 - too restrictive
                min_depth_usdc: 1000.0,         // Was 5000 - too restrictive for some markets
                max_volatility_5m: 0.05,        // Was 0.02 - too restrictive
                min_volatility_5m: 0.0001,      // Was 0.001 - allow very low vol
                optimal_hours_only: false,      // Was true - don't restrict by hour
                min_btc_eth_correlation: 0.0,   // Was 0.6 - correlation not required
                max_btc_eth_correlation: 1.0,   // Was 0.95 - allow any correlation
                max_window_progress: 0.90,      // Was 0.70 - allow later entries
                min_time_to_close_minutes: 2.0, // Was 3.0 - slightly more permissive
                min_model_confidence: 0.52,     // Was 0.55 - lower threshold for exploration
            },
            training: TrainingConfig {
                retrain_interval_trades: 50,
                min_samples_for_training: 30,
                walk_forward_train_days: 30,
                walk_forward_test_days: 7,
                early_stopping_patience: 10,
            },
            min_confidence: 0.52, // Lower default for exploration phase
        }
    }
}

/// Estado del ML Engine para persistencia
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLEngineState {
    pub config: MLEngineConfig,
    pub model_weights: EnsembleWeights,
    pub calibration_curve: CalibrationCurve,
    pub feature_importance: HashMap<String, f64>,
    pub filter_stats: HashMap<String, FilterStats>,
    pub total_predictions: usize,
    pub correct_predictions: usize,
    pub incorrect_predictions: usize,
    pub last_retraining: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FilterStats {
    pub times_applied: usize,
    pub trades_allowed: usize,
    pub trades_rejected: usize,
    pub win_rate_allowed: f64,
    pub win_rate_rejected: f64,
}

impl MLEngineState {
    pub fn new(config: MLEngineConfig) -> Self {
        Self {
            config: config.clone(),
            model_weights: EnsembleWeights::from_config(&config.ensemble),
            calibration_curve: CalibrationCurve::default(),
            feature_importance: HashMap::new(),
            filter_stats: HashMap::new(),
            total_predictions: 0,
            correct_predictions: 0,
            incorrect_predictions: 0,
            last_retraining: None,
        }
    }

    pub fn accuracy(&self) -> f64 {
        if self.total_predictions == 0 {
            0.5
        } else {
            self.correct_predictions as f64 / self.total_predictions as f64
        }
    }

    /// Loss rate = incorrect / total
    pub fn loss_rate(&self) -> f64 {
        if self.total_predictions == 0 {
            0.0
        } else {
            self.incorrect_predictions as f64 / self.total_predictions as f64
        }
    }

    /// Win rate = correct / (correct + incorrect)
    pub fn win_rate(&self) -> f64 {
        let decided = self.correct_predictions + self.incorrect_predictions;
        if decided == 0 {
            0.0
        } else {
            self.correct_predictions as f64 / decided as f64
        }
    }

    /// Check if retraining is needed
    pub fn should_retrain(&self, interval: usize) -> bool {
        if let Some(last) = self.last_retraining {
            let trades_since = self.total_predictions.saturating_sub(last as usize);
            trades_since >= interval
        } else {
            true
        }
    }

    /// Add a prediction result (correct or incorrect)
    pub fn add_prediction_result(&mut self, was_correct: bool) {
        self.total_predictions += 1;
        if was_correct {
            self.correct_predictions += 1;
        } else {
            self.incorrect_predictions += 1;
        }
    }
}

/// Resultado de una predicción
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    /// Timestamp de la predicción
    pub timestamp: i64,
    /// Asset
    pub asset: crate::types::Asset,
    /// Timeframe
    pub timeframe: crate::types::Timeframe,
    /// Probabilidad de que suba (0.0 - 1.0)
    pub prob_up: f64,
    /// Confianza en la predicción (0.0 - 1.0)
    pub confidence: f64,
    /// Dirección predicha
    pub direction: crate::types::Direction,
    /// Edge calculado
    pub edge: f64,
    /// Features usadas
    pub features_used: Vec<String>,
    /// Modelos que contribuyeron
    pub model_contributions: HashMap<String, f64>,
    /// Peso del ensemble
    pub ensemble_weight: f64,
}

impl Prediction {
    /// Verificar si la predicción fue correcta
    pub fn is_correct(&self, actual_outcome: bool) -> bool {
        let predicted_up = self.prob_up > 0.5;
        predicted_up == actual_outcome
    }
}
