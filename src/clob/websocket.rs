//! CLOB WebSocket Client
//!
//! Handles real-time updates from Polymarket CLOB WebSocket
//! Supports:
//! - Order updates (private channel, requires auth)
//! - Trade updates
//! - Market data (orderbook, price changes) via "market" channel

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message as TungsteniteMessage};
use tracing::{debug, error, info, warn};

use super::types::{BookLevel, Order, OrderBook, Side, Trade};

fn parse_book_level(price: &str, size: &str) -> Option<BookLevel> {
    let price = price.parse::<f64>().ok()?;
    let size = size.parse::<f64>().ok()?;
    if !price.is_finite() || !size.is_finite() || price <= 0.0 || size <= 0.0 {
        return None;
    }
    Some(BookLevel { price, size })
}

fn build_normalized_book(
    token_id: String,
    bids: Vec<(String, String)>,
    asks: Vec<(String, String)>,
    timestamp: i64,
) -> OrderBook {
    let mut book = OrderBook {
        token_id,
        bids: bids
            .into_iter()
            .filter_map(|(price, size)| parse_book_level(&price, &size))
            .collect(),
        asks: asks
            .into_iter()
            .filter_map(|(price, size)| parse_book_level(&price, &size))
            .collect(),
        timestamp,
    };
    book.normalize_levels();
    book
}

const MARKET_FEED_WATCHDOG_SILENCE_SECS: u64 = 20;
const MARKET_FEED_WATCHDOG_TICK_SECS: u64 = 5;
const MARKET_FEED_PING_INTERVAL_SECS: u64 = 10;
const MARKET_FEED_BASE_BACKOFF_SECS: u64 = 1;
const MARKET_FEED_MAX_BACKOFF_SECS: u64 = 60;
const MARKET_FEED_BACKOFF_JITTER_RATIO: f64 = 0.20;

fn should_reconnect_due_to_silence(last_useful_message: Instant, silence_secs: u64) -> bool {
    last_useful_message.elapsed().as_secs() >= silence_secs
}

fn backoff_with_jitter_secs(attempt: u32) -> u64 {
    let capped_attempt = attempt.min(16);
    let base = MARKET_FEED_BASE_BACKOFF_SECS.saturating_mul(1u64 << capped_attempt);
    let bounded = base.min(MARKET_FEED_MAX_BACKOFF_SECS).max(1);

    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_micros() as f64;
    let unit = (micros % 1_000.0) / 1_000.0;
    let jitter = 1.0 + ((unit * 2.0) - 1.0) * MARKET_FEED_BACKOFF_JITTER_RATIO;
    ((bounded as f64) * jitter)
        .round()
        .clamp(1.0, MARKET_FEED_MAX_BACKOFF_SECS as f64) as u64
}

