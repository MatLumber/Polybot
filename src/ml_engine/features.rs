//! Feature Engineering - Vector de features para ML
//!
//! Transforma datos de mercado en features numéricos para los modelos ML

use crate::features::{Features, MarketRegime};
use crate::types::{Asset, Direction, Timeframe};
use serde::{Deserialize, Serialize};

/// Vector completo de features para ML
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MLFeatureVector {
    // ============ Técnicos Básicos ============
    /// RSI (0-100)
    pub rsi: f64,
    /// RSI normalizado (-1 a 1)
    pub rsi_normalized: f64,
    /// Divergencia RSI (1 = bullish div, -1 = bearish div, 0 = none)
    pub rsi_divergence: f64,

    /// MACD line
    pub macd: f64,
    /// MACD signal
    pub macd_signal: f64,
    /// Histograma MACD
    pub macd_histogram: f64,
    /// Pendiente del histograma (momentum del momentum)
    pub macd_hist_slope: f64,

    /// Posición en Bollinger Bands (0 = lower, 0.5 = middle, 1 = upper)
    pub bb_position: f64,
    /// Ancho de BB como % del precio
    pub bb_width_pct: f64,
    /// Squeeze intensity (1 = muy comprimido)
    pub bb_squeeze: f64,

    /// ADX (0-100)
    pub adx: f64,
    /// +DI
    pub plus_di: f64,
    /// -DI
    pub minus_di: f64,
    /// Fuerza de tendencia (adx * (plus_di - minus_di))
    pub trend_strength: f64,

    // ============ Momentum Avanzado ============
    /// Velocidad de precio (% cambio por minuto)
    pub price_velocity: f64,
    /// Aceleración (cambio en la velocidad)
    pub price_acceleration: f64,
    /// Momentum de 2do orden
    pub momentum_2nd_order: f64,

    /// Distancia al VWAP (%)
    pub vwap_distance_pct: f64,

    /// StochRSI (0-1)
    pub stoch_rsi: f64,
    /// StochRSI sobrecompra (>0.8)
    pub stoch_rsi_overbought: f64,
    /// StochRSI sobrevendido (<0.2)
    pub stoch_rsi_oversold: f64,

    // ============ Microestructura ============
    /// Spread en basis points
    pub spread_bps: f64,
    /// Spread como percentil histórico (0-1)
    pub spread_percentile: f64,

    /// Imbalance de orderbook (bids - asks) / total
    pub orderbook_imbalance: f64,
    /// Profundidad top 5 en USDC
    pub depth_top5: f64,
    /// Concentración de liquidez (0 = dispersa, 1 = concentrada)
    pub liquidity_concentration: f64,

    /// Intensidad de trades (trades por minuto)
    pub trade_intensity: f64,
    /// Z-score de intensidad vs histórico
    pub trade_intensity_zscore: f64,

    /// Order flow imbalance (buy_vol - sell_vol) / total_vol
    pub order_flow_imbalance: f64,

    // ============ Temporales ============
    /// Minutos hasta cierre
    pub minutes_to_close: f64,
    /// Progreso de la ventana (0.0 - 1.0)
    pub window_progress: f64,
    /// Hora del día (0-23)
    pub hour_of_day: f64,
    /// Encoding cíclico hora (sin)
    pub hour_sin: f64,
    /// Encoding cíclico hora (cos)
    pub hour_cos: f64,
    /// Día de semana (0-6)
    pub day_of_week: f64,
    /// Es fin de semana
    pub is_weekend: f64,
    /// Minutos desde apertura de mercado tradicional (9:30 NY)
    pub minutes_since_market_open: f64,

    // ============ Contexto de Mercado ============
    /// Regimen de mercado (0=ranging, 1=trending, 2=volatile)
    pub market_regime: f64,
    /// Volatilidad actual (ATR %)
    pub volatility_5m: f64,
    /// Volatilidad en percentil histórico
    pub volatility_percentile: f64,

    /// Correlación BTC-ETH (últimos 15m)
    pub btc_eth_correlation: f64,
    /// Cambio de correlación reciente
    pub correlation_change: f64,

    /// Sentimiento de mercado basado en order flow
    pub market_sentiment: f64,

    // ============ Features del Calibrador Actual ============
    /// Confianza del sistema de calibración actual
    pub calibrator_confidence: f64,
    /// Número de indicadores que coinciden
    pub num_indicators_agreeing: f64,
    /// Win rate promedio de indicadores activos
    pub indicators_avg_win_rate: f64,
    /// Peso total de indicadores bullish
    pub bullish_weight: f64,
    /// Peso total de indicadores bearish
    pub bearish_weight: f64,

    // ============ Meta-features ============
    /// Asset (one-hot encoded: BTC=1, ETH=0)
    pub is_btc: f64,
    /// Timeframe (one-hot: 15m=1, 1h=0)
    pub is_15m: f64,

    // ============ Polymarket ============
    /// Polymarket implied probability
    pub polymarket_price: f64,
    /// Probability momentum
    pub polymarket_price_momentum: f64,
    /// Polymarket 24h volume
    pub polymarket_volume_24hr: f64,
    /// Polymarket total liquidity
    pub polymarket_liquidity: f64,
}

