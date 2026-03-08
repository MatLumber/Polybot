//! PolyBot - Directional Trading Bot for Polymarket
//!
//! Predicts UP/DOWN direction for 15m/1h BTC/ETH/SOL/XRP markets
//! using Polymarket-native RTDS + CLOB market data.
mod backtesting;
mod clob;
mod config;
#[cfg(feature = "dashboard")]
mod dashboard;
mod features;
mod ml_engine;
mod oracle;
mod paper_trading;
mod persistence;
mod polymarket;
mod risk;
mod strategy;
mod types;
use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::clob::{ClobClient, Order};
use crate::config::AppConfig;
use crate::features::{FeatureEngine, Features, MarketRegime, OrderbookImbalanceTracker};
use crate::ml_engine::config_bridge::MLConfigConvertible;
use crate::oracle::PriceAggregator;
use crate::paper_trading::{PaperTradingConfig, PaperTradingEngine};
use crate::persistence::{BalanceTracker, CsvPersistence, HardResetOptions};
use crate::risk::RiskManager;
use crate::strategy::{Strategy, StrategyConfig, StrategyEngine, TradeResult};
use crate::types::{Asset, Direction, FeatureSet, PriceSource, PriceTick, Signal, Timeframe};

#[cfg(feature = "dashboard")]
use crate::dashboard::{PaperStatsResponse, PositionResponse, TradeResponse, WsMessage};

const BOT_TAG: &str = env!("CARGO_PKG_VERSION");

fn looks_like_hourly_updown_market_text(text: &str) -> bool {
    let is_updown = text.contains("bitcoin-up-or-down-")
        || text.contains("ethereum-up-or-down-")
        || text.contains("bitcoin up or down")
        || text.contains("ethereum up or down")
        || text.contains("up-or-down");
    if !is_updown {
        return false;
    }
    if text.contains("15m")
        || text.contains("updown-15m")
        || text.contains("5m")
        || text.contains("updown-5m")
        || text.contains("4h")
        || text.contains("updown-4h")
    {
        return false;
    }
    text.contains("am-et")
        || text.contains("pm-et")
        || text.contains("am et")
        || text.contains("pm et")
}

fn infer_direction_from_outcome(outcome: Option<&str>) -> Option<Direction> {
    let outcome = outcome?.to_ascii_lowercase();
    if ["yes", "up", "higher", "above", "true"]
        .iter()
        .any(|needle| outcome.contains(needle))
    {
        return Some(Direction::Up);
    }
    if ["no", "down", "lower", "below", "false"]
        .iter()
        .any(|needle| outcome.contains(needle))
    {
        return Some(Direction::Down);
    }
    None
}

fn parse_position_number(raw: Option<&str>) -> Option<f64> {
    raw.and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite())
}

fn live_context_is_viable(context: &LivePositionContext) -> bool {
    context.shares_size > 0.0
        && context.entry_share_price > 0.0
        && context.size_usdc > 0.0
}

