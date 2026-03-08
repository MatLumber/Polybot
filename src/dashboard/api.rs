//! Dashboard HTTP API
//!
//! REST endpoints for the React frontend.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use super::types::*;
use super::{ApiResponse, DashboardMemory, WebSocketBroadcaster};

#[derive(Clone)]
pub struct DataDir(pub String);

pub type AppState = (Arc<DashboardMemory>, WebSocketBroadcaster, DataDir);

/// Create the API router with all endpoints
pub fn create_router(
    memory: Arc<DashboardMemory>,
    broadcaster: WebSocketBroadcaster,
    data_dir: String,
) -> Router {
    Router::new()
        // Main endpoints
        .route("/api/stats", get(get_stats))
        .route("/api/trades", get(get_trades))
        .route("/api/signals", get(get_signals))
        .route("/api/prices", get(get_prices))
        .route("/api/prices/history", get(get_prices_history))
        .route("/api/health", get(get_health))
        .route("/api/positions", get(get_positions))
        .route("/api/analytics", get(get_analytics))
        // Indicator calibration stats
        .route("/api/indicator-stats", get(get_indicator_stats))
        .route(
            "/api/calibration/markets",
            get(get_market_learning_progress),
        )
        .route("/api/calibration/quality", get(get_calibration_quality))
        // Data endpoints (NEW)
        .route("/api/data/calibrator", get(get_calibrator_state))
        .route("/api/data/paper-state", get(get_paper_trading_state))
        .route("/api/data/trades/:date", get(get_trades_csv))
        .route("/api/data/trades", get(get_latest_trades_csv))
        .route("/api/data/signals/:date", get(get_signals_csv))
        .route("/api/data/signals", get(get_latest_signals_csv))
        .route("/api/data/prices/:date", get(get_prices_csv))
        .route("/api/data/prices", get(get_latest_prices_csv))
        .route("/api/data/rejections/:date", get(get_rejections_csv))
        .route("/api/data/rejections", get(get_latest_rejections_csv))
        .route("/api/data/files", get(list_data_files))
        // NEW v3.0: Temporal patterns, settlement predictor, cross-asset
        .route("/api/data/temporal-patterns", get(get_temporal_patterns))
        .route("/api/data/settlement-metrics", get(get_settlement_metrics))
        .route(
            "/api/data/cross-asset-correlations",
            get(get_cross_asset_correlations),
        )
        // NEW v3.0: ML Engine endpoints
        .route("/api/ml/state", get(get_ml_state))
        .route("/api/ml/metrics", get(get_ml_metrics))
        .route("/api/ml/models", get(get_ml_models))
        .route("/api/ml/features", get(get_ml_features))
        .route("/api/ml/training", get(get_ml_training_status))
        // Trading mode toggle (paper ↔ live without restart)
        .route(
            "/api/trading-mode",
            get(get_trading_mode).post(set_trading_mode),
        )
        // Manual position close
        .route("/api/close-position", post(close_position_handler))
        // WebSocket
        .route("/ws", axum::routing::get(websocket_handler))
        // State
        .with_state((memory, broadcaster, DataDir(data_dir)))
        // CORS for frontend
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
}

// ─────────────────────────────────────────────────────────────────
// API Handlers
// ─────────────────────────────────────────────────────────────────

/// GET /api/stats - Complete dashboard state
async fn get_stats(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let state = memory.get_state().await;
    Json(ApiResponse::success(state))
}

/// GET /api/trades - Recent trades
async fn get_trades(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let paper = memory.get_paper_state().await;
    let live = memory.get_live_state().await;

    #[derive(serde::Serialize)]
    struct TradesResponse {
        paper: Vec<TradeResponse>,
        live: Vec<TradeResponse>,
    }

    Json(ApiResponse::success(TradesResponse {
        paper: paper.recent_trades,
        live: live.recent_trades,
    }))
}

/// GET /api/signals - Recent signals
async fn get_signals(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let signals = memory.signals.read().await.clone();
    Json(ApiResponse::success(signals))
}

/// GET /api/prices - Current prices by asset
async fn get_prices(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let prices = memory.get_prices().await;
    Json(ApiResponse::success(prices.prices))
}

