//! Binance WebSocket client for real-time price data
//!
//! Connects to Binance spot market streams for trades and book ticker updates.

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::oracle::sources::{PriceSource, SourceEvent};
use crate::oracle::{NormalizedTick, QuoteData, TradeData};
use crate::types::{Asset, Candle, PriceSource as Source, Timeframe};

const BINANCE_WS_URL: &str = "wss://stream.binance.com:9443/stream";
const BINANCE_REST_URL: &str = "https://api.binance.com/api/v3/klines";

#[derive(Debug, Clone)]
pub struct BinanceClient {
    connected: bool,
    subscriptions: Vec<Asset>,
    last_ping: Instant,
}

impl BinanceClient {
    pub fn new() -> Self {
        Self {
            connected: false,
            subscriptions: Vec::new(),
            last_ping: Instant::now(),
        }
    }

    fn build_stream_name(asset: Asset) -> String {
        let pair = asset.trading_pair().to_lowercase();
        format!("{}@aggTrade/{}@bookTicker", pair, pair)
    }

    fn build_stream_url(streams: &[String]) -> String {
        format!("{}?streams={}", BINANCE_WS_URL, streams.join("/"))
    }

    /// Fetch historical klines from Binance REST API
    pub async fn fetch_historical_candles(
        asset: Asset,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<Candle>> {
        let symbol = asset.trading_pair();
        let interval = match timeframe {
            Timeframe::Min15 => "15m",
            Timeframe::Hour1 => "1h",
        };

        let url = format!(
            "{}?symbol={}&interval={}&limit={}",
            BINANCE_REST_URL, symbol, interval, limit
        );

        tracing::info!(
            asset = ?asset,
            timeframe = ?timeframe,
            url = %url.split('?').next().unwrap_or(url.as_str()),
            "📥 Fetching historical candles from Binance..."
        );

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        let response = client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch historical candles from Binance")?;

        if !response.status().is_success() {
            bail!("Binance API returned error: {}", response.status());
        }

        // Parse response: array of arrays
        // [[open_time, open, high, low, close, volume, close_time, ...], ...]
        let klines: Vec<Vec<serde_json::Value>> = response
            .json()
            .await
            .context("Failed to parse Binance klines response")?;

        let candles: Vec<Candle> = klines
            .into_iter()
            .filter_map(|kline| {
                if kline.len() < 7 {
                    return None;
                }

                let open_time = kline[0].as_i64()?;
                let open: f64 = kline[1].as_str()?.parse().ok()?;
                let high: f64 = kline[2].as_str()?.parse().ok()?;
                let low: f64 = kline[3].as_str()?.parse().ok()?;
                let close: f64 = kline[4].as_str()?.parse().ok()?;
                let volume: f64 = kline[5].as_str()?.parse().ok()?;
                let close_time = kline[6].as_i64()?;

                Some(Candle {
                    open_time,
                    close_time,
                    asset,
                    timeframe,
                    open,
                    high,
                    low,
                    close,
                    volume,
                    trades: 0, // Not provided by this endpoint
                })
            })
            .collect();

        tracing::info!(
            asset = ?asset,
            timeframe = ?timeframe,
            count = candles.len(),
            "✅ Historical candles fetched"
        );

        Ok(candles)
    }
}

#[async_trait]
impl PriceSource for BinanceClient {
    fn name(&self) -> &'static str {
        "Binance"
    }

