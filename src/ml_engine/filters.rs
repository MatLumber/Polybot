//! Smart Filters - Filtros inteligentes de entrada

use crate::ml_engine::features::MarketContext;
use crate::ml_engine::MLEngineState;
use crate::types::{Asset, Timeframe};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Decisión del filtro
#[derive(Debug, Clone, PartialEq)]
pub enum FilterDecision {
    Allow,
    Reject(FilterReason),
}

/// Razón de rechazo
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FilterReason {
    InsufficientLiquidity,
    ExcessiveSpread,
    HighVolatility,
    LowVolatility,
    SuboptimalHour,
    UnstableCorrelation,
    LateEntry,
    InsufficientTime,
    MacroEvent,
    MarketClosed,
    Custom(String),
}

impl std::fmt::Display for FilterReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterReason::InsufficientLiquidity => write!(f, "insufficient_liquidity"),
            FilterReason::ExcessiveSpread => write!(f, "excessive_spread"),
            FilterReason::HighVolatility => write!(f, "high_volatility"),
            FilterReason::LowVolatility => write!(f, "low_volatility"),
            FilterReason::SuboptimalHour => write!(f, "suboptimal_hour"),
            FilterReason::UnstableCorrelation => write!(f, "unstable_correlation"),
            FilterReason::LateEntry => write!(f, "late_entry"),
            FilterReason::InsufficientTime => write!(f, "insufficient_time"),
            FilterReason::MacroEvent => write!(f, "macro_event"),
            FilterReason::MarketClosed => write!(f, "market_closed"),
            FilterReason::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// Contexto de mercado para evaluación de filtros
#[derive(Debug, Clone, Default)]
pub struct FilterContext {
    pub asset: Asset,
    pub timeframe: Timeframe,
    pub timestamp: i64,
    pub spread_bps: f64,
    pub depth_usdc: f64,
    pub orderbook_depth: f64,
    pub volatility_5m: f64,
    pub hour: u8,
    pub day_of_week: u8,
    pub minutes_to_close: f64,
    pub window_progress: f64,
    pub btc_eth_correlation: f64,
    pub is_macro_event_near: bool,
    pub model_confidence: f64,
}

/// Motor de filtros inteligentes
pub struct SmartFilterEngine {
    /// Configuración
    pub config: FilterConfig,
    /// Estadísticas por filtro
    pub stats: HashMap<String, FilterPerformanceStats>,
    /// Horas óptimas de trading (aprendidas)
    pub optimal_hours: Vec<u8>,
    /// Thresholds adaptativos
    pub adaptive_thresholds: AdaptiveThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    pub max_spread_bps_15m: f64,
    pub max_spread_bps_1h: f64,
    pub min_depth_usdc: f64,
    pub max_volatility_5m: f64,
    pub min_volatility_5m: f64,
    pub optimal_hours_only: bool,
    pub min_btc_eth_correlation: f64,
    pub max_btc_eth_correlation: f64,
    pub max_window_progress: f64,
    pub min_time_to_close_minutes: f64,
    pub min_model_confidence: f64,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            max_spread_bps_15m: 100.0,
            max_spread_bps_1h: 150.0,
            min_depth_usdc: 0.0,
            max_volatility_5m: 0.02,
            min_volatility_5m: 0.001,
            optimal_hours_only: false,
            min_btc_eth_correlation: 0.6,
            max_btc_eth_correlation: 0.95,
            max_window_progress: 0.70,
            min_time_to_close_minutes: 3.0,
            min_model_confidence: 0.55,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FilterPerformanceStats {
    pub times_applied: usize,
    pub allowed: usize,
    pub rejected: usize,
    pub wins_allowed: usize,
    pub losses_allowed: usize,
    pub wins_rejected: usize,
    pub losses_rejected: usize,
}

impl FilterPerformanceStats {
    pub fn win_rate_allowed(&self) -> f64 {
        let total = self.wins_allowed + self.losses_allowed;
        if total == 0 {
            0.5
        } else {
            self.wins_allowed as f64 / total as f64
        }
    }

    pub fn win_rate_rejected(&self) -> f64 {
        let total = self.wins_rejected + self.losses_rejected;
        if total == 0 {
            0.5
        } else {
            self.wins_rejected as f64 / total as f64
        }
    }

    /// ¿Mejora el filtro el win rate?
    pub fn is_effective(&self) -> bool {
        self.win_rate_allowed() > self.win_rate_rejected()
    }
}

/// Thresholds adaptativos que se ajustan según performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveThresholds {
    pub spread_multiplier: f64,
    pub volatility_multiplier: f64,
    pub correlation_range_expansion: f64,
}