/// GET /api/health - Feed health/staleness/reconnect status.
async fn get_health(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let health = memory.get_health().await;
    Json(ApiResponse::success(health))
}

#[derive(Debug, Deserialize)]
struct PriceHistoryQuery {
    assets: Option<String>,
    window_secs: Option<i64>,
    bucket_ms: Option<i64>,
}

/// GET /api/prices/history?assets=BTC,ETH&window_secs=3600&bucket_ms=1000
async fn get_prices_history(
    Query(query): Query<PriceHistoryQuery>,
    State((memory, _, _)): State<AppState>,
) -> impl IntoResponse {
    if let Some(window_secs) = query.window_secs {
        if !(60..=86_400).contains(&window_secs) {
            return Json(ApiResponse::<
                std::collections::HashMap<String, Vec<PriceHistoryPointResponse>>,
            >::error(
                "window_secs must be between 60 and 86400"
            ));
        }
    }

    if let Some(bucket_ms) = query.bucket_ms {
        if !(250..=60_000).contains(&bucket_ms) {
            return Json(ApiResponse::<
                std::collections::HashMap<String, Vec<PriceHistoryPointResponse>>,
            >::error(
                "bucket_ms must be between 250 and 60000"
            ));
        }
    }

    let assets: Vec<String> = query
        .assets
        .as_deref()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|asset| !asset.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let history = memory
        .get_price_history(&assets, query.window_secs, query.bucket_ms)
        .await;
    Json(ApiResponse::success(history))
}

/// GET /api/positions - Open positions (paper + live)
async fn get_positions(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let paper_positions = memory.paper_positions.read().await.clone();
    let live_positions = memory.live_positions.read().await.clone();

    #[derive(serde::Serialize)]
    struct PositionsResponse {
        paper: Vec<PositionResponse>,
        live: Vec<PositionResponse>,
    }

    Json(ApiResponse::success(PositionsResponse {
        paper: paper_positions,
        live: live_positions,
    }))
}

/// GET /api/analytics - Per-asset analytics
async fn get_analytics(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let paper = memory.get_paper_state().await;
    let live = memory.get_live_state().await;

    #[derive(serde::Serialize)]
    struct AnalyticsResponse {
        paper: std::collections::HashMap<String, AssetStatsResponse>,
        live: std::collections::HashMap<String, AssetStatsResponse>,
    }

    Json(ApiResponse::success(AnalyticsResponse {
        paper: paper.asset_stats,
        live: live.asset_stats,
    }))
}

/// GET /api/indicator-stats - Indicator calibration statistics
async fn get_indicator_stats(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let stats = memory.indicator_stats.read().await.clone();
    Json(ApiResponse::success(stats))
}

/// GET /api/calibration/markets - Market-level training progress
async fn get_market_learning_progress(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let stats = memory.get_market_learning_progress().await;
    Json(ApiResponse::success(stats))
}

/// GET /api/calibration/quality - ECE/Brier quality by market
async fn get_calibration_quality(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let stats = memory.get_calibration_quality().await;
    Json(ApiResponse::success(stats))
}

// ─────────────────────────────────────────────────────────────────
// WebSocket Handler
// ─────────────────────────────────────────────────────────────────

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};

/// WebSocket upgrade handler
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State((memory, broadcaster, _)): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_websocket(socket, memory, broadcaster))
}

/// Outgoing message type for WebSocket
enum OutgoingMessage {
    Text(String),
    Pong(Vec<u8>),
}

