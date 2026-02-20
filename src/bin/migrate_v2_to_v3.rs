//! Script de migración V2 → V3
//!
//! Uso: cargo run --bin migrate_v2_to_v3
//!
//! Este script migra datos del calibrador V2 al dataset ML de V3,
//! generando TradeSamples sintéticos basados en las estadísticas históricas.

use polybot::ml_engine::dataset::{Dataset, TradeSample};
use polybot::ml_engine::features::MLFeatureVector;
use polybot::strategy::calibrator::IndicatorCalibrator;
use polybot::types::{Asset, Direction, Timeframe};
use std::collections::HashMap;
use std::fs;
use tracing::{info, warn};

fn main() -> anyhow::Result<()> {
    // Inicializar logging
    tracing_subscriber::fmt::init();

    info!("🔄 Iniciando migración V2 → V3...");
    info!("Este proceso generará samples sintéticos desde el calibrador V2");

    // 1. Cargar calibrador V2
    let calibrator = load_calibrator_v2()?;
    info!("✅ Calibrador V2 cargado");

    // 2. Crear dataset V3
    let mut dataset = Dataset::new();
    info!("📝 Dataset V3 creado");

    // 3. Generar samples sintéticos
    let total_samples = generate_synthetic_samples(&calibrator, &mut dataset)?;
    info!("✅ Generados {} samples sintéticos", total_samples);

    // 4. Guardar dataset
    let dataset_path = "data/ml_engine/dataset.json";
    fs::create_dir_all("data/ml_engine")?;
    dataset.save(dataset_path)?;
    info!("💾 Dataset guardado en {}", dataset_path);

    // 5. Crear estado inicial de ML
    create_initial_ml_state(total_samples)?;
    info!("💾 Estado ML inicial creado");

    info!("");
    info!("🎉 MIGRACIÓN COMPLETADA");
    info!("============================");
    info!("Total samples: {}", total_samples);
    info!("Dataset: data/ml_engine/dataset.json");
    info!("Estado ML: data/ml_engine/ml_state.json");
    info!("");
    info!("Ahora puedes reiniciar el bot con V3 activado:");
    info!("  use_v3_strategy: true");

    Ok(())
}

fn load_calibrator_v2() -> anyhow::Result<IndicatorCalibrator> {
    let path = "data/calibrator_state_v2.json";

    if !std::path::Path::new(path).exists() {
        warn!("No se encontró calibrator_state_v2.json, creando calibrador vacío");
        return Ok(IndicatorCalibrator::new());
    }

    let json = fs::read_to_string(path)?;
    let stats: HashMap<String, Vec<polybot::strategy::IndicatorStats>> =
        serde_json::from_str(&json)?;

    let mut calibrator = IndicatorCalibrator::new();
    calibrator.load_stats_by_market(stats);

    Ok(calibrator)
}

fn generate_synthetic_samples(
    calibrator: &IndicatorCalibrator,
    dataset: &mut Dataset,
) -> anyhow::Result<usize> {
    let mut total = 0;

    // Definir los markets a migrar
    let markets = vec![
        ("BTC", Asset::BTC, Timeframe::Min15, "BTC_15M"),
        ("BTC", Asset::BTC, Timeframe::Hour1, "BTC_1H"),
        ("ETH", Asset::ETH, Timeframe::Min15, "ETH_15M"),
        ("ETH", Asset::ETH, Timeframe::Hour1, "ETH_1H"),
    ];

    let now = chrono::Utc::now().timestamp_millis();
    let time_range = 30 * 24 * 60 * 60 * 1000i64; // 30 días en ms

    for (asset_name, asset, timeframe, market_key) in markets {
        info!("Procesando market: {}", market_key);

        // Obtener estadísticas del calibrador para este market
        let stats_by_market = calibrator.export_stats_by_market();

        if let Some(indicators) = stats_by_market.get(market_key) {
            for indicator in indicators {
                if indicator.total_signals == 0 {
                    continue;
                }

                let num_trades = indicator.total_signals;
                let num_wins = (num_trades as f64 * indicator.win_rate) as usize;
                let num_losses = num_trades - num_wins;

                info!(
                    "  {}: {} trades ({} wins, {} losses)",
                    indicator.name, num_trades, num_wins, num_losses
                );

                // Generar trades ganadores
                for i in 0..num_wins {
                    let time_offset = (i as i64 * time_range) / num_trades as i64;
                    let timestamp = now - time_range + time_offset;

                    let sample = create_trade_sample(
                        asset,
                        timeframe,
                        &indicator.name,
                        true,
                        timestamp,
                        indicator.calibrated_weight,
                        indicator.win_rate,
                    );

                    dataset.add_trade(sample);
                    total += 1;
                }

                // Generar trades perdedores
                for i in 0..num_losses {
                    let idx = num_wins + i;
                    let time_offset = (idx as i64 * time_range) / num_trades as i64;
                    let timestamp = now - time_range + time_offset;

                    let sample = create_trade_sample(
                        asset,
                        timeframe,
                        &indicator.name,
                        false,
                        timestamp,
                        indicator.calibrated_weight,
                        indicator.win_rate,
                    );

                    dataset.add_trade(sample);
                    total += 1;
                }
            }
        } else {
            warn!("  No hay datos para {}", market_key);
        }
    }

    Ok(total)
}

