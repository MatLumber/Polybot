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
}

impl Default for WebSocketBroadcaster {
    fn default() -> Self {
        Self::new(100)
    }
}
