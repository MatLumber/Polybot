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
    /// Divergencia RSI — never computed, kept for serde compat only
    #[serde(default)]
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

    // Dead features kept for serde backward compat but excluded from ML vector
    #[serde(default)]
    pub bb_width_pct: f64,
    #[serde(default)]
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

    // Dead features kept for serde backward compat but excluded from ML vector
    #[serde(default)]
    pub trade_intensity: f64,
    #[serde(default)]
    pub trade_intensity_zscore: f64,
    #[serde(default)]
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

    // Dead feature kept for serde backward compat but excluded from ML vector
    #[serde(default)]
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

    // ============ Window History & Cross-TF ============
    /// Resultado de la ventana previa para este mercado (1=UP, 0=DOWN, 0.5=desconocido)
    pub prev_window_dir_1: f64,
    /// Resultado de hace 2 ventanas
    pub prev_window_dir_2: f64,
    /// Resultado de hace 3 ventanas
    pub prev_window_dir_3: f64,
    /// Racha de ventanas consecutivas en la misma dirección, normalizada -1..1 (cap=5).
    /// Positivo = rachas UP, negativo = rachas DOWN.
    pub window_streak: f64,
    /// Alineación cross-timeframe: +1=acuerdo (15m y 1h apuntan igual), -1=desacuerdo, 0=desconocido
    pub cross_tf_alignment: f64,
    /// Precio del token Polymarket (YES) al abrir la ventana actual (probabilidad implícita)
    pub token_price_window_open: f64,
    /// Cambio relativo del precio del token desde apertura de ventana (%, clamp -1..1)
    pub token_price_change_window: f64,
    /// Tendencia de volumen 24h: +1=subiendo, -1=bajando, 0=estable
    pub volume_trend: f64,

    // ============ Polymarket Alpha ============
    /// BTC/ETH distance from window-open strike price (%).
    /// Positive = price is above strike (UP prediction favored).
    /// Clipped to [-10, +10] to avoid extreme outliers.
    pub price_vs_strike_pct: f64,

    // ============ Market Edge ============
    /// Market certainty: |polymarket_price - 0.5|, range [0, 0.49].
    /// High value = market has strong consensus (little room for edge).
    /// Low value = market is uncertain (50/50) — more room for ML divergence.
    pub market_certainty: f64,

    // ============ Intra-Window Wicks ============
    /// Upper shadow ratio: (window_high - close) / (window_high - window_low), [0, 1].
    /// High = price rejected from the highs (bearish pressure).
    #[serde(default)]
    pub window_upper_shadow: f64,
    /// Lower shadow ratio: (close - window_low) / (window_high - window_low), [0, 1].
    /// High = price bounced from the lows (bullish pressure).
    #[serde(default)]
    pub window_lower_shadow: f64,
}

impl MLFeatureVector {
    /// Convertir a vector f64 para los modelos ML
    /// NOTE: Dead features (bb_width_pct, bb_squeeze, trade_intensity,
    /// trade_intensity_zscore, order_flow_imbalance, correlation_change)
    /// are excluded — they were always 0.0 and added noise.
    pub fn to_vec(&self) -> Vec<f64> {
        vec![
            self.rsi,
            self.rsi_normalized,
            // rsi_divergence REMOVED (always 0.0, adds noise)
            self.macd,
            self.macd_signal,
            self.macd_histogram,
            self.macd_hist_slope,
            self.bb_position,
            // bb_width_pct REMOVED (always 0)
            // bb_squeeze REMOVED (always 0)
            self.adx,
            self.plus_di,
            self.minus_di,
            self.trend_strength,
            self.price_velocity,
            self.price_acceleration,
            self.momentum_2nd_order,
            // vwap_distance_pct REMOVED (always 0 in native_only mode: candles have no volume)
            self.stoch_rsi,
            self.stoch_rsi_overbought,
            self.stoch_rsi_oversold,
            self.spread_bps,
            self.spread_percentile,
            self.orderbook_imbalance,
            self.depth_top5,
            self.liquidity_concentration,
            // trade_intensity REMOVED (always 0)
            // trade_intensity_zscore REMOVED (always 0)
            // order_flow_imbalance REMOVED (always 0)
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
            // correlation_change REMOVED (always 0)
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
            // Window History & Cross-TF (8)
            self.prev_window_dir_1,
            self.prev_window_dir_2,
            self.prev_window_dir_3,
            self.window_streak,
            self.cross_tf_alignment,
            self.token_price_window_open,
            self.token_price_change_window,
            self.volume_trend,
            // Polymarket Alpha (1)
            self.price_vs_strike_pct,
            // Market Edge (1)
            self.market_certainty,
            // Intra-Window Wicks (2)
            self.window_upper_shadow,
            self.window_lower_shadow,
        ]
    }

    /// 58 (prior) - 1 (vwap_distance_pct removed, always 0 in native_only mode) = 57
    pub const NUM_FEATURES: usize = 57;

