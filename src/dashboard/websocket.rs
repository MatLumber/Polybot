//! WebSocket Broadcaster
//!
//! Broadcasts dashboard updates to all connected WebSocket clients.

use super::types::WsMessage;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Channel for broadcasting updates to WebSocket clients
#[derive(Debug, Clone)]
pub struct WebSocketBroadcaster {
    tx: broadcast::Sender<String>,
}

impl WebSocketBroadcaster {
    /// Create a new broadcaster with the given channel capacity
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Subscribe to receive broadcast messages
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// Broadcast a message to all connected clients
    pub fn broadcast(&self, msg: &WsMessage) {
        if let Ok(json) = serde_json::to_string(msg) {
            // Ignore send errors (no receivers is fine)
            let _ = self.tx.send(json);
        }
    }

    /// Broadcast stats update
    pub fn broadcast_stats(&self, stats: super::types::PaperStatsResponse) {
        self.broadcast(&WsMessage::StatsUpdate(stats));
    }

    /// Broadcast new trade
    pub fn broadcast_trade(&self, trade: super::types::TradeResponse) {
        self.broadcast(&WsMessage::NewTrade(trade));
    }

    /// Broadcast new signal
    pub fn broadcast_signal(&self, signal: super::types::SignalResponse) {
        self.broadcast(&WsMessage::NewSignal(signal));
    }

    /// Broadcast price update
    pub fn broadcast_prices(
        &self,
        prices: std::collections::HashMap<String, super::types::AssetPriceResponse>,
    ) {
        self.broadcast(&WsMessage::PriceUpdate(prices));
    }

    /// Broadcast position opened
    pub fn broadcast_position_opened(&self, position: super::types::PositionResponse) {
        self.broadcast(&WsMessage::PositionOpened(position));
    }

    /// Broadcast position closed
    pub fn broadcast_position_closed(
        &self,
        position_id: String,
        trade: super::types::TradeResponse,
    ) {
        self.broadcast(&WsMessage::PositionClosed(
            super::types::PositionClosedPayload { position_id, trade },
        ));
    }

    /// Broadcast full paper positions snapshot
    pub fn broadcast_positions(&self, positions: Vec<super::types::PositionResponse>) {
        self.broadcast(&WsMessage::PositionsUpdate(positions));
    }

    /// Broadcast heartbeat
    pub fn broadcast_heartbeat(&self) {
        self.broadcast(&WsMessage::Heartbeat(chrono::Utc::now().timestamp_millis()));
    }

    /// Broadcast ML state update
    pub fn broadcast_ml_state(&self, enabled: bool, model_type: &str, version: &str) {
        self.broadcast(&WsMessage::MLStateUpdate(
            super::types::MLStateUpdatePayload {
                enabled,
                model_type: model_type.to_string(),
                version: version.to_string(),
                timestamp: chrono::Utc::now().timestamp_millis(),
            },
        ));
    }

    /// Broadcast ML prediction
    pub fn broadcast_ml_prediction(
        &self,
        asset: &str,
        timeframe: &str,
        direction: &str,
        confidence: f64,
        prob_up: f64,
        model_name: &str,
        features: Vec<String>,
    ) {
        self.broadcast(&WsMessage::MLPrediction(
            super::types::MLPredictionPayload {
                asset: asset.to_string(),
                timeframe: timeframe.to_string(),
                direction: direction.to_string(),
                confidence,
                prob_up,
                model_name: model_name.to_string(),
                features_triggered: features,
                timestamp: chrono::Utc::now().timestamp_millis(),
            },
        ));
    }

    /// Broadcast ML metrics update
    pub fn broadcast_ml_metrics(
        &self,
        accuracy: f64,
        win_rate: f64,
        total_predictions: usize,
        correct_predictions: usize,
        weights: Vec<(String, f64, f64)>,
    ) {
        let ensemble_weights = weights
            .into_iter()
            .map(|(name, weight, accuracy)| super::types::ModelWeightInfo {
                name,
                weight,
                accuracy,
            })
            .collect();

        self.broadcast(&WsMessage::MLMetricsUpdate(
            super::types::MLMetricsPayload {
                accuracy,
                win_rate,
                total_predictions,
                correct_predictions,
                ensemble_weights,
                timestamp: chrono::Utc::now().timestamp_millis(),
            },
        ));
    }
}

impl Default for WebSocketBroadcaster {
    fn default() -> Self {
        Self::new(100)
    }
}
