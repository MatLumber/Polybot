//! Polymarket RTDS WebSocket client
//!
//! Connects to Polymarket's Real-Time Data Service for live crypto prices.
//! RTDS provides aggregated crypto price feeds from both Binance and Chainlink.
//!
//! Documentation: https://docs.polymarket.com/developers/RTDS/RTDS-crypto-prices
//!
//! # Price Sources
//! - **Binance** (`crypto_prices`): Real-time prices from Binance exchange
//! - **Chainlink** (`crypto_prices_chainlink`): Oracle-based prices from Chainlink
//!
//! Both sources are subscribed simultaneously for price redundancy and accuracy.
//! **Chainlink is the primary price source used by Polymarket for market resolution.**

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::oracle::sources::{CommentEvent, PriceSource, SourceEvent};
use crate::oracle::NormalizedTick;
use crate::types::{Asset, PriceSource as Source};

const RTDS_WS_URL: &str = "wss://ws-live-data.polymarket.com";
const WATCHDOG_SILENCE_SECS: u64 = 20;
const PING_INTERVAL_SECS: u64 = 15;
const WATCHDOG_TICK_SECS: u64 = 5;

fn should_reconnect_due_to_silence(last_useful_message: Instant, silence_secs: u64) -> bool {
    last_useful_message.elapsed().as_secs() >= silence_secs
}

fn normalize_epoch_millis(ts: i64) -> i64 {
    if ts > 0 && ts < 1_000_000_000_000 {
        ts.saturating_mul(1000)
    } else {
        ts
    }
}

/// RTDS subscription request (correct format per documentation)
#[derive(Debug, Clone, Serialize)]
struct RtdsSubscribeRequest {
    action: String,
    subscriptions: Vec<RtdsSubscription>,
}

#[derive(Debug, Clone, Serialize)]
struct RtdsSubscription {
    topic: String,
    #[serde(rename = "type")]
    msg_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    filters: Option<String>,
}

/// RTDS message from server
#[derive(Debug, Clone, Deserialize)]
struct RtdsMessage {
    topic: Option<String>,
    #[serde(rename = "type")]
    msg_type: Option<String>,
    timestamp: Option<i64>,
    payload: Option<RtdsPayload>,
    message: Option<String>,
    #[serde(rename = "statusCode")]
    status_code: Option<u16>,
    #[serde(rename = "connectionId")]
    connection_id: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RtdsPayload {
    symbol: Option<String>,
    timestamp: Option<i64>,
    value: Option<f64>,
    data: Option<Vec<RtdsPriceRow>>,
    body: Option<String>,
    username: Option<String>,
    entity_id: Option<String>,
    entity_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RtdsPriceRow {
    symbol: Option<String>,
    timestamp: Option<i64>,
    value: Option<f64>,
}

/// Price source within RTDS
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtdsPriceSource {
    /// Binance exchange prices (topic: crypto_prices)
    Binance,
    /// Chainlink oracle prices (topic: crypto_prices_chainlink)
    Chainlink,
}

#[derive(Debug, Clone)]
pub struct RtdsClient {
    connected: bool,
    subscriptions: Vec<Asset>,
    last_ping: Instant,
    /// Which price sources to subscribe to (default: both)
    sources: Vec<RtdsPriceSource>,
}

impl RtdsClient {
    pub fn new() -> Self {
        Self {
            connected: false,
            subscriptions: Vec::new(),
            last_ping: Instant::now(),
            // Default to Chainlink-only to reduce RTDS rate-limit pressure and
            // align with Polymarket's primary oracle reference source.
            sources: vec![RtdsPriceSource::Chainlink],
        }
    }

    /// Create client with specific sources
    pub fn with_sources(sources: Vec<RtdsPriceSource>) -> Self {
        Self {
            connected: false,
            subscriptions: Vec::new(),
            last_ping: Instant::now(),
            sources,
        }
    }

    /// Convert Asset to RTDS Binance symbol format (lowercase concatenated: btcusdt)
    fn asset_to_binance_symbol(asset: Asset) -> &'static str {
        match asset {
            Asset::BTC => "btcusdt",
            Asset::ETH => "ethusdt",
            Asset::SOL => "solusdt",
            Asset::XRP => "xrpusdt",
        }
    }

    /// Convert Asset to RTDS Chainlink symbol format (slash-separated: btc/usd)
    fn asset_to_chainlink_symbol(asset: Asset) -> &'static str {
        match asset {
            Asset::BTC => "btc/usd",
            Asset::ETH => "eth/usd",
            Asset::SOL => "sol/usd",
            Asset::XRP => "xrp/usd",
        }
    }