    /// Nombres de las features (para importancia)
    pub fn feature_names() -> Vec<&'static str> {
        vec![
            "rsi",
            "rsi_normalized",
            // rsi_divergence removed
            "macd",
            "macd_signal",
            "macd_histogram",
            "macd_hist_slope",
            "bb_position",
            "adx",
            "plus_di",
            "minus_di",
            "trend_strength",
            "price_velocity",
            "price_acceleration",
            "momentum_2nd_order",
            // vwap_distance_pct REMOVED
            "stoch_rsi",
            "stoch_rsi_overbought",
            "stoch_rsi_oversold",
            "spread_bps",
            "spread_percentile",
            "orderbook_imbalance",
            "depth_top5",
            "liquidity_concentration",
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
            // Window History & Cross-TF
            "prev_window_dir_1",
            "prev_window_dir_2",
            "prev_window_dir_3",
            "window_streak",
            "cross_tf_alignment",
            "token_price_window_open",
            "token_price_change_window",
            "volume_trend",
            // Polymarket Alpha
            "price_vs_strike_pct",
            // Market Edge
            "market_certainty",
            // Intra-Window Wicks
            "window_upper_shadow",
            "window_lower_shadow",
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
}

impl FeatureEngine {
    pub fn new() -> Self {
        Self {
            feature_history: Vec::with_capacity(100),
            max_history: 100,
            spread_history: Vec::with_capacity(1000),
            volatility_history: Vec::with_capacity(1000),
        }
    }

    /// Calcular vector de features desde Features existentes
    pub fn compute(&mut self, features: &Features, context: &MarketContext) -> MLFeatureVector {
        let mut ml_features = MLFeatureVector::default();

        // ============ Técnicos Básicos ============
        ml_features.rsi = features.rsi.unwrap_or(50.0);
        ml_features.rsi_normalized = (ml_features.rsi - 50.0) / 50.0;

        ml_features.macd = features.macd.unwrap_or(0.0);
        ml_features.macd_signal = features.macd_signal.unwrap_or(0.0);
        ml_features.macd_histogram = features.macd_hist.unwrap_or(0.0);
        ml_features.macd_hist_slope = self.calculate_macd_slope();

        ml_features.bb_position = features.bb_position.unwrap_or(0.5);
        // bb_width_pct and bb_squeeze removed (were always 0)

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
        // Update spread history so percentile calculation is meaningful.
        // Was never populated before → spread_percentile was always 0.5 (constant).
        if ml_features.spread_bps > 0.0 {
            self.spread_history.push(ml_features.spread_bps);
            if self.spread_history.len() > 1000 {
                self.spread_history.remove(0);
            }
        }
        ml_features.spread_percentile = self.calculate_spread_percentile(ml_features.spread_bps);

        ml_features.orderbook_imbalance = features.orderbook_imbalance.unwrap_or(0.0);
        ml_features.depth_top5 = features.orderbook_depth_top5.unwrap_or(0.0);
        ml_features.liquidity_concentration = self.calculate_liquidity_concentration(features);
        // trade_intensity, trade_intensity_zscore, order_flow_imbalance removed (were always 0)

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
        // Update volatility history so percentile calculation is meaningful.
        // Was never populated before → volatility_percentile was always 0.5 (constant).
        if ml_features.volatility_5m > 0.0 {
            self.volatility_history.push(ml_features.volatility_5m);
            if self.volatility_history.len() > 1000 {
                self.volatility_history.remove(0);
            }
        }
        ml_features.volatility_percentile =
            self.calculate_volatility_percentile(ml_features.volatility_5m);
        // correlation_change removed (was always 0)
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
        ml_features.polymarket_price_momentum =
            self.calculate_polymarket_momentum(ml_features.polymarket_price);
        ml_features.polymarket_volume_24hr = features.polymarket_volume_24hr.unwrap_or(0.0);
        ml_features.polymarket_liquidity = features.polymarket_liquidity.unwrap_or(0.0);

        // ============ Market Edge ============
        // market_certainty = how far the token price is from 50% (market consensus strength).
        // Range [0, 0.49]. High = market is certain (less room for ML divergence).
        ml_features.market_certainty = (ml_features.polymarket_price - 0.5).abs();

        // ============ Intra-Window Wicks ============
        ml_features.window_upper_shadow = features.window_upper_shadow.unwrap_or(0.5);
        ml_features.window_lower_shadow = features.window_lower_shadow.unwrap_or(0.5);

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

    fn calculate_macd_slope(&self) -> f64 {
        if self.feature_history.len() < 2 {
            return 0.0;
        }
        let last = &self.feature_history.last().unwrap().1;
        let prev = &self.feature_history[self.feature_history.len() - 2].1;
        last.macd_histogram - prev.macd_histogram
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
        if let Some(depth) = features.orderbook_depth_top5 {
            (depth / 1000.0).min(1.0)
        } else {
            0.5
        }
    }

    fn calculate_volatility_percentile(&self, vol: f64) -> f64 {
        if self.volatility_history.is_empty() {
            return 0.5;
        }
        let count_below = self.volatility_history.iter().filter(|&&v| v < vol).count();
        count_below as f64 / self.volatility_history.len() as f64
    }

    fn calculate_market_sentiment(&self, features: &Features) -> f64 {
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
    pub calibrator_confidence: f64,
    pub num_indicators_agreeing: usize,
    pub indicators_avg_win_rate: f64,
    pub bullish_weight: f64,
    pub bearish_weight: f64,
}