/// Handle WebSocket connection
async fn handle_websocket(
    socket: WebSocket,
    memory: Arc<DashboardMemory>,
    broadcaster: WebSocketBroadcaster,
) {
    use futures_util::{SinkExt, StreamExt};

    tracing::info!("🖥️ New WebSocket connection");

    let (mut sender, mut receiver) = socket.split();

    // Send initial state
    let initial_state = memory.get_state().await;
    let msg = WsMessage::FullState(initial_state);
    if let Ok(json) = serde_json::to_string(&msg) {
        if sender.send(Message::Text(json)).await.is_err() {
            return;
        }
    }

    // Subscribe to broadcasts
    let mut rx = broadcaster.subscribe();

    // Channel for outgoing messages
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<OutgoingMessage>(32);

    // Spawn task to send outgoing messages
    let send_task = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let result = match msg {
                OutgoingMessage::Text(text) => sender.send(Message::Text(text)).await,
                OutgoingMessage::Pong(data) => sender.send(Message::Pong(data)).await,
            };
            if result.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages (ping/pong) and broadcast updates
    loop {
        tokio::select! {
            // Broadcast updates
            broadcast_msg = rx.recv() => {
                if let Ok(msg) = broadcast_msg {
                    if out_tx.send(OutgoingMessage::Text(msg)).await.is_err() {
                        break;
                    }
                }
            }
            // Incoming messages
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Ping(data))) => {
                        // Respond with pong via the outgoing channel
                        if out_tx.send(OutgoingMessage::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        tracing::debug!("Received WebSocket message: {}", text);
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }

    send_task.abort();
    tracing::info!("🖥️ WebSocket connection closed");
}

// ─────────────────────────────────────────────────────────────────
// Data Endpoints (CSV/JSON downloads)
// ─────────────────────────────────────────────────────────────────

/// GET /api/data/calibrator - Returns calibrator_state_v2.json
async fn get_calibrator_state(State((_, _, data_dir)): State<AppState>) -> impl IntoResponse {
    let path = format!("{}/calibrator_state_v2.json", data_dir.0);
    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(json) => Json(ApiResponse::success(json)),
            Err(_) => Json(ApiResponse::<serde_json::Value>::error(
                "Failed to parse calibrator JSON",
            )),
        },
        Err(_) => Json(ApiResponse::<serde_json::Value>::error(
            "Calibrator state file not found",
        )),
    }
}

/// GET /api/data/paper-state - Returns paper_trading_state.json
async fn get_paper_trading_state(State((_, _, data_dir)): State<AppState>) -> impl IntoResponse {
    let path = format!("{}/paper_trading_state.json", data_dir.0);
    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(json) => Json(ApiResponse::success(json)),
            Err(_) => Json(ApiResponse::<serde_json::Value>::error(
                "Failed to parse paper state JSON",
            )),
        },
        Err(_) => Json(ApiResponse::<serde_json::Value>::error(
            "Paper trading state file not found",
        )),
    }
}

fn find_latest_csv(data_dir: &str, subfolder: &str, prefix: &str) -> Option<String> {
    let folder_path = format!("{}/{}", data_dir, subfolder);
    if let Ok(entries) = std::fs::read_dir(&folder_path) {
        let mut files: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "csv")
                    .unwrap_or(false)
            })
            .filter(|e| e.file_name().to_string_lossy().starts_with(prefix))
            .map(|e| e.path().to_string_lossy().to_string())
            .collect();
        files.sort();
        files.last().cloned()
    } else {
        None
    }
}

fn serve_csv_file(path: &str, filename: &str) -> Response {
    match std::fs::read_to_string(path) {
        Ok(content) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/csv")
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            )
            .body(Body::from(content))
            .unwrap(),
        Err(_) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("File not found"))
            .unwrap(),
    }
}

/// GET /api/data/trades/:date - Returns trades CSV for specific date (YYYY-MM-DD)
async fn get_trades_csv(
    Path(date): Path<String>,
    State((_, _, data_dir)): State<AppState>,
) -> Response {
    let filename = format!("trades_{}.csv", date);
    let path = format!("{}/trades/{}", data_dir.0, filename);
    serve_csv_file(&path, &filename)
}

/// GET /api/data/trades - Returns latest trades CSV
async fn get_latest_trades_csv(State((_, _, data_dir)): State<AppState>) -> Response {
    if let Some(path) = find_latest_csv(&data_dir.0, "trades", "trades_") {
        let filename = path.rsplit('/').next().unwrap_or("trades.csv");
        serve_csv_file(&path, filename)
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("No trades CSV files found"))
            .unwrap()
    }
}