    /// Parse RTDS symbol to Asset (handles both Binance and Chainlink formats)
    fn parse_symbol(symbol: &str) -> Option<Asset> {
        let normalized: String = symbol
            .trim()
            .to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();

        match normalized.as_str() {
            "btc" | "btcusd" | "btcusdt" | "btcusdc" => Some(Asset::BTC),
            "eth" | "ethusd" | "ethusdt" | "ethusdc" => Some(Asset::ETH),
            "sol" | "solusd" | "solusdt" | "solusdc" => Some(Asset::SOL),
            "xrp" | "xrpusd" | "xrpusdt" | "xrpusdc" => Some(Asset::XRP),
            _ => None,
        }
    }

    /// Determine which price source based on topic
    fn get_source_from_topic(topic: &str) -> Option<RtdsPriceSource> {
        match topic {
            "crypto_prices" => Some(RtdsPriceSource::Binance),
            "crypto_prices_chainlink" => Some(RtdsPriceSource::Chainlink),
            _ => None,
        }
    }

    /// Some RTDS snapshot messages can arrive on `crypto_prices` even for `/usd` symbols.
    /// Infer Chainlink when symbol clearly identifies an oracle pair.
    fn infer_source_from_symbol(topic_source: RtdsPriceSource, symbol: &str) -> RtdsPriceSource {
        let s = symbol.trim().to_lowercase();
        if s.contains('/') || (s.ends_with("usd") && !s.ends_with("usdt")) {
            RtdsPriceSource::Chainlink
        } else {
            topic_source
        }
    }
}

