//! ML Integration - Integración de V3 Strategy (simplificada)

use crate::config::AppConfig;
use crate::strategy::v3_strategy::V3Strategy;
use crate::strategy::{StrategyConfig, StrategyEngine, TradeResult};
use crate::types::{Asset, Timeframe};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Tipo de estrategia activa
pub enum ActiveStrategy {
    V2(StrategyEngine),
    V3(V3Strategy),
}

impl ActiveStrategy {
    pub fn process(
        &mut self,
        features: &crate::features::Features,
    ) -> Option<crate::strategy::GeneratedSignal> {
        match self {
            ActiveStrategy::V2(engine) => engine.process(features),
            ActiveStrategy::V3(engine) => engine.process(features),
        }
    }

    pub fn last_filter_reason(&self) -> Option<String> {
        match self {
            ActiveStrategy::V2(engine) => engine.last_filter_reason(),
            ActiveStrategy::V3(engine) => engine.last_filter_reason(),
        }
    }

    pub fn export_calibrator_state_v2(
        &self,
    ) -> HashMap<String, Vec<crate::strategy::IndicatorStats>> {
        match self {
            ActiveStrategy::V2(engine) => engine.export_calibrator_state_v2(),
            ActiveStrategy::V3(engine) => engine.export_calibrator_state_v2(),
        }
    }

    pub fn import_calibrator_state_v2(
        &mut self,
        stats: HashMap<String, Vec<crate::strategy::IndicatorStats>>,
    ) {
        match self {
            ActiveStrategy::V2(engine) => engine.import_calibrator_state_v2(stats),
            ActiveStrategy::V3(engine) => engine.import_calibrator_state_v2(stats),
        }
    }

    pub fn record_trade_with_indicators_for_market(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        indicators: &[String],
        result: TradeResult,
    ) {
        match self {
            ActiveStrategy::V2(engine) => {
                engine
                    .record_trade_with_indicators_for_market(asset, timeframe, indicators, result);
            }
            ActiveStrategy::V3(engine) => {
                let is_win = matches!(result, TradeResult::Win);
                engine.record_trade_result(asset, timeframe, indicators, is_win, 0.6, 0.05);
            }
        }
    }

    pub fn record_prediction_outcome_for_market(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        p_pred: f64,
        is_win: bool,
    ) {
        match self {
            ActiveStrategy::V2(engine) => {
                engine.record_prediction_outcome_for_market(asset, timeframe, p_pred, is_win);
            }
            ActiveStrategy::V3(_engine) => {
                // V3 maneja esto internamente
            }
        }
    }

    pub fn calibrator_total_trades(&self) -> usize {
        match self {
            ActiveStrategy::V2(engine) => engine.calibrator_total_trades(),
            ActiveStrategy::V3(engine) => engine.calibrator_total_trades(),
        }
    }

    pub fn is_calibrated(&self) -> bool {
        match self {
            ActiveStrategy::V2(engine) => engine.is_calibrated(),
            ActiveStrategy::V3(engine) => engine.is_calibrated(),
        }
    }
}

/// Inicializar estrategía según configuración
pub async fn initialize_strategy(config: &AppConfig) -> anyhow::Result<Arc<Mutex<ActiveStrategy>>> {
    if config.use_v3_strategy {
        info!("🤖 Initializing V3 ML Strategy (simplified)...");

        let mut strategy_config = StrategyConfig::default();
        strategy_config.min_confidence = config.strategy.min_confidence;

        // Crear config mínima para V3
        let ml_config = crate::ml_engine::MLEngineConfig::default();

        let v3_strategy = V3Strategy::new(ml_config, strategy_config);

        info!("✅ V3 Strategy initialized");

        Ok(Arc::new(Mutex::new(ActiveStrategy::V3(v3_strategy))))
    } else {
        info!("🤖 Initializing V2 Strategy...");

        let mut strategy_config = StrategyConfig::default();
        strategy_config.min_confidence = config.strategy.min_confidence.clamp(0.01, 0.99);

        let engine = StrategyEngine::with_calibration_min_samples(
            strategy_config,
            config.strategy.calibration_min_samples_per_market,
        );

        Ok(Arc::new(Mutex::new(ActiveStrategy::V2(engine))))
    }
}
