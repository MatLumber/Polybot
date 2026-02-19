//! Strategy Trait - Trait unificado para V2 y V3 strategies

use crate::features::Features;
use crate::strategy::GeneratedSignal;
use crate::types::{Asset, Timeframe};
use std::collections::HashMap;

/// Trait unificado que implementan tanto StrategyEngine (V2) como V3Strategy
pub trait Strategy: Send + Sync {
    /// Procesar features y generar señal
    fn process(&mut self, features: &Features) -> Option<GeneratedSignal>;

    /// Obtener último motivo de filtrado
    fn last_filter_reason(&self) -> Option<String>;

    /// Exportar estado del calibrador (formato V2)
    fn export_calibrator_state_v2(&self) -> HashMap<String, Vec<crate::strategy::IndicatorStats>>;

    /// Importar estado del calibrador (formato V2)
    fn import_calibrator_state_v2(
        &mut self,
        stats: HashMap<String, Vec<crate::strategy::IndicatorStats>>,
    );

    /// Importar estado del calibrador (formato legacy V1) - para compatibilidad
    fn import_calibrator_state(&mut self, stats: Vec<crate::strategy::IndicatorStats>);

    /// Verificar si está calibrado
    fn is_calibrated(&self) -> bool;

    /// Total de trades en calibrador
    fn calibrator_total_trades(&self) -> usize;

    /// Registrar trade con indicadores para calibración
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

    /// Exportar calidad de calibración por mercado (V2 feature, V3 devuelve vacío)
    fn export_calibration_quality_by_market(
        &self,
    ) -> HashMap<String, crate::strategy::CalibrationQualitySnapshot>;

    /// Obtener estadísticas de indicadores (V2 feature, V3 devuelve vacío)
    fn get_indicator_stats(&self) -> Vec<crate::strategy::IndicatorStats>;
}

// Implementación para StrategyEngine (V2)
impl Strategy for crate::strategy::StrategyEngine {
    fn process(&mut self, features: &Features) -> Option<GeneratedSignal> {
        self.process(features)
    }

    fn last_filter_reason(&self) -> Option<String> {
        self.last_filter_reason()
    }

    fn export_calibrator_state_v2(&self) -> HashMap<String, Vec<crate::strategy::IndicatorStats>> {
        self.export_calibrator_state_v2()
    }

    fn import_calibrator_state_v2(
        &mut self,
        stats: HashMap<String, Vec<crate::strategy::IndicatorStats>>,
    ) {
        self.import_calibrator_state_v2(stats)
    }

    fn import_calibrator_state(&mut self, stats: Vec<crate::strategy::IndicatorStats>) {
        self.import_calibrator_state(stats)
    }

    fn is_calibrated(&self) -> bool {
        self.is_calibrated()
    }

    fn calibrator_total_trades(&self) -> usize {
        self.calibrator_total_trades()
    }

    fn record_trade_with_indicators_for_market(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        indicators: &[String],
        result: crate::strategy::TradeResult,
    ) {
        self.record_trade_with_indicators_for_market(asset, timeframe, indicators, result)
    }

    fn record_prediction_outcome_for_market(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        p_pred: f64,
        is_win: bool,
    ) {
        self.record_prediction_outcome_for_market(asset, timeframe, p_pred, is_win)
    }

    fn export_calibration_quality_by_market(
        &self,
    ) -> HashMap<String, crate::strategy::CalibrationQualitySnapshot> {
        self.export_calibration_quality_by_market()
    }

    fn get_indicator_stats(&self) -> Vec<crate::strategy::IndicatorStats> {
        self.get_indicator_stats()
    }
}

// Implementación para V3Strategy
impl Strategy for crate::strategy::V3Strategy {
    fn process(&mut self, features: &Features) -> Option<GeneratedSignal> {
        self.process(features)
    }

    fn last_filter_reason(&self) -> Option<String> {
        self.last_filter_reason()
    }

    fn export_calibrator_state_v2(&self) -> HashMap<String, Vec<crate::strategy::IndicatorStats>> {
        self.export_calibrator_state_v2()
    }

    fn import_calibrator_state_v2(
        &mut self,
        stats: HashMap<String, Vec<crate::strategy::IndicatorStats>>,
    ) {
        self.import_calibrator_state_v2(stats)
    }

    fn import_calibrator_state(&mut self, stats: Vec<crate::strategy::IndicatorStats>) {
        // V3 no usa formato legacy, lo convertimos a formato V2
        let mut map = HashMap::new();
        for stat in stats {
            // Usar un key genérico ya que V3 maneja el calibrador globalmente
            map.insert("global".to_string(), vec![stat]);
        }
        self.import_calibrator_state_v2(map)
    }

    fn is_calibrated(&self) -> bool {
        self.is_calibrated()
    }

    fn calibrator_total_trades(&self) -> usize {
        self.calibrator_total_trades()
    }

    fn record_trade_with_indicators_for_market(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        indicators: &[String],
        result: crate::strategy::TradeResult,
    ) {
        let is_win = matches!(result, crate::strategy::TradeResult::Win);
        // V3 usa confidence 0.6 y edge 0.05 como defaults
        self.record_trade_result(asset, timeframe, indicators, is_win, 0.6, 0.05);
    }

    fn record_prediction_outcome_for_market(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        p_pred: f64,
        is_win: bool,
    ) {
        // V3 maneja esto internamente, no necesita implementación externa
        let _ = (asset, timeframe, p_pred, is_win);
    }

    fn export_calibration_quality_by_market(
        &self,
    ) -> HashMap<String, crate::strategy::CalibrationQualitySnapshot> {
        // V3 usa ML, no calibración tradicional - devuelve vacío
        HashMap::new()
    }

    fn get_indicator_stats(&self) -> Vec<crate::strategy::IndicatorStats> {
        // V3 usa ML ensemble, no indicadores individuales - devuelve vacío
        Vec::new()
    }
}
