//! Dashboard API Types
//!
//! DTOs for HTTP/WebSocket communication with the React frontend.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────
// Dashboard State (in-memory)
// ─────────────────────────────────────────────────────────────────

/// Complete dashboard state shared with frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardState {
    pub paper: PaperDashboard,
    pub live: LiveDashboard,
    pub prices: PriceDashboard,
    pub execution: ExecutionDiagnosticsResponse,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PaperDashboard {
    pub balance: f64,
    pub available: f64,
    pub locked: f64,
    pub total_equity: f64,
    pub unrealized_pnl: f64,
    pub stats: PaperStatsResponse,
    pub open_positions: Vec<PositionResponse>,
    pub recent_trades: Vec<TradeResponse>,
    pub asset_stats: HashMap<String, AssetStatsResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LiveDashboard {
    pub balance: f64,
    pub available: f64,
    pub locked: f64,
    pub total_equity: f64,
    pub unrealized_pnl: f64,
    pub open_positions: Vec<PositionResponse>,
    pub daily_pnl: f64,
    pub daily_trades: u32,
    pub kill_switch_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PriceDashboard {
    pub prices: HashMap<String, AssetPriceResponse>,
    pub last_update: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionDiagnosticsResponse {
    pub processed_features: u64,
    pub generated_signals: u64,
    pub filtered_features: u64,
    pub strategy_filter_reasons: HashMap<String, u64>,
    pub last_strategy_filter_reason: Option<String>,
    pub last_strategy_filter_ts: i64,
    pub accepted_signals: u64,
    pub rejected_signals: u64,
    pub rejection_reasons: HashMap<String, u64>,
    pub last_rejection_reason: Option<String>,
    pub last_rejection_ts: i64,
    pub stale_rejections: u64,
    pub reconnect_events: u64,
    pub stale_assets: HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AssetHealthResponse {
    pub asset: String,
    pub last_tick_ts: i64,
    pub tick_age_ms: i64,
    pub stale: bool,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HealthResponse {
    pub rtds_connected: bool,
    pub orderbook_connected: bool,
    pub rtds_reconnect_count: u64,
    pub orderbook_reconnect_count: u64,
    pub stale_threshold_ms: i64,
    pub assets: HashMap<String, AssetHealthResponse>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceHistoryPointResponse {
    pub timestamp: i64,
    pub price: f64,
    pub source: String,
}

// ─────────────────────────────────────────────────────────────────
// Response Types
// ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperStatsResponse {
    pub total_trades: u32,
    pub wins: u32,
    pub losses: u32,
    pub win_rate: f64,
    pub total_pnl: f64,
    pub total_fees: f64,
    pub largest_win: f64,
    pub largest_loss: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub max_drawdown: f64,
    pub current_drawdown: f64,
    pub peak_balance: f64,
    pub profit_factor: f64,
    pub current_streak: i32,
    pub best_streak: i32,
    pub worst_streak: i32,
    pub exits_trailing_stop: u32,
    pub exits_take_profit: u32,
    pub exits_market_expiry: u32,
    pub exits_time_expiry: u32,
    #[serde(default)]
    pub predictions_correct: u32,
    #[serde(default)]
    pub predictions_incorrect: u32,
    #[serde(default)]
    pub prediction_win_rate: f64,
    #[serde(default)]
    pub trading_wins: u32,
    #[serde(default)]
    pub trading_losses: u32,
    #[serde(default)]
    pub trading_win_rate: f64,
}

impl Default for PaperStatsResponse {
    fn default() -> Self {
        Self {
            total_trades: 0,
            wins: 0,
            losses: 0,
            win_rate: 0.0,
            total_pnl: 0.0,
            total_fees: 0.0,
            largest_win: 0.0,
            largest_loss: 0.0,
            avg_win: 0.0,
            avg_loss: 0.0,
            max_drawdown: 0.0,
            current_drawdown: 0.0,
            peak_balance: 0.0,
            profit_factor: 0.0,
            current_streak: 0,
            best_streak: 0,
            worst_streak: 0,
            exits_trailing_stop: 0,
            exits_take_profit: 0,
            exits_market_expiry: 0,
            exits_time_expiry: 0,
            predictions_correct: 0,
            predictions_incorrect: 0,
            prediction_win_rate: 0.0,
            trading_wins: 0,
            trading_losses: 0,
            trading_win_rate: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionResponse {
    pub id: String,
    pub asset: String,
    pub timeframe: String,
    pub direction: String,
    pub entry_price: f64,
    pub current_price: f64,
    pub size_usdc: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub opened_at: i64,
    pub market_slug: String,
    pub confidence: f64,
    pub peak_price: f64,
    pub trough_price: f64,
    pub market_close_ts: i64,
    pub time_remaining_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeResponse {
    pub timestamp: i64,
    pub trade_id: String,
    pub asset: String,
    pub timeframe: String,
    pub direction: String,
    pub confidence: f64,
    pub entry_price: f64,
    pub exit_price: f64,
    pub size_usdc: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub result: String,
    pub prediction_correct: bool,
    pub exit_reason: String,
    pub hold_duration_secs: i64,
    pub balance_after: f64,
    // Technical indicators at entry
    pub rsi_at_entry: Option<f64>,
    pub macd_hist_at_entry: Option<f64>,
    pub bb_position_at_entry: Option<f64>,
    pub adx_at_entry: Option<f64>,
    pub volatility_at_entry: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalResponse {
    pub timestamp: i64,
    pub signal_id: String,
    pub asset: String,
    pub timeframe: String,
    pub direction: String,
    pub confidence: f64,
    pub entry_price: f64,
    pub market_slug: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetStatsResponse {
    pub asset: String,
    pub trades: u32,
    pub wins: u32,
    pub losses: u32,
    pub win_rate: f64,
    pub pnl: f64,
    pub avg_confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetPriceResponse {
    pub asset: String,
    pub price: f64,
    pub bid: f64,
    pub ask: f64,
    pub source: String,
    pub timestamp: i64,
    /// Change from session open price (percentage, e.g., 0.05 = 5%)
    pub change_24h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketLearningProgressResponse {
    pub market_key: String,
    pub asset: String,
    pub timeframe: String,
    pub sample_count: u32,
    pub target_samples: u32,
    pub progress_pct: f64,
    pub indicators_active: u32,
    pub avg_win_rate_pct: f64,
    pub last_updated_ts: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationQualityResponse {
    pub market_key: String,
    pub asset: String,
    pub timeframe: String,
    pub sample_count: u32,
    pub brier_score: Option<f64>,
    pub ece: Option<f64>,
    pub ece_target: f64,
    pub ece_pass: bool,
}

// ─────────────────────────────────────────────────────────────────
// WebSocket Message Types
// ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    /// Full state update (sent on connect)
    FullState(DashboardState),
    /// Stats updated
    StatsUpdate(PaperStatsResponse),
    /// New trade closed
    NewTrade(TradeResponse),
    /// New signal generated
    NewSignal(SignalResponse),
    /// Price update
    PriceUpdate(HashMap<String, AssetPriceResponse>),
    /// Position opened
    PositionOpened(PositionResponse),
    /// Position closed
    PositionClosed(PositionClosedPayload),
    /// Full paper positions snapshot (for real-time position PnL/time updates)
    PositionsUpdate(Vec<PositionResponse>),
    /// ML Engine state update
    MLStateUpdate(MLStateUpdatePayload),
    /// ML Prediction made
    MLPrediction(MLPredictionPayload),
    /// ML Model metrics update
    MLMetricsUpdate(MLMetricsPayload),
    /// Heartbeat
    Heartbeat(i64),
}

/// ML State update payload for WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLStateUpdatePayload {
    pub enabled: bool,
    pub model_type: String,
    pub version: String,
    pub timestamp: i64,
}

/// ML Prediction payload for WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLPredictionPayload {
    pub asset: String,
    pub timeframe: String,
    pub direction: String,
    pub confidence: f64,
    pub prob_up: f64,
    pub model_name: String,
    pub features_triggered: Vec<String>,
    pub timestamp: i64,
}

/// ML Metrics payload for WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLMetricsPayload {
    pub accuracy: f64,
    pub win_rate: f64,
    pub loss_rate: f64,
    pub total_predictions: usize,
    pub correct_predictions: usize,
    pub incorrect_predictions: usize,
    pub ensemble_weights: Vec<ModelWeightInfo>,
    pub epoch: usize,
    pub dataset_size: usize,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelWeightInfo {
    pub name: String,
    pub weight: f64,
    pub accuracy: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionClosedPayload {
    pub position_id: String,
    pub trade: TradeResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}