impl Default for AdaptiveThresholds {
    fn default() -> Self {
        Self {
            spread_multiplier: 1.0,
            volatility_multiplier: 1.0,
            correlation_range_expansion: 0.0,
        }
    }
}

impl SmartFilterEngine {
    pub fn new(config: FilterConfig) -> Self {
        Self {
            config,
            stats: HashMap::new(),
            optimal_hours: vec![], // Se aprende dinámicamente
            adaptive_thresholds: AdaptiveThresholds::default(),
        }
    }

    /// Evaluar todos los filtros
    pub fn evaluate(&self, context: &FilterContext) -> FilterDecision {
        // Filtro 1: Liquidez
        if let Some(decision) = self.check_liquidity(context) {
            return decision;
        }

        // Filtro 2: Spread
        if let Some(decision) = self.check_spread(context) {
            return decision;
        }

        // Filtro 3: Volatilidad
        if let Some(decision) = self.check_volatility(context) {
            return decision;
        }

        // Filtro 4: Tiempo
        if let Some(decision) = self.check_timing(context) {
            return decision;
        }

        // Filtro 5: Correlación
        if let Some(decision) = self.check_correlation(context) {
            return decision;
        }

        // Filtro 6: Eventos macro
        if let Some(decision) = self.check_macro_events(context) {
            return decision;
        }

        FilterDecision::Allow
    }

    /// Check 1: Suficiente liquidez
    fn check_liquidity(&self, context: &FilterContext) -> Option<FilterDecision> {
        if context.orderbook_depth < self.config.min_depth_usdc {
            return Some(FilterDecision::Reject(FilterReason::InsufficientLiquidity));
        }
        None
    }

    /// Check 2: Spread razonable
    fn check_spread(&self, context: &FilterContext) -> Option<FilterDecision> {
        let max_spread = match context.timeframe {
            Timeframe::Min15 => self.config.max_spread_bps_15m,
            Timeframe::Hour1 => self.config.max_spread_bps_1h,
        } * self.adaptive_thresholds.spread_multiplier;

        if context.spread_bps > max_spread {
            return Some(FilterDecision::Reject(FilterReason::ExcessiveSpread));
        }
        None
    }

    /// Check 3: Volatilidad en rango óptimo
    fn check_volatility(&self, context: &FilterContext) -> Option<FilterDecision> {
        let vol = context.volatility_5m;
        let max_vol =
            self.config.max_volatility_5m * self.adaptive_thresholds.volatility_multiplier;
        let min_vol = self.config.min_volatility_5m;

        if vol > max_vol {
            return Some(FilterDecision::Reject(FilterReason::HighVolatility));
        }
        if vol < min_vol {
            return Some(FilterDecision::Reject(FilterReason::LowVolatility));
        }
        None
    }

    /// Check 4: Timing óptimo
    fn check_timing(&self, context: &FilterContext) -> Option<FilterDecision> {
        // Suficiente tiempo hasta cierre
        if context.minutes_to_close < self.config.min_time_to_close_minutes {
            return Some(FilterDecision::Reject(FilterReason::InsufficientTime));
        }

        // No muy tarde en la ventana
        if context.window_progress > self.config.max_window_progress {
            return Some(FilterDecision::Reject(FilterReason::LateEntry));
        }

        // Horas óptimas (si está habilitado)
        if self.config.optimal_hours_only && !self.optimal_hours.is_empty() {
            if !self.optimal_hours.contains(&context.hour) {
                return Some(FilterDecision::Reject(FilterReason::SuboptimalHour));
            }
        }

        None
    }

    /// Check 5: Correlación estable
    fn check_correlation(&self, context: &FilterContext) -> Option<FilterDecision> {
        let corr = context.btc_eth_correlation;
        let expansion = self.adaptive_thresholds.correlation_range_expansion;

        let min_corr = (self.config.min_btc_eth_correlation - expansion).max(0.0);
        let max_corr = (self.config.max_btc_eth_correlation + expansion).min(1.0);

        if corr < min_corr || corr > max_corr {
            return Some(FilterDecision::Reject(FilterReason::UnstableCorrelation));
        }
        None
    }

    /// Check 6: Eventos macro
    fn check_macro_events(&self, context: &FilterContext) -> Option<FilterDecision> {
        if context.is_macro_event_near {
            return Some(FilterDecision::Reject(FilterReason::MacroEvent));
        }
        None
    }