    async fn connect(&mut self, tx: Sender<SourceEvent>) -> Result<()> {
        let streams: Vec<String> = self
            .subscriptions
            .iter()
            .map(|&a| Self::build_stream_name(a))
            .collect();

        if streams.is_empty() {
            bail!("No subscriptions configured for Binance");
        }

        let url = Self::build_stream_url(&streams);
        let mut reconnect_attempts = 0u32;
        let max_reconnect_attempts = 10u32;
        let base_delay = Duration::from_secs(1);
        let max_delay = Duration::from_secs(60);

        'reconnect_loop: loop {
            tracing::info!(
                source = %"Binance",
                url = %url.split('?').next().unwrap_or(url.as_str()),
                attempt = reconnect_attempts,
                "Connecting to Binance WebSocket..."
            );

            let (ws_stream, _) = match connect_async(&url).await {
                Ok(stream) => stream,
                Err(e) => {
                    tracing::error!(source = %"Binance", error = %e, "Connection failed");
                    let _ = tx
                        .send(SourceEvent::Error("Binance".to_string(), e.to_string()))
                        .await;

                    if reconnect_attempts >= max_reconnect_attempts {
                        bail!(
                            "Max reconnection attempts ({}) reached",
                            max_reconnect_attempts
                        );
                    }

                    reconnect_attempts += 1;
                    let delay = std::cmp::min(base_delay * reconnect_attempts, max_delay);
                    tracing::info!(
                        source = %"Binance",
                        delay_secs = delay.as_secs(),
                        "Reconnecting in {} seconds...", delay.as_secs()
                    );
                    tokio::time::sleep(delay).await;
                    continue 'reconnect_loop;
                }
            };

            let (mut write, mut read) = ws_stream.split();
            self.connected = true;
            reconnect_attempts = 0; // Reset on successful connection

            // Send connected event
            let _ = tx.send(SourceEvent::Connected("Binance".to_string())).await;
            tracing::info!(source = %"Binance", "✅ Connected to Binance WebSocket");

            // Spawn ping task
            let ping_tx = tx.clone();
            let ping_handle = tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(20));
                loop {
                    interval.tick().await;
                    // Binance doesn't require explicit ping, but we track it
                }
            });

            // Handle incoming messages
            let should_reconnect = loop {
                match read.next().await {
                    Some(Ok(Message::Text(text))) => {
                        if let Err(e) = Self::handle_message(&text, &tx).await {
                            tracing::warn!(source = %"Binance", error = %e, "Failed to parse message");
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = write.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) => {
                        tracing::warn!(source = %"Binance", "Connection closed by server");
                        break true;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        // Pong received, connection is alive
                    }
                    Some(Err(e)) => {
                        tracing::error!(source = %"Binance", error = %e, "WebSocket error");
                        let _ = tx
                            .send(SourceEvent::Error("Binance".to_string(), e.to_string()))
                            .await;
                        break true;
                    }
                    None => {
                        tracing::warn!(source = %"Binance", "Stream ended");
                        break true;
                    }
                    _ => {}
                }
            };

            ping_handle.abort();
            self.connected = false;
            let _ = tx
                .send(SourceEvent::Disconnected("Binance".to_string()))
                .await;

            if should_reconnect {
                reconnect_attempts += 1;
                if reconnect_attempts > max_reconnect_attempts {
                    bail!(
                        "Max reconnection attempts ({}) reached",
                        max_reconnect_attempts
                    );
                }
                let delay = std::cmp::min(base_delay * reconnect_attempts, max_delay);
                tracing::info!(
                    source = %"Binance",
                    delay_secs = delay.as_secs(),
                    attempt = reconnect_attempts,
                    "🔄 Reconnecting in {} seconds...", delay.as_secs()
                );
                tokio::time::sleep(delay).await;
            } else {
                break 'reconnect_loop;
            }
        }

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

impl BinanceClient {
    async fn handle_message(text: &str, tx: &Sender<SourceEvent>) -> Result<()> {
        // Combined stream messages have format: {"stream":"btcusdt@aggTrade","data":{...}}
        let wrapper: serde_json::Value = serde_json::from_str(text)?;

        let stream = wrapper["stream"]
            .as_str()
            .context("Missing stream name in message")?;
        let data = &wrapper["data"];

        if stream.contains("@aggTrade") {
            Self::handle_trade(data, tx).await?;
        } else if stream.contains("@bookTicker") {
            Self::handle_quote(data, tx).await?;
        }

        Ok(())
    }

    async fn handle_trade(data: &serde_json::Value, tx: &Sender<SourceEvent>) -> Result<()> {
        // Extract symbol and parse asset
        let symbol = data["s"].as_str().context("Missing symbol")?;
        let asset = parse_binance_symbol(symbol)?;

        let trade = TradeData {
            ts: data["T"].as_u64().unwrap_or(0) as i64,
            asset,
            price: data["p"].as_str().context("Missing price")?.parse()?,
            size: data["q"].as_str().context("Missing quantity")?.parse()?,
            source: Source::Binance,
        };

        // Also emit as normalized tick
        let tick = NormalizedTick {
            ts: trade.ts,
            asset: trade.asset,
            bid: trade.price, // Use trade price as approximation
            ask: trade.price,
            mid: trade.price,
            source: Source::Binance,
            latency_ms: (chrono::Utc::now().timestamp_millis() as u64)
                .saturating_sub(trade.ts as u64),
        };

        let _ = tx.send(SourceEvent::Trade(trade)).await;
        let _ = tx.send(SourceEvent::Tick(tick)).await;

        Ok(())
    }

    async fn handle_quote(data: &serde_json::Value, tx: &Sender<SourceEvent>) -> Result<()> {
        let symbol = data["s"].as_str().context("Missing symbol")?;
        let asset = parse_binance_symbol(symbol)?;

        let ts = data["T"]
            .as_u64()
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis() as u64)
            as i64;

        let quote = QuoteData {
            ts,
            asset,
            bid: data["b"].as_str().context("Missing bid")?.parse()?,
            bid_size: data["B"].as_str().context("Missing bid size")?.parse()?,
            ask: data["a"].as_str().context("Missing ask")?.parse()?,
            ask_size: data["A"].as_str().context("Missing ask size")?.parse()?,
            source: Source::Binance,
        };

        // Emit as normalized tick
        let mid = (quote.bid + quote.ask) / 2.0;
        let tick = NormalizedTick {
            ts: quote.ts,
            asset: quote.asset,
            bid: quote.bid,
            ask: quote.ask,
            mid,
            source: Source::Binance,
            latency_ms: (chrono::Utc::now().timestamp_millis() as u64)
                .saturating_sub(quote.ts as u64),
        };

        let _ = tx.send(SourceEvent::Quote(quote)).await;
        let _ = tx.send(SourceEvent::Tick(tick)).await;

        Ok(())
    }
}

fn parse_binance_symbol(symbol: &str) -> Result<Asset> {
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