impl MLFeatureVector {
    /// Convertir a vector f64 para los modelos ML
    pub fn to_vec(&self) -> Vec<f64> {
        vec![
            self.rsi,
            self.rsi_normalized,
            self.rsi_divergence,
            self.macd,
            self.macd_signal,
            self.macd_histogram,
            self.macd_hist_slope,
            self.bb_position,
            self.bb_width_pct,
            self.bb_squeeze,
            self.adx,
            self.plus_di,
            self.minus_di,
            self.trend_strength,
            self.price_velocity,
            self.price_acceleration,
            self.momentum_2nd_order,
            self.vwap_distance_pct,
            self.stoch_rsi,
            self.stoch_rsi_overbought,
            self.stoch_rsi_oversold,
            self.spread_bps,
            self.spread_percentile,
            self.orderbook_imbalance,
            self.depth_top5,
            self.liquidity_concentration,
            self.trade_intensity,
            self.trade_intensity_zscore,
            self.order_flow_imbalance,
            self.minutes_to_close,
            self.window_progress,
            self.hour_of_day,
            self.hour_sin,
            self.hour_cos,
            self.day_of_week,
            self.is_weekend,
            self.minutes_since_market_open,
            self.market_regime,
            self.volatility_5m,
            self.volatility_percentile,
            self.btc_eth_correlation,
            self.correlation_change,
            self.market_sentiment,
            self.calibrator_confidence,
            self.num_indicators_agreeing,
            self.indicators_avg_win_rate,
            self.bullish_weight,
            self.bearish_weight,
            self.is_btc,
            self.is_15m,
            self.polymarket_price,
            self.polymarket_price_momentum,
            self.polymarket_volume_24hr,
            self.polymarket_liquidity,
        ]
    }

    /// Número total de features
    pub const NUM_FEATURES: usize = 54;

    /// Nombres de las features (para importancia)
    pub fn feature_names() -> Vec<&'static str> {
        vec![
            "rsi",
            "rsi_normalized",
            "rsi_divergence",
            "macd",
            "macd_signal",
            "macd_histogram",
            "macd_hist_slope",
            "bb_position",
            "bb_width_pct",
            "bb_squeeze",
            "adx",
            "plus_di",
            "minus_di",
            "trend_strength",
            "price_velocity",
            "price_acceleration",
            "momentum_2nd_order",
            "vwap_distance_pct",
            "stoch_rsi",
            "stoch_rsi_overbought",
            "stoch_rsi_oversold",
            "spread_bps",
            "spread_percentile",
            "orderbook_imbalance",
            "depth_top5",
            "liquidity_concentration",
            "trade_intensity",
            "trade_intensity_zscore",
            "order_flow_imbalance",
            "minutes_to_close",
            "window_progress",
            "hour_of_day",
            "hour_sin",
            "hour_cos",
            "day_of_week",
            "is_weekend",
            "minutes_since_market_open",
            "market_regime",
            "volatility_5m",
            "volatility_percentile",
            "btc_eth_correlation",
            "correlation_change",
            "market_sentiment",
            "calibrator_confidence",
            "num_indicators_agreeing",
            "indicators_avg_win_rate",
            "bullish_weight",
            "bearish_weight",
            "is_btc",
            "is_15m",
            "polymarket_price",
            "polymarket_price_momentum",
            "polymarket_volume_24hr",
            "polymarket_liquidity",
        ]
    }
}

/// Engine para calcular features
pub struct FeatureEngine {
    /// Historial de features para cálculos de momentum
    feature_history: Vec<(i64, MLFeatureVector)>,
    /// Máximo historial a mantener
    max_history: usize,
    /// Estadísticas de spread para percentiles
    spread_history: Vec<f64>,
    /// Estadísticas de volatilidad
    volatility_history: Vec<f64>,
    /// Estadísticas de intensidad de trades
    intensity_history: Vec<f64>,
}

impl FeatureEngine {
    pub fn new() -> Self {
        Self {
            feature_history: Vec::with_capacity(100),
            max_history: 100,
            spread_history: Vec::with_capacity(1000),
            volatility_history: Vec::with_capacity(1000),
            intensity_history: Vec::with_capacity(1000),
        }
    }