/// GET /api/data/signals/:date - Returns signals CSV for specific date
async fn get_signals_csv(
    Path(date): Path<String>,
    State((_, _, data_dir)): State<AppState>,
) -> Response {
    let filename = format!("signals_{}.csv", date);
    let path = format!("{}/signals/{}", data_dir.0, filename);
    serve_csv_file(&path, &filename)
}

/// GET /api/data/signals - Returns latest signals CSV
async fn get_latest_signals_csv(State((_, _, data_dir)): State<AppState>) -> Response {
    if let Some(path) = find_latest_csv(&data_dir.0, "signals", "signals_") {
        let filename = path.rsplit('/').next().unwrap_or("signals.csv");
        serve_csv_file(&path, filename)
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("No signals CSV files found"))
            .unwrap()
    }
}

/// GET /api/data/prices/:date - Returns prices CSV for specific date
async fn get_prices_csv(
    Path(date): Path<String>,
    State((_, _, data_dir)): State<AppState>,
) -> Response {
    let filename = format!("prices_{}.csv", date);
    let path = format!("{}/prices/{}", data_dir.0, filename);
    serve_csv_file(&path, &filename)
}

/// GET /api/data/prices - Returns latest prices CSV
async fn get_latest_prices_csv(State((_, _, data_dir)): State<AppState>) -> Response {
    if let Some(path) = find_latest_csv(&data_dir.0, "prices", "prices_") {
        let filename = path.rsplit('/').next().unwrap_or("prices.csv");
        serve_csv_file(&path, filename)
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("No prices CSV files found"))
            .unwrap()
    }
}

/// GET /api/data/rejections/:date - Returns rejections CSV for specific date
async fn get_rejections_csv(
    Path(date): Path<String>,
    State((_, _, data_dir)): State<AppState>,
) -> Response {
    let filename = format!("rejections_{}.csv", date);
    let path = format!("{}/rejections/{}", data_dir.0, filename);
    serve_csv_file(&path, &filename)
}

/// GET /api/data/rejections - Returns latest rejections CSV
async fn get_latest_rejections_csv(State((_, _, data_dir)): State<AppState>) -> Response {
    if let Some(path) = find_latest_csv(&data_dir.0, "rejections", "rejections_") {
        let filename = path.rsplit('/').next().unwrap_or("rejections.csv");
        serve_csv_file(&path, filename)
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("No rejections CSV files found"))
            .unwrap()
    }
}

#[derive(serde::Serialize)]
struct DataFileInfo {
    name: String,
    size_bytes: u64,
    modified: String,
}

#[derive(serde::Serialize)]
struct DataFilesResponse {
    data_dir: String,
    calibrator_state: Option<DataFileInfo>,
    paper_trading_state: Option<DataFileInfo>,
    trades: Vec<DataFileInfo>,
    signals: Vec<DataFileInfo>,
    prices: Vec<DataFileInfo>,
    rejections: Vec<DataFileInfo>,
}

/// GET /api/data/files - Lists all available data files
async fn list_data_files(State((_, _, data_dir)): State<AppState>) -> impl IntoResponse {
    let get_file_info = |path: &str| -> Option<DataFileInfo> {
        let metadata = std::fs::metadata(path).ok()?;
        let modified: String = metadata
            .modified()
            .ok()
            .map(|t| {
                let datetime: chrono::DateTime<chrono::Utc> = t.into();
                datetime.to_rfc3339()
            })
            .unwrap_or_default();
        let name = path.rsplit('/').next().unwrap_or("unknown").to_string();
        Some(DataFileInfo {
            name,
            size_bytes: metadata.len(),
            modified,
        })
    };

    let list_csv_files = |subfolder: &str| -> Vec<DataFileInfo> {
        let folder_path = format!("{}/{}", data_dir.0, subfolder);
        if let Ok(entries) = std::fs::read_dir(&folder_path) {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .map(|ext| ext == "csv")
                        .unwrap_or(false)
                })
                .filter_map(|e| get_file_info(&e.path().to_string_lossy()))
                .collect()
        } else {
            Vec::new()
        }
    };

    let response = DataFilesResponse {
        data_dir: data_dir.0.clone(),
        calibrator_state: get_file_info(&format!("{}/calibrator_state_v2.json", data_dir.0)),
        paper_trading_state: get_file_info(&format!("{}/paper_trading_state.json", data_dir.0)),
        trades: list_csv_files("trades"),
        signals: list_csv_files("signals"),
        prices: list_csv_files("prices"),
        rejections: list_csv_files("rejections"),
    };

    Json(ApiResponse::success(response))
}

