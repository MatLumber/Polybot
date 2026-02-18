//! Bybit WebSocket client for real-time price data
//!
//! Connects to Bybit V5 public spot streams for trades and order book updates.

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::oracle::sources::{PriceSource, SourceEvent};
use crate::oracle::{NormalizedTick, QuoteData, TradeData};
use crate::types::{Asset, PriceSource as Source};

const BYBIT_WS_URL: &str = "wss://stream.bybit.com/v5/public/spot";

#[derive(Debug, Clone, Serialize)]
struct SubscribeMsg {
    req_id: Option<String>,
    op: String,
    args: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct BybitMessage {
    topic: Option<String>,
    #[serde(rename = "type")]
    msg_type: Option<String>,
    data: Option<serde_json::Value>,
    ts: Option<i64>,
    success: Option<bool>,
    op: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct BybitTrade {
    #[serde(rename = "T")]
    ts: i64,
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "p")]
    price: String,
    #[serde(rename = "v")]
    size: String,
    #[serde(rename = "S")]
    side: String,
}

#[derive(Debug, Clone, Deserialize)]
struct BybitOrderbook {
    #[serde(rename = "T")]
    ts: i64,
    #[serde(rename = "s")]
    symbol: String,
    b: Vec<Vec<String>>, // [[price, size], ...]
    a: Vec<Vec<String>>, // [[price, size], ...]
}

#[derive(Debug, Clone)]
pub struct BybitClient {
    connected: bool,
    subscriptions: Vec<Asset>,
}

impl BybitClient {
    pub fn new() -> Self {
        Self {
            connected: false,
            subscriptions: Vec::new(),
        }
    }

    fn build_topic(asset: Asset) -> (String, String) {
        let symbol = format!("{}USDT", asset);
        let trade_topic = format!("publicTrade.{}", symbol);
        let book_topic = format!("orderbook.50.{}", symbol);
        (trade_topic, book_topic)
    }
}

#[async_trait]
impl PriceSource for BybitClient {
    fn name(&self) -> &'static str {
        "Bybit"
    }

    async fn connect(&mut self, tx: Sender<SourceEvent>) -> Result<()> {
        let topics: Vec<String> = self
            .subscriptions
            .iter()
            .flat_map(|&a| {
                let (trade, book) = Self::build_topic(a);
                vec![trade, book]
            })
            .collect();

        if topics.is_empty() {
            bail!("No subscriptions configured for Bybit");
        }

        tracing::info!(
            source = %"Bybit",
            url = %BYBIT_WS_URL,
            "Connecting to Bybit WebSocket..."
        );

        let (ws_stream, _) = connect_async(BYBIT_WS_URL)
            .await
            .context("Failed to connect to Bybit WebSocket")?;

        let (mut write, mut read) = ws_stream.split();

        // Subscribe to topics
        let sub_msg = SubscribeMsg {
            req_id: Some("sub_1".to_string()),
            op: "subscribe".to_string(),
            args: topics,
        };
        write
            .send(Message::Text(serde_json::to_string(&sub_msg)?))
            .await?;

        self.connected = true;
        let _ = tx.send(SourceEvent::Connected("Bybit".to_string())).await;

        tracing::info!(source = %"Bybit", "✅ Connected to Bybit WebSocket");

        // Handle incoming messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Err(e) = Self::handle_message(&text, &tx).await {
                        tracing::warn!(source = %"Bybit", error = %e, "Failed to parse message");
                    }
                }
                Ok(Message::Ping(data)) => {
                    // Bybit expects pong with same payload
                    let _ = write.send(Message::Pong(data)).await;
                }
                Ok(Message::Pong(_)) => {}
                Ok(Message::Close(_)) => {
                    tracing::warn!(source = %"Bybit", "Connection closed by server");
                    self.connected = false;
                    let _ = tx
                        .send(SourceEvent::Disconnected("Bybit".to_string()))
                        .await;
                    break;
                }
                Err(e) => {
                    tracing::error!(source = %"Bybit", error = %e, "WebSocket error");
                    let _ = tx
                        .send(SourceEvent::Error("Bybit".to_string(), e.to_string()))
                        .await;
                    break;
                }
                _ => {}
            }
        }

        self.connected = false;
        Ok(())
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