#[async_trait]
impl PriceSource for RtdsClient {
    fn name(&self) -> &'static str {
        "RTDS"
    }

    async fn connect(&mut self, tx: Sender<SourceEvent>) -> Result<()> {
        let mut attempt = 0u32;

        loop {
            attempt = attempt.saturating_add(1);

            // Exponential backoff (capped at 60s) + jitter up to 20%.
            let base_delay_secs = (1u64 << attempt.saturating_sub(1).min(6)).min(60);
            let jitter_cap = ((base_delay_secs as f64) * 0.20).round() as u64;
            let jitter_secs = if jitter_cap > 0 {
                rand::thread_rng().gen_range(0..=jitter_cap)
            } else {
                0
            };
            let delay_secs = base_delay_secs.saturating_add(jitter_secs);
            if attempt > 1 {
                tracing::warn!(
                    source = %"RTDS",
                    attempt = attempt,
                    delay_secs = delay_secs,
                    "Reconnecting RTDS with exponential backoff + jitter"
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(delay_secs)).await;
            }

            tracing::info!(
                source = %"RTDS",
                url = %RTDS_WS_URL,
                attempt = attempt,
                "Connecting to Polymarket RTDS WebSocket..."
            );

            match connect_async(RTDS_WS_URL).await {
                Ok((ws_stream, _)) => {
                    let (mut write, mut read) = ws_stream.split();

                    // Build subscriptions for both Binance and Chainlink sources.
                    let mut subscriptions: Vec<RtdsSubscription> = Vec::new();

                    // Subscribe to Binance source (crypto_prices) per symbol.
                    if self.sources.contains(&RtdsPriceSource::Binance) {
                        for &asset in &self.subscriptions {
                            let symbol = Self::asset_to_binance_symbol(asset);
                            let filters = format!(r#"{{"symbol":"{}"}}"#, symbol);
                            subscriptions.push(RtdsSubscription {
                                topic: "crypto_prices".to_string(),
                                msg_type: "update".to_string(),
                                filters: Some(filters),
                            });
                        }
                        let symbols: Vec<&str> = self
                            .subscriptions
                            .iter()
                            .copied()
                            .map(Self::asset_to_binance_symbol)
                            .collect();
                        tracing::info!(
                            source = %"RTDS-Binance",
                            symbols = ?symbols,
                            "Adding Binance subscriptions"
                        );
                    }

                    // Subscribe to Chainlink source (crypto_prices_chainlink) per symbol.
                    if self.sources.contains(&RtdsPriceSource::Chainlink) {
                        for &asset in &self.subscriptions {
                            let symbol = Self::asset_to_chainlink_symbol(asset);
                            let filters = format!(r#"{{"symbol":"{}"}}"#, symbol);
                            subscriptions.push(RtdsSubscription {
                                topic: "crypto_prices_chainlink".to_string(),
                                msg_type: "update".to_string(),
                                filters: Some(filters),
                            });
                        }
                        let chainlink_symbols: Vec<&str> = self
                            .subscriptions
                            .iter()
                            .copied()
                            .map(Self::asset_to_chainlink_symbol)
                            .collect();
                        tracing::info!(
                            source = %"RTDS-Chainlink",
                            symbols = ?chainlink_symbols,
                            "Adding Chainlink subscriptions"
                        );
                    }

                    if subscriptions.is_empty() {
                        tracing::warn!(source = %"RTDS", "No subscriptions configured");
                        continue;
                    }

                    let sub_msg = RtdsSubscribeRequest {
                        action: "subscribe".to_string(),
                        subscriptions,
                    };

                    let sub_json = serde_json::to_string(&sub_msg)?;
                    tracing::info!(source = %"RTDS", "📡 Sending dual-source subscription (Binance + Chainlink)");

                    if let Err(e) = write.send(Message::Text(sub_json)).await {
                        tracing::error!(source = %"RTDS", error = %e, "Failed to send subscription");
                        continue;
                    }

                    self.connected = true;
                    self.last_ping = Instant::now();
                    attempt = 0;
                    let _ = tx.send(SourceEvent::Connected("RTDS".to_string())).await;

                    tracing::info!(source = %"RTDS", "✅ Connected to RTDS WebSocket");

                    let mut ping_interval =
                        tokio::time::interval(tokio::time::Duration::from_secs(PING_INTERVAL_SECS));
                    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    let mut watchdog_interval =
                        tokio::time::interval(tokio::time::Duration::from_secs(WATCHDOG_TICK_SECS));
                    watchdog_interval
                        .set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    let mut last_useful_message = Instant::now();

                    // Message handling loop with inactivity watchdog.
                    'socket: loop {
                        tokio::select! {
                            _ = ping_interval.tick() => {
                                if let Err(e) = write.send(Message::Text("ping".to_string())).await {
                                    tracing::warn!(source = %"RTDS", error = %e, "Failed to send ping");
                                    break 'socket;
                                }
                                self.last_ping = Instant::now();
                            }
                            _ = watchdog_interval.tick() => {
                                if should_reconnect_due_to_silence(last_useful_message, WATCHDOG_SILENCE_SECS) {
                                    let reason = format!(
                                        "RTDS watchdog timeout: no useful messages for {}s",
                                        WATCHDOG_SILENCE_SECS
                                    );
                                    tracing::warn!(source = %"RTDS", reason = %reason, "Forcing reconnect");
                                    let _ = tx
                                        .send(SourceEvent::Error("RTDS".to_string(), reason))
                                        .await;
                                    let _ = write.send(Message::Close(None)).await;
                                    break 'socket;
                                }
                            }
                            msg = read.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        if text.trim() == "pong" {
                                            continue;
                                        }
                                        match Self::handle_message(&text, &tx).await {
                                            Ok(was_useful) => {
                                                if was_useful {
                                                    last_useful_message = Instant::now();
                                                }
                                            }
                                            Err(e) => {
                                                let msg = e.to_string();
                                                tracing::warn!(source = %"RTDS", error = %msg, "Failed to parse/process message");
                                                if msg.contains("RTDS server error") {
                                                    let _ = tx
                                                        .send(SourceEvent::Error("RTDS".to_string(), msg.clone()))
                                                        .await;
                                                    if msg.contains("Too Many Requests") {
                                                        let cooldown_secs = 30u64;
                                                        tracing::warn!(
                                                            source = %"RTDS",
                                                            cooldown_secs = cooldown_secs,
                                                            "RTDS rate-limited; backing off before reconnect"
                                                        );
                                                        tokio::time::sleep(tokio::time::Duration::from_secs(
                                                            cooldown_secs,
                                                        ))
                                                        .await;
                                                    }
                                                    break 'socket;
                                                }
                                            }
                                        }
                                    }
                                    Some(Ok(Message::Ping(data))) => {
                                        let _ = write.send(Message::Pong(data)).await;
                                    }
                                    Some(Ok(Message::Pong(_))) => {
                                        // Protocol-level pong keeps socket alive.
                                    }
                                    Some(Ok(Message::Close(frame))) => {
                                        tracing::warn!(source = %"RTDS", ?frame, "Connection closed by server");
                                        break 'socket;
                                    }
                                    Some(Err(e)) => {
                                        tracing::error!(source = %"RTDS", error = %e, "WebSocket error");
                                        let _ = tx
                                            .send(SourceEvent::Error("RTDS".to_string(), e.to_string()))
                                            .await;
                                        break 'socket;
                                    }
                                    None => {
                                        tracing::warn!(source = %"RTDS", "WebSocket stream ended");
                                        break 'socket;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    // Connection lost
                    self.connected = false;
                    let _ = tx.send(SourceEvent::Disconnected("RTDS".to_string())).await;
                }
                Err(e) => {
                    tracing::error!(source = %"RTDS", error = %e, "Failed to connect");
                    let _ = tx
                        .send(SourceEvent::Error("RTDS".to_string(), e.to_string()))
                        .await;
                }
            }
        }
    }

    async fn subscribe(&mut self, assets: &[Asset]) -> Result<()> {
        self.subscriptions = assets.to_vec();
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

impl RtdsClient {
    async fn handle_message(text: &str, tx: &Sender<SourceEvent>) -> Result<bool> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(false);
        }
        let msg: RtdsMessage = serde_json::from_str(text)?;

        // Get the topic and determine the source
        let topic = match &msg.topic {
            Some(t) => t.as_str(),
            None => {
                if let Some(server_message) = &msg.message {
                    let status = msg.status_code.unwrap_or_default();
                    let connection_id = msg.connection_id.as_deref().unwrap_or("n/a");
                    let request_id = msg.request_id.as_deref().unwrap_or("n/a");
                    bail!(
                        "RTDS server error (status={} connection_id={} request_id={}): {}",
                        status,
                        connection_id,
                        request_id,
                        server_message
                    );
                }
                tracing::debug!(source = %"RTDS", "Message without topic");
                return Ok(false);
            }
        };

        if topic == "comments" {
            if let Some(payload) = msg.payload {
                let comment = CommentEvent {
                    topic: "comments".to_string(),
                    symbol: payload.symbol,
                    body: payload.body,
                    username: payload.username,
                    timestamp: payload
                        .timestamp
                        .or(msg.timestamp)
                        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
                };
                let _ = tx.send(SourceEvent::Comment(comment)).await;
                return Ok(true);
            }
            return Ok(false);
        }

        // Only process crypto price messages (both Binance and Chainlink)
        let price_source = match Self::get_source_from_topic(topic) {
            Some(s) => s,
            None => {
                tracing::debug!(source = %"RTDS", topic = %topic, "Received non-price message");
                return Ok(false);
            }
        };

        // Extract payload
        let payload = match msg.payload {
            Some(ref p) => p,
            None => return Ok(false),
        };

        let mut emitted = false;
        // Snapshot payloads include an array of rows.
        if let Some(rows) = &payload.data {
            let fallback_symbol = payload.symbol.as_deref();
            for row in rows {
                if let Some(symbol) = row.symbol.as_deref().or(fallback_symbol) {
                    emitted |= Self::emit_price_tick(
                        tx,
                        price_source,
                        symbol,
                        row.value,
                        row.timestamp.or(msg.timestamp),
                    )
                    .await?;
                }
            }
            return Ok(emitted);
        }

        // Standard update payload (single symbol/value pair).
        let symbol = match payload.symbol.as_deref() {
            Some(s) => s,
            None => return Ok(false),
        };
        emitted |= Self::emit_price_tick(
            tx,
            price_source,
            symbol,
            payload.value,
            payload.timestamp.or(msg.timestamp),
        )
        .await?;

        Ok(emitted)
    }
    async fn emit_price_tick(
        tx: &Sender<SourceEvent>,
        price_source: RtdsPriceSource,
        symbol: &str,
        price_opt: Option<f64>,
        ts_opt: Option<i64>,
    ) -> Result<bool> {
        let asset = match Self::parse_symbol(symbol) {
            Some(a) => a,
            None => {
                tracing::debug!(source = %"RTDS", symbol = %symbol, "Unknown symbol");
                return Ok(false);
            }
        };

        let price = match price_opt {
            Some(v) if v > 0.0 => v,
            _ => return Ok(false),
        };

        // Use payload timestamp, fallback to current timestamp.
        let ts =
            normalize_epoch_millis(ts_opt.unwrap_or_else(|| chrono::Utc::now().timestamp_millis()));

        let effective_source = Self::infer_source_from_symbol(price_source, symbol);

        let source_label = match effective_source {
            RtdsPriceSource::Binance => "RTDS-Binance",
            RtdsPriceSource::Chainlink => "RTDS-Chainlink",
        };
        tracing::debug!(
            source = %source_label,
            asset = ?asset,
            price = price,
            symbol = %symbol,
            "RTDS price received"
        );

        let source = match effective_source {
            RtdsPriceSource::Binance => Source::RTDS,
            RtdsPriceSource::Chainlink => Source::RtdsChainlink,
        };

        let tick = NormalizedTick {
            ts,
            asset,
            bid: price, // RTDS gives a single price, treat as mid/bid/ask for paper analytics.
            ask: price,
            mid: price,
            source,
            latency_ms: (chrono::Utc::now().timestamp_millis() as u64).saturating_sub(ts as u64),
        };

        let _ = tx.send(SourceEvent::Tick(tick)).await;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[test]
    fn normalize_epoch_millis_converts_seconds_to_millis() {
        let seconds = 1_707_771_234_i64;
        assert_eq!(normalize_epoch_millis(seconds), seconds * 1000);

        let millis = 1_707_771_234_567_i64;
        assert_eq!(normalize_epoch_millis(millis), millis);
    }

    #[test]
    fn watchdog_silence_helper_detects_timeout() {
        let now = Instant::now();
        assert!(!should_reconnect_due_to_silence(now, 20));

        let stale = now
            .checked_sub(std::time::Duration::from_secs(25))
            .expect("instant subtraction should work");
        assert!(should_reconnect_due_to_silence(stale, 20));
    }

    #[tokio::test]
    async fn handle_message_emits_tick_and_marks_useful() {
        let (tx, mut rx) = mpsc::channel(4);
        let msg = serde_json::json!({
            "topic": "crypto_prices_chainlink",
            "timestamp": 1707771234,
            "payload": {
                "symbol": "btc/usd",
                "value": 52000.5
            }
        });

        let useful = RtdsClient::handle_message(&msg.to_string(), &tx)
            .await
            .expect("message handling should succeed");
        assert!(useful);

        match rx.recv().await {
            Some(SourceEvent::Tick(tick)) => {
                assert_eq!(tick.asset, Asset::BTC);
                assert_eq!(tick.source, Source::RtdsChainlink);
                assert_eq!(tick.ts, 1_707_771_234_000);
            }
            other => panic!("expected tick event, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn handle_message_ignores_non_price_topics() {
        let (tx, mut rx) = mpsc::channel(2);
        let msg = serde_json::json!({
            "topic": "unknown_topic",
            "payload": {
                "symbol": "btc/usd",
                "value": 51000.0
            }
        });

        let useful = RtdsClient::handle_message(&msg.to_string(), &tx)
            .await
            .expect("non-price message should not error");
        assert!(!useful);
        assert!(rx.try_recv().is_err());
    }
}

fn parse_rtds_asset(s: &str) -> Result<Asset> {
    match s.to_uppercase().as_str() {
        "BTC" | "BTCUSD" | "BTC-USD" | "BTCUSDT" => Ok(Asset::BTC),
        "ETH" | "ETHUSD" | "ETH-USD" | "ETHUSDT" => Ok(Asset::ETH),
        "SOL" | "SOLUSD" | "SOL-USD" | "SOLUSDT" => Ok(Asset::SOL),
        "XRP" | "XRPUSD" | "XRP-USD" | "XRPUSDT" => Ok(Asset::XRP),
        _ => bail!("Unknown RTDS asset: {}", s),
    }
}