/// Subscription channel types for Polymarket WebSocket
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionChannel {
    /// Order updates (private, requires auth)
    Order,
    /// Trade updates
    Trade,
    /// Market data (orderbook, price changes)
    Market,
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
#[serde(rename_all = "snake_case")]
pub enum WsMessage {
    /// Subscribe to channel
    #[serde(rename = "subscribe")]
    Subscribe {
        channel: String,
        auth: Option<String>,
    },
    /// Unsubscribe from channel
    #[serde(rename = "unsubscribe")]
    Unsubscribe { channel: String },
    /// Order update
    #[serde(rename = "order")]
    OrderUpdate { data: OrderUpdateData },
    /// Trade update
    #[serde(rename = "trade")]
    TradeUpdate { data: TradeUpdateData },
    /// Book update (from market channel)
    #[serde(rename = "book")]
    BookUpdate { data: BookUpdateData },
    /// Price change event (from market channel)
    #[serde(rename = "price_change")]
    PriceChange { data: PriceChangeData },
    /// Last trade price event (from market channel)
    #[serde(rename = "last_trade_price")]
    LastTradePrice { data: LastTradePriceData },
    /// Heartbeat
    #[serde(rename = "heartbeat")]
    Heartbeat { ts: i64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderUpdateData {
    pub order_id: String,
    pub status: String,
    pub filled_size: Option<f64>,
    pub avg_fill_price: Option<f64>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeUpdateData {
    pub trade_id: String,
    pub order_id: String,
    pub token_id: String,
    pub side: String,
    pub price: f64,
    pub size: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookUpdateData {
    pub token_id: String,
    pub bids: Vec<(String, String)>,
    pub asks: Vec<(String, String)>,
    pub timestamp: i64,
}

/// Price change event from market channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceChangeData {
    pub token_id: String,
    pub price: String,
    pub side: Option<String>,
    pub timestamp: Option<i64>,
}

/// Last trade price event from market channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastTradePriceData {
    pub token_id: String,
    pub price: String,
    pub side: String,
    pub size: Option<String>,
    pub timestamp: Option<i64>,
}

/// Market data event (aggregates book, price_change, last_trade_price)
#[derive(Debug, Clone)]
pub struct MarketData {
    pub token_id: String,
    pub orderbook: Option<OrderBook>,
    pub last_price: Option<f64>,
    pub last_trade_side: Option<Side>,
    pub timestamp: i64,
}

/// Events from the WebSocket client
#[derive(Debug, Clone)]
pub enum WsEvent {
    OrderUpdate(Order),
    Trade(Trade),
    BookUpdate(OrderBook),
    /// Market data update (includes orderbook changes)
    MarketUpdate(MarketData),
    Connected,
    Disconnected,
    Error(String),
}

/// WebSocket client for CLOB updates
pub struct WsClient {
    url: String,
    api_key: Option<String>,
    event_tx: mpsc::Sender<WsEvent>,
    shutdown_rx: mpsc::Receiver<()>,
}

impl WsClient {
    /// Create a new WebSocket client
    pub fn new(
        url: &str,
        api_key: Option<String>,
        event_tx: mpsc::Sender<WsEvent>,
        shutdown_rx: mpsc::Receiver<()>,
    ) -> Self {
        Self {
            url: url.to_string(),
            api_key,
            event_tx,
            shutdown_rx,
        }
    }

    /// Start the WebSocket connection
    pub async fn run(mut self) -> Result<()> {
        info!("Connecting to CLOB WebSocket: {}", self.url);

        let (ws_stream, _) = connect_async(&self.url)
            .await
            .context("Failed to connect to CLOB WebSocket")?;

        info!("Connected to CLOB WebSocket");

        let (mut write, mut read) = ws_stream.split();

        // Send subscription for order updates if we have auth
        if let Some(ref key) = self.api_key {
            let sub_msg = serde_json::json!({
                "auth": key,
                "subscribe": ["order", "trade"]
            });
            write
                .send(TungsteniteMessage::Text(sub_msg.to_string()))
                .await?;
        }

        // Notify connected
        let _ = self.event_tx.send(WsEvent::Connected).await;

        // Message loop
        loop {
            tokio::select! {
                // Handle incoming messages
                msg = read.next() => {
                    match msg {
                        Some(Ok(TungsteniteMessage::Text(text))) => {
                            if let Err(e) = self.handle_message(&text).await {
                                warn!("Error handling WebSocket message: {}", e);
                            }
                        }
                        Some(Ok(TungsteniteMessage::Ping(data))) => {
                            let _ = write.send(TungsteniteMessage::Pong(data)).await;
                        }
                        Some(Ok(TungsteniteMessage::Close(_))) => {
                            info!("WebSocket closed by server");
                            let _ = self.event_tx.send(WsEvent::Disconnected).await;
                            break;
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            let _ = self.event_tx.send(WsEvent::Error(e.to_string())).await;
                            break;
                        }
                        None => {
                            info!("WebSocket stream ended");
                            break;
                        }
                        _ => {}
                    }
                }

                // Handle shutdown
                _ = self.shutdown_rx.recv() => {
                    info!("Shutting down WebSocket client");
                    let _ = write.send(TungsteniteMessage::Close(None)).await;
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle incoming message
    async fn handle_message(&self, text: &str) -> Result<()> {
        debug!("Received WebSocket message: {}", text);

        // Try to parse as generic message
        let value: serde_json::Value = serde_json::from_str(text)?;

        if let Some(event_type) = value.get("event_type").and_then(|v| v.as_str()) {
            match event_type {
                "order" => {
                    if let Some(data) = value.get("data") {
                        let update: OrderUpdateData = serde_json::from_value(data.clone())?;
                        self.handle_order_update(update).await?;
                    }
                }
                "trade" => {
                    if let Some(data) = value.get("data") {
                        let update: TradeUpdateData = serde_json::from_value(data.clone())?;
                        self.handle_trade_update(update).await?;
                    }
                }
                "book" => {
                    if let Some(data) = value.get("data") {
                        let update: BookUpdateData = serde_json::from_value(data.clone())?;
                        self.handle_book_update(update).await?;
                    }
                }
                "price_change" => {
                    if let Some(data) = value.get("data") {
                        let update: PriceChangeData = serde_json::from_value(data.clone())?;
                        self.handle_price_change(update).await?;
                    }
                }
                "last_trade_price" => {
                    if let Some(data) = value.get("data") {
                        let update: LastTradePriceData = serde_json::from_value(data.clone())?;
                        self.handle_last_trade_price(update).await?;
                    }
                }
                "heartbeat" => {
                    debug!("Received heartbeat");
                }
                _ => {
                    debug!("Unknown event type: {}", event_type);
                }
            }
        }

        Ok(())
    }

    async fn handle_order_update(&self, update: OrderUpdateData) -> Result<()> {
        info!("Order update: {} -> {}", update.order_id, update.status);

        let order = Order {
            id: ethers::types::H256::from_slice(
                &hex::decode(update.order_id.replace("0x", "")).unwrap_or_default(),
            ),
            token_id: String::new(),
            side: Side::Buy,
            price: 0.0,
            size: 0.0,
            status: super::types::OrderStatus::Open,
            filled_size: update.filled_size.unwrap_or(0.0),
            avg_fill_price: update.avg_fill_price.unwrap_or(0.0),
            created_at: 0,
            expires_at: 0,
            signature_type: 0,
            signature: None,
            maker: None,
            salt: ethers::types::U256::zero(),
            nonce: 0,
            expiration: 0,
        };

        let _ = self.event_tx.send(WsEvent::OrderUpdate(order)).await;
        Ok(())
    }

    async fn handle_trade_update(&self, update: TradeUpdateData) -> Result<()> {
        info!(
            "Trade: {} {} @ {}",
            update.size, update.token_id, update.price
        );

        let trade = Trade {
            id: update.trade_id,
            order_id: ethers::types::H256::zero(),
            token_id: update.token_id,
            side: match update.side.as_str() {
                "BUY" => Side::Buy,
                "SELL" => Side::Sell,
                _ => Side::Buy,
            },
            price: update.price,
            size: update.size,
            timestamp: update.timestamp,
            taker: ethers::types::Address::zero(),
            maker: ethers::types::Address::zero(),
        };

        let _ = self.event_tx.send(WsEvent::Trade(trade)).await;
        Ok(())
    }

    async fn handle_book_update(&self, update: BookUpdateData) -> Result<()> {
        let book =
            build_normalized_book(update.token_id, update.bids, update.asks, update.timestamp);

        let _ = self.event_tx.send(WsEvent::BookUpdate(book)).await;
        Ok(())
    }

    async fn handle_price_change(&self, update: PriceChangeData) -> Result<()> {
        debug!("Price change: {} -> {}", update.token_id, update.price);

        let price: f64 = update.price.parse().unwrap_or(0.0);
        let side = update.side.and_then(|s| match s.to_uppercase().as_str() {
            "BUY" => Some(Side::Buy),
            "SELL" => Some(Side::Sell),
            _ => None,
        });

        let market_data = MarketData {
            token_id: update.token_id,
            orderbook: None,
            last_price: Some(price),
            last_trade_side: side,
            timestamp: update.timestamp.unwrap_or(chrono::Utc::now().timestamp()),
        };

        let _ = self.event_tx.send(WsEvent::MarketUpdate(market_data)).await;
        Ok(())
    }

    async fn handle_last_trade_price(&self, update: LastTradePriceData) -> Result<()> {
        debug!(
            "Last trade: {} {} @ {}",
            update.side, update.token_id, update.price
        );

        let price: f64 = update.price.parse().unwrap_or(0.0);
        let size: f64 = update
            .size
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let side = match update.side.to_uppercase().as_str() {
            "BUY" => Side::Buy,
            "SELL" => Side::Sell,
            _ => Side::Buy,
        };

        let token_id = update.token_id.clone();
        let market_data = MarketData {
            token_id: token_id.clone(),
            orderbook: None,
            last_price: Some(price),
            last_trade_side: Some(side),
            timestamp: update.timestamp.unwrap_or(chrono::Utc::now().timestamp()),
        };

        let _ = self.event_tx.send(WsEvent::MarketUpdate(market_data)).await;

        let trade = Trade {
            id: format!("trade-{}", update.timestamp.unwrap_or(0)),
            order_id: ethers::types::H256::zero(),
            token_id,
            side,
            price,
            size,
            timestamp: update.timestamp.unwrap_or(chrono::Utc::now().timestamp()),
            taker: ethers::types::Address::zero(),
            maker: ethers::types::Address::zero(),
        };

        let _ = self.event_tx.send(WsEvent::Trade(trade)).await;
        Ok(())
    }
}

/// Subscribe to order book updates for a token
pub async fn subscribe_to_book(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        TungsteniteMessage,
    >,
    token_id: &str,
) -> Result<()> {
    let msg = serde_json::json!({
        "subscribe": ["book"],
        "token_id": token_id
    });
    write
        .send(TungsteniteMessage::Text(msg.to_string()))
        .await?;
    Ok(())
}

/// Subscribe to market channel for real-time orderbook and price updates
/// According to Polymarket docs: {"subscribe": "market", "assets_ids": ["token_id_1", "token_id_2"]}
pub async fn subscribe_to_market(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        TungsteniteMessage,
    >,
    token_ids: &[&str],
) -> Result<()> {
    let msg = serde_json::json!({
        "subscribe": "market",
        "assets_ids": token_ids
    });
    info!(
        "Subscribing to market channel for {} tokens",
        token_ids.len()
    );
    write
        .send(TungsteniteMessage::Text(msg.to_string()))
        .await?;
    Ok(())
}

/// Market data feed client - manages WebSocket connection for orderbook data
pub struct MarketFeedClient {
    url: String,
    event_tx: mpsc::Sender<WsEvent>,
    subscribed_tokens: HashSet<String>,
}

impl MarketFeedClient {
    pub fn new(url: &str, event_tx: mpsc::Sender<WsEvent>) -> Self {
        Self {
            url: url.to_string(),
            event_tx,
            subscribed_tokens: HashSet::new(),
        }
    }

    pub async fn run(
        mut self,
        token_ids: Vec<String>,
        mut subscribe_rx: mpsc::Receiver<Vec<String>>,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) -> Result<()> {
        for token in &token_ids {
            self.subscribed_tokens.insert(token.clone());
        }

        let mut reconnect_attempt: u32 = 0;
        loop {
            if shutdown_rx.try_recv().is_ok() {
                info!("Market feed client shutdown requested");
                break;
            }

            info!(
                attempt = reconnect_attempt + 1,
                "Connecting to market feed WebSocket: {}", self.url
            );

            let (ws_stream, _) = match connect_async(&self.url).await {
                Ok(result) => result,
                Err(e) => {
                    warn!(error = %e, "Failed to connect market feed WebSocket");
                    let _ = self
                        .event_tx
                        .send(WsEvent::Error(format!("market_feed_connect_failed: {e}")))
                        .await;
                    let _ = self.event_tx.send(WsEvent::Disconnected).await;
                    reconnect_attempt = reconnect_attempt.saturating_add(1);
                    let sleep_secs = backoff_with_jitter_secs(reconnect_attempt);
                    warn!(
                        sleep_secs,
                        "Retrying market feed connection with exponential backoff + jitter"
                    );
                    tokio::time::sleep(Duration::from_secs(sleep_secs)).await;
                    continue;
                }
            };

            info!("Connected to market feed WebSocket");
            reconnect_attempt = 0;
            let (mut write, mut read) = ws_stream.split();

            let snapshot_tokens: Vec<String> = self.subscribed_tokens.iter().cloned().collect();
            if !snapshot_tokens.is_empty() {
                let token_refs: Vec<&str> = snapshot_tokens.iter().map(String::as_str).collect();
                if let Err(e) = subscribe_to_market(&mut write, &token_refs).await {
                    warn!(
                        error = %e,
                        "Failed to subscribe initial market tokens after reconnect"
                    );
                    let _ = self
                        .event_tx
                        .send(WsEvent::Error(format!("market_feed_subscribe_failed: {e}")))
                        .await;
                    let _ = self.event_tx.send(WsEvent::Disconnected).await;
                    reconnect_attempt = reconnect_attempt.saturating_add(1);
                    let sleep_secs = backoff_with_jitter_secs(reconnect_attempt);
                    tokio::time::sleep(Duration::from_secs(sleep_secs)).await;
                    continue;
                }
            }

            let _ = self.event_tx.send(WsEvent::Connected).await;

            let mut ping_interval =
                tokio::time::interval(Duration::from_secs(MARKET_FEED_PING_INTERVAL_SECS));
            ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

            let mut watchdog_interval =
                tokio::time::interval(Duration::from_secs(MARKET_FEED_WATCHDOG_TICK_SECS));
            watchdog_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

            let mut last_useful_message = Instant::now();
            let reconnect_reason: &'static str = loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(TungsteniteMessage::Text(text))) => {
                                match self.handle_message(&text).await {
                                    Ok(useful_message) => {
                                        if useful_message {
                                            last_useful_message = Instant::now();
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Error handling market feed message: {}", e);
                                    }
                                }
                            }
                            Some(Ok(TungsteniteMessage::Ping(data))) => {
                                let _ = write.send(TungsteniteMessage::Pong(data)).await;
                            }
                            Some(Ok(TungsteniteMessage::Pong(_))) => {
                                last_useful_message = Instant::now();
                            }
                            Some(Ok(TungsteniteMessage::Close(_))) => {
                                info!("Market feed WebSocket closed by server");
                                break "remote_close";
                            }
                            Some(Err(e)) => {
                                error!("Market feed WebSocket error: {}", e);
                                let _ = self.event_tx.send(WsEvent::Error(format!("market_feed_stream_error: {e}"))).await;
                                break "stream_error";
                            }
                            None => {
                                info!("Market feed WebSocket stream ended");
                                break "stream_ended";
                            }
                            _ => {}
                        }
                    }

                    new_tokens = subscribe_rx.recv() => {
                        if let Some(tokens) = new_tokens {
                            let mut new_tokens = Vec::new();
                            for token in tokens {
                                if self.subscribed_tokens.insert(token.clone()) {
                                    new_tokens.push(token);
                                }
                            }
                            if !new_tokens.is_empty() {
                                let token_refs: Vec<&str> = new_tokens.iter().map(String::as_str).collect();
                                if let Err(e) = subscribe_to_market(&mut write, &token_refs).await {
                                    warn!(error = %e, "Failed to subscribe incremental market tokens");
                                    let _ = self.event_tx.send(WsEvent::Error(format!("market_feed_subscribe_send_failed: {e}"))).await;
                                    break "subscribe_send_failed";
                                }
                            }
                        }
                    }

                    _ = ping_interval.tick() => {
                        if let Err(e) = write.send(TungsteniteMessage::Ping(Vec::new())).await {
                            warn!(error = %e, "Market feed ping failed; reconnecting");
                            break "ping_send_failed";
                        }
                    }

                    _ = watchdog_interval.tick() => {
                        if should_reconnect_due_to_silence(
                            last_useful_message,
                            MARKET_FEED_WATCHDOG_SILENCE_SECS
                        ) {
                            warn!(
                                silence_secs = MARKET_FEED_WATCHDOG_SILENCE_SECS,
                                "Market feed watchdog timeout: reconnecting"
                            );
                            let _ = self.event_tx.send(WsEvent::Error(
                                format!(
                                    "market_feed_watchdog_timeout: no useful messages for {}s",
                                    MARKET_FEED_WATCHDOG_SILENCE_SECS
                                ),
                            )).await;
                            let _ = write.send(TungsteniteMessage::Close(None)).await;
                            break "watchdog_timeout";
                        }
                    }

                    _ = shutdown_rx.recv() => {
                        info!("Shutting down market feed client");
                        let _ = write.send(TungsteniteMessage::Close(None)).await;
                        return Ok(());
                    }
                }
            };

            let _ = self.event_tx.send(WsEvent::Disconnected).await;
            reconnect_attempt = reconnect_attempt.saturating_add(1);
            let sleep_secs = backoff_with_jitter_secs(reconnect_attempt);
            warn!(
                reason = reconnect_reason,
                attempt = reconnect_attempt,
                sleep_secs,
                "Market feed reconnect scheduled"
            );
            tokio::time::sleep(Duration::from_secs(sleep_secs)).await;
        }

        Ok(())
    }

    async fn handle_message(&self, text: &str) -> Result<bool> {
        let value: serde_json::Value = serde_json::from_str(text)?;

        if let Some(event_type) = value.get("event_type").and_then(|v| v.as_str()) {
            match event_type {
                "book" => {
                    if let Some(data) = value.get("data") {
                        let update: BookUpdateData = serde_json::from_value(data.clone())?;
                        let book = build_normalized_book(
                            update.token_id,
                            update.bids,
                            update.asks,
                            update.timestamp,
                        );

                        let market_data = MarketData {
                            token_id: book.token_id.clone(),
                            orderbook: Some(book.clone()),
                            last_price: book.mid_price(),
                            last_trade_side: None,
                            timestamp: book.timestamp,
                        };

                        let _ = self.event_tx.send(WsEvent::MarketUpdate(market_data)).await;
                        let _ = self.event_tx.send(WsEvent::BookUpdate(book)).await;
                        return Ok(true);
                    }
                }
                "price_change" => {
                    if let Some(data) = value.get("data") {
                        let update: PriceChangeData = serde_json::from_value(data.clone())?;
                        let price: f64 = update.price.parse().unwrap_or(0.0);
                        let side = update.side.and_then(|s| match s.to_uppercase().as_str() {
                            "BUY" => Some(Side::Buy),
                            "SELL" => Some(Side::Sell),
                            _ => None,
                        });

                        let market_data = MarketData {
                            token_id: update.token_id,
                            orderbook: None,
                            last_price: Some(price),
                            last_trade_side: side,
                            timestamp: update.timestamp.unwrap_or(chrono::Utc::now().timestamp()),
                        };

                        let _ = self.event_tx.send(WsEvent::MarketUpdate(market_data)).await;
                        return Ok(true);
                    }
                }
                "last_trade_price" => {
                    if let Some(data) = value.get("data") {
                        let update: LastTradePriceData = serde_json::from_value(data.clone())?;
                        let price: f64 = update.price.parse().unwrap_or(0.0);
                        let side = match update.side.to_uppercase().as_str() {
                            "BUY" => Side::Buy,
                            "SELL" => Side::Sell,
                            _ => Side::Buy,
                        };

                        let market_data = MarketData {
                            token_id: update.token_id.clone(),
                            orderbook: None,
                            last_price: Some(price),
                            last_trade_side: Some(side),
                            timestamp: update.timestamp.unwrap_or(chrono::Utc::now().timestamp()),
                        };

                        let _ = self.event_tx.send(WsEvent::MarketUpdate(market_data)).await;
                        return Ok(true);
                    }
                }
                "heartbeat" => {
                    debug!("Market feed heartbeat");
                    return Ok(false);
                }
                _ => {
                    debug!("Unknown market feed event type: {}", event_type);
                    return Ok(false);
                }
            }
        }

        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn ws_client_book_update_emits_normalized_book() {
        let (event_tx, mut event_rx) = mpsc::channel(4);
        let (_shutdown_tx, shutdown_rx) = mpsc::channel(1);
        let client = WsClient::new("wss://example.com", None, event_tx, shutdown_rx);

        client
            .handle_book_update(BookUpdateData {
                token_id: "token".to_string(),
                bids: vec![
                    ("0.45".to_string(), "4".to_string()),
                    ("0.50".to_string(), "0".to_string()),
                    ("0.55".to_string(), "2".to_string()),
                ],
                asks: vec![
                    ("0.70".to_string(), "1".to_string()),
                    ("0.60".to_string(), "3".to_string()),
                    ("0.61".to_string(), "-4".to_string()),
                ],
                timestamp: 1,
            })
            .await
            .expect("book update should parse");

        let event = event_rx.recv().await.expect("missing ws event");
        let book = match event {
            WsEvent::BookUpdate(book) => book,
            other => panic!("expected BookUpdate, got {:?}", other),
        };

        assert_eq!(book.best_bid().map(|row| row.price), Some(0.55));
        assert_eq!(book.best_ask().map(|row| row.price), Some(0.60));
    }

    #[tokio::test]
    async fn market_feed_book_event_emits_normalized_update() {
        let (event_tx, mut event_rx) = mpsc::channel(4);
        let client = MarketFeedClient::new("wss://example.com", event_tx);

        let msg = serde_json::json!({
            "event_type": "book",
            "data": {
                "token_id": "token",
                "bids": [["0.42", "2"], ["0.58", "1"], ["-1.0", "3"]],
                "asks": [["0.65", "4"], ["0.61", "2"], ["0.60", "0"]],
                "timestamp": 10
            }
        });

        client
            .handle_message(&msg.to_string())
            .await
            .expect("market feed message should parse");

        // First event is MarketUpdate, second one is BookUpdate.
        let _ = event_rx.recv().await.expect("missing market update");
        let event = event_rx.recv().await.expect("missing book update");
        let book = match event {
            WsEvent::BookUpdate(book) => book,
            other => panic!("expected BookUpdate, got {:?}", other),
        };

        assert_eq!(book.best_bid().map(|row| row.price), Some(0.58));
        assert_eq!(book.best_ask().map(|row| row.price), Some(0.61));
    }

    #[test]
    fn market_feed_watchdog_helper_detects_timeout() {
        let now = Instant::now();
        assert!(!should_reconnect_due_to_silence(now, 20));
        let stale = now
            .checked_sub(Duration::from_secs(21))
            .expect("instant subtraction should succeed");
        assert!(should_reconnect_due_to_silence(stale, 20));
    }

    #[test]
    fn market_feed_backoff_is_bounded() {
        let first = backoff_with_jitter_secs(1);
        let later = backoff_with_jitter_secs(20);
        assert!(first >= 1);
        assert!(first <= MARKET_FEED_MAX_BACKOFF_SECS);
        assert!(later >= 1);
        assert!(later <= MARKET_FEED_MAX_BACKOFF_SECS);
    }
}