fn create_trade_sample(
    asset: Asset,
    timeframe: Timeframe,
    indicator: &str,
    is_win: bool,
    timestamp: i64,
    weight: f64,
    win_rate: f64,
) -> TradeSample {
    let direction = infer_direction(indicator, is_win);

    TradeSample {
        trade_id: format!(
            "migrated_{:?}_{:?}_{}_{}_{}",
            asset,
            timeframe,
            timestamp,
            indicator,
            if is_win { "win" } else { "loss" }
        ),
        entry_ts: timestamp,
        exit_ts: timestamp + (timeframe.duration_secs() as i64 * 1000),
        asset,
        timeframe,
        direction,
        is_win,
        entry_features: generate_features(indicator, is_win, weight, win_rate, asset, timeframe),
        entry_price: 50000.0,
        exit_price: if is_win { 50500.0 } else { 49500.0 },
        pnl: if is_win { 50.0 } else { -50.0 },
        estimated_edge: weight,
        indicators_triggered: vec![indicator.to_string()],
    }
}

fn infer_direction(indicator: &str, is_win: bool) -> Direction {
    // Indicadores bullish (compran en Up)
    let bullish = vec![
        "adx_trend",
        "ema_trend",
        "macd_histogram",
        "momentum_acceleration",
        "heikin_ashi",
        "short_term_momentum",
    ];

    // Indicadores bearish (venden en Down)
    let bearish = vec!["rsi_extreme", "bollinger_band", "rsi_divergence"];

    if bullish.contains(&indicator) {
        if is_win {
            Direction::Up // Subió, ganamos comprando
        } else {
            Direction::Down // Bajó, perdimos comprando
        }
    } else if bearish.contains(&indicator) {
        if is_win {
            Direction::Down // Bajó, ganamos vendiendo
        } else {
            Direction::Up // Subió, perdimos vendiendo
        }
    } else {
        // Default: aleatorio basado en win
        if is_win {
            Direction::Up
        } else {
            Direction::Down
        }
    }
}