    /// Actualizar estadísticas después de un trade
    pub fn update_stats(&mut self, filter_name: &str, applied: bool, was_win: bool, allowed: bool) {
        let stats = self.stats.entry(filter_name.to_string()).or_default();

        if applied {
            stats.times_applied += 1;
            if allowed {
                stats.allowed += 1;
                if was_win {
                    stats.wins_allowed += 1;
                } else {
                    stats.losses_allowed += 1;
                }
            } else {
                stats.rejected += 1;
                if was_win {
                    stats.wins_rejected += 1;
                } else {
                    stats.losses_rejected += 1;
                }
            }
        }
    }

    /// Auto-optimizar thresholds basado en performance
    pub fn auto_optimize(&mut self) {
        for (name, stats) in &self.stats {
            if stats.times_applied < 20 {
                continue; // Necesitamos más datos
            }

            if !stats.is_effective() {
                // El filtro está rechazando trades ganadores
                // Relajar thresholds
                match name.as_str() {
                    "spread" => {
                        self.adaptive_thresholds.spread_multiplier *= 1.1;
                    }
                    "volatility" => {
                        self.adaptive_thresholds.volatility_multiplier *= 1.1;
                    }
                    "correlation" => {
                        self.adaptive_thresholds.correlation_range_expansion += 0.05;
                    }
                    _ => {}
                }
            }
        }
    }

    /// Aprender horas óptimas
    pub fn learn_optimal_hours(&mut self, trades: &[(u8, bool)]) {
        // Agrupar por hora
        let mut hour_stats: HashMap<u8, (usize, usize)> = HashMap::new(); // (wins, total)

        for (hour, is_win) in trades {
            let entry = hour_stats.entry(*hour).or_insert((0, 0));
            entry.1 += 1;
            if *is_win {
                entry.0 += 1;
            }
        }

        // Encontrar horas con win rate > 55%
        self.optimal_hours = hour_stats
            .iter()
            .filter(|(_, (wins, total))| *total >= 5 && (*wins as f64 / *total as f64) > 0.55)
            .map(|(hour, _)| *hour)
            .collect();
    }

    /// Guardar estado
    pub fn save(&self, path: &str) -> anyhow::Result<()> {
        let state = FilterEngineState {
            config: self.config.clone(),
            stats: self.stats.clone(),
            optimal_hours: self.optimal_hours.clone(),
            adaptive_thresholds: self.adaptive_thresholds.clone(),
        };
        let json = serde_json::to_string_pretty(&state)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Cargar estado
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let state: FilterEngineState = serde_json::from_str(&json)?;
        Ok(Self {
            config: state.config,
            stats: state.stats,
            optimal_hours: state.optimal_hours,
            adaptive_thresholds: state.adaptive_thresholds,
        })
    }
}

/// Estado para persistencia
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FilterEngineState {
    config: FilterConfig,
    stats: HashMap<String, FilterPerformanceStats>,
    optimal_hours: Vec<u8>,
    adaptive_thresholds: AdaptiveThresholds,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_liquidity_filter() {
        let config = FilterConfig::default();
        let engine = SmartFilterEngine::new(config);

        let mut context = FilterContext::default();
        context.orderbook_depth = 1000.0; // Muy bajo

        let decision = engine.evaluate(&context);
        assert!(matches!(
            decision,
            FilterDecision::Reject(FilterReason::InsufficientLiquidity)
        ));
    }

    #[test]
    fn test_spread_filter() {
        let config = FilterConfig::default();
        let engine = SmartFilterEngine::new(config);

        let mut context = FilterContext::default();
        context.timeframe = Timeframe::Min15;
        context.spread_bps = 150.0; // Muy alto
        context.orderbook_depth = 10000.0;

        let decision = engine.evaluate(&context);
        assert!(matches!(
            decision,
            FilterDecision::Reject(FilterReason::ExcessiveSpread)
        ));
    }

    #[test]
    fn test_optimal_hours_learning() {
        let config = FilterConfig::default();
        let mut engine = SmartFilterEngine::new(config);

        // Simular trades: hora 14 tiene buen performance
        let trades = vec![
            (14, true),
            (14, true),
            (14, false),
            (14, true),
            (14, true), // 80% win
            (15, false),
            (15, false),
            (15, true),
            (15, false), // 25% win
        ];

        engine.learn_optimal_hours(&trades);

        assert!(engine.optimal_hours.contains(&14));
        assert!(!engine.optimal_hours.contains(&15));
    }
}