fn load_persisted_prediction_counters(data_dir: &str) -> Option<(usize, usize, usize)> {
    let path = std::path::PathBuf::from(data_dir).join("paper_trading_state.json");
    let json = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&json).ok()?;
    let stats = value.get("stats")?;

    let total = stats
        .get("total_trades")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as usize;
    if total == 0 {
        return None;
    }

    let correct = stats
        .get("predictions_correct")
        .and_then(serde_json::Value::as_u64)
        .or_else(|| stats.get("wins").and_then(serde_json::Value::as_u64))
        .unwrap_or(0) as usize;
    let incorrect = stats
        .get("predictions_incorrect")
        .and_then(serde_json::Value::as_u64)
        .or_else(|| stats.get("losses").and_then(serde_json::Value::as_u64))
        .unwrap_or_else(|| total.saturating_sub(correct) as u64) as usize;

    Some((total, correct.min(total), incorrect.min(total)))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct LivePositionContext {
    signal_id: String,
    asset: Asset,
    timeframe: Timeframe,
    direction: Direction,
    confidence: f64,
    market_slug: String,
    token_id: String,
    condition_id: String,
    size_usdc: f64,
    shares_size: f64,
    entry_share_price: f64,
    opened_at_ms: i64,
    expires_at_ms: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logging()?;
    info!(bot_tag = %BOT_TAG, "🤖 PolyBot v{} starting...", BOT_TAG);
    #[cfg(feature = "dashboard")]
    info!("🖥️ Dashboard feature ENABLED - server will start when initialized");
    #[cfg(not(feature = "dashboard"))]
    info!("🖥️ Dashboard feature DISABLED");

    let runtime_args = parse_runtime_args()?;
    let config = AppConfig::load()?;
    info!(config_digest = %config.digest(), "✅ Configuration loaded");
    let _startup_reset_executed = maybe_run_startup_reset(&config, &runtime_args)?;
    if runtime_args.reset_mode.is_some() {
        info!("Reset command completed; exiting by CLI request");
        return Ok(());
    } // Validate environment
    config.validate_env()?; // Create channels for inter-component communication
    let (price_tx, mut price_rx) = mpsc::channel::<PriceTick>(1000);
    let (paper_price_tx, mut paper_price_rx) = mpsc::channel::<PriceTick>(1000);
    let (feature_tx, mut feature_rx) = mpsc::channel::<Features>(500);
    let (signal_tx, mut signal_rx) = mpsc::channel::<Signal>(100);
    let (order_tx, mut order_rx) = mpsc::channel::<Order>(100);
    let staleness_gate_ms: i64 = config.oracle.staleness_ms.max(1_000) as i64;
    let last_tick_ts: Arc<tokio::sync::Mutex<std::collections::HashMap<Asset, i64>>> =
        Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())); // Initialize components
    let oracle = Arc::new(PriceAggregator::new(5000, 2, 100));
    let feature_engine = Arc::new(Mutex::new(FeatureEngine::new())); // ── Orderbook Imbalance Tracker (for microstructure analysis) ─────────────
    let orderbook_tracker = Arc::new(std::sync::Mutex::new(OrderbookImbalanceTracker::new()));
    {
        let tracker_clone = orderbook_tracker.clone();
        feature_engine
            .lock()
            .await
            .set_orderbook_tracker(tracker_clone);
    }
    info!("📊 OrderbookImbalanceTracker connected to FeatureEngine"); // Initialize strategy (V2 or V3 based on config)
    let strategy: Arc<Mutex<Box<dyn Strategy>>> = if config.use_v3_strategy {
        info!("🤖 Initializing V3 ML Strategy...");

        // Load ML configuration from config file, with fallback to defaults
        let ml_config = config.ml_engine.to_ml_engine_config();
        info!(
            ml_enabled = ml_config.enabled,
            model_type = ?ml_config.model_type,
            min_confidence = ml_config.min_confidence,
            "🧠 ML Engine configuration loaded"
        );

        let v3_strategy = crate::strategy::V3Strategy::new_with_data_dir(
            ml_config,
            StrategyConfig::default(),
            &config.persistence.data_dir,
        );

        // Load persisted ML state if exists
        let persistence =
            crate::ml_engine::persistence::MLPersistenceManager::new(&config.persistence.data_dir);
        if let Ok(Some((state, dataset))) = persistence.load_ml_state() {
            info!(
                "🧠 Loaded persisted ML state: {} predictions, {} samples",
                state.total_predictions,
                dataset.len()
            );
        }

        Arc::new(Mutex::new(Box::new(v3_strategy)))
    } else {
        info!("🤖 Initializing V2 Strategy...");
        let mut strategy_config = StrategyConfig::default();
        strategy_config.min_confidence = config.strategy.min_confidence.clamp(0.01, 0.99);
        Arc::new(Mutex::new(Box::new(
            StrategyEngine::with_calibration_min_samples(
                strategy_config,
                config.strategy.calibration_min_samples_per_market,
            ),
        )))
    }; // Load calibrator state (v2 preferred, v1 fallback)
    let calibrator_state_file_v1 =
        std::path::PathBuf::from(&config.persistence.data_dir).join("calibrator_state.json");
    let calibrator_state_file_v2 =
        std::path::PathBuf::from(&config.persistence.data_dir).join("calibrator_state_v2.json");
    let mut loaded_calibrator_state = false;
    if calibrator_state_file_v2.exists() {
        match std::fs::read_to_string(&calibrator_state_file_v2) {
            Ok(json) => match serde_json::from_str::<
                std::collections::HashMap<String, Vec<strategy::IndicatorStats>>,
            >(&json)
            {
                Ok(stats_by_market) => {
                    strategy
                        .lock()
                        .await
                        .import_calibrator_state_v2(stats_by_market);
                    loaded_calibrator_state = true;
                    info!(                        path = %calibrator_state_file_v2.display(),                        "Loaded calibrator v2 state from disk"                    );
                }
                Err(e) => warn!(error = %e, "Failed to parse calibrator v2 state"),
            },
            Err(e) => warn!(error = %e, "Failed to read calibrator v2 state file"),
        }
    } else if calibrator_state_file_v1.exists() {
        match std::fs::read_to_string(&calibrator_state_file_v1) {
            Ok(json) => match serde_json::from_str::<Vec<strategy::IndicatorStats>>(&json) {
                Ok(stats) => {
                    let total: usize = stats.iter().map(|s| s.total_signals).max().unwrap_or(0);
                    strategy.lock().await.import_calibrator_state(stats);
                    loaded_calibrator_state = true;
                    info!(                        trades = total,                        path = %calibrator_state_file_v1.display(),                        "Loaded legacy calibrator v1 state from disk"                    );
                }
                Err(e) => warn!(error = %e, "Failed to parse calibrator v1 state"),
            },
            Err(e) => warn!(error = %e, "Failed to read calibrator v1 state file"),
        }
    } else {
        info!("No calibrator state file found, starting with fresh market learning");
    }
    if let Some((total, correct, incorrect)) =
        load_persisted_prediction_counters(&config.persistence.data_dir)
    {
        let mut strategy_guard = strategy.lock().await;
        // Only sync counters if the dataset has samples. If the dataset is empty
        // the old counters are from a previous model session and are meaningless
        // without the training data -- syncing them would show stale accuracy on
        // the dashboard. Skip so the dashboard starts clean at 0.
        let min_samples = config.ml_engine.min_samples_for_training;
        if strategy_guard.get_ml_state().dataset_size >= min_samples {
            strategy_guard.sync_prediction_counters(total, correct, incorrect);
            strategy_guard.force_save_state();
            info!(
                total_predictions = total,
                correct_predictions = correct,
                incorrect_predictions = incorrect,
                "Synchronized ML counters from persisted paper trading state"
            );
        } else {
            info!(
                dataset = strategy_guard.get_ml_state().dataset_size,
                min_needed = min_samples,
                "Skipping counter sync - not enough data to have a trained model, starting fresh"
            );
        }
    }
    let mut risk_cfg = risk::RiskConfig::default();
    risk_cfg.max_position_size = config.risk.max_position_usdc;
    risk_cfg.max_daily_loss = config.risk.max_daily_loss_usdc;
    risk_cfg.max_total_exposure = (config.risk.max_position_usdc
        * config.risk.max_open_positions as f64)
        .max(config.risk.max_position_usdc);
    risk_cfg.min_confidence = config.strategy.min_confidence.max(0.01).min(0.99);
    risk_cfg.take_profit_pct = config.risk.take_profit_pct;
    risk_cfg.trailing_stop_pct = config.risk.trailing_stop_pct;

    // Scale confidence thresholds to match strategy min_confidence
    let mc = risk_cfg.min_confidence;
    risk_cfg.confidence_scale.low = (mc, 0.4);
    risk_cfg.confidence_scale.medium = (mc + (0.90 - mc) * 0.4, 0.7);
    risk_cfg.confidence_scale.high = (mc + (0.90 - mc) * 0.8, 1.0);

    risk_cfg.kill_switch_enabled = config.risk.kill_switch_enabled;
    risk_cfg.take_profit_pct = config.risk.checkpoint_arm_roi.max(0.005);
    risk_cfg.checkpoint_arm_roi = config.risk.checkpoint_arm_roi.max(0.005);
    risk_cfg.checkpoint_initial_floor_roi = config.risk.checkpoint_initial_floor_roi.max(0.0);
    risk_cfg.checkpoint_trail_gap_roi = config.risk.checkpoint_trail_gap_roi.max(0.002);
    risk_cfg.hard_stop_roi = config.risk.hard_stop_roi.min(-0.001);
    risk_cfg.max_trades_per_day = usize::MAX; // No daily limit — bot runs 24/7
    let live_signature_type = config.execution.signature_type;
    let live_stop_loss_pct = config.risk.hard_stop_roi.abs() * 100.0;
    let live_take_profit_pct = config.risk.checkpoint_arm_roi.max(0.0) * 100.0;
    let risk_manager = Arc::new(RiskManager::new(risk_cfg));
    let dry_run = config.bot.dry_run;
    let paper_trading_enabled = config.paper_trading.enabled;
    let clob_client = Arc::new(ClobClient::with_dry_run(config.execution.clone(), dry_run));
    let mut live_ready = false;
    // Derive L2 HMAC credentials from PRIVATE_KEY so get_balance() / order submission work
    if !dry_run {
        if let Err(e) = clob_client.initialize().await {
            if paper_trading_enabled {
                warn!(error = %e, "⚠️  CLOB L2 init failed — LIVE mode toggle will stay disabled");
            } else {
                return Err(e).context("CLOB L2 init failed while starting in LIVE mode");
            }
        } else {
            live_ready = true;
            info!("🔑 CLOB L2 credentials initialized (live balance + order submission ready)");
            clob_client.clone().start_heartbeat_loop();
        }
    }
    if paper_trading_enabled {
        info!(
            "📋 PAPER TRADING mode enabled - virtual balance: ${:.2}",
            config.paper_trading.initial_balance
        );
    } else if dry_run {
        info!("🧪 DRY_RUN mode enabled - no real orders will be submitted");
    } else {
        warn!("⚠️ LIVE mode enabled - real orders will be submitted!");
    }
    let csv_persistence = Arc::new(CsvPersistence::new(&config.persistence.data_dir)?);
    let balance_tracker = Arc::new(BalanceTracker::new());
    if !loaded_calibrator_state {
        info!("Fresh calibrator start: bootstrap from Binance is disabled");
    } // ── Dashboard API (optional, only with feature flag) ─────────────
    #[cfg(feature = "dashboard")]
    let (dashboard_memory, dashboard_broadcaster) = {
        let memory = std::sync::Arc::new(dashboard::DashboardMemory::new(
            config.paper_trading.initial_balance,
        ));
        // Set initial trading mode from config (true = paper, false = live)
        memory
            .trading_mode
            .store(paper_trading_enabled, std::sync::atomic::Ordering::SeqCst);
        memory
            .live_ready
            .store(live_ready, std::sync::atomic::Ordering::SeqCst);
        let broadcaster = dashboard::WebSocketBroadcaster::new(100);
        (memory, broadcaster)
    };
    #[cfg(feature = "dashboard")]
    {
        let strat = strategy.lock().await;
        let snapshot = strat.export_calibrator_state_v2();
        let quality = strat.export_calibration_quality_by_market();
        let initial_ml_metrics = strat.get_ml_state();
        let initial_dataset_stats = strat.get_dataset_stats();
        drop(strat);
        dashboard_memory.set_market_learning_stats(snapshot).await;
        dashboard_memory
            .set_calibration_quality_stats(quality)
            .await;
        dashboard_memory.update_ml_metrics(initial_ml_metrics).await;
        dashboard_memory
            .update_ml_dataset_stats(initial_dataset_stats)
            .await;
        tracing::info!("📊 Dashboard seeded with initial ML metrics");
    }
    #[cfg(feature = "dashboard")]
    let dashboard_handle = {
        let memory = dashboard_memory.clone();
        let broadcaster = dashboard_broadcaster.clone();
        let csv_persistence_clone = csv_persistence.clone();
        let reset_executed = _startup_reset_executed;
        let data_dir_for_server = config.persistence.data_dir.clone();
        tokio::spawn(async move {
            info!("Dashboard spawn started - initializing server...");
            if reset_executed {
                info!("[1/6] Startup reset executed; skipping dashboard historical bootstrap");
            } else {
                // Load historical paper trades on startup
                info!("[1/6] Loading historical paper trades...");
                match csv_persistence_clone.load_recent_paper_trades(10_000) {
                    Ok(trades) => {
                        info!("[2/6] Loaded {} trades from CSV", trades.len());
                        if !trades.is_empty() {
                            info!("Loaded {} historical paper trades", trades.len());
                            memory.set_paper_trades(trades).await;
                            info!("[2b/6] set_paper_trades completed");
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to load historical paper trades");
                    }
                }
                info!("[2c/6] Loading historical live trades...");
                match csv_persistence_clone.load_recent_live_trades(10_000) {
                    Ok(trades) => {
                        if !trades.is_empty() {
                            info!("Loaded {} historical live trades", trades.len());
                            memory.set_live_trades(trades).await;
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to load historical live trades");
                    }
                }
                info!("[3/6] Loading recent BTC/ETH price history...");
                match csv_persistence_clone
                    .load_recent_price_history(&[Asset::BTC, Asset::ETH], 86_400)
                {
                    Ok(rows) => {
                        info!("[4/6] Loaded {} rows for chart bootstrap", rows.len());
                        for (asset, timestamp, price, source) in rows {
                            memory
                                .seed_price_history_point(asset, price, source, timestamp)
                                .await;
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to load recent price history for dashboard bootstrap");
                    }
                }
            } // Heartbeat helps the frontend detect stale connections.
            let heartbeat_broadcaster = broadcaster.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
                loop {
                    interval.tick().await;
                    heartbeat_broadcaster.broadcast_heartbeat();
                }
            });
            let dashboard_port: u16 = std::env::var("DASHBOARD_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000);
            info!(
                "[5/6] Starting dashboard server on port {}...",
                dashboard_port
            );
            match dashboard::start_server(memory, broadcaster, dashboard_port, data_dir_for_server)
                .await
            {
                Ok(()) => {
                    info!("[6/6] start_server returned successfully (unexpected)");
                }
                Err(e) => {
                    error!(error = %e, "Dashboard server failed to start");
                }
            }
        })
    }; // Paper trading engine (only used when paper_trading.enabled = true)
       // Create share price provider that will be populated by orderbook feed
    let polymarket_share_prices = std::sync::Arc::new(paper_trading::PolymarketSharePrices::new());
    let share_prices_for_orderbook = polymarket_share_prices.clone();
    let paper_engine = if paper_trading_enabled {
        let pt_config = PaperTradingConfig {
            initial_balance: config.paper_trading.initial_balance,
            slippage_bps: config.paper_trading.slippage_bps,
            fee_bps: config.paper_trading.fee_bps,
            trailing_stop_pct: config.paper_trading.trailing_stop_pct,
            take_profit_pct: config.paper_trading.take_profit_pct,
            max_hold_duration_ms: config.paper_trading.max_hold_duration_ms,
            dashboard_interval_secs: config.paper_trading.dashboard_interval_secs,
            prefer_chainlink: config.paper_trading.prefer_chainlink,
            native_only: config
                .polymarket
                .data_mode
                .eq_ignore_ascii_case("native_only"),
            checkpoint_arm_roi: config.risk.checkpoint_arm_roi,
            checkpoint_initial_floor_roi: config.risk.checkpoint_initial_floor_roi,
            checkpoint_trail_gap_roi: config.risk.checkpoint_trail_gap_roi,
            hard_stop_roi: config.risk.hard_stop_roi,
            time_stop_seconds_to_expiry: config.risk.time_stop_seconds_to_expiry,
            time_stop_seconds_to_expiry_15m: config.risk.time_stop_seconds_to_expiry_15m,
            kelly_enabled: config.kelly.enabled,
            kelly_fraction_15m: config.kelly.fraction_15m,
            kelly_fraction_1h: config.kelly.fraction_1h,
            kelly_cap_15m: config.kelly.max_bankroll_fraction_15m,
            kelly_cap_1h: config.kelly.max_bankroll_fraction_1h,
            min_edge_net: config.paper_trading.min_edge_net,
            // NUEVO v2.0: Stops adaptativos y sizing
            hard_stop_atr_multiplier: 1.5,
            adaptive_stops_enabled: true,
            eth_size_multiplier: 0.8,
            atr_period: 14,
        }; // State file path for persistence
        let state_file =
            std::path::PathBuf::from(&config.persistence.data_dir).join("paper_trading_state.json");
        let engine = PaperTradingEngine::new(pt_config)
            .with_persistence(csv_persistence.clone())
            .with_state_file(state_file)
            .with_polymarket_share_prices(polymarket_share_prices.clone()); // Load previous state if exists
        let engine = Arc::new(engine);
        if let Err(e) = engine.load_state() {
            warn!(error = %e, "Failed to load paper trading state, starting fresh");
        }
        info!("📋 [PAPER] Connected to real Polymarket share prices via orderbook feed"); // ── Connect calibration callback via channel ──
                                                                                          // This allows the paper engine to update indicator weights when trades close
                                                                                          // We use a channel because the callback is sync but strategy uses async Mutex
        let (calibration_tx, mut calibration_rx) =
            mpsc::channel::<(Asset, Timeframe, Vec<String>, bool, f64)>(100);
        let strategy_for_calibration = strategy.clone();
        let calibrator_save_path = calibrator_state_file_v2.clone(); // Spawn task to process calibration events and persist to disk
        tokio::spawn(async move {
            while let Some((asset, timeframe, indicators, is_win, p_model)) =
                calibration_rx.recv().await
            {
                let result = if is_win {
                    TradeResult::Win
                } else {
                    TradeResult::Loss
                };
                let mut s = strategy_for_calibration.lock().await;
                s.record_trade_with_indicators_for_market(asset, timeframe, &indicators, result);
                s.record_prediction_outcome_for_market(asset, timeframe, p_model, is_win); // Save calibrator state to disk after each trade
                let stats = s.export_calibrator_state_v2();
                let total_trades = s.calibrator_total_trades();
                let is_calibrated = s.is_calibrated();
                drop(s); // Persist to JSON file
                match serde_json::to_string_pretty(&stats) {
                    Ok(json) => {
                        if let Err(e) = std::fs::write(&calibrator_save_path, &json) {
                            warn!(error = %e, "Failed to save calibrator state");
                        }
                    }
                    Err(e) => warn!(error = %e, "Failed to serialize calibrator state"),
                }
                info!(                    asset = ?asset,                    timeframe = ?timeframe,                    is_win = is_win,                    p_model = p_model,                    indicators_count = indicators.len(),                    total_trades = total_trades,                    calibrated = is_calibrated,                    "🧠 [CALIBRATION] Recorded trade result & saved to disk"                );
            }
        });
        let calibration_callback: std::sync::Arc<paper_trading::CalibrationCallback> =
            std::sync::Arc::new(Box::new(
                move |_asset: Asset,
                      timeframe: Timeframe,
                      indicators: &[String],
                      is_win: bool,
                      p_model: f64| {
                    let _ = calibration_tx.try_send((
                        _asset,
                        timeframe,
                        indicators.to_vec(),
                        is_win,
                        p_model,
                    ));
                },
            ) as paper_trading::CalibrationCallback); // Recreate engine with callback
        let pt_config = PaperTradingConfig {
            initial_balance: config.paper_trading.initial_balance,
            slippage_bps: config.paper_trading.slippage_bps,
            fee_bps: config.paper_trading.fee_bps,
            trailing_stop_pct: config.paper_trading.trailing_stop_pct,
            take_profit_pct: config.paper_trading.take_profit_pct,
            max_hold_duration_ms: config.paper_trading.max_hold_duration_ms,
            dashboard_interval_secs: config.paper_trading.dashboard_interval_secs,
            prefer_chainlink: config.paper_trading.prefer_chainlink,
            native_only: config
                .polymarket
                .data_mode
                .eq_ignore_ascii_case("native_only"),
            checkpoint_arm_roi: config.risk.checkpoint_arm_roi,
            checkpoint_initial_floor_roi: config.risk.checkpoint_initial_floor_roi,
            checkpoint_trail_gap_roi: config.risk.checkpoint_trail_gap_roi,
            hard_stop_roi: config.risk.hard_stop_roi,
            time_stop_seconds_to_expiry: config.risk.time_stop_seconds_to_expiry,
            time_stop_seconds_to_expiry_15m: config.risk.time_stop_seconds_to_expiry_15m,
            kelly_enabled: config.kelly.enabled,
            kelly_fraction_15m: config.kelly.fraction_15m,
            kelly_fraction_1h: config.kelly.fraction_1h,
            kelly_cap_15m: config.kelly.max_bankroll_fraction_15m,
            kelly_cap_1h: config.kelly.max_bankroll_fraction_1h,
            min_edge_net: config.paper_trading.min_edge_net,
            // NUEVO v2.0: Stops adaptativos y sizing
            hard_stop_atr_multiplier: 1.5,
            adaptive_stops_enabled: true,
            eth_size_multiplier: 0.8,
            atr_period: 14,
        };
        let state_file =
            std::path::PathBuf::from(&config.persistence.data_dir).join("paper_trading_state.json");
        let mut new_engine = PaperTradingEngine::new(pt_config)
            .with_persistence(csv_persistence.clone())
            .with_state_file(state_file)
            .with_polymarket_share_prices(polymarket_share_prices.clone())
            .with_calibration_callback(calibration_callback); // Copy state from old engine
        if let Err(e) = new_engine.load_state() {
            warn!(error = %e, "Failed to load paper trading state in new engine");
        }
        Some(Arc::new(new_engine))
    } else {
        None
    };
    #[cfg(feature = "dashboard")]
    if let Some(ref engine) = paper_engine {
        let stats = engine.get_stats();
        let balance = engine.get_balance();
        let locked = engine.get_locked_balance();
        let equity = engine.get_total_equity();
        let unrealized = equity - balance - locked;
        *dashboard_memory.paper_balance.write().await = balance;
        *dashboard_memory.paper_locked.write().await = locked;
        *dashboard_memory.paper_unrealized_pnl.write().await = unrealized;
        *dashboard_memory.paper_peak_balance.write().await = stats.peak_balance;
        let stats_response = PaperStatsResponse {
            total_trades: stats.total_trades,
            wins: stats.wins,
            losses: stats.losses,
            win_rate: if stats.total_trades > 0 {
                (stats.wins as f64 / stats.total_trades as f64) * 100.0
            } else {
                0.0
            },
            total_pnl: stats.total_pnl,
            total_fees: stats.total_fees,
            largest_win: stats.largest_win,
            largest_loss: stats.largest_loss,
            avg_win: if stats.wins > 0 {
                stats.sum_win_pnl / stats.wins as f64
            } else {
                0.0
            },
            avg_loss: if stats.losses > 0 {
                stats.sum_loss_pnl / stats.losses as f64
            } else {
                0.0
            },
            max_drawdown: stats.max_drawdown,
            current_drawdown: {
                let peak = stats.peak_balance;
                if peak > 0.0 {
                    ((peak - equity) / peak * 100.0).max(0.0)
                } else {
                    0.0
                }
            },
            peak_balance: stats.peak_balance,
            profit_factor: if stats.gross_loss > 0.0 {
                stats.gross_profit / stats.gross_loss
            } else if stats.gross_profit > 0.0 {
                f64::INFINITY
            } else {
                0.0
            },
            current_streak: stats.current_streak,
            best_streak: stats.best_streak,
            worst_streak: stats.worst_streak,
            exits_trailing_stop: stats.exits_trailing_stop,
            exits_take_profit: stats.exits_take_profit,
            exits_market_expiry: stats.exits_market_expiry,
            exits_time_expiry: stats.exits_time_expiry,
            predictions_correct: stats.predictions_correct,
            predictions_incorrect: stats.predictions_incorrect,
            prediction_win_rate: if stats.total_trades > 0 {
                (stats.predictions_correct as f64 / stats.total_trades as f64) * 100.0
            } else {
                0.0
            },
            trading_wins: stats.trading_wins,
            trading_losses: stats.trading_losses,
            trading_win_rate: if stats.trading_wins + stats.trading_losses > 0 {
                (stats.trading_wins as f64 / (stats.trading_wins + stats.trading_losses) as f64)
                    * 100.0
            } else {
                0.0
            },
        };
        *dashboard_memory.paper_stats.write().await = stats_response.clone();
        let positions: Vec<PositionResponse> = engine
            .get_positions()
            .into_iter()
            .map(|p| {
                let trading_roi = if p.size_usdc > 0.0 {
                    p.unrealized_pnl / p.size_usdc * 100.0
                } else {
                    0.0
                };
                PositionResponse {
                    id: p.id.clone(),
                    asset: format!("{:?}", p.asset),
                    timeframe: format!("{:?}", p.timeframe),
                    direction: format!("{:?}", p.direction),
                    entry_price: if p.price_at_market_open > 0.0 {
                        p.price_at_market_open
                    } else {
                        p.entry_price
                    },
                    current_price: p.current_price,
                    size_usdc: p.size_usdc,
                    pnl: p.unrealized_pnl,
                    pnl_pct: trading_roi, // token-based, consistent with pnl
                    opened_at: p.opened_at,
                    market_slug: p.market_slug.clone(),
                    confidence: p.confidence,
                    peak_price: p.peak_price,
                    trough_price: p.trough_price,
                    market_close_ts: p.market_close_ts,
                    time_remaining_secs: ((p.market_close_ts
                        - chrono::Utc::now().timestamp_millis())
                        / 1000)
                        .max(0),
                    stop_loss_pct: p.dynamic_hard_stop_roi.abs() * 100.0,
                    take_profit_pct:
                        crate::paper_trading::PaperTradingEngine::dynamic_take_profit_roi(
                            &p,
                            chrono::Utc::now().timestamp_millis(),
                        ) * 100.0,
                    checkpoint_armed: p.checkpoint_armed,
                    checkpoint_floor_pct: p.checkpoint_floor_roi * 100.0,
                    checkpoint_peak_pct: p.checkpoint_peak_roi * 100.0,
                    trading_roi,
                    prediction_roi: p.prediction_roi * 100.0,
                    entry_share_price: p.share_price,
                    current_share_price: p.current_share_price,
                }
            })
            .collect();
        *dashboard_memory.paper_positions.write().await = positions.clone();
        dashboard_broadcaster.broadcast_positions(positions);
        dashboard_broadcaster.broadcast_stats(stats_response);
    }
    info!("✅ All components initialized"); // ── Polymarket Orderbook Feed (WebSocket) ─────────────────────────────────
                                            // Dynamic Orderbook Feed with automatic market discovery
                                            // Automatically detects expired markets and connects to new ones
    let orderbook_feed_tracker = orderbook_tracker.clone();
    let orderbook_share_prices = share_prices_for_orderbook.clone();
    let gamma_url = config.execution.gamma_url.clone();

    let orderbook_feed_handle = tokio::spawn(async move {
        use crate::clob::DynamicOrderbookFeed;

        let feed =
            DynamicOrderbookFeed::new(gamma_url, orderbook_feed_tracker, orderbook_share_prices);

        feed.run().await;
    }); // Spawn oracle sources (native mode = RTDS only, otherwise RTDS + optional Binance).
    let oracle_event_tx = price_tx.clone();
    let oracle_paper_tx = paper_price_tx.clone();
    let oracle_paper_enabled = paper_trading_enabled;
    let oracle_assets = config.bot.assets.clone();
    let oracle_persistence = csv_persistence.clone();
    let oracle_cfg = config.oracle.clone();
    let polymarket_cfg = config.polymarket.clone();
    let oracle_last_tick_ts = last_tick_ts.clone();
    #[cfg(feature = "dashboard")]
    let oracle_dashboard_memory = dashboard_memory.clone();
    #[cfg(feature = "dashboard")]
    let oracle_dashboard_broadcaster = dashboard_broadcaster.clone();
    let oracle_handle = tokio::spawn(async move {
        use crate::oracle::sources::{BinanceClient, PriceSource as _, RtdsClient, SourceEvent};
        use crate::persistence::PriceRecord;
        let assets: Vec<Asset> = oracle_assets
            .iter()
            .filter_map(|s| Asset::from_str(s))
            .collect();
        let native_only = polymarket_cfg.data_mode.eq_ignore_ascii_case("native_only");
        let enable_rtds = oracle_cfg.rtds_enabled && polymarket_cfg.rtds.enabled;
        let enable_binance = oracle_cfg.binance_enabled && !native_only; // Create event channel for oracle (shared between Binance and RTDS)
        let (event_tx, mut event_rx) = mpsc::channel::<SourceEvent>(1000);
        if !enable_binance && !enable_rtds {
            tracing::warn!(
                "No oracle source enabled; set oracle.rtds_enabled=true for native mode"
            );
            return;
        } // Spawn Binance connection only when native_only is disabled.
        let binance_handle = if enable_binance {
            let binance_tx = event_tx.clone();
            let binance_assets = assets.clone();
            Some(tokio::spawn(async move {
                let mut client = BinanceClient::new();
                if let Err(e) = client.subscribe(&binance_assets).await {
                    tracing::error!(error = %e, "Binance subscribe failed");
                    return;
                }
                if let Err(e) = client.connect(binance_tx).await {
                    tracing::error!(error = %e, "Binance connection failed");
                }
            }))
        } else {
            tracing::info!("Binance feed disabled by polymarket.data_mode=native_only");
            None
        }; // Spawn RTDS connection (Polymarket feed).
        let rtds_handle = if enable_rtds {
            let rtds_tx = event_tx.clone();
            let rtds_assets = assets.clone();
            Some(tokio::spawn(async move {
                let mut client = RtdsClient::new();
                if let Err(e) = client.subscribe(&rtds_assets).await {
                    tracing::error!(error = %e, "RTDS subscribe failed");
                    return;
                }
                if let Err(e) = client.connect(rtds_tx).await {
                    tracing::error!(error = %e, "RTDS connection failed");
                }
            }))
        } else {
            tracing::warn!("RTDS feed disabled; native Polymarket data will be unavailable");
            None
        };
        let mut tick_count: u64 = 0;
        #[cfg(feature = "dashboard")]
        let mut last_live_price_broadcast: i64 = 0;
        // Process events and convert to PriceTicks
        while let Some(event) = event_rx.recv().await {
            match event {
                SourceEvent::Tick(tick) => {
                    tick_count += 1;
                    if tick_count % 100 == 0 {
                        tracing::info!(                            count = tick_count,                            asset = ?tick.asset,                            source = ?tick.source,                            "📈 Received price tick"                        );
                    }
                    let price_tick = PriceTick {
                        exchange_ts: tick.ts,
                        local_ts: chrono::Utc::now().timestamp_millis(),
                        asset: tick.asset,
                        bid: tick.bid,
                        ask: tick.ask,
                        mid: tick.mid,
                        source: tick.source,
                        latency_ms: tick.latency_ms as u64,
                    };
                    let received_ts = chrono::Utc::now().timestamp_millis();
                    {
                        let mut tick_map = oracle_last_tick_ts.lock().await;
                        tick_map.insert(tick.asset, received_ts);
                    }
                    #[cfg(feature = "dashboard")]
                    oracle_dashboard_memory
                        .record_asset_tick(tick.asset, tick.source, received_ts, staleness_gate_ms)
                        .await;
                    // In live mode (paper disabled), update dashboard price history directly
                    // since paper_price_rx loop won't be running.
                    #[cfg(feature = "dashboard")]
                    if !oracle_paper_enabled {
                        oracle_dashboard_memory
                            .update_price_at(
                                tick.asset,
                                tick.mid,
                                tick.bid,
                                tick.ask,
                                tick.source,
                                tick.ts,
                            )
                            .await;
                        // Broadcast price updates throttled to ~2s to avoid UI flicker
                        let now_ms = chrono::Utc::now().timestamp_millis();
                        if now_ms - last_live_price_broadcast >= 2_000 {
                            last_live_price_broadcast = now_ms;
                            oracle_dashboard_broadcaster
                                .broadcast_prices(oracle_dashboard_memory.get_prices().await.prices);
                        }
                    } // Forward to paper trading engine if enabled
                    if oracle_paper_enabled {
                        let _ = oracle_paper_tx.send(price_tick.clone()).await;
                    }
                    if let Err(e) = oracle_event_tx.send(price_tick).await {
                        tracing::error!(error = %e, "Failed to send price tick");
                    } // Save price to CSV every 100 ticks (sampling)
                    if tick_count % 100 == 0 {
                        let record = PriceRecord {
                            timestamp: tick.ts,
                            asset: format!("{:?}", tick.asset),
                            price: tick.mid,
                            source: format!("{:?}", tick.source),
                            volume: None,
                        };
                        if let Err(e) = oracle_persistence.save_price(record).await {
                            tracing::warn!(error = %e, "Failed to save price to CSV");
                        }
                    }
                }
                SourceEvent::Connected(source) => {
                    tracing::info!(source = %source, "🔌 Oracle source connected");
                    #[cfg(feature = "dashboard")]
                    if source.eq_ignore_ascii_case("RTDS") {
                        oracle_dashboard_memory
                            .record_source_connection("RTDS", true)
                            .await;
                    }
                }
                SourceEvent::Disconnected(source) => {
                    tracing::warn!(source = %source, "🔌 Oracle source disconnected");
                    #[cfg(feature = "dashboard")]
                    if source.eq_ignore_ascii_case("RTDS") {
                        oracle_dashboard_memory
                            .record_source_connection("RTDS", false)
                            .await;
                    }
                }
                SourceEvent::Comment(comment) => {
                    tracing::debug!(                        topic = %comment.topic,                        username = ?comment.username,                        symbol = ?comment.symbol,                        body = ?comment.body,                        "RTDS comment event received"                    );
                }
                SourceEvent::Error(source, err) => {
                    tracing::error!(source = %source, error = %err, "Oracle error");
                    #[cfg(feature = "dashboard")]
                    if source.eq_ignore_ascii_case("RTDS") {
                        oracle_dashboard_memory
                            .record_source_connection("RTDS", false)
                            .await;
                    }
                }
                _ => {}
            }
        }
        if let Some(handle) = binance_handle {
            handle.abort();
        }
        if let Some(handle) = rtds_handle {
            handle.abort();
        }
    }); // Feature engine task - processes price ticks and generates features
    let feature_engine_inner = feature_engine.clone();
    let feature_native_only = config
        .polymarket
        .data_mode
        .eq_ignore_ascii_case("native_only");
    let feature_clob_client = clob_client.clone();
    let feature_data_dir = config.persistence.data_dir.clone();
    let feature_handle = tokio::spawn(async move {
        use crate::clob::PriceHistoryInterval;
        use crate::oracle::sources::BinanceClient;
        use crate::oracle::CandleBuilder;
        use std::collections::HashMap;
        let mut candle_builder = CandleBuilder::new(500);
        let mut last_feature_time: HashMap<(Asset, Timeframe), i64> = HashMap::new();
        let mut tick_count: u64 = 0; // Fetch historical candles at startup only when native-only mode is disabled.
        if !feature_native_only {
            tracing::info!("Fetching historical candles from Binance...");
            for asset in [Asset::BTC, Asset::ETH] {
                for timeframe in [Timeframe::Min15, Timeframe::Hour1] {
                    match BinanceClient::fetch_historical_candles(asset, timeframe, 100).await {
                        Ok(candles) => {
                            let count = candles.len();
                            candle_builder.seed_history(candles);
                            tracing::info!(                                asset = ?asset,                                timeframe = ?timeframe,                                candle_count = count,                                "Seeded historical candles"                            );
                        }
                        Err(e) => {
                            tracing::warn!(                                asset = ?asset,                                timeframe = ?timeframe,                                error = %e,                                "Failed to fetch historical candles, will build from live data"                            );
                        }
                    }
                }
            }
            tracing::info!("Historical candle fetch complete");
        } else {
            tracing::info!("Native-only mode: bootstrapping from Polymarket historical data");
            if let Err(e) = feature_clob_client.refresh_markets().await {
                tracing::warn!(                    error = %e,                    "Failed to refresh markets before Polymarket warmup bootstrap"                );
            }
            let native_targets: [(&str, Asset, Timeframe); 4] = [
                ("btc-15m", Asset::BTC, Timeframe::Min15),
                ("btc-1h", Asset::BTC, Timeframe::Hour1),
                ("eth-15m", Asset::ETH, Timeframe::Min15),
                ("eth-1h", Asset::ETH, Timeframe::Hour1),
            ];
            for (slug, asset, timeframe) in native_targets {
                let local_points = load_local_price_points(&feature_data_dir, asset, 96);
                let mut merged = build_candles_from_points(asset, timeframe, &local_points);
                let local_count = merged.len();
                let anchor = local_points
                    .last()
                    .map(|(_, price)| *price)
                    .unwrap_or_else(|| default_anchor_price(asset));
                if local_count < 30 {
                    match bootstrap_polymarket_history_candles(
                        feature_clob_client.as_ref(),
                        slug,
                        asset,
                        timeframe,
                        anchor,
                        match timeframe {
                            Timeframe::Min15 => PriceHistoryInterval::OneDay,
                            Timeframe::Hour1 => PriceHistoryInterval::OneWeek,
                        },
                    )
                    .await
                    {
                        Ok(mut candles) => {
                            let remote_count = candles.len();
                            if remote_count > 0 {
                                merged.append(&mut candles);
                                merged = dedup_candles_by_open_time(merged);
                            }
                            tracing::info!(                                market_slug = slug,                                asset = ?asset,                                timeframe = ?timeframe,                                local_candles = local_count,                                polymarket_candles = remote_count,                                total_candles = merged.len(),                                "Native warmup candles prepared"                            );
                        }
                        Err(e) => {
                            tracing::warn!(                                market_slug = slug,                                asset = ?asset,                                timeframe = ?timeframe,                                local_candles = local_count,                                error = %e,                                "Failed to bootstrap Polymarket history for native warmup"                            );
                        }
                    }
                } else {
                    tracing::info!(                        market_slug = slug,                        asset = ?asset,                        timeframe = ?timeframe,                        local_candles = local_count,                        "Native warmup satisfied from local RTDS history"                    );
                }
                if !merged.is_empty() {
                    let keep = 200usize;
                    let mut seeded = merged;
                    if seeded.len() > keep {
                        seeded = seeded.split_off(seeded.len() - keep);
                    }
                    let seeded_count = seeded.len();
                    candle_builder.seed_history(seeded);
                    tracing::info!(                        market_slug = slug,                        asset = ?asset,                        timeframe = ?timeframe,                        seeded_candles = seeded_count,                        "Seeded native warmup candles"                    );
                } else {
                    tracing::warn!(                        market_slug = slug,                        asset = ?asset,                        timeframe = ?timeframe,                        "No historical candles available for native warmup"                    );
                }
            }
        } // Warmup indicators from seeded candles when available.
        tracing::info!("Warming up FeatureEngine from historical candles...");
        for asset in [Asset::BTC, Asset::ETH] {
            for timeframe in [Timeframe::Min15, Timeframe::Hour1] {
                let candles = candle_builder.get_last_n(asset, timeframe, 100);
                let n = candles.len();
                if n < 30 {
                    tracing::warn!(                        asset = ?asset,                        timeframe = ?timeframe,                        candle_count = n,                        "Not enough historical candles for warmup (need 30+)"                    );
                    continue;
                } // Progressively feed increasing slices to build stateful indicators.
                let mut fe = feature_engine_inner.lock().await;
                let start = 15.min(n);
                for end in start..=n {
                    let slice = &candles[..end];
                    fe.compute(slice);
                }
                drop(fe);
                tracing::info!(                    asset = ?asset,                    timeframe = ?timeframe,                    warmup_steps = n - start + 1,                    "FeatureEngine warmed up ({} candles replayed)",                    n                );
            }
        }
        tracing::info!("FeatureEngine warmup complete");
        while let Some(tick) = price_rx.recv().await {
            tick_count += 1;
            if tick_count % 100 == 0 {
                tracing::debug!(count = tick_count, asset = ?tick.asset, "🔧 Feature task received ticks");
            } // Convert PriceTick to NormalizedTick for candle builder
            let normalized = crate::oracle::NormalizedTick {
                ts: tick.exchange_ts,
                asset: tick.asset,
                bid: tick.bid,
                ask: tick.ask,
                mid: tick.mid,
                source: tick.source,
                latency_ms: tick.latency_ms as u64,
            }; // Process each timeframe
            for timeframe in [Timeframe::Min15, Timeframe::Hour1] {
                // Add tick to candle builder for this timeframe
                candle_builder.add_tick(&normalized, timeframe); // Get candles (returns Vec<Candle>, not Option)
                let candles = candle_builder.get_last_n(tick.asset, timeframe, 50);
                let candle_count = candles.len(); // Log candle count periodically (every 10 seconds per asset/timeframe)
                let now = chrono::Utc::now().timestamp_millis();
                let key = (tick.asset, timeframe);
                let last = last_feature_time.entry(key).or_insert(0);
                if now - *last > 10000 {
                    *last = now;
                    tracing::info!(                        asset = ?tick.asset,                        timeframe = ?timeframe,                        candle_count = candle_count,                        "🕯️ Candle count"                    );
                } // Need at least 30 candles for meaningful technical indicators
                if candle_count >= 30 {
                    if let Some(mut features) = feature_engine_inner.lock().await.compute(&candles)
                    {
                        // Let's enrich the features with Polymarket volume and liquidity context
                        if let Some(market) = feature_clob_client
                            .find_tradeable_market_for_signal(tick.asset, timeframe)
                            .await
                        {
                            features.polymarket_volume_24hr = Some(market.volume_24hr);
                            features.polymarket_liquidity = Some(market.liquidity_num);
                        }

                        // Log features at DEBUG level to avoid log spam
                        // ALWAYS send features to strategy (V3 handles partial features)
                        // Log indicator availability for debugging
                        if features.rsi.is_none() && features.macd.is_none() {
                            tracing::warn!(
                                asset = ?tick.asset,
                                timeframe = ?timeframe,
                                candle_count = candle_count,
                                "⚠️ Features computed but RSI/MACD are None - sending anyway"
                            );
                        } else {
                            tracing::debug!(
                                asset = ?tick.asset,
                                timeframe = ?timeframe,
                                rsi = ?features.rsi,
                                macd = ?features.macd,
                                momentum = ?features.momentum,
                                trend = ?features.trend_strength,
                                vol_24h = ?features.polymarket_volume_24hr,
                                "📊 Features computed with indicators"
                            );
                        }
                        // Send Features directly to strategy
                        if let Err(e) = feature_tx.send(features).await {
                            tracing::error!(error = %e, "Failed to send features");
                        }
                    }
                }
            }
        }
    }); // Strategy engine task - processes features and generates signals
    let strategy_inner = strategy.clone();
    let strategy_persistence = csv_persistence.clone();
    let strategy_client = clob_client.clone(); // For market lookup
    let strategy_risk = risk_manager.clone(); // For position sizing
    let strategy_kelly_cfg = config.kelly.clone();
    let strategy_edge_floor = if paper_trading_enabled {
        config.paper_trading.min_edge_net
    } else {
        0.0
    };
    #[cfg(feature = "dashboard")]
    let strategy_dashboard_memory = dashboard_memory.clone();
    #[cfg(feature = "dashboard")]
    let strategy_dashboard_broadcaster = dashboard_broadcaster.clone(); // ML broadcast counter for real-time dashboard updates
    #[cfg(feature = "dashboard")]
    let ml_broadcast_counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    // Circuit breaker: track last feature timestamp per asset
    let last_feature_ts: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<Asset, i64>>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
    let last_feature_ts_clone = last_feature_ts.clone();
    let last_tick_ts_for_strategy = last_tick_ts.clone();
    let strategy_handle = tokio::spawn(async move {
        use crate::persistence::{RejectionRecord, SignalRecord};
        use crate::polymarket::{
            compute_fractional_kelly, estimate_expected_value, fee_rate_from_price,
        };
        while let Some(features) = feature_rx.recv().await {
            // ── CIRCUIT BREAKER: Check for stale price data ──
            let now = chrono::Utc::now().timestamp_millis();
            let last_tick_age_ms = {
                let tick_map = last_tick_ts_for_strategy.lock().await;
                tick_map
                    .get(&features.asset)
                    .copied()
                    .map(|ts| now.saturating_sub(ts))
                    .unwrap_or(i64::MAX)
            };
            if last_tick_age_ms > staleness_gate_ms {
                tracing::warn!(                    asset = ?features.asset,                    tick_age_ms = last_tick_age_ms,                    stale_after_ms = staleness_gate_ms,                    "Stale RTDS ticks detected - proceeding with signal generation anyway"                );
                let _ = strategy_persistence
                    .save_rejection(RejectionRecord {
                        timestamp: features.ts,
                        signal_id: format!(
                            "{}-{:?}-{:?}",
                            features.ts, features.asset, features.timeframe
                        ),
                        asset: features.asset.to_string(),
                        timeframe: features.timeframe.to_string(),
                        market_slug: String::new(),
                        token_id: String::new(),
                        reason: "price_stale".to_string(),
                        spread: 0.0,
                        depth_top5: features.orderbook_depth_top5.unwrap_or(0.0),
                        p_market: 0.0,
                        p_model: 0.0,
                        edge_net: 0.0,
                    })
                    .await;
                #[cfg(feature = "dashboard")]
                {
                    // Update diagnostics but don't reject execution
                    let mut diagnostics = strategy_dashboard_memory
                        .execution_diagnostics
                        .write()
                        .await;
                    diagnostics
                        .stale_assets
                        .insert(features.asset.to_string(), true);
                }
                // continue; // Commented out to prevent skipping signals
            }
            #[cfg(feature = "dashboard")]
            {
                let mut diagnostics = strategy_dashboard_memory
                    .execution_diagnostics
                    .write()
                    .await;
                diagnostics
                    .stale_assets
                    .insert(features.asset.to_string(), false);
            }
            {
                let mut ts_map = last_feature_ts_clone.lock().await;
                let last_ts = ts_map.get(&features.asset).copied().unwrap_or(0);
                ts_map.insert(features.asset, now); // If last feature was more than 60 seconds ago, we had a gap
                if last_ts > 0 && now - last_ts > 180_000 {
                    tracing::warn!(                        asset = ?features.asset,                        gap_ms = now - last_ts,                        "⚠️ Price data gap detected - proceeding with signal generation anyway"                    );
                    // continue;
                }
            } // Process features and potentially generate signal (using global strategy for calibration)
            let (generated_signal, strategy_filter_reason) = {
                let mut guard = strategy_inner.lock().await;
                let generated = guard.process(&features);
                let reason = if generated.is_none() {
                    guard.last_filter_reason()
                } else {
                    None
                };
                (generated, reason)
            };
            #[cfg(feature = "dashboard")]
            strategy_dashboard_memory
                .record_strategy_evaluation(
                    generated_signal.is_some(),
                    strategy_filter_reason.clone(),
                )
                .await;
            if let Some(signal) = generated_signal {
                let model_prob_up = signal
                    .model_prob_up
                    .unwrap_or_else(|| {
                        if signal.direction == Direction::Up {
                            signal.confidence
                        } else {
                            1.0 - signal.confidence
                        }
                    })
                    .clamp(0.01, 0.99);
                let signal_prob_side = if signal.direction == Direction::Up {
                    model_prob_up
                } else {
                    1.0 - model_prob_up
                };
                tracing::info!(                    asset = ?signal.asset,                    direction = ?signal.direction,                    confidence = %signal.confidence,                    reasons = ?signal.reasons,                    "🎯 Signal generated!"                ); // Broadcast ML prediction to dashboard in real-time
                #[cfg(feature = "dashboard")]
                {
                    let prob_up = model_prob_up;
                    let features_triggered: Vec<String> =
                        signal.reasons.iter().take(5).cloned().collect();
                    strategy_dashboard_broadcaster.broadcast_ml_prediction(
                        &format!("{:?}", signal.asset),
                        &format!("{:?}", signal.timeframe),
                        &format!("{:?}", signal.direction),
                        signal.confidence,
                        prob_up,
                        "Ensemble",
                        features_triggered,
                    );

                    // Broadcast ML metrics en tiempo real (cada predicción)
                    let metrics = strategy_inner.lock().await.get_ml_state();
                    strategy_dashboard_broadcaster.broadcast_ml_metrics(
                        metrics.model_accuracy,
                        metrics.win_rate,
                        metrics.loss_rate,
                        metrics.total_predictions,
                        metrics.correct_predictions,
                        metrics.incorrect_predictions,
                        metrics.model_info,
                        metrics.training_epoch,
                        metrics.dataset_size,
                    );
                }

                // Save signal to CSV
                let record = SignalRecord {
                    timestamp: signal.ts,
                    market_id: format!("{:?}-{:?}", signal.asset, signal.timeframe),
                    direction: format!("{:?}", signal.direction),
                    confidence: signal.confidence,
                    entry_price: 0.0, // Will be set on execution
                    features_hash: format!(
                        "rsi:{:.2}_macd:{:.2}",
                        features.rsi.unwrap_or(0.0),
                        features.macd.unwrap_or(0.0)
                    ),
                    token_id: None,
                    market_slug: None,
                    quote_bid: None,
                    quote_ask: None,
                    quote_mid: None,
                    quote_depth_top5: None,
                    spread: None,
                    edge_net: None,
                    rejection_reason: None,
                };
                if let Err(e) = strategy_persistence.save_signal(record).await {
                    tracing::warn!(error = %e, "Failed to save signal to CSV");
                } // Convert Features to FeatureSet for Signal
                let regime_i8 = match features.regime {
                    MarketRegime::Trending => 1,
                    MarketRegime::Ranging => 0,
                    MarketRegime::Volatile => -1,
                };
                let feature_set = FeatureSet {
                    ts: features.ts,
                    asset: features.asset,
                    timeframe: features.timeframe,
                    rsi: features.rsi.unwrap_or(50.0),
                    macd_line: features.macd.unwrap_or(0.0),
                    macd_signal: features.macd_signal.unwrap_or(0.0),
                    macd_hist: features.macd_hist.unwrap_or(0.0),
                    vwap: features.vwap.unwrap_or(0.0),
                    bb_upper: features.bb_upper.unwrap_or(0.0),
                    bb_lower: features.bb_lower.unwrap_or(0.0),
                    atr: features.atr.unwrap_or(0.0),
                    momentum: features.momentum.unwrap_or(0.0),
                    momentum_accel: features.velocity.unwrap_or(0.0),
                    book_imbalance: 0.0,
                    spread_bps: 0.0,
                    trade_intensity: 0.0,
                    ha_close: features.ha_close.unwrap_or(0.0),
                    ha_trend: features
                        .ha_trend
                        .map(|d| if d == Direction::Up { 1 } else { -1 })
                        .unwrap_or(0) as i8,
                    oracle_confidence: 1.0,
                    adx: features.adx.unwrap_or(0.0),
                    stoch_rsi: features.stoch_rsi.unwrap_or(0.5),
                    obv: features.obv.unwrap_or(0.0),
                    relative_volume: features.relative_volume.unwrap_or(1.0),
                    regime: regime_i8,
                }; // Look up market for this asset/timeframe to get expiry and token info
                let selected_market = match strategy_client
                    .find_tradeable_market_for_signal(signal.asset, signal.timeframe)
                    .await
                {
                    Some(market) => market,
                    None => {
                        tracing::warn!(                            asset = ?signal.asset,                            timeframe = ?signal.timeframe,                            "Skipping signal: no tradeable market found"                        );
                        let _ = strategy_persistence
                            .save_rejection(RejectionRecord {
                                timestamp: signal.ts,
                                signal_id: format!(
                                    "{}-{:?}-{:?}",
                                    signal.ts, signal.asset, signal.timeframe
                                ),
                                asset: signal.asset.to_string(),
                                timeframe: signal.timeframe.to_string(),
                                market_slug: String::new(),
                                token_id: String::new(),
                                reason: "market_not_found".to_string(),
                                spread: 0.0,
                                depth_top5: features.orderbook_depth_top5.unwrap_or(0.0),
                                p_market: 0.0,
                                p_model: signal_prob_side,
                                edge_net: 0.0,
                            })
                            .await;
                        #[cfg(feature = "dashboard")]
                        strategy_dashboard_memory
                            .record_execution_rejection("market_not_found")
                            .await;
                        continue;
                    }
                };
                let market_slug = selected_market
                    .slug
                    .clone()
                    .unwrap_or_else(|| selected_market.question.clone());
                let condition_id = selected_market.condition_id.clone();
                let expires_at = selected_market
                    .end_date
                    .as_ref()
                    .and_then(|d| crate::clob::ClobClient::parse_expiry_to_timestamp(d))
                    .or_else(|| {
                        selected_market
                            .end_date_iso
                            .as_ref()
                            .and_then(|d| crate::clob::ClobClient::parse_expiry_to_timestamp(d))
                    })
                    .unwrap_or(0);
                let token_id = crate::clob::ClobClient::resolve_token_id_for_direction(
                    &selected_market,
                    signal.direction,
                )
                .unwrap_or_default();
                if token_id.is_empty() {
                    tracing::warn!(                        market_slug = %market_slug,                        direction = ?signal.direction,                        "Skipping signal: token_id not found for market direction"                    );
                    let _ = strategy_persistence
                        .save_rejection(RejectionRecord {
                            timestamp: signal.ts,
                            signal_id: format!(
                                "{}-{:?}-{:?}",
                                signal.ts, signal.asset, signal.timeframe
                            ),
                            asset: signal.asset.to_string(),
                            timeframe: signal.timeframe.to_string(),
                            market_slug: market_slug.clone(),
                            token_id: String::new(),
                            reason: "token_not_found".to_string(),
                            spread: 0.0,
                            depth_top5: features.orderbook_depth_top5.unwrap_or(0.0),
                            p_market: 0.0,
                            p_model: signal_prob_side,
                            edge_net: 0.0,
                        })
                        .await;
                    #[cfg(feature = "dashboard")]
                    strategy_dashboard_memory
                        .record_execution_rejection("token_not_found")
                        .await;
                    continue;
                } // Convert to Signal type using Polymarket-native EV + Kelly sizing.
                let quote = match strategy_client.quote_token(&token_id).await {
                    Ok(q) if q.bid > 0.0 && q.ask > 0.0 && q.mid > 0.0 => q,
                    Ok(_) => {
                        tracing::warn!(                            market_slug = %market_slug,                            token_id = %token_id,                            "Skipping signal: invalid quote values"                        );
                        let _ = strategy_persistence
                            .save_rejection(RejectionRecord {
                                timestamp: signal.ts,
                                signal_id: format!(
                                    "{}-{:?}-{:?}",
                                    signal.ts, signal.asset, signal.timeframe
                                ),
                                asset: signal.asset.to_string(),
                                timeframe: signal.timeframe.to_string(),
                                market_slug: market_slug.clone(),
                                token_id: token_id.clone(),
                                reason: "quote_invalid".to_string(),
                                spread: 0.0,
                                depth_top5: 0.0,
                                p_market: 0.0,
                                p_model: signal_prob_side,
                                edge_net: 0.0,
                            })
                            .await;
                        #[cfg(feature = "dashboard")]
                        strategy_dashboard_memory
                            .record_execution_rejection("quote_invalid")
                            .await;
                        continue;
                    }
                    Err(e) => {
                        tracing::warn!(                            market_slug = %market_slug,                            token_id = %token_id,                            error = %e,                            "Skipping signal: failed to fetch token quote"                        );
                        let _ = strategy_persistence
                            .save_rejection(RejectionRecord {
                                timestamp: signal.ts,
                                signal_id: format!(
                                    "{}-{:?}-{:?}",
                                    signal.ts, signal.asset, signal.timeframe
                                ),
                                asset: signal.asset.to_string(),
                                timeframe: signal.timeframe.to_string(),
                                market_slug: market_slug.clone(),
                                token_id: token_id.clone(),
                                reason: "quote_fetch_error".to_string(),
                                spread: 0.0,
                                depth_top5: 0.0,
                                p_market: 0.0,
                                p_model: signal_prob_side,
                                edge_net: 0.0,
                            })
                            .await;
                        #[cfg(feature = "dashboard")]
                        strategy_dashboard_memory
                            .record_execution_rejection("quote_fetch_error")
                            .await;
                        continue;
                    }
                };
                let p_market = quote.mid.clamp(0.01, 0.99);
                let spread = quote.spread.max(0.0);
                let max_spread = match signal.timeframe {
                    Timeframe::Min15 => 0.15,
                    Timeframe::Hour1 => 0.25,
                };
                if spread > max_spread {
                    tracing::info!(                        market_slug = %market_slug,                        token_id = %token_id,                        spread = spread,                        max_spread = max_spread,                        "Skipping signal: spread too wide for timeframe policy"                    );
                    let _ = strategy_persistence
                        .save_rejection(RejectionRecord {
                            timestamp: signal.ts,
                            signal_id: format!(
                                "{}-{:?}-{:?}",
                                signal.ts, signal.asset, signal.timeframe
                            ),
                            asset: signal.asset.to_string(),
                            timeframe: signal.timeframe.to_string(),
                            market_slug: market_slug.clone(),
                            token_id: token_id.clone(),
                            reason: "spread_too_wide".to_string(),
                            spread,
                            depth_top5: quote.depth_top5,
                            p_market,
                            p_model: signal_prob_side,
                            edge_net: 0.0,
                        })
                        .await;
                    #[cfg(feature = "dashboard")]
                    strategy_dashboard_memory
                        .record_execution_rejection("spread_too_wide")
                        .await;
                    continue;
                }
                let min_depth_top5 = match signal.timeframe {
                    Timeframe::Min15 => 50.0,
                    Timeframe::Hour1 => 25.0,
                };
                if quote.depth_top5 > 0.0 && quote.depth_top5 < min_depth_top5 {
                    tracing::info!(                        market_slug = %market_slug,                        token_id = %token_id,                        depth_top5 = quote.depth_top5,                        min_depth_top5 = min_depth_top5,                        "Skipping signal: depth below liquidity policy"                    );
                    let _ = strategy_persistence
                        .save_rejection(RejectionRecord {
                            timestamp: signal.ts,
                            signal_id: format!(
                                "{}-{:?}-{:?}",
                                signal.ts, signal.asset, signal.timeframe
                            ),
                            asset: signal.asset.to_string(),
                            timeframe: signal.timeframe.to_string(),
                            market_slug: market_slug.clone(),
                            token_id: token_id.clone(),
                            reason: "depth_too_low".to_string(),
                            spread,
                            depth_top5: quote.depth_top5,
                            p_market,
                            p_model: signal_prob_side,
                            edge_net: 0.0,
                        })
                        .await;
                    #[cfg(feature = "dashboard")]
                    strategy_dashboard_memory
                        .record_execution_rejection("depth_too_low")
                        .await;
                    continue;
                }
                let p_model = signal_prob_side.clamp(0.01, 0.99);
                let fee_rate = fee_rate_from_price(p_market);
                let ev =
                    estimate_expected_value(p_market, p_model, p_market, fee_rate, spread, 0.005);
                if ev.edge_net <= strategy_edge_floor {
                    tracing::info!(                        market_slug = %market_slug,                        p_market = p_market,                        p_model = p_model,                        edge_net = ev.edge_net,                        edge_floor = strategy_edge_floor,                        "Skipping signal due to edge below configured floor"                    );
                    let _ = strategy_persistence
                        .save_rejection(RejectionRecord {
                            timestamp: signal.ts,
                            signal_id: format!(
                                "{}-{:?}-{:?}",
                                signal.ts, signal.asset, signal.timeframe
                            ),
                            asset: signal.asset.to_string(),
                            timeframe: signal.timeframe.to_string(),
                            market_slug: market_slug.clone(),
                            token_id: token_id.clone(),
                            reason: "edge_below_floor".to_string(),
                            spread,
                            depth_top5: quote.depth_top5,
                            p_market,
                            p_model,
                            edge_net: ev.edge_net,
                        })
                        .await;
                    #[cfg(feature = "dashboard")]
                    strategy_dashboard_memory
                        .record_execution_rejection("edge_below_floor")
                        .await;
                    continue;
                }
                let (kelly_fraction, cap) = match signal.timeframe {
                    Timeframe::Min15 => (
                        strategy_kelly_cfg.fraction_15m,
                        strategy_kelly_cfg.max_bankroll_fraction_15m,
                    ),
                    Timeframe::Hour1 => (
                        strategy_kelly_cfg.fraction_1h,
                        strategy_kelly_cfg.max_bankroll_fraction_1h,
                    ),
                };
                let kelly = compute_fractional_kelly(p_model, 0.05, p_market, kelly_fraction, cap);
                let fallback_size = strategy_risk.calculate_size_from_confidence(signal.confidence);
                let balance = strategy_risk.get_balance();
                let bankroll = if balance > 0.0 { balance } else { 1000.0 };
                let kelly_size = bankroll * kelly.f_fractional;
                let calculated_size = if strategy_kelly_cfg.enabled && kelly_size >= 1.0 {
                    kelly_size
                } else {
                    fallback_size
                };
                let sig = Signal {
                    id: uuid::Uuid::new_v4().to_string(),
                    ts: signal.ts,
                    asset: signal.asset,
                    timeframe: signal.timeframe,
                    direction: signal.direction,
                    confidence: signal.confidence,
                    model_prob_up,
                    features: feature_set,
                    strategy_id: "rules_v1".to_string(),
                    market_slug,
                    condition_id,
                    token_id,
                    expires_at,
                    suggested_size_usdc: calculated_size,
                    quote_bid: quote.bid,
                    quote_ask: quote.ask,
                    quote_mid: quote.mid,
                    quote_depth_top5: quote.depth_top5,
                    indicators_used: signal.indicators_used,
                };
                #[cfg(feature = "dashboard")]
                let dashboard_signal = crate::dashboard::SignalResponse {
                    timestamp: sig.ts,
                    signal_id: sig.id.clone(),
                    asset: format!("{:?}", sig.asset),
                    timeframe: format!("{}", sig.timeframe),
                    direction: format!("{:?}", sig.direction),
                    confidence: sig.confidence,
                    entry_price: 0.0,
                    market_slug: sig.market_slug.clone(),
                    expires_at: sig.expires_at,
                };
                if let Err(e) = signal_tx.send(sig).await {
                    tracing::error!(error = %e, "Failed to send signal");
                    #[cfg(feature = "dashboard")]
                    strategy_dashboard_memory
                        .record_execution_rejection("signal_channel_send_error")
                        .await;
                } else {
                    #[cfg(feature = "dashboard")]
                    {
                        strategy_dashboard_memory.record_execution_accept().await;
                        strategy_dashboard_memory
                            .add_signal(dashboard_signal.clone())
                            .await;
                        strategy_dashboard_broadcaster.broadcast_signal(dashboard_signal);
                    }
                }
            }
        }
    }); // Order execution task - consumes order_rx and submits to Polymarket
    let execution_client = clob_client.clone();
    let execution_handle = tokio::spawn(async move {
        if let Err(e) = execution_client.run(order_rx).await {
            error!(error = %e, "Execution client task failed");
        }
    }); // ── Live position → indicators tracking (for calibration in live mode) ──
        // When a live signal is executed, we store which indicators generated it.
        // When the position closes (detected by position monitor), we use this to
        // feed back into the calibrator — so the brain learns from live trades too.
    let live_position_indicators: Arc<
        Mutex<std::collections::HashMap<(Asset, Timeframe), (Vec<String>, f64)>>,
    > = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let live_indicators_for_monitor = live_position_indicators.clone();
    let live_indicators_for_main = live_position_indicators.clone();
    let live_contexts_path = live_position_contexts_path(&config.persistence.data_dir);
    let persisted_live_contexts = load_live_position_contexts(&live_contexts_path);
    let live_position_contexts: Arc<Mutex<std::collections::HashMap<String, LivePositionContext>>> =
        Arc::new(Mutex::new(persisted_live_contexts));
    let live_contexts_for_monitor = live_position_contexts.clone();
    let live_contexts_for_main = live_position_contexts.clone();
    // Manual close channel: dashboard sends token_id → close task executes SELL
    let (manual_close_tx, mut manual_close_rx) =
        tokio::sync::mpsc::unbounded_channel::<String>();
    #[cfg(feature = "dashboard")]
    {
        *dashboard_memory.manual_close_tx.lock().unwrap() = Some(manual_close_tx);
    }
    let live_contexts_path_for_monitor = live_contexts_path.clone();
    let live_contexts_path_for_main = live_contexts_path.clone();
    let live_pending_closes: Arc<Mutex<std::collections::HashSet<String>>> =
        Arc::new(Mutex::new(std::collections::HashSet::new()));
    let live_pending_closes_for_monitor = live_pending_closes.clone();
    let live_pending_closes_for_main = live_pending_closes.clone();
    let live_sell_retries: Arc<Mutex<std::collections::HashMap<String, u32>>> =
        Arc::new(Mutex::new(std::collections::HashMap::new()));
    let live_sell_retries_for_monitor = live_sell_retries.clone();
    let live_recently_closed: Arc<Mutex<std::collections::HashSet<String>>> =
        Arc::new(Mutex::new(std::collections::HashSet::new()));
    let live_recently_closed_for_monitor = live_recently_closed.clone();
    let live_recently_closed_for_main = live_recently_closed.clone();
    let live_window_bias: Arc<
        Mutex<std::collections::HashMap<(Asset, Timeframe, i64), Direction>>,
    > = Arc::new(Mutex::new(std::collections::HashMap::new())); // Strategy + calibrator state path for live calibration saving
    let strategy_for_live_calibration = strategy.clone();
    let calibrator_save_path_live = calibrator_state_file_v2.clone(); // Position monitoring task - fetches wallet positions for TP/SL
    let position_client = clob_client.clone();
    let position_risk = risk_manager.clone();
    let position_risk_for_monitor = position_risk.clone();
    let position_tracker = balance_tracker.clone();
    let position_csv = csv_persistence.clone();
    #[cfg(feature = "dashboard")]
    let live_dashboard_memory_for_positions = dashboard_memory.clone();
    #[cfg(feature = "dashboard")]
    let live_dashboard_broadcaster_for_positions = dashboard_broadcaster.clone();
    let redeemed_claims = Arc::new(tokio::sync::Mutex::new(
        std::collections::HashSet::<String>::new(),
    ));
    let redeemed_claims_for_monitor = redeemed_claims.clone();
    let wallet_address = std::env::var("POLYMARKET_WALLET")
        .ok()
        .or_else(|| std::env::var("POLYMARKET_ADDRESS").ok())
        .unwrap_or_default();
    let position_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            if wallet_address.is_empty() {
                continue;
            }
            match position_client
                .fetch_wallet_positions(&wallet_address)
                .await
            {
                Ok(positions) => {
                    #[cfg(feature = "dashboard")]
                    let mut live_dashboard_positions: Vec<PositionResponse> = Vec::new();
                    #[cfg(feature = "dashboard")]
                    let mut total_live_unrealized = 0.0;

                    for pos in positions {
                        let size: f64 = pos.size.parse().unwrap_or(0.0);
                        if size <= 0.0 {
                            continue;
                        }

                        let token_id = pos.token_id.clone().unwrap_or_else(|| pos.asset.clone());
                        let avg_price = parse_position_number(Some(&pos.avg_price)).unwrap_or(0.0);
                        let current_value =
                            parse_position_number(pos.current_value.as_deref()).unwrap_or(0.0);
                        let fallback_price = parse_position_number(pos.current_price.as_deref())
                            .or_else(|| {
                                if current_value > 0.0 && size > 0.0 {
                                    Some(current_value / size)
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(avg_price);

                        let current_price = match position_client.quote_token(&token_id).await {
                            Ok(quote) if quote.bid >= 0.01 => quote.bid.clamp(0.01, 1.0),
                            _ => fallback_price,
                        };
                        if pos.redeemable {
                            let mut contexts = live_contexts_for_monitor.lock().await;
                            if contexts.remove(&token_id).is_some() {
                                persist_live_position_contexts(
                                    &live_contexts_path_for_monitor,
                                    &contexts,
                                );
                            }
                            drop(contexts);
                            continue;
                        }
                        let mut live_context = {
                            live_contexts_for_monitor
                                .lock()
                                .await
                                .get(&token_id)
                                .cloned()
                        };
                        // Backfill market_slug if it was empty when the position was opened.
                        if let Some(ctx) = live_context.as_mut() {
                            if ctx.market_slug.trim().is_empty() {
                                let backfill_slug = pos.slug.clone()
                                    .unwrap_or_default();
                                if !backfill_slug.is_empty() {
                                    ctx.market_slug = backfill_slug;
                                    let mut contexts = live_contexts_for_monitor.lock().await;
                                    if let Some(stored) = contexts.get_mut(&token_id) {
                                        stored.market_slug = ctx.market_slug.clone();
                                        persist_live_position_contexts(
                                            &live_contexts_path_for_monitor,
                                            &contexts,
                                        );
                                    }
                                }
                            }
                        }
                        if live_context
                            .as_ref()
                            .map(|ctx| {
                                let viable = live_context_is_viable(ctx);
                                if !viable {
                                    tracing::debug!(
                                        token_id = %token_id,
                                        shares_size = ctx.shares_size,
                                        entry_share_price = ctx.entry_share_price,
                                        size_usdc = ctx.size_usdc,
                                        "live_context_is_viable=false — removing context"
                                    );
                                }
                                !viable
                            })
                            .unwrap_or(false)
                        {
                            let mut contexts = live_contexts_for_monitor.lock().await;
                            contexts.remove(&token_id);
                            persist_live_position_contexts(
                                &live_contexts_path_for_monitor,
                                &contexts,
                            );
                            drop(contexts);
                            live_context = None;
                        }
                        if live_context.is_none()
                            && live_recently_closed_for_monitor
                                .lock()
                                .await
                                .contains(&token_id)
                        {
                            continue;
                        }

                        let fallback_market = if live_context.is_none() {
                            if let Some(condition_id) = pos.condition_id.as_deref() {
                                position_client.get_market(condition_id).await
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        let fallback_market_text = if let Some(market) = fallback_market.as_ref() {
                            format!(
                                "{} {}",
                                market.slug.clone().unwrap_or_default(),
                                market.question
                            )
                        } else {
                            format!(
                                "{} {} {}",
                                pos.slug.clone().unwrap_or_default(),
                                pos.title.clone().unwrap_or_default(),
                                pos.asset
                            )
                        };
                        let derived_entry_share_price = if avg_price > 0.0 {
                            avg_price
                        } else if current_price > 0.0 {
                            current_price
                        } else {
                            0.0
                        };
                        let derived_size_usdc = if avg_price > 0.0 {
                            size * avg_price
                        } else if current_value > 0.0 {
                            current_value
                        } else if current_price > 0.0 {
                            size * current_price
                        } else {
                            0.0
                        };

                        if live_context.is_none() {
                            let inferred_asset = infer_asset_from_market_text(&fallback_market_text)
                                .or_else(|| infer_asset_from_market_text(&pos.asset));
                            let inferred_timeframe =
                                parse_timeframe_from_market_text(&fallback_market_text)
                                    .or_else(|| parse_timeframe_from_market_text(&pos.asset));
                            if inferred_asset.is_none() || inferred_timeframe.is_none() {
                                tracing::debug!(
                                    token_id = %token_id,
                                    fallback_text = %fallback_market_text,
                                    asset_text = %pos.asset,
                                    inferred_asset = ?inferred_asset,
                                    inferred_timeframe = ?inferred_timeframe,
                                    "Cannot rehydrate live context — asset or timeframe could not be inferred from market text"
                                );
                            }
                            if let (Some(asset), Some(timeframe)) = (inferred_asset, inferred_timeframe) {
                                let inferred_expires_at_ms = fallback_market
                                    .as_ref()
                                    .and_then(|market| {
                                        market
                                            .end_date
                                            .as_deref()
                                            .or(market.end_date_iso.as_deref())
                                    })
                                    .and_then(crate::clob::ClobClient::parse_expiry_to_timestamp)
                                    .unwrap_or(0);
                                let now_ms_rehydrate = chrono::Utc::now().timestamp_millis();
                                if inferred_expires_at_ms > 0
                                    && now_ms_rehydrate > inferred_expires_at_ms + 60_000
                                {
                                    tracing::debug!(
                                        token_id = %token_id,
                                        "Expired market position awaiting UMA resolution — skipping rehydration"
                                    );
                                    continue;
                                }
                                let inferred_context = LivePositionContext {
                                    signal_id: format!("rehydrated_{}", token_id),
                                    asset,
                                    timeframe,
                                    direction: infer_direction_from_outcome(pos.outcome.as_deref())
                                        .unwrap_or(Direction::Up),
                                    confidence: 0.0,
                                    market_slug: fallback_market
                                        .as_ref()
                                        .and_then(|market| market.slug.clone())
                                        .or_else(|| pos.slug.clone())
                                        .unwrap_or_else(|| pos.asset.clone()),
                                    token_id: token_id.clone(),
                                    condition_id: pos.condition_id.clone().unwrap_or_default(),
                                    size_usdc: derived_size_usdc,
                                    shares_size: size,
                                    entry_share_price: derived_entry_share_price,
                                    opened_at_ms: chrono::Utc::now().timestamp_millis(),
                                    expires_at_ms: inferred_expires_at_ms,
                                };
                                {
                                    let mut contexts = live_contexts_for_monitor.lock().await;
                                    contexts
                                        .entry(token_id.clone())
                                        .or_insert_with(|| inferred_context.clone());
                                    persist_live_position_contexts(
                                        &live_contexts_path_for_monitor,
                                        &contexts,
                                    );
                                }
                                live_context = Some(inferred_context);
                            }
                        }

                        let timeframe = live_context
                            .as_ref()
                            .map(|ctx| ctx.timeframe)
                            .or_else(|| parse_timeframe_from_market_text(&fallback_market_text))
                            .or_else(|| parse_timeframe_from_market_text(&pos.asset))
                            .unwrap_or(Timeframe::Min15);
                        let direction = live_context
                            .as_ref()
                            .map(|ctx| ctx.direction)
                            .or_else(|| infer_direction_from_outcome(pos.outcome.as_deref()))
                            .unwrap_or(Direction::Up);
                        let confidence = live_context
                            .as_ref()
                            .map(|ctx| ctx.confidence)
                            .unwrap_or(0.0);
                        let size_usdc = live_context
                            .as_ref()
                            .map(|ctx| ctx.size_usdc)
                            .filter(|value| *value > 0.0)
                            .unwrap_or(derived_size_usdc);
                        let entry_share_price = live_context
                            .as_ref()
                            .map(|ctx| ctx.entry_share_price)
                            .filter(|value| *value > 0.0)
                            .unwrap_or(derived_entry_share_price);
                        let entry_cost = size * entry_share_price;
                        let open_fee =
                            entry_cost * crate::polymarket::fee_rate_from_price(entry_share_price);
                        let gross_exit_value = size * current_price;
                        let unrealized_close_fee = gross_exit_value
                            * crate::polymarket::fee_rate_from_price(current_price);
                        let unrealized_pnl =
                            gross_exit_value - unrealized_close_fee - entry_cost - open_fee;

                        let asset = live_context
                            .as_ref()
                            .map(|ctx| ctx.asset)
                            .or_else(|| infer_asset_from_market_text(&fallback_market_text))
                            .or_else(|| infer_asset_from_market_text(&pos.asset));

                        if let (Some(asset), Some(ctx)) = (asset, live_context.as_ref()) {
                            if position_risk_for_monitor
                                .get_position_by_token_id(&token_id)
                                .is_none()
                            {
                                position_risk_for_monitor.restore_position(
                                    asset,
                                    ctx.timeframe,
                                    direction,
                                    size_usdc,
                                    entry_share_price,
                                    current_price,
                                    ctx.opened_at_ms,
                                    ctx.expires_at_ms,
                                    ctx.market_slug.clone(),
                                    token_id.clone(),
                                );
                            }
                        }

                        #[cfg(feature = "dashboard")]
                        {
                            total_live_unrealized += unrealized_pnl;
                            let live_checkpoint_state = position_risk_for_monitor
                                .get_position_by_token_id(&token_id)
                                .or_else(|| {
                                    asset.and_then(|asset_value| {
                                        position_risk_for_monitor.get_position(asset_value)
                                    })
                                });
                            live_dashboard_positions.push(PositionResponse {
                                id: token_id.clone(),
                                asset: asset
                                    .map(|value| value.to_string())
                                    .or_else(|| {
                                        fallback_market
                                            .as_ref()
                                            .and_then(|market| market.slug.clone())
                                    })
                                    .unwrap_or_else(|| pos.asset.clone()),
                                timeframe: timeframe.to_string(),
                                direction: direction.to_string(),
                                entry_price: entry_share_price,
                                current_price,
                                size_usdc,
                                pnl: unrealized_pnl,
                                pnl_pct: {
                                    let cost_basis = (entry_cost + open_fee).max(0.01);
                                    (unrealized_pnl / cost_basis) * 100.0
                                },
                                opened_at: live_context
                                    .as_ref()
                                    .map(|ctx| ctx.opened_at_ms)
                                    .unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
                                market_slug: live_context
                                    .as_ref()
                                    .map(|ctx| ctx.market_slug.clone())
                                    .or_else(|| {
                                        fallback_market
                                            .as_ref()
                                            .and_then(|market| market.slug.clone())
                                    })
                                    .unwrap_or_else(|| pos.asset.clone()),
                                confidence,
                                peak_price: current_price,
                                trough_price: current_price,
                                market_close_ts: live_context
                                    .as_ref()
                                    .map(|ctx| ctx.expires_at_ms)
                                    .unwrap_or(0),
                                time_remaining_secs: live_context
                                    .as_ref()
                                    .map(|ctx| {
                                        ((ctx.expires_at_ms
                                            - chrono::Utc::now().timestamp_millis())
                                            / 1000)
                                            .max(0)
                                    })
                                    .unwrap_or(0),
                                stop_loss_pct: live_checkpoint_state
                                    .as_ref()
                                    .map(|position| position.dynamic_hard_stop_roi.abs() * 100.0)
                                    .unwrap_or(live_stop_loss_pct),
                                take_profit_pct: live_take_profit_pct,
                                checkpoint_armed: live_checkpoint_state
                                    .as_ref()
                                    .map(|position| position.checkpoint_armed)
                                    .unwrap_or(false),
                                checkpoint_floor_pct: live_checkpoint_state
                                    .as_ref()
                                    .map(|position| position.checkpoint_floor_roi * 100.0)
                                    .unwrap_or(0.0),
                                checkpoint_peak_pct: live_checkpoint_state
                                    .as_ref()
                                    .map(|position| position.checkpoint_peak_roi * 100.0)
                                    .unwrap_or(0.0),
                                trading_roi: {
                                    let cost_basis = (entry_cost + open_fee).max(0.01);
                                    (unrealized_pnl / cost_basis) * 100.0
                                },
                                prediction_roi: 0.0,
                                entry_share_price,
                                current_share_price: current_price,
                            });
                        }

                        if let Some(asset) = asset {
                            if let Some(exit_reason) = position_risk_for_monitor
                                .update_position_by_token_id(&token_id, current_price)
                                .or_else(|| {
                                    position_risk_for_monitor.update_position(asset, current_price)
                                })
                                .and_then(|r| {
                                    use crate::risk::ExitReason;
                                    match r {
                                        ExitReason::HardStop
                                        | ExitReason::TakeProfit
                                        | ExitReason::CheckpointTakeProfit
                                        | ExitReason::TrailingStop => {
                                            // Allow exit when share price hits 90¢ (guaranteed profit territory)
                                            if current_price >= 0.90 {
                                                tracing::info!(
                                                    token_id = %token_id,
                                                    share_price = current_price,
                                                    "💰 Share price TP at 90¢ — exiting live position early"
                                                );
                                                Some(ExitReason::TakeProfit)
                                            } else {
                                                tracing::debug!(reason = %r, token_id = %token_id, "Live SL/TP suppressed — binary options exit by time only");
                                                None
                                            }
                                        }
                                        _ => Some(r),
                                    }
                                })
                            {
                                let should_close = {
                                    let mut pending = live_pending_closes_for_monitor.lock().await;
                                    pending.insert(token_id.clone())
                                };
                                if !should_close {
                                    continue;
                                }

                                let official_result =
                                    if let Some(condition_id) = pos.condition_id.as_deref() {
                                        position_client
                                            .official_result_for_token(condition_id, &token_id)
                                            .await
                                    } else {
                                        None
                                    };

                                let (exit_share_price, final_exit_reason) = if official_result.as_deref() == Some("WIN")
                                {
                                    if let Some(condition_id) = pos.condition_id.as_deref() {
                                        let redeem_key = format!("{}:{}", condition_id, token_id);
                                        let should_attempt = {
                                            let mut redeemed =
                                                redeemed_claims_for_monitor.lock().await;
                                            redeemed.insert(redeem_key.clone())
                                        };
                                        if !should_attempt {
                                            live_pending_closes_for_monitor
                                                .lock()
                                                .await
                                                .remove(&token_id);
                                            continue;
                                        }
                                        if let Err(e) = position_client
                                            .redeem_winning_tokens(condition_id, &token_id, size)
                                            .await
                                        {
                                            tracing::warn!(error = %e, condition_id = %condition_id, token_id = %token_id, "Redemption attempt failed");
                                            redeemed_claims_for_monitor
                                                .lock()
                                                .await
                                                .remove(&redeem_key);
                                            live_pending_closes_for_monitor
                                                .lock()
                                                .await
                                                .remove(&token_id);
                                            continue;
                                        }
                                    }
                                    (1.0, "RESOLUTION_WIN".to_string())
                                } else if official_result.as_deref() == Some("LOSS") {
                                    (0.0, "RESOLUTION_LOSS".to_string())
                                } else {
                                    let now_ms = chrono::Utc::now().timestamp_millis();
                                    let is_expired = live_context.as_ref()
                                        .map(|ctx| now_ms > (ctx.expires_at_ms + 60_000))
                                        .unwrap_or(false);

                                    if is_expired {
                                        tracing::info!(
                                            token_id = %token_id,
                                            "Market is expired. Awaiting UMA resolution, removing from live contexts"
                                        );
                                        {
                                            let mut contexts = live_contexts_for_monitor.lock().await;
                                            if contexts.remove(&token_id).is_some() {
                                                persist_live_position_contexts(
                                                    &live_contexts_path_for_monitor,
                                                    &contexts,
                                                );
                                            }
                                        }
                                        // Remove from risk manager so it no longer blocks new entries
                                        // for the same asset+timeframe after market expiry.
                                        use crate::risk::ExitReason;
                                        let _ = position_risk_for_monitor
                                            .close_position_by_token_id(
                                                &token_id,
                                                0.0,
                                                ExitReason::MarketExpiry,
                                            )
                                            .or_else(|| {
                                                position_risk_for_monitor.close_position(
                                                    asset,
                                                    0.0,
                                                    ExitReason::MarketExpiry,
                                                )
                                            });
                                        live_pending_closes_for_monitor
                                            .lock()
                                            .await
                                            .remove(&token_id);
                                        continue;
                                    }

                                    let quote = match position_client.quote_token(&token_id).await {
                                        Ok(quote) => quote,
                                        Err(e) => {
                                            tracing::warn!(error = %e, token_id = %token_id, "Failed to quote token for live close");
                                            live_pending_closes_for_monitor
                                                .lock()
                                                .await
                                                .remove(&token_id);
                                            continue;
                                        }
                                    };
                                    let mut close_order = Order::new(
                                        token_id.clone(),
                                        clob::Side::Sell,
                                        (quote.bid - 0.05).max(0.01).clamp(0.01, 0.99),
                                        size,
                                    )
                                    .with_auth(config.execution.signature_type, None, None);
                                    close_order.condition_id = pos.condition_id.clone();
                                    close_order.order_type = Some("FAK".to_string());
                                    close_order.expiration = 0;
                                    let sell_filled = match position_client
                                        .execute_sell_confirmed(&close_order, 3)
                                        .await
                                    {
                                        Ok(filled) => filled,
                                        Err(e) => {
                                            let retries = {
                                                let mut retries_map =
                                                    live_sell_retries_for_monitor.lock().await;
                                                let count = retries_map
                                                    .entry(token_id.clone())
                                                    .or_insert(0);
                                                *count += 1;
                                                *count
                                            };
                                            if retries > 5 {
                                                tracing::error!(
                                                    token_id = %token_id,
                                                    retries = retries,
                                                    "SELL retry limit exceeded — abandoning position internally"
                                                );
                                                live_sell_retries_for_monitor
                                                    .lock()
                                                    .await
                                                    .remove(&token_id);
                                                live_pending_closes_for_monitor
                                                    .lock()
                                                    .await
                                                    .remove(&token_id);
                                            } else {
                                                tracing::warn!(
                                                    error = %e,
                                                    token_id = %token_id,
                                                    retries = retries,
                                                    "SELL execute_sell_confirmed error — will retry next tick"
                                                );
                                                live_pending_closes_for_monitor
                                                    .lock()
                                                    .await
                                                    .remove(&token_id);
                                            }
                                            continue;
                                        }
                                    };
                                    if !sell_filled {
                                        tracing::error!(
                                            token_id = %token_id,
                                            "SELL could not be confirmed filled after 3 attempts — keeping position tracked"
                                        );
                                        live_sell_retries_for_monitor
                                            .lock()
                                            .await
                                            .remove(&token_id);
                                        live_pending_closes_for_monitor
                                            .lock()
                                            .await
                                            .remove(&token_id);
                                        continue;
                                    }
                                    live_sell_retries_for_monitor
                                        .lock()
                                        .await
                                        .remove(&token_id);
                                    (close_order.price, exit_reason.to_string())
                                };

                                let _ = position_risk_for_monitor
                                    .close_position_by_token_id(
                                        &token_id,
                                        exit_share_price,
                                        exit_reason,
                                    )
                                    .or_else(|| {
                                        position_risk_for_monitor.close_position(
                                            asset,
                                            exit_share_price,
                                            exit_reason,
                                        )
                                    });
                                let pnl = (size * exit_share_price)
                                    - ((size * exit_share_price)
                                        * crate::polymarket::fee_rate_from_price(exit_share_price))
                                    - entry_cost
                                    - open_fee;
                                let internal_result = if pnl >= 0.0 { "WIN" } else { "LOSS" };
                                use crate::persistence::{LiveTradeRecord, WinLossRecord};
                                let record = WinLossRecord {
                                    timestamp: chrono::Utc::now().timestamp(),
                                    market_slug: live_context
                                        .as_ref()
                                        .map(|ctx| ctx.market_slug.clone())
                                        .unwrap_or_else(|| pos.asset.clone()),
                                    token_id: token_id.clone(),
                                    entry_price: entry_share_price,
                                    exit_price: exit_share_price,
                                    size,
                                    pnl,
                                    internal_result: internal_result.to_string(),
                                    exit_reason: final_exit_reason.clone(),
                                    official_result: official_result.clone(),
                                };
                                position_tracker.record_winloss(record.clone());
                                if let Err(e) = position_csv.save_live_winloss(record).await {
                                    tracing::warn!(error = %e, token_id = %token_id, "Failed to persist live win/loss record");
                                }

                                let live_context = {
                                    let mut contexts = live_contexts_for_monitor.lock().await;
                                    let removed = contexts.remove(&token_id);
                                    persist_live_position_contexts(
                                        &live_contexts_path_for_monitor,
                                        &contexts,
                                    );
                                    removed
                                };
                                let close_fee = (size * exit_share_price)
                                    * crate::polymarket::fee_rate_from_price(exit_share_price);
                                let live_trade_record = LiveTradeRecord {
                                    timestamp: chrono::Utc::now().timestamp_millis(),
                                    trade_id: live_context
                                        .as_ref()
                                        .map(|ctx| format!("{}_close", ctx.signal_id))
                                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                                    asset: asset.to_string(),
                                    timeframe: timeframe.to_string(),
                                    direction: direction.to_string(),
                                    confidence,
                                    entry_price: entry_share_price,
                                    exit_price: exit_share_price,
                                    size_usdc,
                                    pnl,
                                    pnl_pct: {
                                        let cost_basis = (entry_cost + open_fee).max(0.01);
                                        (pnl / cost_basis) * 100.0
                                    },
                                    result: internal_result.to_string(),
                                    prediction_correct: official_result
                                        .as_deref()
                                        .map(|value| value == "WIN")
                                        .unwrap_or(pnl >= 0.0),
                                    exit_reason: exit_reason.to_string(),
                                    hold_duration_secs: live_context
                                        .as_ref()
                                        .map(|ctx| {
                                            ((chrono::Utc::now().timestamp_millis()
                                                - ctx.opened_at_ms)
                                                / 1000)
                                                .max(0)
                                        })
                                        .unwrap_or(0),
                                    balance_after: position_tracker.available_balance()
                                        + ((size * exit_share_price) - close_fee),
                                    entry_share_price,
                                    exit_share_price,
                                    trading_win: pnl >= 0.0,
                                };
                                if let Err(e) = position_csv
                                    .save_live_trade(live_trade_record.clone())
                                    .await
                                {
                                    tracing::warn!(error = %e, token_id = %token_id, "Failed to persist live trade");
                                }

                                let parsed_timeframe = live_context
                                    .as_ref()
                                    .map(|ctx| ctx.timeframe)
                                    .or_else(|| {
                                        parse_timeframe_from_market_text(&fallback_market_text)
                                    })
                                    .or_else(|| parse_timeframe_from_market_text(&pos.asset));
                                let (timeframe, indicators, p_model) = {
                                    let mut pending = live_indicators_for_monitor.lock().await;
                                    if let Some(tf) = parsed_timeframe {
                                        pending
                                            .remove(&(asset, tf))
                                            .map(|ctx| (tf, ctx.0, ctx.1))
                                            .unwrap_or((tf, Vec::new(), 0.5))
                                    } else {
                                        let keys: Vec<(Asset, Timeframe)> = pending
                                            .keys()
                                            .copied()
                                            .filter(|(current_asset, _)| *current_asset == asset)
                                            .collect();
                                        if keys.len() == 1 {
                                            let key = keys[0];
                                            let (inds, p_model) =
                                                pending.remove(&key).unwrap_or((Vec::new(), 0.5));
                                            (key.1, inds, p_model)
                                        } else {
                                            (Timeframe::Min15, Vec::new(), 0.5)
                                        }
                                    }
                                };

                                let ml_trade_record = crate::paper_trading::PaperTradeRecord {
                                    timestamp: live_trade_record.timestamp,
                                    trade_id: live_trade_record.trade_id.clone(),
                                    signal_id: live_context
                                        .as_ref()
                                        .map(|ctx| ctx.signal_id.clone())
                                        .unwrap_or_else(|| live_trade_record.trade_id.clone()),
                                    asset: asset.to_string(),
                                    timeframe: timeframe.to_string(),
                                    direction: direction.to_string(),
                                    confidence,
                                    entry_price: entry_share_price,
                                    exit_price: exit_share_price,
                                    size_usdc,
                                    shares: size,
                                    fee_paid: open_fee + close_fee,
                                    pnl,
                                    pnl_pct: {
                                        let cost_basis = (entry_cost + open_fee).max(0.01);
                                        (pnl / cost_basis) * 100.0
                                    },
                                    result: internal_result.to_string(),
                                    exit_reason: exit_reason.to_string(),
                                    hold_duration_ms: live_trade_record.hold_duration_secs * 1000,
                                    balance_after: live_trade_record.balance_after,
                                    market_open_ts: live_context
                                        .as_ref()
                                        .map(|ctx| ctx.opened_at_ms)
                                        .unwrap_or(live_trade_record.timestamp),
                                    market_close_ts: live_context
                                        .as_ref()
                                        .map(|ctx| ctx.expires_at_ms)
                                        .unwrap_or(live_trade_record.timestamp),
                                    time_remaining_at_entry_secs: live_context
                                        .as_ref()
                                        .map(|ctx| {
                                            ((ctx.expires_at_ms - ctx.opened_at_ms) / 1000).max(0)
                                        })
                                        .unwrap_or(0),
                                    indicators_used: indicators.clone(),
                                    market_id: pos.condition_id.clone().unwrap_or_default(),
                                    token_id: token_id.clone(),
                                    outcome: pos.outcome.clone().unwrap_or_default(),
                                    entry_bid: entry_share_price,
                                    entry_ask: entry_share_price,
                                    entry_mid: entry_share_price,
                                    exit_bid: exit_share_price,
                                    exit_ask: exit_share_price,
                                    exit_mid: exit_share_price,
                                    fee_open: open_fee,
                                    fee_close: close_fee,
                                    slippage_open: 0.0,
                                    slippage_close: 0.0,
                                    p_market: 0.0,
                                    p_model,
                                    edge_net: 0.0,
                                    kelly_raw: 0.0,
                                    kelly_applied: 0.0,
                                    exit_reason_detail: exit_reason.to_string(),
                                    window_open_price: entry_share_price,
                                    window_close_price: exit_share_price,
                                    actual_price_direction: if live_trade_record.prediction_correct
                                    {
                                        direction.to_string()
                                    } else if direction == Direction::Up {
                                        Direction::Down.to_string()
                                    } else {
                                        Direction::Up.to_string()
                                    },
                                    prediction_correct: live_trade_record.prediction_correct,
                                    trading_pnl: pnl,
                                    trading_win: pnl >= 0.0,
                                };

                                {
                                    let mut strat = strategy_for_live_calibration.lock().await;
                                    strat.register_closed_trade_result(&ml_trade_record);
                                    #[cfg(feature = "dashboard")]
                                    {
                                        let ml_state = strat.get_ml_state();
                                        let weights = ml_state.model_info.clone();
                                        live_dashboard_memory_for_positions
                                            .update_ml_metrics(ml_state.clone())
                                            .await;
                                        live_dashboard_memory_for_positions
                                            .update_ml_dataset_stats(strat.get_dataset_stats())
                                            .await;
                                        live_dashboard_broadcaster_for_positions
                                            .broadcast_ml_metrics(
                                                ml_state.model_accuracy,
                                                ml_state.win_rate,
                                                ml_state.loss_rate,
                                                ml_state.total_predictions,
                                                ml_state.correct_predictions,
                                                ml_state.incorrect_predictions,
                                                weights,
                                                ml_state.training_epoch,
                                                ml_state.dataset_size,
                                            );
                                    }
                                }

                                if !indicators.is_empty() {
                                    let is_win = pnl >= 0.0;
                                    let result = if is_win {
                                        TradeResult::Win
                                    } else {
                                        TradeResult::Loss
                                    };
                                    let mut strat = strategy_for_live_calibration.lock().await;
                                    strat.record_trade_with_indicators_for_market(
                                        asset,
                                        timeframe,
                                        &indicators,
                                        result,
                                    );
                                    strat.record_prediction_outcome_for_market(
                                        asset, timeframe, p_model, is_win,
                                    );
                                    let stats = strat.export_calibrator_state_v2();
                                    drop(strat);
                                    if let Ok(json) = serde_json::to_string_pretty(&stats) {
                                        if let Err(e) =
                                            std::fs::write(&calibrator_save_path_live, &json)
                                        {
                                            tracing::warn!(error = %e, "Failed to save calibrator state (live)");
                                        }
                                    }
                                }

                                #[cfg(feature = "dashboard")]
                                {
                                    live_dashboard_memory_for_positions
                                        .add_live_trade(crate::dashboard::TradeResponse {
                                            timestamp: live_trade_record.timestamp,
                                            trade_id: live_trade_record.trade_id.clone(),
                                            asset: live_trade_record.asset.clone(),
                                            timeframe: live_trade_record.timeframe.clone(),
                                            direction: live_trade_record.direction.clone(),
                                            confidence: live_trade_record.confidence,
                                            entry_price: live_trade_record.entry_price,
                                            exit_price: live_trade_record.exit_price,
                                            size_usdc: live_trade_record.size_usdc,
                                            pnl: live_trade_record.pnl,
                                            pnl_pct: live_trade_record.pnl_pct,
                                            result: live_trade_record.result.clone(),
                                            prediction_correct: live_trade_record
                                                .prediction_correct,
                                            exit_reason: live_trade_record.exit_reason.clone(),
                                            hold_duration_secs: live_trade_record
                                                .hold_duration_secs,
                                            balance_after: live_trade_record.balance_after,
                                            entry_share_price: live_trade_record.entry_share_price,
                                            exit_share_price: live_trade_record.exit_share_price,
                                            trading_win: live_trade_record.trading_win,
                                            rsi_at_entry: None,
                                            macd_hist_at_entry: None,
                                            bb_position_at_entry: None,
                                            adx_at_entry: None,
                                            volatility_at_entry: None,
                                        })
                                        .await;
                                }
                                live_recently_closed_for_monitor
                                    .lock()
                                    .await
                                    .insert(token_id.clone());
                                live_pending_closes_for_monitor
                                    .lock()
                                    .await
                                    .remove(&token_id);
                            }
                        }
                    }

                    #[cfg(feature = "dashboard")]
                    {
                        *live_dashboard_memory_for_positions
                            .live_positions
                            .write()
                            .await = live_dashboard_positions.clone();
                        *live_dashboard_memory_for_positions
                            .live_unrealized_pnl
                            .write()
                            .await = total_live_unrealized;
                        // Avoid broadcasting FullState to prevent chart freezing
                        live_dashboard_broadcaster_for_positions
                            .broadcast_live_positions(live_dashboard_positions);
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to fetch wallet positions");
                }
            }
        }
    }); // Manual close task — executes immediate SELL when dashboard button is pressed
    let manual_close_client = clob_client.clone();
    let manual_close_contexts = live_position_contexts.clone();
    let manual_close_contexts_path = live_contexts_path.clone();
    let manual_close_risk = risk_manager.clone();
    let manual_close_pending = live_pending_closes.clone();
    #[cfg(feature = "dashboard")]
    let manual_close_dashboard_memory = dashboard_memory.clone();
    #[cfg(feature = "dashboard")]
    let manual_close_broadcaster = dashboard_broadcaster.clone();
    let manual_close_config = config.execution.clone();
    tokio::spawn(async move {
        while let Some(token_id) = manual_close_rx.recv().await {
            tracing::warn!(token_id = %token_id, "🔴 Manual close triggered from dashboard");

            // Prevent double-close
            {
                let mut pending = manual_close_pending.lock().await;
                if !pending.insert(token_id.clone()) {
                    tracing::warn!(token_id = %token_id, "Manual close already in progress, skipping");
                    continue;
                }
            }

            let ctx = {
                let contexts = manual_close_contexts.lock().await;
                contexts.get(&token_id).cloned()
            };

            let Some(ctx) = ctx else {
                tracing::warn!(token_id = %token_id, "Manual close: no live context found");
                manual_close_pending.lock().await.remove(&token_id);
                continue;
            };

            // Quote current price for sell
            let sell_price = match manual_close_client.quote_token(&token_id).await {
                Ok(q) if q.bid >= 0.01 => (q.bid - 0.02).max(0.01).clamp(0.01, 0.99),
                _ => 0.10_f64,
            };

            let mut close_order = crate::clob::types::Order::new(
                token_id.clone(),
                crate::clob::Side::Sell,
                sell_price,
                ctx.shares_size,
            )
            .with_auth(manual_close_config.signature_type, None, None);
            close_order.condition_id = Some(ctx.condition_id.clone());
            close_order.order_type = Some("FAK".to_string());
            close_order.expiration = 0;

            match manual_close_client.execute_order(&close_order).await {
                Ok(order_id) => {
                    tracing::info!(token_id = %token_id, order_id = %order_id, "✅ Manual close SELL executed");

                    // Remove from risk manager
                    use crate::risk::ExitReason;
                    let _ = manual_close_risk
                        .close_position_by_token_id(&token_id, sell_price, ExitReason::Manual)
                        .or_else(|| manual_close_risk.close_position(ctx.asset, sell_price, ExitReason::Manual));

                    // Remove from live_contexts
                    {
                        let mut contexts = manual_close_contexts.lock().await;
                        contexts.remove(&token_id);
                        persist_live_position_contexts(&manual_close_contexts_path, &contexts);
                    }

                    // Update dashboard
                    #[cfg(feature = "dashboard")]
                    {
                        let mut live_positions = manual_close_dashboard_memory.live_positions.write().await;
                        live_positions.retain(|p| p.id != token_id);
                        let snapshot = live_positions.clone();
                        drop(live_positions);
                        manual_close_broadcaster.broadcast_live_positions(snapshot);
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, token_id = %token_id, "❌ Manual close SELL failed");
                }
            }

            manual_close_pending.lock().await.remove(&token_id);
        }
    });
    // Balance tracking task - fetches balance periodically and updates trackers
    let balance_client = clob_client.clone();
    let balance_tracker_clone = balance_tracker.clone();
    let balance_risk = risk_manager.clone();
    let balance_csv = csv_persistence.clone();
    #[cfg(feature = "dashboard")]
    let live_dashboard_memory = dashboard_memory.clone();
    #[cfg(feature = "dashboard")]
    let live_dashboard_broadcaster = dashboard_broadcaster.clone();
    let balance_wallet_address = std::env::var("POLYMARKET_WALLET")
        .ok()
        .or_else(|| std::env::var("POLYMARKET_ADDRESS").ok())
        .unwrap_or_default();
    let balance_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
        let mut initialized = false;
        loop {
            interval.tick().await; // Fetch balance from Polymarket
            match balance_client.get_balance().await {
                Ok(balance) => {
                    tracing::info!(balance_usdc = balance, "💰 Balance fetched"); // Initialize balance tracker on first fetch
                    if !initialized {
                        balance_tracker_clone.initialize(balance);
                        initialized = true;
                        tracing::info!(initial_balance = balance, "🏁 Balance tracker initialized");
                    }
                    let locked_in_positions = if !balance_wallet_address.is_empty() {
                        match balance_client
                            .fetch_wallet_positions(&balance_wallet_address)
                            .await
                        {
                            Ok(positions) => positions
                                .iter()
                                .map(|p| {
                                    let size = p.size.parse::<f64>().unwrap_or(0.0).max(0.0);
                                    let price = p
                                        .current_price
                                        .as_ref()
                                        .and_then(|v| v.parse::<f64>().ok())
                                        .unwrap_or_else(|| {
                                            p.avg_price.parse::<f64>().unwrap_or(0.0)
                                        })
                                        .clamp(0.0, 1.0);
                                    size * price
                                })
                                .sum::<f64>(),
                            Err(e) => {
                                tracing::warn!(                                    error = %e,                                    "Failed to fetch wallet positions for locked-balance estimate"                                );
                                0.0
                            }
                        }
                    } else {
                        0.0
                    }; // Update balance tracker
                    balance_tracker_clone.update_balance(
                        balance, // available
                        locked_in_positions,
                    ); // Update risk manager
                    balance_risk.set_balance(balance);
                    if let Some(snapshot) = balance_tracker_clone.record_snapshot() {
                        if let Err(e) = balance_csv.save_live_balance(snapshot).await {
                            tracing::warn!(error = %e, "Failed to persist live balance snapshot");
                        }
                    }
                    // Update live dashboard memory with real wallet balance
                    // Do NOT broadcast FullState here — it resets the chart on every balance tick.
                    // Live positions loop (broadcast_live_positions every 5s) keeps the UI fresh.
                    #[cfg(feature = "dashboard")]
                    {
                        *live_dashboard_memory.live_balance.write().await = balance;
                        *live_dashboard_memory.live_locked.write().await = locked_in_positions;
                        live_dashboard_broadcaster.broadcast_live_balance(balance, locked_in_positions);
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to fetch balance");
                }
            }
        }
    }); // ── Orderbook sync task - periodically sync tracker to feature engine ─────────────
    let orderbook_sync_tracker = orderbook_tracker.clone();
    let orderbook_sync_feature = feature_engine.clone();
    #[cfg(feature = "dashboard")]
    let orderbook_sync_dashboard = dashboard_memory.clone();
    let orderbook_sync_strategy = strategy.clone();
    let paper_ml_strategy = orderbook_sync_strategy.clone();
    let expired_ml_strategy = orderbook_sync_strategy.clone();
    let orderbook_sync_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        loop {
            interval.tick().await; // Update feature engine with latest orderbook data
            for asset in [Asset::BTC, Asset::ETH] {
                for timeframe in [Timeframe::Min15, Timeframe::Hour1] {
                    orderbook_sync_feature
                        .lock()
                        .await
                        .update_from_tracker(asset, timeframe);
                }
            } // Update dashboard with indicator stats
            #[cfg(feature = "dashboard")]
            {
                let strat = orderbook_sync_strategy.lock().await;
                let indicator_stats = strat.get_indicator_stats();
                let market_learning = strat.export_calibrator_state_v2();
                let calibration_quality = strat.export_calibration_quality_by_market();
                drop(strat);
                *orderbook_sync_dashboard.indicator_stats.write().await = indicator_stats;
                orderbook_sync_dashboard
                    .set_market_learning_stats(market_learning)
                    .await;
                orderbook_sync_dashboard
                    .set_calibration_quality_stats(calibration_quality)
                    .await;
            }
        }
    }); // Paper trading: price monitor task (feeds ticks to paper engine for position tracking)
    let paper_monitor_engine = paper_engine.clone();
    #[cfg(feature = "dashboard")]
    let paper_dashboard_memory = dashboard_memory.clone();
    #[cfg(feature = "dashboard")]
    let paper_dashboard_memory_for_monitor = paper_dashboard_memory.clone();
    #[cfg(feature = "dashboard")]
    let paper_dashboard_broadcaster = dashboard_broadcaster.clone();
    #[cfg(feature = "dashboard")]
    let paper_csv_persistence = csv_persistence.clone(); // Clone for main loop (paper_monitor_handle takes ownership of paper_dashboard_broadcaster)
    #[cfg(feature = "dashboard")]
    let main_loop_broadcaster = dashboard_broadcaster.clone();
    #[cfg(feature = "dashboard")]
    let live_main_dashboard_memory = dashboard_memory.clone();
    let main_loop_share_prices = polymarket_share_prices.clone();
    let paper_monitor_handle = if paper_trading_enabled {
        let engine = paper_monitor_engine.unwrap();
        #[cfg(feature = "dashboard")]
        let paper_dashboard_memory = paper_dashboard_memory_for_monitor.clone();
        #[cfg(feature = "dashboard")]
        let csv_persistence_for_backfill = paper_csv_persistence.clone();
        #[cfg(feature = "dashboard")]
        let mut last_price_broadcast: i64 = 0;
        #[cfg(feature = "dashboard")]
        const PRICE_BROADCAST_INTERVAL_MS: i64 = 1000; // Broadcast prices every 1 second max
        #[cfg(feature = "dashboard")]
        let mut last_trade_backfill_ms: i64 = 0;
        #[cfg(feature = "dashboard")]
        const TRADE_BACKFILL_INTERVAL_MS: i64 = 30_000; // Rehydrate trades from CSV every 30s if empty
        #[cfg(feature = "dashboard")]
        let mut last_ml_broadcast_ms: i64 = 0;
        #[cfg(feature = "dashboard")]
        const ML_METRICS_BROADCAST_INTERVAL_MS: i64 = 30_000; // Broadcast ML metrics every 30s
        Some(tokio::spawn(async move {
            while let Some(tick) = paper_price_rx.recv().await {
                // Update the paper engine with the new price
                let exits = engine.update_price(tick.asset, tick.mid, tick.source);
                #[cfg(feature = "dashboard")]
                let mut closed_dashboard_trades: Vec<TradeResponse> = Vec::new();

                let mut closed_ml_trades: Vec<crate::paper_trading::PaperTradeRecord> = Vec::new();

                let had_exits = !exits.is_empty();
                for ((asset, timeframe), reason) in exits {
                    if let Some(record) = engine.close_and_save(asset, timeframe, reason).await {
                        closed_ml_trades.push(record.clone());
                        #[cfg(feature = "dashboard")]
                        closed_dashboard_trades
                            .push(paper_trade_record_to_dashboard_trade(&record));
                    }
                }

                // Allow V3 ML Strategy to learn continuously from paper trading resolved outcomes
                if !closed_ml_trades.is_empty() {
                    let mut strat = paper_ml_strategy.lock().await;
                    for record in &closed_ml_trades {
                        // Register trade outcome into the ML model so it can learn continuously
                        strat.register_closed_trade_result(record);
                    }
                }

                // Broadcast position updates if any positions were closed
                #[cfg(feature = "dashboard")]
                if !closed_dashboard_trades.is_empty() {
                    for trade in &closed_dashboard_trades {
                        paper_dashboard_memory.add_trade(trade.clone()).await;
                        paper_dashboard_broadcaster.broadcast_trade(trade.clone());
                    }
                }

                #[cfg(feature = "dashboard")]
                if had_exits {
                    let positions: Vec<PositionResponse> = engine
                        .get_positions()
                        .into_iter()
                        .map(|p| {
                            let now_ms = chrono::Utc::now().timestamp_millis();
                            // Use engine's computed unrealized_pnl directly — it already uses
                            // the correct per-asset price from the Polymarket orderbook (or
                            // asset-specific simulation). Never use tick.mid across all assets,
                            // as that applies BTC price to ETH positions (and vice versa).
                            let display_pnl = p.unrealized_pnl;
                            let trading_roi = if p.size_usdc > 0.0 { display_pnl / p.size_usdc * 100.0 } else { 0.0 };
                            PositionResponse {
                            id: p.id.clone(),
                            asset: format!("{:?}", p.asset),
                            timeframe: format!("{:?}", p.timeframe),
                            direction: format!("{:?}", p.direction),
                            entry_price: if p.price_at_market_open > 0.0 { p.price_at_market_open } else { p.entry_price },
                            current_price: p.current_price,
                            size_usdc: p.size_usdc,
                            pnl: display_pnl,
                            pnl_pct: trading_roi,
                            opened_at: p.opened_at,
                            market_slug: p.market_slug.clone(),
                            confidence: p.confidence,
                            peak_price: p.peak_price,
                            trough_price: p.trough_price,
                            market_close_ts: p.market_close_ts,
                            time_remaining_secs: ((p.market_close_ts - now_ms) / 1000).max(0),
                            stop_loss_pct: p.dynamic_hard_stop_roi.abs() * 100.0,
                            take_profit_pct: crate::paper_trading::PaperTradingEngine::dynamic_take_profit_roi(&p, now_ms) * 100.0,
                            checkpoint_armed: p.checkpoint_armed,
                            checkpoint_floor_pct: p.checkpoint_floor_roi * 100.0,
                            checkpoint_peak_pct: p.checkpoint_peak_roi * 100.0,
                            trading_roi,
                            prediction_roi: p.prediction_roi * 100.0,
                            entry_share_price: p.share_price,
                            current_share_price: p.current_share_price,
                        }})
                        .collect();
                    *paper_dashboard_memory.paper_positions.write().await = positions.clone();
                    paper_dashboard_broadcaster.broadcast_positions(positions.clone());
                    // Broadcast stats immediately after position closed
                    let stats = engine.get_stats();
                    let balance = engine.get_balance();
                    let locked = engine.get_locked_balance();
                    let equity = engine.get_total_equity();
                    let stats_response = PaperStatsResponse {
                        total_trades: stats.total_trades,
                        wins: stats.wins,
                        losses: stats.losses,
                        win_rate: if stats.total_trades > 0 {
                            (stats.wins as f64 / stats.total_trades as f64) * 100.0
                        } else {
                            0.0
                        },
                        total_pnl: stats.total_pnl,
                        total_fees: stats.total_fees,
                        largest_win: stats.largest_win,
                        largest_loss: stats.largest_loss,
                        avg_win: if stats.wins > 0 {
                            stats.sum_win_pnl / stats.wins as f64
                        } else {
                            0.0
                        },
                        avg_loss: if stats.losses > 0 {
                            stats.sum_loss_pnl / stats.losses as f64
                        } else {
                            0.0
                        },
                        max_drawdown: stats.max_drawdown,
                        current_drawdown: {
                            let peak = stats.peak_balance;
                            if peak > 0.0 {
                                ((peak - equity) / peak * 100.0).max(0.0)
                            } else {
                                0.0
                            }
                        },
                        peak_balance: stats.peak_balance,
                        profit_factor: if stats.gross_loss > 0.0 {
                            stats.gross_profit / stats.gross_loss
                        } else if stats.gross_profit > 0.0 {
                            f64::INFINITY
                        } else {
                            0.0
                        },
                        current_streak: stats.current_streak,
                        best_streak: stats.best_streak,
                        worst_streak: stats.worst_streak,
                        exits_trailing_stop: stats.exits_trailing_stop,
                        exits_take_profit: stats.exits_take_profit,
                        exits_market_expiry: stats.exits_market_expiry,
                        exits_time_expiry: stats.exits_time_expiry,
                        predictions_correct: stats.predictions_correct,
                        predictions_incorrect: stats.predictions_incorrect,
                        prediction_win_rate: if stats.total_trades > 0 {
                            (stats.predictions_correct as f64 / stats.total_trades as f64) * 100.0
                        } else {
                            0.0
                        },
                        trading_wins: stats.trading_wins,
                        trading_losses: stats.trading_losses,
                        trading_win_rate: if stats.trading_wins + stats.trading_losses > 0 {
                            (stats.trading_wins as f64
                                / (stats.trading_wins + stats.trading_losses) as f64)
                                * 100.0
                        } else {
                            0.0
                        },
                    };
                    paper_dashboard_broadcaster.broadcast_stats(stats_response);
                }
                engine.maybe_print_dashboard(); // ── Update Dashboard API ──
                #[cfg(feature = "dashboard")]
                {
                    use crate::dashboard::{PaperStatsResponse, PositionResponse}; // Update price in dashboard
                    paper_dashboard_memory
                        .update_price_at(
                            tick.asset,
                            tick.mid,
                            tick.bid,
                            tick.ask,
                            tick.source,
                            tick.exchange_ts,
                        )
                        .await; // Update paper trading state
                    let stats = engine.get_stats();
                    let balance = engine.get_balance();
                    let locked = engine.get_locked_balance();
                    let equity = engine.get_total_equity();
                    let unrealized = equity - balance - locked; // Update dashboard memory
                    *paper_dashboard_memory.paper_balance.write().await = balance;
                    *paper_dashboard_memory.paper_locked.write().await = locked;
                    *paper_dashboard_memory.paper_unrealized_pnl.write().await = unrealized; // Convert stats to response type
                    let stats_response = PaperStatsResponse {
                        total_trades: stats.total_trades,
                        wins: stats.wins,
                        losses: stats.losses,
                        win_rate: if stats.total_trades > 0 {
                            (stats.wins as f64 / stats.total_trades as f64) * 100.0
                        } else {
                            0.0
                        },
                        total_pnl: stats.total_pnl,
                        total_fees: stats.total_fees,
                        largest_win: stats.largest_win,
                        largest_loss: stats.largest_loss,
                        avg_win: if stats.wins > 0 {
                            stats.sum_win_pnl / stats.wins as f64
                        } else {
                            0.0
                        },
                        avg_loss: if stats.losses > 0 {
                            stats.sum_loss_pnl / stats.losses as f64
                        } else {
                            0.0
                        },
                        max_drawdown: stats.max_drawdown,
                        current_drawdown: {
                            let peak = stats.peak_balance;
                            if peak > 0.0 {
                                ((peak - equity) / peak * 100.0).max(0.0)
                            } else {
                                0.0
                            }
                        },
                        peak_balance: stats.peak_balance,
                        profit_factor: if stats.gross_loss > 0.0 {
                            stats.gross_profit / stats.gross_loss
                        } else if stats.gross_profit > 0.0 {
                            f64::INFINITY
                        } else {
                            0.0
                        },
                        current_streak: stats.current_streak,
                        best_streak: stats.best_streak,
                        worst_streak: stats.worst_streak,
                        exits_trailing_stop: stats.exits_trailing_stop,
                        exits_take_profit: stats.exits_take_profit,
                        exits_market_expiry: stats.exits_market_expiry,
                        exits_time_expiry: stats.exits_time_expiry,
                        predictions_correct: stats.predictions_correct,
                        predictions_incorrect: stats.predictions_incorrect,
                        prediction_win_rate: if stats.total_trades > 0 {
                            (stats.predictions_correct as f64 / stats.total_trades as f64) * 100.0
                        } else {
                            0.0
                        },
                        trading_wins: stats.trading_wins,
                        trading_losses: stats.trading_losses,
                        trading_win_rate: if stats.trading_wins + stats.trading_losses > 0 {
                            (stats.trading_wins as f64
                                / (stats.trading_wins + stats.trading_losses) as f64)
                                * 100.0
                        } else {
                            0.0
                        },
                    };
                    *paper_dashboard_memory.paper_stats.write().await = stats_response.clone(); // Update positions
                    let positions: Vec<PositionResponse> = engine
                        .get_positions()
                        .into_iter()
                        .map(|p| {
                            let now_ms = chrono::Utc::now().timestamp_millis();
                            // Use engine's computed unrealized_pnl — correct per-asset prices.
                            let display_pnl = p.unrealized_pnl;
                            let trading_roi = if p.size_usdc > 0.0 { display_pnl / p.size_usdc * 100.0 } else { 0.0 };
                            PositionResponse {
                            id: p.id.clone(),
                            asset: format!("{:?}", p.asset),
                            timeframe: format!("{:?}", p.timeframe),
                            direction: format!("{:?}", p.direction),
                            entry_price: if p.price_at_market_open > 0.0 { p.price_at_market_open } else { p.entry_price },
                            current_price: p.current_price,
                            size_usdc: p.size_usdc,
                            pnl: display_pnl,
                            pnl_pct: trading_roi,
                            opened_at: p.opened_at,
                            market_slug: p.market_slug.clone(),
                            confidence: p.confidence,
                            peak_price: p.peak_price,
                            trough_price: p.trough_price,
                            market_close_ts: p.market_close_ts,
                            time_remaining_secs: ((p.market_close_ts - now_ms) / 1000).max(0),
                            stop_loss_pct: p.dynamic_hard_stop_roi.abs() * 100.0,
                            take_profit_pct: crate::paper_trading::PaperTradingEngine::dynamic_take_profit_roi(&p, now_ms) * 100.0,
                            checkpoint_armed: p.checkpoint_armed,
                            checkpoint_floor_pct: p.checkpoint_floor_roi * 100.0,
                            checkpoint_peak_pct: p.checkpoint_peak_roi * 100.0,
                            trading_roi,
                            prediction_roi: p.prediction_roi * 100.0,
                            entry_share_price: p.share_price,
                            current_share_price: p.current_share_price,
                        }})
                        .collect();
                    *paper_dashboard_memory.paper_positions.write().await = positions.clone(); // Safety net: if dashboard started before CSV had data, rehydrate recent trades from CSV.
                    let now = chrono::Utc::now().timestamp_millis();
                    if now - last_trade_backfill_ms >= TRADE_BACKFILL_INTERVAL_MS {
                        last_trade_backfill_ms = now;
                        let trades_empty =
                            paper_dashboard_memory.paper_trades.read().await.is_empty();
                        if trades_empty {
                            match csv_persistence_for_backfill.load_recent_paper_trades(10_000) {
                                Ok(backfilled) if !backfilled.is_empty() => {
                                    let loaded = backfilled.len();
                                    paper_dashboard_memory.set_paper_trades(backfilled).await;
                                    tracing::info!(
                                        loaded_trades = loaded,
                                        "Dashboard recent_trades rehydrated from CSV"
                                    );
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    tracing::warn!(error = %e, "Failed to backfill dashboard recent_trades from CSV");
                                }
                            }
                        }
                    } // Broadcast price update (throttled to prevent UI flickering)
                    if now - last_price_broadcast >= PRICE_BROADCAST_INTERVAL_MS {
                        last_price_broadcast = now;
                        paper_dashboard_broadcaster
                            .broadcast_prices(paper_dashboard_memory.get_prices().await.prices);
                        paper_dashboard_broadcaster.broadcast_positions(positions.clone()); // Also broadcast stats for real-time dashboard updates
                        paper_dashboard_broadcaster.broadcast_stats(stats_response);
                    }
                }

                // ML metrics heartbeat: broadcast every 30s regardless of exits
                #[cfg(feature = "dashboard")]
                {
                    let now_ml = chrono::Utc::now().timestamp_millis();
                    if now_ml - last_ml_broadcast_ms >= ML_METRICS_BROADCAST_INTERVAL_MS {
                        last_ml_broadcast_ms = now_ml;
                        let strat = paper_ml_strategy.lock().await;
                        let ml_state = strat.get_ml_state();
                        drop(strat);
                        let weights: Vec<(String, f64, f64)> = ml_state.model_info.clone();
                        paper_dashboard_broadcaster.broadcast_ml_metrics(
                            ml_state.model_accuracy,
                            ml_state.win_rate,
                            ml_state.loss_rate,
                            ml_state.total_predictions,
                            ml_state.correct_predictions,
                            ml_state.incorrect_predictions,
                            weights,
                            ml_state.training_epoch,
                            ml_state.dataset_size,
                        );
                        paper_dashboard_memory.update_ml_metrics(ml_state).await;
                    }
                }
            }
        }))
    } else {
        // Drain the channel so it doesn't block
        tokio::spawn(async move { while paper_price_rx.recv().await.is_some() {} });
        None
    };
    // Paper trading: periodic task to close expired positions (every 5 seconds)
    let expired_positions_engine = paper_engine.clone();
    #[cfg(feature = "dashboard")]
    let expired_positions_dashboard = dashboard_memory.clone();
    #[cfg(feature = "dashboard")]
    let expired_positions_broadcaster = dashboard_broadcaster.clone();
    let expired_positions_handle = if paper_trading_enabled {
        Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                if let Some(ref engine) = expired_positions_engine {
                    let closed_trades = engine.close_all_expired_positions().await;
                    if !closed_trades.is_empty() {
                        tracing::info!(count = closed_trades.len(), "📋 Closed expired positions");

                        let mut strat = expired_ml_strategy.lock().await;
                        for record in &closed_trades {
                            // Register trade outcome into the ML model so it can learn continuously
                            strat.register_closed_trade_result(record);
                        }
                        drop(strat);

                        #[cfg(feature = "dashboard")]
                        {
                            use crate::dashboard::TradeResponse;
                            for record in &closed_trades {
                                let trade = paper_trade_record_to_dashboard_trade(record);
                                expired_positions_dashboard.add_trade(trade.clone()).await;
                                expired_positions_broadcaster.broadcast_trade(trade);
                            }
                        }
                    }
                }
            }
        }))
    } else {
        None
    };
    // Clone strategy for the signal processing loop so we can confirm windows after risk approval.
    let strategy_for_signals = strategy.clone();
    // Main loop: process signals through risk manager and execute
    info!("🎯 Entering main trading loop...");
    loop {
        tokio::select! {
            Some(signal) = signal_rx.recv() => {
                let signal_id = signal.id.clone();

                #[cfg(feature = "dashboard")]
                let paper_trading_active = paper_dashboard_memory
                    .trading_mode
                    .load(std::sync::atomic::Ordering::Relaxed);
                #[cfg(not(feature = "dashboard"))]
                let paper_trading_active = paper_trading_enabled;

                if paper_trading_active {
                    if let Some(ref engine) = paper_engine {
                        match risk_manager.evaluate(&signal) {
                            Ok(approved) => {
                                if approved {
                                    info!(
                                        signal_id = %signal_id,
                                        asset = %signal.asset,
                                        direction = ?signal.direction,
                                        confidence = %signal.confidence,
                                        "📋 [PAPER] Signal approved"
                                    );
                                    // Mark window as entered now that risk manager approved
                                    {
                                        let window_ms = match signal.timeframe {
                                            Timeframe::Min15 => 15 * 60 * 1000_i64,
                                            Timeframe::Hour1 => 60 * 60 * 1000_i64,
                                        };
                                        let win_open_ts = (signal.ts / window_ms) * window_ms;
                                        strategy_for_signals.lock().await.confirm_window_entered(signal.asset, signal.timeframe, win_open_ts);
                                    }

                                    if !signal.token_id.trim().is_empty() {
                                        match clob_client.quote_token(&signal.token_id).await {
                                            Ok(q) if q.bid > 0.0 && q.ask > 0.0 && q.mid > 0.0 => {
                                                let direction_str = match signal.direction {
                                                    Direction::Up => "UP",
                                                    Direction::Down => "DOWN",
                                                };
                                                main_loop_share_prices.update_quote_with_depth(
                                                    signal.asset,
                                                    signal.timeframe,
                                                    direction_str,
                                                    q.bid,
                                                    q.ask,
                                                    q.mid,
                                                    q.bid_size,
                                                    q.ask_size,
                                                    q.depth_top5,
                                                );
                                            }
                                            Ok(_) => {
                                                warn!(
                                                    signal_id = %signal_id,
                                                    token_id = %signal.token_id,
                                                    "Paper pre-execution quote invalid; keeping existing quote state"
                                                );
                                            }
                                            Err(e) => {
                                                warn!(
                                                    signal_id = %signal_id,
                                                    token_id = %signal.token_id,
                                                    error = %e,
                                                    "Paper pre-execution quote fetch failed; keeping existing quote state"
                                                );
                                            }
                                        }
                                    }

                                    match engine.execute_signal(&signal) {
                                        Ok(true) => {
                                            info!(signal_id = %signal_id, "📋 [PAPER] Order filled");
                                            #[cfg(feature = "dashboard")]
                                            {
                                                use crate::dashboard::PositionResponse;

                                                if let Some(pos) = engine
                                                    .get_positions()
                                                    .iter()
                                                    .find(|p| p.market_slug == signal.market_slug)
                                                {
                                                    let position_response = PositionResponse {
                                                        id: pos.id.clone(),
                                                        asset: format!("{:?}", pos.asset),
                                                        timeframe: format!("{:?}", pos.timeframe),
                                                        direction: format!("{:?}", pos.direction),
                                                        entry_price: if pos.price_at_market_open > 0.0 {
                                                            pos.price_at_market_open
                                                        } else {
                                                            pos.entry_price
                                                        },
                                                        current_price: pos.current_price,
                                                        size_usdc: pos.size_usdc,
                                                        pnl: pos.unrealized_pnl,
                                                        pnl_pct: 0.0,
                                                        opened_at: pos.opened_at,
                                                        market_slug: pos.market_slug.clone(),
                                                        confidence: pos.confidence,
                                                        peak_price: pos.peak_price,
                                                        trough_price: pos.trough_price,
                                                        market_close_ts: pos.market_close_ts,
                                                        time_remaining_secs: ((pos.market_close_ts - chrono::Utc::now().timestamp_millis()) / 1000).max(0),
                                                        stop_loss_pct: pos.dynamic_hard_stop_roi.abs() * 100.0,
                                                        take_profit_pct: crate::paper_trading::PaperTradingEngine::dynamic_take_profit_roi(
                                                            pos,
                                                            chrono::Utc::now().timestamp_millis(),
                                                        ) * 100.0,
                                                        checkpoint_armed: pos.checkpoint_armed,
                                                        checkpoint_floor_pct: pos.checkpoint_floor_roi * 100.0,
                                                        checkpoint_peak_pct: pos.checkpoint_peak_roi * 100.0,
                                                        trading_roi: 0.0,
                                                        prediction_roi: 0.0,
                                                        entry_share_price: pos.share_price,
                                                        current_share_price: pos.current_share_price,
                                                    };
                                                    main_loop_broadcaster.broadcast_position_opened(position_response);
                                                }

                                                let stats = engine.get_stats();
                                                let equity = engine.get_total_equity();
                                                let stats_response = crate::dashboard::PaperStatsResponse {
                                                    total_trades: stats.total_trades,
                                                    wins: stats.wins,
                                                    losses: stats.losses,
                                                    win_rate: if stats.total_trades > 0 {
                                                        (stats.wins as f64 / stats.total_trades as f64) * 100.0
                                                    } else {
                                                        0.0
                                                    },
                                                    total_pnl: stats.total_pnl,
                                                    total_fees: stats.total_fees,
                                                    largest_win: stats.largest_win,
                                                    largest_loss: stats.largest_loss,
                                                    avg_win: if stats.wins > 0 {
                                                        stats.sum_win_pnl / stats.wins as f64
                                                    } else {
                                                        0.0
                                                    },
                                                    avg_loss: if stats.losses > 0 {
                                                        stats.sum_loss_pnl / stats.losses as f64
                                                    } else {
                                                        0.0
                                                    },
                                                    max_drawdown: stats.max_drawdown,
                                                    current_drawdown: {
                                                        let peak = stats.peak_balance;
                                                        if peak > 0.0 {
                                                            ((peak - equity) / peak * 100.0).max(0.0)
                                                        } else {
                                                            0.0
                                                        }
                                                    },
                                                    peak_balance: stats.peak_balance,
                                                    profit_factor: if stats.gross_loss > 0.0 {
                                                        stats.gross_profit / stats.gross_loss
                                                    } else if stats.gross_profit > 0.0 {
                                                        f64::INFINITY
                                                    } else {
                                                        0.0
                                                    },
                                                    current_streak: stats.current_streak,
                                                    best_streak: stats.best_streak,
                                                    worst_streak: stats.worst_streak,
                                                    exits_trailing_stop: stats.exits_trailing_stop,
                                                    exits_take_profit: stats.exits_take_profit,
                                                    exits_market_expiry: stats.exits_market_expiry,
                                                    exits_time_expiry: stats.exits_time_expiry,
                                                    predictions_correct: stats.predictions_correct,
                                                    predictions_incorrect: stats.predictions_incorrect,
                                                    prediction_win_rate: if stats.total_trades > 0 {
                                                        (stats.predictions_correct as f64 / stats.total_trades as f64) * 100.0
                                                    } else {
                                                        0.0
                                                    },
                                                    trading_wins: stats.trading_wins,
                                                    trading_losses: stats.trading_losses,
                                                    trading_win_rate: if stats.trading_wins + stats.trading_losses > 0 {
                                                        (stats.trading_wins as f64
                                                            / (stats.trading_wins + stats.trading_losses) as f64)
                                                            * 100.0
                                                    } else {
                                                        0.0
                                                    },
                                                };
                                                main_loop_broadcaster.broadcast_stats(stats_response);
                                            }
                                        }
                                        Ok(false) => {
                                            info!(signal_id = %signal_id, "📋 [PAPER] Order rejected (balance/position)");
                                            #[cfg(feature = "dashboard")]
                                            paper_dashboard_memory
                                                .record_execution_rejection("paper_engine_rejected")
                                                .await;
                                        }
                                        Err(e) => {
                                            error!(error = %e, "📋 [PAPER] Execute failed");
                                            #[cfg(feature = "dashboard")]
                                            paper_dashboard_memory
                                                .record_execution_rejection("paper_execute_error")
                                                .await;
                                        }
                                    }
                                } else {
                                    info!(signal_id = %signal_id, "⏸️ Signal rejected by risk manager");
                                    #[cfg(feature = "dashboard")]
                                    paper_dashboard_memory
                                        .record_execution_rejection("risk_manager_rejected")
                                        .await;
                                }
                            }
                            Err(e) => {
                                error!(error = %e, "Risk evaluation failed");
                                #[cfg(feature = "dashboard")]
                                paper_dashboard_memory
                                    .record_execution_rejection("risk_evaluation_error")
                                    .await;
                            }
                        }
                    }
                    continue;
                }

                let window_ms = signal.timeframe.duration_secs() as i64 * 1000;
                let window_start = if signal.expires_at > 0 {
                    signal.expires_at - window_ms
                } else {
                    (signal.ts / window_ms) * window_ms
                };
                let live_bias_key = (signal.asset, signal.timeframe, window_start);
                {
                    let mut bias_map = live_window_bias.lock().await;
                    let cutoff = chrono::Utc::now().timestamp_millis()
                        - (Timeframe::Hour1.duration_secs() as i64 * 1000 * 2);
                    bias_map.retain(|(_, _, ws), _| *ws >= cutoff);
                    if let Some(existing) = bias_map.get(&live_bias_key) {
                        if *existing != signal.direction {
                            warn!(
                                signal_id = %signal_id,
                                asset = ?signal.asset,
                                timeframe = ?signal.timeframe,
                                window_start = window_start,
                                existing = ?existing,
                                incoming = ?signal.direction,
                                "Skipping live signal: opposite bias already active for window"
                            );
                            continue;
                        }
                    }
                }

                match risk_manager.evaluate(&signal) {
                    Ok(approved) => {
                        if approved {
                            warn!(
                                signal_id = %signal_id,
                                asset = %signal.asset,
                                direction = ?signal.direction,
                                confidence = %signal.confidence,
                                "🟥 [LIVE] Signal approved by risk manager - preparing REAL order submission"
                            );
                            // Mark window as entered now that risk manager approved
                            {
                                let window_ms = match signal.timeframe {
                                    Timeframe::Min15 => 15 * 60 * 1000_i64,
                                    Timeframe::Hour1 => 60 * 60 * 1000_i64,
                                };
                                let win_open_ts = (signal.ts / window_ms) * window_ms;
                                strategy_for_signals.lock().await.confirm_window_entered(signal.asset, signal.timeframe, win_open_ts);
                            }

                            let market_slug = &signal.market_slug;
                            let token_id = signal.token_id.clone();
                            if token_id.trim().is_empty() {
                                warn!(
                                    signal_id = %signal_id,
                                    market_slug = %market_slug,
                                    "Could not resolve token_id from strategy signal, skipping order"
                                );
                                continue;
                            }

                            let quote = match clob_client.quote_token(&token_id).await {
                                Ok(q) => q,
                                Err(e) => {
                                    warn!(
                                        signal_id = %signal_id,
                                        token_id = %token_id,
                                        error = %e,
                                        "Could not quote token, skipping order"
                                    );
                                    continue;
                                }
                            };

                            let p_market = quote.mid.clamp(0.01, 0.99);
                            let p_model = if signal.direction == Direction::Up {
                                signal.model_prob_up.clamp(0.01, 0.99)
                            } else {
                                (1.0 - signal.model_prob_up).clamp(0.01, 0.99)
                            };
                            let max_spread = match signal.timeframe {
                                Timeframe::Min15 => 1.0,
                                Timeframe::Hour1 => 1.0,
                            };
                            if quote.spread > max_spread {
                                info!(signal_id = %signal_id, spread = quote.spread, max_spread = max_spread, "Skipping order due to spread policy");
                                #[cfg(feature = "dashboard")]
                                live_main_dashboard_memory.record_execution_rejection("spread_too_wide").await;
                                continue;
                            }

                            let min_depth_top5 = match signal.timeframe {
                                Timeframe::Min15 => 50.0,
                                Timeframe::Hour1 => 25.0,
                            };
                            if quote.depth_top5 > 0.0 && quote.depth_top5 < min_depth_top5 {
                                info!(signal_id = %signal_id, depth_top5 = quote.depth_top5, min_depth_top5 = min_depth_top5, "Skipping order due to low depth policy");
                                #[cfg(feature = "dashboard")]
                                live_main_dashboard_memory.record_execution_rejection("depth_too_low").await;
                                continue;
                            }

                            let fee_rate = crate::polymarket::fee_rate_from_price(p_market);
                            let ev = crate::polymarket::estimate_expected_value(
                                p_market,
                                p_model,
                                p_market,
                                fee_rate,
                                quote.spread.max(0.0),
                                0.005,
                            );
                            if ev.edge_net <= 0.0 {
                                info!(signal_id = %signal_id, edge_net = ev.edge_net, "Skipping order due to non-positive edge after costs");
                                #[cfg(feature = "dashboard")]
                                live_main_dashboard_memory.record_execution_rejection("edge_below_floor").await;
                                continue;
                            }

                            let now_ms = chrono::Utc::now().timestamp_millis();
                            let seconds_to_expiry = if signal.expires_at > 0 {
                                ((signal.expires_at - now_ms) / 1000).max(0)
                            } else {
                                i64::MAX
                            };

                            if seconds_to_expiry < 60 {
                                info!(signal_id = %signal_id, seconds_to_expiry, "Skipping order due to close expiry (< 60s remaining)");
                                #[cfg(feature = "dashboard")]
                                live_main_dashboard_memory.record_execution_rejection("too_close_to_expiry").await;
                                continue;
                            }

                            let exec_plan = match crate::polymarket::plan_buy_execution(
                                quote.bid,
                                quote.ask,
                                0.001,
                                config.execution.maker_first,
                                config.execution.post_only,
                                seconds_to_expiry,
                                config.execution.fallback_taker_seconds_to_expiry,
                                ev.edge_net,
                            ) {
                                Some(plan) => plan,
                                None => {
                                    warn!(signal_id = %signal_id, bid = quote.bid, ask = quote.ask, "Invalid quote for execution plan");
                                    continue;
                                }
                            };

                            let live_available_usdc = position_risk.get_balance().max(0.0);
                            let live_sizing_cap = config.risk.max_position_usdc.max(0.0);
                            let mut effective_size_usdc = if live_available_usdc > 0.0 {
                                position_risk
                                    .calculate_position_size(&signal)
                                    .min(live_available_usdc)
                            } else {
                                signal
                                    .suggested_size_usdc
                                    .max(0.0)
                                    .min(live_sizing_cap)
                            };

                            if effective_size_usdc <= 0.0 {
                                warn!(
                                    signal_id = %signal_id,
                                    available_usdc = live_available_usdc,
                                    suggested_usdc = signal.suggested_size_usdc,
                                    "Skipping LIVE order due to non-positive effective notional"
                                );
                                continue;
                            }

                            let mut shares_size = if exec_plan.entry_price > 0.0 {
                                (effective_size_usdc / exec_plan.entry_price).max(0.0)
                            } else {
                                0.0
                            };
                            if shares_size <= 0.0 {
                                warn!(signal_id = %signal_id, notional_usdc = effective_size_usdc, entry_price = exec_plan.entry_price, "Skipping order due to non-positive share size");
                                continue;
                            }

                            if let Some(market) = clob_client.get_market(&signal.condition_id).await {
                                let min_order_size = market.order_min_size.max(0.0);
                                if min_order_size > 0.0 && shares_size + f64::EPSILON < min_order_size {
                                    let required_size_usdc = min_order_size * exec_plan.entry_price;
                                    let within_live_cap =
                                        required_size_usdc <= live_sizing_cap + f64::EPSILON;
                                    let can_raise_with_balance = live_available_usdc > 0.0
                                        && required_size_usdc <= live_available_usdc + f64::EPSILON
                                        && within_live_cap;
                                    let can_raise_without_balance = live_available_usdc <= 0.0
                                        && required_size_usdc <= effective_size_usdc + f64::EPSILON
                                        && within_live_cap;

                                    if can_raise_with_balance || can_raise_without_balance {
                                        info!(
                                            signal_id = %signal_id,
                                            old_shares_size = shares_size,
                                            new_shares_size = min_order_size,
                                            old_size_usdc = effective_size_usdc,
                                            new_size_usdc = required_size_usdc,
                                            condition_id = %signal.condition_id,
                                            "Raised LIVE order to market minimum size"
                                        );
                                        shares_size = min_order_size;
                                        effective_size_usdc = required_size_usdc;
                                    } else {
                                        warn!(
                                            signal_id = %signal_id,
                                            shares_size,
                                            min_order_size,
                                            effective_size_usdc,
                                            required_size_usdc,
                                            live_available_usdc,
                                            live_sizing_cap,
                                            condition_id = %signal.condition_id,
                                            "Skipping LIVE order below market minimum size"
                                        );
                                        continue;
                                    }
                                }
                            } else {
                                warn!(
                                    signal_id = %signal_id,
                                    condition_id = %signal.condition_id,
                                    "LIVE order proceeding without cached market minimum size metadata"
                                );
                            }

                            let mut order = Order::new(
                                token_id.clone(),
                                clob::Side::Buy,
                                exec_plan.entry_price,
                                shares_size,
                            )
                            .with_auth(config.execution.signature_type, None, None);
                            order.condition_id = Some(signal.condition_id.clone());
                            if exec_plan.post_only {
                                // Polymarket expects GTD expiration as a UTC seconds timestamp.
                                order.expiration = (signal.expires_at.max(0) / 1000) as u64;
                            } else {
                                // Taker path: use FAK so the order is immediately filled or cancelled.
                                // Without this, the order defaults to GTC which may sit on the book.
                                order.order_type = Some("FAK".to_string());
                                order.expiration = 0;
                            }

                            match clob_client.execute_order(&order).await {
                                Ok(order_id) => {
                                    live_recently_closed_for_main.lock().await.remove(&signal.token_id);
                                    live_pending_closes_for_main.lock().await.remove(&signal.token_id);

                                    position_risk.open_position(&signal, effective_size_usdc, exec_plan.entry_price);
                                    {
                                        let mut contexts = live_contexts_for_main.lock().await;
                                        contexts.insert(
                                            signal.token_id.clone(),
                                            LivePositionContext {
                                                signal_id: signal.id.clone(),
                                                asset: signal.asset,
                                                timeframe: signal.timeframe,
                                                direction: signal.direction,
                                                confidence: signal.confidence,
                                                market_slug: signal.market_slug.clone(),
                                                token_id: signal.token_id.clone(),
                                                condition_id: signal.condition_id.clone(),
                                                size_usdc: effective_size_usdc,
                                                shares_size,
                                                entry_share_price: exec_plan.entry_price,
                                                opened_at_ms: chrono::Utc::now().timestamp_millis(),
                                                expires_at_ms: signal.expires_at,
                                            },
                                        );
                                        persist_live_position_contexts(
                                            &live_contexts_path_for_main,
                                            &contexts,
                                        );
                                    }
                                    live_window_bias.lock().await.insert(live_bias_key, signal.direction);

                                    if !signal.indicators_used.is_empty() {
                                        let p_model = if signal.direction == Direction::Up {
                                            signal.model_prob_up.clamp(0.01, 0.99)
                                        } else {
                                            (1.0 - signal.model_prob_up).clamp(0.01, 0.99)
                                        };
                                        live_indicators_for_main.lock().await.insert(
                                            (signal.asset, signal.timeframe),
                                            (signal.indicators_used.clone(), p_model),
                                        );
                                    }

                                    #[cfg(feature = "dashboard")]
                                    {
                                        let live_checkpoint_state = position_risk
                                            .get_position_by_token_id(&signal.token_id)
                                            .or_else(|| position_risk.get_position(signal.asset));
                                        let position = PositionResponse {
                                            id: signal.token_id.clone(),
                                            asset: signal.asset.to_string(),
                                            timeframe: signal.timeframe.to_string(),
                                            direction: signal.direction.to_string(),
                                            entry_price: exec_plan.entry_price,
                                            current_price: quote.mid,
                                            size_usdc: effective_size_usdc,
                                            pnl: 0.0,
                                            pnl_pct: 0.0,
                                            opened_at: chrono::Utc::now().timestamp_millis(),
                                            market_slug: signal.market_slug.clone(),
                                            confidence: signal.confidence,
                                            peak_price: quote.mid,
                                            trough_price: quote.mid,
                                            market_close_ts: signal.expires_at,
                                            time_remaining_secs: ((signal.expires_at - chrono::Utc::now().timestamp_millis()) / 1000).max(0),
                                            stop_loss_pct: live_checkpoint_state
                                                .as_ref()
                                                .map(|position| position.dynamic_hard_stop_roi.abs() * 100.0)
                                                .unwrap_or(live_stop_loss_pct),
                                            take_profit_pct: live_take_profit_pct,
                                            checkpoint_armed: live_checkpoint_state
                                                .as_ref()
                                                .map(|position| position.checkpoint_armed)
                                                .unwrap_or(false),
                                            checkpoint_floor_pct: live_checkpoint_state
                                                .as_ref()
                                                .map(|position| position.checkpoint_floor_roi * 100.0)
                                                .unwrap_or(0.0),
                                            checkpoint_peak_pct: live_checkpoint_state
                                                .as_ref()
                                                .map(|position| position.checkpoint_peak_roi * 100.0)
                                                .unwrap_or(0.0),
                                            trading_roi: 0.0,
                                            prediction_roi: 0.0,
                                            entry_share_price: exec_plan.entry_price,
                                            current_share_price: quote.mid,
                                        };
                                        let snapshot = {
                                            let mut live_positions = live_main_dashboard_memory.live_positions.write().await;
                                            live_positions.retain(|existing| existing.id != position.id);
                                            live_positions.push(position);
                                            live_positions.clone()
                                        };
                                        main_loop_broadcaster.broadcast_live_positions(snapshot);
                                    }

                                    info!(signal_id = %signal_id, order_id = %order_id, "📈 LIVE order submitted successfully");
                                }
                                Err(e) => {
                                    error!(error = %e, signal_id = %signal_id, "Failed to execute live order");
                                    #[cfg(feature = "dashboard")]
                                    live_main_dashboard_memory.record_execution_rejection("live_execute_error").await;
                                }
                            }
                        } else {
                            info!(signal_id = %signal_id, "⏸️ Signal rejected by risk manager");
                            #[cfg(feature = "dashboard")]
                            live_main_dashboard_memory.record_execution_rejection("risk_manager_rejected").await;
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Risk evaluation failed");
                        #[cfg(feature = "dashboard")]
                        live_main_dashboard_memory.record_execution_rejection("risk_evaluation_error").await;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("🛑 Shutdown signal received");
                break;
            }
        }
    }

    info!("Shutting down...");
    oracle_handle.abort();
    feature_handle.abort();
    strategy_handle.abort();
    execution_handle.abort();
    position_handle.abort();
    balance_handle.abort();
    if let Some(handle) = paper_monitor_handle {
        handle.abort();
    }
    if let Some(handle) = expired_positions_handle {
        handle.abort();
    }
    // Print final paper trading stats
    if let Some(ref engine) = paper_engine {
        info!("📋 ═══ FINAL PAPER TRADING REPORT ═══");
        engine.print_dashboard();
        info!("{}", engine.summary_string());
        if let Err(e) = engine.save_state() {
            warn!("Failed to save paper engine state on shutdown: {}", e);
        }
    }

    // Explicitly explicitly save ML state and calibrators for safe restarts
    {
        let mut s = strategy.lock().await;
        s.force_save_state();
    }

    // Flush pending data
    // csv_persistence automatically flushes on each write
    info!("👋 PolyBot stopped");
    Ok(())
}
#[cfg(feature = "dashboard")]
fn paper_trade_record_to_dashboard_trade(
    record: &crate::paper_trading::PaperTradeRecord,
) -> TradeResponse {
    TradeResponse {
        timestamp: record.timestamp,
        trade_id: record.trade_id.clone(),
        asset: record.asset.clone(),
        timeframe: record.timeframe.clone(),
        direction: record.direction.clone(),
        confidence: record.confidence,
        entry_price: if record.window_open_price > 0.0 {
            record.window_open_price
        } else {
            record.entry_price
        },
        exit_price: record.exit_price,
        size_usdc: record.size_usdc,
        pnl: record.pnl,
        pnl_pct: record.pnl_pct,
        result: record.result.clone(),
        prediction_correct: record.prediction_correct,
        exit_reason: record.exit_reason.clone(),
        hold_duration_secs: record.hold_duration_ms / 1000,
        balance_after: record.balance_after,
        entry_share_price: record.entry_mid,
        exit_share_price: record.exit_mid,
        trading_win: record.trading_win,
        rsi_at_entry: None,
        macd_hist_at_entry: None,
        bb_position_at_entry: None,
        adx_at_entry: None,
        volatility_at_entry: None,
    }
}
#[derive(Debug, Default)]
struct RuntimeArgs {
    reset_mode: Option<String>,
    no_backup: bool,
}
fn parse_runtime_args() -> Result<RuntimeArgs> {
    let mut args = RuntimeArgs::default();
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        if arg == "--no-backup" {
            args.no_backup = true;
            continue;
        }
        if arg == "--reset" {
            let mode = iter.next().ok_or_else(|| {
                anyhow::anyhow!("--reset requires a mode (supported: hard-all-history)")
            })?;
            args.reset_mode = Some(mode);
            continue;
        }
        if let Some(mode) = arg.strip_prefix("--reset=") {
            if mode.trim().is_empty() {
                anyhow::bail!("--reset requires a mode (supported: hard-all-history)");
            }
            args.reset_mode = Some(mode.to_string());
        }
    }
    Ok(args)
}
fn maybe_run_startup_reset(config: &AppConfig, runtime_args: &RuntimeArgs) -> Result<bool> {
    let requested_mode = runtime_args.reset_mode.as_ref().map(|s| s.as_str());
    if requested_mode.is_none() && config.reset.enabled_on_start {
        warn!(
            configured_mode = %config.reset.mode,
            "Ignoring deprecated startup reset configuration; use --reset hard_all_history for one-time wipes"
        );
    }
    let Some(mode) = requested_mode else {
        return Ok(false);
    };
    let normalized_mode = mode.to_ascii_lowercase().replace('-', "_");
    if normalized_mode != "hard_all_history" {
        anyhow::bail!(
            "Unsupported reset mode '{}'. Supported: hard_all_history / hard-all-history",
            mode
        );
    }
    let options = HardResetOptions {
        no_backup: runtime_args.no_backup || config.reset.no_backup,
        delete_prices: config.reset.delete_prices,
        delete_learning_state: config.reset.delete_learning_state,
        delete_paper_state: config.reset.delete_paper_state,
    };
    info!(        mode = %normalized_mode,        no_backup = options.no_backup,        delete_prices = options.delete_prices,        delete_learning_state = options.delete_learning_state,        delete_paper_state = options.delete_paper_state,        data_dir = %config.persistence.data_dir,        "Executing startup hard reset"    );
    CsvPersistence::hard_reset_with_options(&config.persistence.data_dir, options)?;
    Ok(true)
}
fn default_anchor_price(asset: Asset) -> f64 {
    match asset {
        Asset::BTC => 100_000.0,
        Asset::ETH => 3_000.0,
        Asset::SOL => 200.0,
        Asset::XRP => 1.0,
    }
}
fn normalize_history_timestamp_ms(ts: i64) -> i64 {
    if ts.abs() < 100_000_000_000 {
        ts.saturating_mul(1000)
    } else {
        ts
    }
}
fn load_local_price_points(data_dir: &str, asset: Asset, lookback_hours: i64) -> Vec<(i64, f64)> {
    use chrono::{Duration, Utc};
    use std::path::PathBuf;
    let mut rows: Vec<(i64, f64)> = Vec::new();
    let now = Utc::now();
    let start_ts = now.timestamp_millis() - lookback_hours.max(1) * 3_600_000;
    let days = ((lookback_hours.max(1) + 23) / 24) + 2;
    let prices_dir = PathBuf::from(data_dir).join("prices");
    let asset_label = asset.to_string();
    for day_offset in 0..=days {
        let date = now - Duration::days(day_offset);
        let filename = format!("prices_{}.csv", date.format("%Y-%m-%d"));
        let path = prices_dir.join(filename);
        if !path.exists() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let cols: Vec<&str> = trimmed.split(',').map(str::trim).collect();
            if cols.len() < 3 {
                continue;
            }
            let Ok(raw_ts) = cols[0].parse::<i64>() else {
                continue;
            };
            let ts = normalize_history_timestamp_ms(raw_ts);
            if ts < start_ts {
                continue;
            }
            if !cols[1].eq_ignore_ascii_case(&asset_label) {
                continue;
            }
            let Ok(price) = cols[2].parse::<f64>() else {
                continue;
            };
            if !price.is_finite() || price <= 0.0 {
                continue;
            }
            rows.push((ts, price));
        }
    }
    rows.sort_by_key(|(ts, _)| *ts);
    rows
}
fn build_candles_from_points(
    asset: Asset,
    timeframe: Timeframe,
    points: &[(i64, f64)],
) -> Vec<crate::types::Candle> {
    use std::collections::BTreeMap;
    #[derive(Clone, Copy)]
    struct Bucket {
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        trades: u64,
    }
    if points.is_empty() {
        return Vec::new();
    }
    let tf_ms = timeframe.duration_secs() as i64 * 1000;
    let mut buckets: BTreeMap<i64, Bucket> = BTreeMap::new();
    for (raw_ts, price) in points.iter().copied() {
        if !price.is_finite() || price <= 0.0 {
            continue;
        }
        let ts = normalize_history_timestamp_ms(raw_ts);
        let bucket_open = (ts / tf_ms) * tf_ms;
        buckets
            .entry(bucket_open)
            .and_modify(|b| {
                b.high = b.high.max(price);
                b.low = b.low.min(price);
                b.close = price;
                b.trades = b.trades.saturating_add(1);
            })
            .or_insert(Bucket {
                open: price,
                high: price,
                low: price,
                close: price,
                trades: 1,
            });
    }
    let mut candles = Vec::with_capacity(buckets.len());
    for (open_time, bucket) in buckets {
        candles.push(crate::types::Candle {
            open_time,
            close_time: open_time + tf_ms - 1,
            asset,
            timeframe,
            open: bucket.open,
            high: bucket.high,
            low: bucket.low,
            close: bucket.close,
            volume: 0.0,
            trades: bucket.trades,
        });
    }
    candles
}
fn dedup_candles_by_open_time(candles: Vec<crate::types::Candle>) -> Vec<crate::types::Candle> {
    use std::collections::BTreeMap;
    let mut by_open: BTreeMap<i64, crate::types::Candle> = BTreeMap::new();
    for candle in candles {
        by_open.insert(candle.open_time, candle);
    }
    by_open.into_values().collect()
}
async fn bootstrap_polymarket_history_candles(
    client: &ClobClient,
    market_slug: &str,
    asset: Asset,
    timeframe: Timeframe,
    anchor_price: f64,
    interval: crate::clob::PriceHistoryInterval,
) -> Result<Vec<crate::types::Candle>> {
    let mut market = client.find_market_by_slug(market_slug).await;
    if market.is_none() {
        let keyword = match asset {
            Asset::BTC => "bitcoin",
            Asset::ETH => "ethereum",
            Asset::SOL => "solana",
            Asset::XRP => "xrp",
        };
        let mut fallback = client.find_markets(keyword).await;
        fallback.retain(|m| m.active);
        fallback.sort_by(|a, b| {
            let text_a = format!(
                "{} {}",
                a.slug.clone().unwrap_or_default(),
                a.question.to_ascii_lowercase()
            );
            let text_b = format!(
                "{} {}",
                b.slug.clone().unwrap_or_default(),
                b.question.to_ascii_lowercase()
            );
            let tf_match_a = parse_timeframe_from_market_text(&text_a)
                .map(|tf| tf == timeframe)
                .unwrap_or(false);
            let tf_match_b = parse_timeframe_from_market_text(&text_b)
                .map(|tf| tf == timeframe)
                .unwrap_or(false);
            let expiry_a = a
                .end_date_iso
                .as_deref()
                .and_then(crate::clob::ClobClient::parse_expiry_to_timestamp)
                .unwrap_or(i64::MAX);
            let expiry_b = b
                .end_date_iso
                .as_deref()
                .and_then(crate::clob::ClobClient::parse_expiry_to_timestamp)
                .unwrap_or(i64::MAX);
            tf_match_b
                .cmp(&tf_match_a)
                .then_with(|| expiry_a.cmp(&expiry_b))
        });
        market = fallback.into_iter().next();
    }
    let market =
        market.ok_or_else(|| anyhow::anyhow!("Market slug '{}' not found", market_slug))?;
    let token_id = client
        .find_token_id_for_direction(market_slug, Direction::Up)
        .await
        .or_else(|| {
            market
                .tokens
                .iter()
                .find(|t| {
                    let out = t.outcome.to_ascii_lowercase();
                    out.contains("yes") || out.contains("up")
                })
                .map(|t| t.token_id.clone())
        })
        .or_else(|| market.tokens.first().map(|t| t.token_id.clone()))
        .ok_or_else(|| anyhow::anyhow!("No token available for market '{}'", market_slug))?;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let lookback_ms = match timeframe {
        Timeframe::Min15 => 7 * 24 * 3_600_000i64,
        Timeframe::Hour1 => 30 * 24 * 3_600_000i64,
    };
    let points = match client
        .get_token_price_history(
            &token_id,
            interval,
            Some((now_ms - lookback_ms) / 1000),
            Some(now_ms / 1000),
            None,
        )
        .await
    {
        Ok(points) => points,
        Err(primary_err) => {
            // Some markets reject explicit time bounds on specific intervals.
            match client
                .get_token_price_history(&token_id, interval, None, None, None)
                .await
            {
                Ok(points) => points,
                Err(fallback_err) => {
                    let alt_max = crate::clob::PriceHistoryInterval::Max;
                    match client
                        .get_token_price_history(&token_id, alt_max, None, None, None)
                        .await
                    {
                        Ok(points) => points,
                        Err(max_err) => {
                            let alt_day = crate::clob::PriceHistoryInterval::OneDay;
                            client                                .get_token_price_history(&token_id, alt_day, None, None, None)                                .await                                .map_err(|day_err| {                                    anyhow::anyhow!(                                        "price history failed primary='{}' fallback='{}' max='{}' day='{}'",                                        primary_err,                                        fallback_err,                                        max_err,                                        day_err                                    )                                })?
                        }
                    }
                }
            }
        }
    };
    if points.is_empty() {
        return Ok(Vec::new());
    }
    let mut prob_points: Vec<(i64, f64)> = points
        .into_iter()
        .filter_map(|point| {
            if !point.p.is_finite() || point.p <= 0.0 {
                return None;
            }
            Some((
                normalize_history_timestamp_ms(point.t),
                point.p.clamp(0.0001, 0.9999),
            ))
        })
        .collect();
    prob_points.sort_by_key(|(ts, _)| *ts);
    prob_points.dedup_by_key(|(ts, _)| *ts);
    if prob_points.len() < 2 {
        return Ok(Vec::new());
    } // Convert token-probability history into a spot-like synthetic series using returns.
    let mut synthetic: Vec<(i64, f64)> = Vec::with_capacity(prob_points.len());
    let (first_ts, first_p) = prob_points[0];
    let mut prev_prob = first_p.max(0.0001);
    let mut synthetic_price = anchor_price.max(1.0);
    synthetic.push((first_ts, synthetic_price));
    for (ts, prob) in prob_points.into_iter().skip(1) {
        let raw_ret = (prob / prev_prob) - 1.0;
        let scaled_ret = (raw_ret * 0.35).clamp(-0.03, 0.03);
        synthetic_price = (synthetic_price * (1.0 + scaled_ret)).max(0.01);
        synthetic.push((ts, synthetic_price));
        prev_prob = prob.max(0.0001);
    }
    Ok(build_candles_from_points(asset, timeframe, &synthetic))
}
fn parse_timeframe_from_market_text(raw: &str) -> Option<Timeframe> {
    let text = raw.to_ascii_lowercase();
    if text.contains("15m")
        || text.contains("min15")
        || text.contains("m15")
        || text.contains("15 min")
        || text.contains("15-min")
        || text.contains("15min")
        || text.contains("15-minute")
        || text.contains("15 minute")
        || text.contains("updown-15m")
    {
        return Some(Timeframe::Min15);
    }
    if text.contains("1h")
        || text.contains("hour1")
        || text.contains("h1")
        || text.contains("1 hour")
        || text.contains("60m")
        || text.contains("60 min")
        || text.contains("60-minute")
        || text.contains("updown-1h")
        || looks_like_hourly_updown_market_text(&text)
        || looks_like_hourly_updown_time_slug(&text)
    {
        return Some(Timeframe::Hour1);
    }
    // For "will-X-be-above-Y-at-HH-MM" slugs: if the time suffix ends in -00 it's
    // top-of-hour (Hour1); any other minute value (e.g. -15, -30, -45) → Min15.
    if let Some(tf) = infer_timeframe_from_time_suffix(&text) {
        return Some(tf);
    }
    None
}