fn generate_features(
    indicator: &str,
    is_win: bool,
    weight: f64,
    win_rate: f64,
    asset: Asset,
    timeframe: Timeframe,
) -> MLFeatureVector {
    let mut features = MLFeatureVector::default();

    use chrono::{Datelike, Timelike, Utc};
    let now = Utc::now();

    // ============ TÉCNICOS BÁSICOS ============
    match indicator {
        "rsi_extreme" => {
            // RSI en extremos
            features.rsi = if is_win { 30.0 } else { 70.0 };
            features.rsi_normalized = (features.rsi - 50.0) / 50.0;
            features.stoch_rsi = if is_win { 0.15 } else { 0.85 };
            features.stoch_rsi_oversold = if is_win { 1.0 } else { 0.0 };
            features.stoch_rsi_overbought = if !is_win { 1.0 } else { 0.0 };
            features.bb_position = if is_win { 0.1 } else { 0.9 };
        }
        "adx_trend" => {
            // Fuerte tendencia
            features.adx = 40.0;
            features.plus_di = if is_win { 35.0 } else { 20.0 };
            features.minus_di = if !is_win { 35.0 } else { 20.0 };
            features.trend_strength = 0.8;
            features.bb_position = if is_win { 0.7 } else { 0.3 };
        }
        "macd_histogram" => {
            // MACD momentum
            features.macd = if is_win { 0.03 } else { -0.03 };
            features.macd_signal = if is_win { 0.01 } else { -0.01 };
            features.macd_histogram = if is_win { 0.02 } else { -0.02 };
            features.price_velocity = if is_win { 0.015 } else { -0.015 };
        }
        "momentum_acceleration" => {
            // Aceleración del precio
            features.price_velocity = if is_win { 0.02 } else { -0.02 };
            features.price_acceleration = if is_win { 0.01 } else { -0.01 };
            features.momentum_2nd_order = if is_win { 0.005 } else { -0.005 };
        }
        "ema_trend" => {
            // Tendencia EMA
            features.bb_position = if is_win { 0.65 } else { 0.35 };
            features.vwap_distance_pct = if is_win { 0.02 } else { -0.02 };
            features.trend_strength = 0.6;
        }
        "heikin_ashi" => {
            // Heikin Ashi tendencia
            features.bb_position = if is_win { 0.6 } else { 0.4 };
            features.stoch_rsi = if is_win { 0.6 } else { 0.4 };
        }
        "bollinger_band" => {
            // Reversión en Bollinger
            features.bb_position = if is_win { 0.1 } else { 0.9 };
            features.bb_width_pct = 0.05;
            features.rsi = if is_win { 25.0 } else { 75.0 };
        }
        "stoch_rsi" => {
            // Stochastic RSI señal
            features.stoch_rsi = if is_win { 0.2 } else { 0.8 };
            features.rsi = if is_win { 35.0 } else { 65.0 };
        }
        "short_term_momentum" => {
            // Momentum corto plazo
            features.price_velocity = if is_win { 0.025 } else { -0.025 };
            features.price_acceleration = if is_win { 0.008 } else { -0.008 };
        }
        _ => {
            // Valores default realistas
            features.rsi = 50.0;
            features.bb_position = 0.5;
            features.stoch_rsi = 0.5;
        }
    }

    // ============ MICROESTRUCTURA ============
    features.spread_bps = 100.0 + (100.0 - weight * 50.0);
    features.spread_percentile = 0.5;
    features.orderbook_imbalance = if is_win { 0.2 } else { -0.2 };
    features.depth_top5 = 5000.0 + (weight * 5000.0);
    features.liquidity_concentration = 0.5;
    features.order_flow_imbalance = if is_win { 0.15 } else { -0.15 };

    // Trade intensity no disponible en V2
    features.trade_intensity = 0.0;
    features.trade_intensity_zscore = 0.0;

    // ============ TEMPORALES ============
    features.minutes_to_close = match timeframe {
        Timeframe::Min15 => 7.5,
        Timeframe::Hour1 => 30.0,
    };
    features.window_progress = 0.5;
    features.hour_of_day = now.hour() as f64;
    features.hour_sin = ((now.hour() as f64) / 24.0 * 2.0 * std::f64::consts::PI).sin();
    features.hour_cos = ((now.hour() as f64) / 24.0 * 2.0 * std::f64::consts::PI).cos();
    features.day_of_week = now.weekday().num_days_from_monday() as f64;
    features.is_weekend = if now.weekday().num_days_from_monday() >= 5 {
        1.0
    } else {
        0.0
    };
    features.minutes_since_market_open = (now.hour() as f64 * 60.0).max(0.0);

    // ============ CONTEXTO ============
    features.market_regime = 1.0; // Trending default
    features.volatility_5m = match timeframe {
        Timeframe::Min15 => 0.015,
        Timeframe::Hour1 => 0.025,
    };
    features.volatility_percentile = 0.5;

    // No disponibles en V2
    features.btc_eth_correlation = 0.0;
    features.correlation_change = 0.0;
    features.market_sentiment = features.orderbook_imbalance;

    // ============ CALIBRADOR ============
    features.calibrator_confidence = weight.clamp(0.3, 0.9);
    features.num_indicators_agreeing = 1.0 + (weight * 2.0);
    features.indicators_avg_win_rate = win_rate.clamp(0.2, 0.8);
    features.bullish_weight = if is_bullish(indicator) { weight } else { 0.0 };
    features.bearish_weight = if is_bearish(indicator) { weight } else { 0.0 };

    // ============ META ============
    features.is_btc = if asset == Asset::BTC { 1.0 } else { 0.0 };
    features.is_15m = if timeframe == Timeframe::Min15 {
        1.0
    } else {
        0.0
    };

    features
}

fn is_bullish(indicator: &str) -> bool {
    matches!(
        indicator,
        "adx_trend"
            | "ema_trend"
            | "macd_histogram"
            | "momentum_acceleration"
            | "heikin_ashi"
            | "short_term_momentum"
    )
}

fn is_bearish(indicator: &str) -> bool {
    matches!(
        indicator,
        "rsi_extreme" | "bollinger_band" | "rsi_divergence"
    )
}

fn create_initial_ml_state(total_samples: usize) -> anyhow::Result<()> {
    use polybot::ml_engine::persistence::{MLPersistenceState, ModelPerformance};

    let state = MLPersistenceState {
        version: "3.0".to_string(),
        config: polybot::ml_engine::MLEngineConfig::default(),
        ensemble_weights: polybot::ml_engine::models::EnsembleWeights::default(),
        total_predictions: 0,
        correct_predictions: 0,
        incorrect_predictions: 0,
        last_retraining: None,
        feature_importance: HashMap::new(),
        model_performances: vec![
            ModelPerformance {
                model_name: "Random Forest".to_string(),
                accuracy: 0.55,
                predictions_count: 0,
                correct_count: 0,
            },
            ModelPerformance {
                model_name: "Gradient Boosting".to_string(),
                accuracy: 0.53,
                predictions_count: 0,
                correct_count: 0,
            },
            ModelPerformance {
                model_name: "Logistic Regression".to_string(),
                accuracy: 0.52,
                predictions_count: 0,
                correct_count: 0,
            },
        ],
        training_history: vec![],
        saved_at: chrono::Utc::now().timestamp_millis(),
    };

    let json = serde_json::to_string_pretty(&state)?;
    fs::write("data/ml_engine/ml_state.json", json)?;

    Ok(())
}