    /// Calcular vector de features desde Features existentes
    pub fn compute(&mut self, features: &Features, context: &MarketContext) -> MLFeatureVector {
        let mut ml_features = MLFeatureVector::default();

        // ============ Técnicos Básicos ============
        ml_features.rsi = features.rsi.unwrap_or(50.0);
        ml_features.rsi_normalized = (ml_features.rsi - 50.0) / 50.0;
        // TODO: Calcular divergencia RSI

        ml_features.macd = features.macd.unwrap_or(0.0);
        ml_features.macd_signal = features.macd_signal.unwrap_or(0.0);
        ml_features.macd_histogram = features.macd_hist.unwrap_or(0.0);
        ml_features.macd_hist_slope = self.calculate_macd_slope();

        ml_features.bb_position = features.bb_position.unwrap_or(0.5);
        ml_features.bb_width_pct = self.calculate_bb_width(features);
        ml_features.bb_squeeze = self.calculate_bb_squeeze(features);

        ml_features.adx = features.adx.unwrap_or(0.0);
        ml_features.plus_di = features.plus_di.unwrap_or(0.0);
        ml_features.minus_di = features.minus_di.unwrap_or(0.0);
        ml_features.trend_strength =
            ml_features.adx * (ml_features.plus_di - ml_features.minus_di) / 100.0;

        // ============ Momentum ============
        ml_features.price_velocity = features.velocity.unwrap_or(0.0);
        ml_features.price_acceleration = features.short_term_velocity.unwrap_or(0.0);
        ml_features.momentum_2nd_order = self.calculate_momentum_2nd_order();

        ml_features.vwap_distance_pct = self.calculate_vwap_distance(features);

        ml_features.stoch_rsi = features.stoch_rsi.unwrap_or(0.5);
        ml_features.stoch_rsi_overbought = if ml_features.stoch_rsi > 0.8 {
            1.0
        } else {
            0.0
        };
        ml_features.stoch_rsi_oversold = if ml_features.stoch_rsi < 0.2 {
            1.0
        } else {
            0.0
        };

        // ============ Microestructura ============
        ml_features.spread_bps = features.spread_bps.unwrap_or(0.0);
        ml_features.spread_percentile = self.calculate_spread_percentile(ml_features.spread_bps);

        ml_features.orderbook_imbalance = features.orderbook_imbalance.unwrap_or(0.0);
        ml_features.depth_top5 = features.orderbook_depth_top5.unwrap_or(0.0);
        ml_features.liquidity_concentration = self.calculate_liquidity_concentration(features);

        ml_features.trade_intensity = 0.0; // Simplificado
        ml_features.trade_intensity_zscore = 0.0;
        ml_features.order_flow_imbalance = self.calculate_order_flow_imbalance(features);

        // ============ Temporales ============
        ml_features.minutes_to_close = context.minutes_to_close;
        ml_features.window_progress = features.window_progress.unwrap_or(1.0);
        ml_features.hour_of_day = context.hour as f64;
        ml_features.hour_sin = ((context.hour as f64) / 24.0 * 2.0 * std::f64::consts::PI).sin();
        ml_features.hour_cos = ((context.hour as f64) / 24.0 * 2.0 * std::f64::consts::PI).cos();
        ml_features.day_of_week = context.day_of_week as f64;
        ml_features.is_weekend = if context.day_of_week >= 5 { 1.0 } else { 0.0 };
        ml_features.minutes_since_market_open = context.minutes_since_market_open;

        // ============ Contexto ============
        ml_features.market_regime = match features.regime {
            MarketRegime::Ranging => 0.0,
            MarketRegime::Trending => 1.0,
            MarketRegime::Volatile => 2.0,
        };
        ml_features.volatility_5m = features.volatility.unwrap_or(0.0);
        ml_features.volatility_percentile =
            self.calculate_volatility_percentile(ml_features.volatility_5m);

        ml_features.btc_eth_correlation = context.btc_eth_correlation;
        ml_features.correlation_change = self.calculate_correlation_change();
        ml_features.market_sentiment = self.calculate_market_sentiment(features);

        // ============ Calibrador ============
        ml_features.calibrator_confidence = context.calibrator_confidence;
        ml_features.num_indicators_agreeing = context.num_indicators_agreeing as f64;
        ml_features.indicators_avg_win_rate = context.indicators_avg_win_rate;
        ml_features.bullish_weight = context.bullish_weight;
        ml_features.bearish_weight = context.bearish_weight;

        // ============ Meta ============
        ml_features.is_btc = if features.asset == Asset::BTC {
            1.0
        } else {
            0.0
        };
        ml_features.is_15m = if features.timeframe == Timeframe::Min15 {
            1.0
        } else {
            0.0
        };

        // ============ Polymarket ============
        ml_features.polymarket_price = features.polymarket_price.unwrap_or(0.5);
        ml_features.polymarket_price_momentum = self.calculate_polymarket_momentum(ml_features.polymarket_price);
        ml_features.polymarket_volume_24hr = features.polymarket_volume_24hr.unwrap_or(0.0);
        ml_features.polymarket_liquidity = features.polymarket_liquidity.unwrap_or(0.0);

        // Guardar en historial
        self.feature_history
            .push((features.ts, ml_features.clone()));
        if self.feature_history.len() > self.max_history {
            self.feature_history.remove(0);
        }

        // Actualizar estadísticas
        self.spread_history.push(ml_features.spread_bps);
        if self.spread_history.len() > 1000 {
            self.spread_history.remove(0);
        }

        ml_features
    }