/// Matches slugs like "will-btc-be-above-93000-at-3-00-pm-et" (top of hour → Hour1).
fn looks_like_hourly_updown_time_slug(text: &str) -> bool {
    // "will-" prefix indicates a Polymarket binary "will X be above/below Y" market.
    if !text.starts_with("will-") && !text.contains("will-btc") && !text.contains("will-eth") {
        return false;
    }
    // If the slug ends with an on-the-hour pattern like "-3-00-pm" or "-12-00-am"
    // but NOT a non-zero minute like "-3-15-pm".
    let re_hour = ["-0-00-", "-1-00-", "-2-00-", "-3-00-", "-4-00-",
                   "-5-00-", "-6-00-", "-7-00-", "-8-00-", "-9-00-",
                   "-10-00-", "-11-00-", "-12-00-"];
    re_hour.iter().any(|pat| text.contains(pat))
}

/// For slugs containing a time suffix "-HH-MM-" where MM != 00, infer Min15.
fn infer_timeframe_from_time_suffix(text: &str) -> Option<Timeframe> {
    // Match minute values that indicate sub-hour windows.
    let min15_patterns = ["-15-pm", "-15-am", "-30-pm", "-30-am", "-45-pm", "-45-am",
                          "-15-et", "-30-et", "-45-et"];
    if min15_patterns.iter().any(|pat| text.contains(pat)) {
        return Some(Timeframe::Min15);
    }
    // On-the-hour → Hour1.
    let hour_patterns = ["-00-pm", "-00-am", "-00-et"];
    if hour_patterns.iter().any(|pat| text.contains(pat)) {
        return Some(Timeframe::Hour1);
    }
    None
}

