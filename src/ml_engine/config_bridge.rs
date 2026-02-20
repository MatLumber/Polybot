//! ML Engine Configuration Bridge
//!
//! Convierte MLConfig (del sistema de config) a MLEngineConfig (del motor ML).
//! Esto permite cargar configuración desde archivos YAML/env vars.

use crate::config::MLConfig;
use crate::ml_engine::{
    EnsembleConfig, FeatureConfig, FilterConfig, MLEngineConfig, ModelType, TrainingConfig,
};

/// Trait para convertir configuración del sistema de config a MLEngineConfig
pub trait MLConfigConvertible {
    fn to_ml_engine_config(&self) -> MLEngineConfig;
}

impl MLConfigConvertible for MLConfig {
    fn to_ml_engine_config(&self) -> MLEngineConfig {
        MLEngineConfig {
            enabled: self.enabled,
            model_type: parse_model_type(&self.model_type),
            features: FeatureConfig {
                use_microstructure: self.use_microstructure,
                use_temporal_patterns: self.use_temporal_patterns,
                use_cross_asset: self.use_cross_asset,
                use_calibrator_features: true, // Default
            },
            ensemble: EnsembleConfig {
                random_forest_weight: self.random_forest_weight,
                gradient_boosting_weight: self.gradient_boosting_weight,
                logistic_regression_weight: self.logistic_regression_weight,
                dynamic_weight_adjustment: self.dynamic_weight_adjustment,
            },
            filters: FilterConfig {
                max_spread_bps_15m: self.max_spread_bps_15m,
                max_spread_bps_1h: self.max_spread_bps_1h,
                min_depth_usdc: self.min_depth_usdc,
                max_volatility_5m: self.max_volatility_5m,
                min_volatility_5m: 0.001, // Default
                optimal_hours_only: self.optimal_hours_only,
                min_btc_eth_correlation: 0.0, // Default - más permisivo
                max_btc_eth_correlation: 1.0, // Default - más permisivo
                max_window_progress: 0.90,    // Default - más permisivo
                min_time_to_close_minutes: 2.0, // Default
                min_model_confidence: 0.52,   // Default - más permisivo para empezar
            },
            training: TrainingConfig {
                retrain_interval_trades: self.retrain_interval_trades,
                min_samples_for_training: self.min_samples_for_training,
                walk_forward_train_days: 30, // Default
                walk_forward_test_days: 7,   // Default
                early_stopping_patience: 10, // Default
            },
            min_confidence: 0.52, // Default más permisivo
        }
    }
}

impl MLConfigConvertible for Option<MLConfig> {
    fn to_ml_engine_config(&self) -> MLEngineConfig {
        match self {
            Some(config) => config.to_ml_engine_config(),
            None => {
                tracing::warn!("MLConfig not found in configuration, using defaults");
                MLEngineConfig::default()
            }
        }
    }
}

fn parse_model_type(s: &str) -> ModelType {
    match s.to_lowercase().as_str() {
        "random_forest" | "randomforest" => ModelType::RandomForest,
        "gradient_boosting" | "gradientboosting" | "gb" => ModelType::GradientBoosting,
        "logistic_regression" | "logisticregression" | "lr" => ModelType::LogisticRegression,
        "ensemble" => ModelType::Ensemble,
        _ => {
            tracing::warn!("Unknown model_type '{}', using Ensemble", s);
            ModelType::Ensemble
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ml_config_conversion() {
        let ml_config = MLConfig {
            enabled: true,
            model_type: "ensemble".to_string(),
            retrain_interval_trades: 50,
            min_samples_for_training: 30,
            use_microstructure: true,
            use_temporal_patterns: true,
            use_cross_asset: true,
            random_forest_weight: 0.4,
            gradient_boosting_weight: 0.35,
            logistic_regression_weight: 0.25,
            dynamic_weight_adjustment: true,
            max_spread_bps_15m: 150.0,
            max_spread_bps_1h: 200.0,
            min_depth_usdc: 0.0,
            max_volatility_5m: 0.03,
            optimal_hours_only: false,
        };

        let engine_config = ml_config.to_ml_engine_config();

        assert!(engine_config.enabled);
        assert!(matches!(engine_config.model_type, ModelType::Ensemble));
        assert_eq!(engine_config.min_confidence, 0.52);
    }

    #[test]
    fn test_parse_model_types() {
        assert!(matches!(parse_model_type("ensemble"), ModelType::Ensemble));
        assert!(matches!(
            parse_model_type("random_forest"),
            ModelType::RandomForest
        ));
        assert!(matches!(
            parse_model_type("gb"),
            ModelType::GradientBoosting
        ));
        assert!(matches!(
            parse_model_type("LR"),
            ModelType::LogisticRegression
        ));
    }
}
