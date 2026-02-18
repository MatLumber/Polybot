//! Coinbase Advanced Trade WebSocket client
//!
//! Connects to Coinbase WebSocket API for real-time market data.

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

const COINBASE_WS_URL: &str = "wss://advanced-trade-ws.coinbase.com";

#[derive(Debug, Clone, Serialize)]
struct SubscribeMsg {
    #[serde(rename = "type")]
    msg_type: String,
    product_ids: Vec<String>,
    channel: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CoinbaseMessage {
    channel: String,
    client_id: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "type")]
    msg_type: Option<String>,
    events: Option<Vec<CoinbaseEvent>>,
}

#[derive(Debug, Clone, Deserialize)]
struct CoinbaseEvent {
    #[serde(rename = "type")]
    event_type: String,
    products: Option<Vec<CoinbaseProduct>>,
    updates: Option<Vec<CoinbaseUpdate>>,
}

#[derive(Debug, Clone, Deserialize)]
struct CoinbaseProduct {
    product_id: String,
    #[serde(flatten)]
    data: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
struct CoinbaseUpdate {
    side: String,
    event_time: Option<String>,
    price_level: Option<String>,
    new_quantity: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CoinbaseClient {
    connected: bool,
    subscriptions: Vec<Asset>,
}

impl CoinbaseClient {
    pub fn new() -> Self {
        Self {
            connected: false,
            subscriptions: Vec::new(),
        }
    }
}

#[async_trait]
impl PriceSource for CoinbaseClient {
    fn name(&self) -> &'static str {
        "Coinbase"
    }

    async fn connect(&mut self, tx: Sender<SourceEvent>) -> Result<()> {
        let product_ids: Vec<String> = self
            .subscriptions
            .iter()
            .map(|&a| a.coinbase_pair().to_string())
            .collect();

        if product_ids.is_empty() {
            bail!("No subscriptions configured for Coinbase");
        }

        tracing::info!(
            source = %"Coinbase",
            url = %COINBASE_WS_URL,
            "Connecting to Coinbase WebSocket..."
        );

        let (ws_stream, _) = connect_async(COINBASE_WS_URL)
            .await
            .context("Failed to connect to Coinbase WebSocket")?;

        let (mut write, mut read) = ws_stream.split();

        // Subscribe to market trades and ticker channels
        for channel in &["market_trades", "ticker", "ticker_batch"] {
            let sub_msg = SubscribeMsg {
                msg_type: "subscribe".to_string(),
                product_ids: product_ids.clone(),
                channel: channel.to_string(),
            };
            write
                .send(Message::Text(serde_json::to_string(&sub_msg)?))
                .await?;
        }

        self.connected = true;
        let _ = tx
            .send(SourceEvent::Connected("Coinbase".to_string()))
            .await;

        tracing::info!(source = %"Coinbase", "✅ Connected to Coinbase WebSocket");

        // Handle incoming messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Err(e) = Self::handle_message(&text, &tx).await {
                        tracing::warn!(source = %"Coinbase", error = %e, "Failed to parse message");
                    }
                }
                Ok(Message::Ping(data)) => {
                    let _ = write.send(Message::Pong(data)).await;
                }
                Ok(Message::Close(_)) => {
                    tracing::warn!(source = %"Coinbase", "Connection closed by server");
                    self.connected = false;
                    let _ = tx
                        .send(SourceEvent::Disconnected("Coinbase".to_string()))
                        .await;
                    break;
                }
                Err(e) => {
                    tracing::error!(source = %"Coinbase", error = %e, "WebSocket error");
                    let _ = tx
                        .send(SourceEvent::Error("Coinbase".to_string(), e.to_string()))
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

impl CoinbaseClient {
    async fn handle_message(text: &str, tx: &Sender<SourceEvent>) -> Result<()> {
        let msg: CoinbaseMessage = serde_json::from_str(text)?;

        match msg.channel.as_str() {
            "market_trades" => {
                Self::handle_trades(&msg, tx).await?;
            }
            "ticker" | "ticker_batch" => {
                Self::handle_ticker(&msg, tx).await?;
            }
            "subscriptions" => {
                tracing::debug!(source = %"Coinbase", "Subscription confirmed");
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_trades(msg: &CoinbaseMessage, tx: &Sender<SourceEvent>) -> Result<()> {
        let events = match &msg.events {
            Some(e) => e,
            None => return Ok(()),
        };

        for event in events {
            if event.event_type != "update" {
                continue;
            }

            if let Some(updates) = &event.updates {
                for update in updates {
                    let product_id = update.side.clone();
                    let price = update
                        .price_level
                        .as_ref()
                        .and_then(|p| p.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    let size = update
                        .new_quantity
                        .as_ref()
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);

                    if price > 0.0 {
                        // Parse asset from product_id
                        // Note: Coinbase trades have different format, parse from events
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_ticker(msg: &CoinbaseMessage, tx: &Sender<SourceEvent>) -> Result<()> {
        let events = match &msg.events {
            Some(e) => e,
            None => return Ok(()),
        };

        for event in events {
            if event.event_type != "update" {
                continue;
            }

            if let Some(products) = &event.products {
                for product in products {
                    let product_id = product.product_id.as_str();
                    let asset = parse_coinbase_symbol(product_id)?;

                    // Extract price from the data field
                    let data = &product.data;
                    let bid = data["bid"]
                        .as_str()
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    let ask = data["ask"]
                        .as_str()
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);

                    if bid > 0.0 && ask > 0.0 {
                        let ts = msg
                            .timestamp
                            .as_ref()
                            .and_then(|t| t.parse::<i64>().ok())
                            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

                        let mid = (bid + ask) / 2.0;
                        let tick = NormalizedTick {
                            ts,
                            asset,
                            bid,
                            ask,
                            mid,
                            source: Source::Coinbase,
                            latency_ms: (chrono::Utc::now().timestamp_millis() as u64)
                                .saturating_sub(ts as u64),
                        };

                        let _ = tx.send(SourceEvent::Tick(tick)).await;
                    }
                }
            }
        }

        Ok(())
    }
}

fn parse_coinbase_symbol(symbol: &str) -> Result<Asset> {
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