// ─────────────────────────────────────────────────────────────────
// NEW v3.0: Advanced Analytics Endpoints
// ─────────────────────────────────────────────────────────────────

/// GET /api/data/temporal-patterns - Returns time-of-day performance stats
async fn get_temporal_patterns(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let state = memory.get_state().await;
    let mut by_hour: std::collections::HashMap<u32, (u32, u32)> = std::collections::HashMap::new();

    for trade in state
        .paper
        .recent_trades
        .iter()
        .chain(state.live.recent_trades.iter())
    {
        let hour = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(trade.timestamp)
            .map(|ts| chrono::Timelike::hour(&ts))
            .unwrap_or(0);
        let entry = by_hour.entry(hour).or_insert((0, 0));
        entry.0 += 1;
        if trade.pnl >= 0.0 {
            entry.1 += 1;
        }
    }

    let mut hours: Vec<serde_json::Value> = by_hour
        .into_iter()
        .map(|(hour, (trades, wins))| {
            serde_json::json!({
                "hour": hour,
                "trades": trades,
                "win_rate": if trades > 0 { wins as f64 / trades as f64 } else { 0.0 }
            })
        })
        .collect();
    hours.sort_by(|a, b| {
        b["win_rate"]
            .as_f64()
            .partial_cmp(&a["win_rate"].as_f64())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Json(ApiResponse::success(serde_json::json!({
        "hours": hours,
        "sample_count": state.paper.recent_trades.len() + state.live.recent_trades.len(),
        "updated_at": chrono::Utc::now().to_rfc3339(),
    })))
}

/// GET /api/data/settlement-metrics - Returns settlement prediction metrics
async fn get_settlement_metrics(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let state = memory.get_state().await;
    let settlement_trades: Vec<_> = state
        .paper
        .recent_trades
        .iter()
        .chain(state.live.recent_trades.iter())
        .filter(|trade| matches!(trade.exit_reason.as_str(), "MARKET_EXPIRY" | "TIME_EXPIRY"))
        .collect();
    let correct = settlement_trades
        .iter()
        .filter(|trade| trade.prediction_correct)
        .count();

    Json(ApiResponse::success(serde_json::json!({
        "total_settlements_recorded": settlement_trades.len(),
        "historical_accuracy": if settlement_trades.is_empty() { 0.0 } else { correct as f64 / settlement_trades.len() as f64 },
        "avg_settlement_pnl": if settlement_trades.is_empty() { 0.0 } else { settlement_trades.iter().map(|trade| trade.pnl).sum::<f64>() / settlement_trades.len() as f64 },
        "updated_at": chrono::Utc::now().to_rfc3339(),
    })))
}

/// GET /api/data/cross-asset-correlations - Returns BTC/ETH correlation data
async fn get_cross_asset_correlations(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let prices = memory.get_prices().await;
    let btc = prices.prices.get("BTC").map(|value| value.price);
    let eth = prices.prices.get("ETH").map(|value| value.price);

    Json(ApiResponse::success(serde_json::json!({
        "pairs": [
            {
                "pair": "BTC_ETH",
                "spot_ratio": match (btc, eth) {
                    (Some(btc_price), Some(eth_price)) if eth_price > 0.0 => Some(btc_price / eth_price),
                    _ => None,
                },
                "sample_count": if btc.is_some() && eth.is_some() { 1 } else { 0 }
            }
        ],
        "last_update": chrono::Utc::now().to_rfc3339()
    })))
}

/// GET /api/ml/state - Returns ML Engine state and configuration
async fn get_ml_state(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let ml_state = memory.get_ml_state().await;
    Json(ApiResponse::success(ml_state))
}

/// GET /api/ml/metrics - Returns ML model performance metrics
async fn get_ml_metrics(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let metrics = memory.get_ml_metrics().await;
    Json(ApiResponse::success(metrics))
}

/// GET /api/ml/models - Returns ensemble model information
async fn get_ml_models(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let models = memory.get_ml_models().await;
    Json(ApiResponse::success(models))
}

/// GET /api/ml/features - Returns feature importance and statistics
async fn get_ml_features(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let features = memory.get_ml_features().await;
    Json(ApiResponse::success(features))
}

/// GET /api/ml/training - Returns training status and history
async fn get_ml_training_status(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let training = memory.get_ml_training_status().await;
    Json(ApiResponse::success(training))
}

// ─────────────────────────────────────────────────────────────────
// Trading Mode Toggle
// ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct TradingModeRequest {
    mode: String, // "paper" or "live"
}

#[derive(Debug, Serialize)]
struct TradingModeResponse {
    mode: String,
    is_paper: bool,
    live_ready: bool,
}

/// GET /api/trading-mode - Returns current trading mode
async fn get_trading_mode(State((memory, _, _)): State<AppState>) -> impl IntoResponse {
    let is_paper = memory
        .trading_mode
        .load(std::sync::atomic::Ordering::Relaxed);
    let live_ready = memory.live_ready.load(std::sync::atomic::Ordering::Relaxed);
    Json(ApiResponse::success(TradingModeResponse {
        mode: if is_paper {
            "paper".into()
        } else {
            "live".into()
        },
        is_paper,
        live_ready,
    }))
}

/// POST /api/trading-mode - Switch between paper and live trading at runtime
async fn set_trading_mode(
    State((memory, broadcaster, _)): State<AppState>,
    Json(req): Json<TradingModeRequest>,
) -> Response {
    let is_paper = match req.mode.as_str() {
        "paper" => true,
        "live" => false,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<TradingModeResponse>::error(
                    "Invalid mode — use \"paper\" or \"live\"",
                )),
            )
                .into_response();
        }
    };
    let live_ready = memory.live_ready.load(std::sync::atomic::Ordering::Relaxed);
    if !is_paper && !live_ready {
        return (
            StatusCode::CONFLICT,
            Json(ApiResponse::<TradingModeResponse>::error(
                "LIVE trading is not ready on this process",
            )),
        )
            .into_response();
    }

    memory
        .trading_mode
        .store(is_paper, std::sync::atomic::Ordering::SeqCst);

    // Broadcast the change so all connected dashboards update instantly
    broadcaster.broadcast(&super::types::WsMessage::TradingModeChanged(is_paper));

    let mode_str = if is_paper { "paper" } else { "live" };
    if is_paper {
        tracing::info!(
            mode = mode_str,
            live_ready = live_ready,
            "🟦 Runtime execution switched to PAPER - no real Polymarket orders will be sent"
        );
    } else {
        tracing::warn!(
            mode = mode_str,
            live_ready = live_ready,
            "🟥 Runtime execution switched to LIVE - approved signals can submit REAL Polymarket orders"
        );
    }

    Json(ApiResponse::success(TradingModeResponse {
        mode: mode_str.into(),
        is_paper,
        live_ready,
    }))
    .into_response()
}

// ─────────────────────────────────────────────────────────────────
// Manual position close
// ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ClosePositionRequest {
    token_id: String,
}

#[derive(Debug, Serialize)]
struct ClosePositionResponse {
    token_id: String,
    status: String,
}

/// POST /api/close-position - Request immediate manual close of a live position
async fn close_position_handler(
    State((memory, _, _)): State<AppState>,
    Json(req): Json<ClosePositionRequest>,
) -> Response {
    let tx = memory.manual_close_tx.lock().unwrap().clone();
    match tx {
        Some(sender) => {
            if sender.send(req.token_id.clone()).is_ok() {
                tracing::info!(token_id = %req.token_id, "Manual close requested via dashboard");
                Json(ApiResponse::success(ClosePositionResponse {
                    token_id: req.token_id,
                    status: "closing".into(),
                }))
                .into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<ClosePositionResponse>::error("Close channel unavailable")),
                )
                    .into_response()
            }
        }
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<ClosePositionResponse>::error(
                "Manual close not available in this mode",
            )),
        )
            .into_response(),
    }
}