impl BybitClient {
    async fn handle_message(text: &str, tx: &Sender<SourceEvent>) -> Result<()> {
        let msg: BybitMessage = serde_json::from_str(text)?;

        // Handle pong response
        if msg.op.as_deref() == Some("pong") {
            return Ok(());
        }

        // Handle subscription confirmation
        if msg.success.is_some() {
            tracing::debug!(source = %"Bybit", success = ?msg.success, "Subscription response");
            return Ok(());
        }

        let topic = match msg.topic {
            Some(t) => t,
            None => return Ok(()), // Ignore messages without topic
        };

        let data = match msg.data {
            Some(d) => d,
            None => return Ok(()),
        };

        if topic.starts_with("publicTrade.") {
            Self::handle_trade(&topic, &data, tx).await?;
        } else if topic.starts_with("orderbook.") {
            Self::handle_orderbook(&topic, &data, tx).await?;
        }

        Ok(())
    }

    async fn handle_trade(
        topic: &str,
        data: &serde_json::Value,
        tx: &Sender<SourceEvent>,
    ) -> Result<()> {
        let trades: Vec<BybitTrade> = serde_json::from_value(data.clone())?;

        for trade in trades {
            let asset = parse_bybit_symbol(&trade.symbol)?;

            let trade_data = TradeData {
                ts: trade.ts,
                asset,
                price: trade.price.parse()?,
                size: trade.size.parse()?,
                source: Source::Bybit,
            };

            let tick = NormalizedTick {
                ts: trade_data.ts,
                asset: trade_data.asset,
                bid: trade_data.price,
                ask: trade_data.price,
                mid: trade_data.price,
                source: Source::Bybit,
                latency_ms: (chrono::Utc::now().timestamp_millis() as u64)
                    .saturating_sub(trade_data.ts as u64),
            };

            let _ = tx.send(SourceEvent::Trade(trade_data)).await;
            let _ = tx.send(SourceEvent::Tick(tick)).await;
        }

        Ok(())
    }

    async fn handle_orderbook(
        topic: &str,
        data: &serde_json::Value,
        tx: &Sender<SourceEvent>,
    ) -> Result<()> {
        let ob: BybitOrderbook = serde_json::from_value(data.clone())?;
        let asset = parse_bybit_symbol(&ob.symbol)?;

        // Get best bid/ask
        let best_bid =
            ob.b.first()
                .and_then(|b| b.first())
                .and_then(|p| p.parse::<f64>().ok())
                .unwrap_or(0.0);

        let best_ask =
            ob.a.first()
                .and_then(|a| a.first())
                .and_then(|p| p.parse::<f64>().ok())
                .unwrap_or(0.0);

        if best_bid > 0.0 && best_ask > 0.0 {
            let mid = (best_bid + best_ask) / 2.0;
            let tick = NormalizedTick {
                ts: ob.ts,
                asset,
                bid: best_bid,
                ask: best_ask,
                mid,
                source: Source::Bybit,
                latency_ms: (chrono::Utc::now().timestamp_millis() as u64)
                    .saturating_sub(ob.ts as u64),
            };

            let _ = tx.send(SourceEvent::Tick(tick)).await;
        }

        Ok(())
    }
}

fn parse_bybit_symbol(symbol: &str) -> Result<Asset> {
    let symbol = symbol.to_uppercase();
    if symbol.starts_with("BTC") {
        Ok(Asset::BTC)
    } else if symbol.starts_with("ETH") {
        Ok(Asset::ETH)
    } else if symbol.starts_with("SOL") {
        Ok(Asset::SOL)
    } else if symbol.starts_with("XRP") {
        Ok(Asset::XRP)
    } else {
        bail!("Unknown symbol: {}", symbol)
    }
}