fn infer_asset_from_market_text(raw: &str) -> Option<Asset> {
    let text = raw.to_ascii_lowercase();
    if text.contains("btc") || text.contains("bitcoin") {
        return Some(Asset::BTC);
    }
    if text.contains("eth") || text.contains("ethereum") {
        return Some(Asset::ETH);
    }
    if text.contains("sol") || text.contains("solana") {
        return Some(Asset::SOL);
    }
    if text.contains("xrp") || text.contains("ripple") {
        return Some(Asset::XRP);
    }
    None
}

fn live_position_contexts_path(data_dir: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(data_dir).join("live_position_contexts.json")
}

fn load_live_position_contexts(
    path: &std::path::Path,
) -> std::collections::HashMap<String, LivePositionContext> {
    match std::fs::read_to_string(path) {
        Ok(json) => match serde_json::from_str::<
            std::collections::HashMap<String, LivePositionContext>,
        >(&json)
        {
            Ok(contexts) => contexts,
            Err(e) => {
                warn!(
                    error = %e,
                    path = %path.display(),
                    "Failed to parse persisted live position contexts"
                );
                std::collections::HashMap::new()
            }
        },
        Err(_) => std::collections::HashMap::new(),
    }
}

fn persist_live_position_contexts(
    path: &std::path::Path,
    contexts: &std::collections::HashMap<String, LivePositionContext>,
) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match serde_json::to_string_pretty(contexts) {
        Ok(json) => {
            if let Err(e) = std::fs::write(path, json) {
                warn!(
                    error = %e,
                    path = %path.display(),
                    "Failed to persist live position contexts"
                );
            }
        }
        Err(e) => {
            warn!(
                error = %e,
                path = %path.display(),
                "Failed to serialize live position contexts"
            );
        }
    }
}

fn init_logging() -> Result<()> {
    // Default to INFO level. Set RUST_LOG=polybot=debug to see verbose logs
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(false).with_thread_ids(false))
        .with(
            fmt::layer()
                .json()
                .with_target(true)
                .with_writer(std::io::stderr),
        )
        .try_init()?;
    Ok(())
}
