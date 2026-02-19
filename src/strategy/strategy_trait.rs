//! Strategy Trait - Interface común para estrategias V2 y V3

use crate::features::Features;
use crate::strategy::{GeneratedSignal, IndicatorStats};
use crate::types::{Asset, Timeframe};
use std::collections::HashMap;

/// Trait común para todas las estrategias
pub trait SignalStrategy: Send + Sync {
    /// Procesar features y generar señal
    fn process(&mut self, features: &Features) -> Option<GeneratedSignal>;

    /// Obtener última razón de filtro
    fn last_filter_reason(&self) -> Option<String>;

    /// Exportar estado de calibración
    fn export_calibrator_state_v2(&self) -> HashMap<String, Vec<IndicatorStats>>;

    /// Importar estado de calibración
    fn import_calibrator_state_v2(&mut self, stats: HashMap<String, Vec<IndicatorStats>>);

    /// Registrar resultado de trade
    fn record_trade_with_indicators_for_market(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        indicators: &[String],
        result: crate::strategy::TradeResult,
    );

    /// Registrar outcome de predicción
    fn record_prediction_outcome_for_market(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        p_pred: f64,
        is_win: bool,
    );

    /// Total de trades en calibrador
    fn calibrator_total_trades(&self) -> usize;

    /// Verificar si está calibrado
    fn is_calibrated(&self) -> bool;
}
