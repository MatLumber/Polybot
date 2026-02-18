//! Dashboard HTTP API
//!
//! REST endpoints for the React frontend.

use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use super::types::*;
use super::{ApiResponse, DashboardMemory, WebSocketBroadcaster};

/// Create the API router with all endpoints
pub fn create_router(memory: Arc<DashboardMemory>, broadcaster: WebSocketBroadcaster) -> Router {
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
        // WebSocket
        .route("/ws", axum::routing::get(websocket_handler))
        // State
        .with_state((memory, broadcaster))
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
async fn get_stats(
    State((memory, _)): State<(Arc<DashboardMemory>, WebSocketBroadcaster)>,
) -> impl IntoResponse {
    let state = memory.get_state().await;
    Json(ApiResponse::success(state))
}

/// GET /api/trades - Recent trades
async fn get_trades(
    State((memory, _)): State<(Arc<DashboardMemory>, WebSocketBroadcaster)>,
) -> impl IntoResponse {
    let paper = memory.get_paper_state().await;
    Json(ApiResponse::success(paper.recent_trades))
}

/// GET /api/signals - Recent signals
async fn get_signals(
    State((memory, _)): State<(Arc<DashboardMemory>, WebSocketBroadcaster)>,
) -> impl IntoResponse {
    let signals = memory.signals.read().await.clone();
    Json(ApiResponse::success(signals))
}

/// GET /api/prices - Current prices by asset
async fn get_prices(
    State((memory, _)): State<(Arc<DashboardMemory>, WebSocketBroadcaster)>,
) -> impl IntoResponse {
    let prices = memory.get_prices().await;
    Json(ApiResponse::success(prices.prices))
}

/// GET /api/health - Feed health/staleness/reconnect status.
async fn get_health(
    State((memory, _)): State<(Arc<DashboardMemory>, WebSocketBroadcaster)>,
) -> impl IntoResponse {
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
    State((memory, _)): State<(Arc<DashboardMemory>, WebSocketBroadcaster)>,
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
async fn get_positions(
    State((memory, _)): State<(Arc<DashboardMemory>, WebSocketBroadcaster)>,
) -> impl IntoResponse {
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
async fn get_analytics(
    State((memory, _)): State<(Arc<DashboardMemory>, WebSocketBroadcaster)>,
) -> impl IntoResponse {
    let paper = memory.get_paper_state().await;
    Json(ApiResponse::success(paper.asset_stats))
}

/// GET /api/indicator-stats - Indicator calibration statistics
async fn get_indicator_stats(
    State((memory, _)): State<(Arc<DashboardMemory>, WebSocketBroadcaster)>,
) -> impl IntoResponse {
    let stats = memory.indicator_stats.read().await.clone();
    Json(ApiResponse::success(stats))
}

/// GET /api/calibration/markets - Market-level training progress
async fn get_market_learning_progress(
    State((memory, _)): State<(Arc<DashboardMemory>, WebSocketBroadcaster)>,
) -> impl IntoResponse {
    let stats = memory.get_market_learning_progress().await;
    Json(ApiResponse::success(stats))
}

/// GET /api/calibration/quality - ECE/Brier quality by market
async fn get_calibration_quality(
    State((memory, _)): State<(Arc<DashboardMemory>, WebSocketBroadcaster)>,
) -> impl IntoResponse {
    let stats = memory.get_calibration_quality().await;
    Json(ApiResponse::success(stats))
}

// ─────────────────────────────────────────────────────────────────
// WebSocket Handler
// ─────────────────────────────────────────────────────────────────

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
};

/// WebSocket upgrade handler
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State((memory, broadcaster)): State<(Arc<DashboardMemory>, WebSocketBroadcaster)>,
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