    // Métodos helper (simplificados - implementación completa después)
    fn calculate_macd_slope(&self) -> f64 {
        if self.feature_history.len() < 2 {
            return 0.0;
        }
        let last = &self.feature_history.last().unwrap().1;
        let prev = &self.feature_history[self.feature_history.len() - 2].1;
        last.macd_histogram - prev.macd_histogram
    }

    fn calculate_bb_width(&self, _features: &Features) -> f64 {
        // TODO: Implementar
        0.0
    }

    fn calculate_bb_squeeze(&self, _features: &Features) -> f64 {
        // TODO: Implementar
        0.0
    }

    fn calculate_momentum_2nd_order(&self) -> f64 {
        if self.feature_history.len() < 3 {
            return 0.0;
        }
        let v1 = self.feature_history[self.feature_history.len() - 3]
            .1
            .price_velocity;
        let v2 = self.feature_history[self.feature_history.len() - 2]
            .1
            .price_velocity;
        let v3 = self.feature_history.last().unwrap().1.price_velocity;
        (v3 - v2) - (v2 - v1)
    }

    fn calculate_polymarket_momentum(&self, current_price: f64) -> f64 {
        if self.feature_history.len() < 3 {
            return 0.0;
        }
        let last_price = self.feature_history.last().unwrap().1.polymarket_price;
        current_price - last_price
    }

    fn calculate_vwap_distance(&self, features: &Features) -> f64 {
        if let Some(vwap) = features.vwap {
            (features.close - vwap) / vwap
        } else {
            0.0
        }
    }

    fn calculate_spread_percentile(&self, spread: f64) -> f64 {
        if self.spread_history.is_empty() {
            return 0.5;
        }
        let count_below = self.spread_history.iter().filter(|&&s| s < spread).count();
        count_below as f64 / self.spread_history.len() as f64
    }

    fn calculate_liquidity_concentration(&self, features: &Features) -> f64 {
        // Ratio entre top 3 niveles y top 10 niveles
        // Mayor = más concentrado
        // Simplificado
        if let Some(depth) = features.orderbook_depth_top5 {
            (depth / 1000.0).min(1.0)
        } else {
            0.5
        }
    }

    fn calculate_intensity_zscore(&self, intensity: f64) -> f64 {
        if self.intensity_history.len() < 10 {
            return 0.0;
        }
        let mean = self.intensity_history.iter().sum::<f64>() / self.intensity_history.len() as f64;
        let variance = self
            .intensity_history
            .iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>()
            / self.intensity_history.len() as f64;
        let std = variance.sqrt();
        if std > 0.0 {
            (intensity - mean) / std
        } else {
            0.0
        }
    }

    fn calculate_order_flow_imbalance(&self, _features: &Features) -> f64 {
        // TODO: Implementar con datos reales de order flow
        0.0
    }

    fn calculate_volatility_percentile(&self, vol: f64) -> f64 {
        if self.volatility_history.is_empty() {
            return 0.5;
        }
        let count_below = self.volatility_history.iter().filter(|&&v| v < vol).count();
        count_below as f64 / self.volatility_history.len() as f64
    }

    fn calculate_correlation_change(&self) -> f64 {
        if self.feature_history.len() < 10 {
            return 0.0;
        }
        let recent_corr = self.feature_history.last().unwrap().1.btc_eth_correlation;
        let old_corr = self.feature_history[self.feature_history.len() - 10]
            .1
            .btc_eth_correlation;
        recent_corr - old_corr
    }

    fn calculate_market_sentiment(&self, features: &Features) -> f64 {
        // Combinar order flow y orderbook imbalance
        features.orderbook_imbalance.unwrap_or(0.0)
    }
}

/// Contexto de mercado para cálculo de features
#[derive(Debug, Clone, Default)]
pub struct MarketContext {
    pub timestamp: i64,
    pub hour: u8,
    pub day_of_week: u8,
    pub minutes_to_close: f64,
    pub minutes_since_market_open: f64,
    pub btc_eth_correlation: f64,
    pub calibrator_confidence: f64,
    pub num_indicators_agreeing: usize,
    pub indicators_avg_win_rate: f64,
    pub bullish_weight: f64,
    pub bearish_weight: f64,
}
