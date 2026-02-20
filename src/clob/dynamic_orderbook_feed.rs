//! Dynamic Orderbook Feed with Market Discovery
//!
//! Automatically discovers and tracks active Polymarket prediction markets
//! for BTC/ETH 15m/1h up/down predictions, reconnecting when markets rollover.

use crate::clob::market_discovery::{DiscoveredMarket, MarketChange, MarketDiscovery};
use crate::clob::websocket::{MarketFeedClient, WsEvent};
use crate::features::OrderbookImbalanceTracker;
use crate::paper_trading::PolymarketSharePrices;
use crate::types::{Asset, Direction, Timeframe};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Dynamic orderbook feed that handles market rollovers
pub struct DynamicOrderbookFeed {
    market_discovery: MarketDiscovery,
    tracker: Arc<Mutex<OrderbookImbalanceTracker>>,
    share_prices: Arc<PolymarketSharePrices>,
    ws_url: String,
    current_token_map: HashMap<String, (Asset, Timeframe, Direction)>,
    current_token_ids: Vec<String>,
}

impl DynamicOrderbookFeed {
    pub fn new(
        gamma_url: impl Into<String>,
        tracker: Arc<Mutex<OrderbookImbalanceTracker>>,
        share_prices: Arc<PolymarketSharePrices>,
    ) -> Self {
        Self {
            market_discovery: MarketDiscovery::new(gamma_url),
            tracker,
            share_prices,
            ws_url: "wss://ws-subscriptions-clob.polymarket.com/ws".to_string(),
            current_token_map: HashMap::new(),
            current_token_ids: Vec::new(),
        }
    }

    /// Run the dynamic feed
    pub async fn run(mut self) {
        info!("🔄 Starting dynamic orderbook feed with market discovery...");

        // Initial market discovery
        if let Err(e) = self.discover_and_connect().await {
            error!(error = %e, "Initial market discovery failed");
            // Continue anyway - will retry
        }

        let mut reconnect_delay = tokio::time::Duration::from_secs(5);
        let max_reconnect_delay = tokio::time::Duration::from_secs(60);

        loop {
            // Check if we need to refresh markets
            if self.market_discovery.needs_refresh() {
                match self.check_market_changes().await {
                    Ok(true) => {
                        info!("🔄 Markets changed, reconnecting WebSocket...");
                        if let Err(e) = self.reconnect().await {
                            error!(error = %e, "Failed to reconnect after market change");
                        }
                        continue;
                    }
                    Ok(false) => {
                        // No changes
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to check market changes");
                    }
                }
            }

            // Check if any market is about to expire
            if let Some(time_to_expiry) = self.market_discovery.time_to_next_expiration() {
                let minutes_to_expiry = time_to_expiry.num_minutes();
                if minutes_to_expiry <= 2 && minutes_to_expiry > 0 {
                    info!(
                        minutes = minutes_to_expiry,
                        "⏰ Market expiring soon, will refresh..."
                    );
                }
            }

            // Sleep before next check
            let sleep_duration = tokio::time::Duration::from_secs(
                self.market_discovery.seconds_until_refresh().max(5) as u64,
            );
            tokio::time::sleep(sleep_duration).await;
        }
    }

    /// Discover markets and connect WebSocket
    async fn discover_and_connect(&mut self) -> anyhow::Result<()> {
        info!("🔍 Discovering active markets...");

        let changes = self.market_discovery.refresh_markets().await?;

        let tracked = self.market_discovery.get_tracked_markets();
        info!(count = tracked.len(), "📊 Markets discovered");

        for ((asset, timeframe), market) in tracked {
            info!(
                asset = ?asset,
                timeframe = ?timeframe,
                slug = %market.slug,
                end_date = %market.end_date,
                "🎯 Tracking market"
            );
        }

        // Update token mapping
        self.current_token_map = self.market_discovery.build_token_map();
        self.current_token_ids = self.market_discovery.get_all_token_ids();

        if self.current_token_ids.is_empty() {
            warn!("No markets found, will retry later");
            return Ok(());
        }

        // Connect WebSocket
        self.connect_websocket().await?;

        Ok(())
    }

    /// Check for market changes
    async fn check_market_changes(&mut self) -> anyhow::Result<bool> {
        let changes = self.market_discovery.refresh_markets().await?;

        let needs_reconnect = changes.iter().any(|c| c.requires_reconnect());

        for change in &changes {
            match change {
                MarketChange::Rollover {
                    asset,
                    timeframe,
                    old_market,
                    new_market,
                } => {
                    info!(
                        asset = ?asset,
                        timeframe = ?timeframe,
                        old = %old_market.slug,
                        new = %new_market.slug,
                        "🔄 Market rollover"
                    );
                }
                MarketChange::Expired {
                    asset,
                    timeframe,
                    market,
                } => {
                    warn!(
                        asset = ?asset,
                        timeframe = ?timeframe,
                        slug = %market.slug,
                        "⚠️ Market expired"
                    );
                }
                _ => {}
            }
        }

        if needs_reconnect {
            self.current_token_map = self.market_discovery.build_token_map();
            self.current_token_ids = self.market_discovery.get_all_token_ids();
        }

        Ok(needs_reconnect)
    }

    /// Reconnect WebSocket with new markets
    async fn reconnect(&mut self) -> anyhow::Result<()> {
        info!("🔄 Reconnecting WebSocket with updated markets...");
        self.connect_websocket().await
    }

    /// Connect WebSocket
    async fn connect_websocket(&mut self) -> anyhow::Result<()> {
        if self.current_token_ids.is_empty() {
            warn!("No token IDs to subscribe to");
            return Ok(());
        }

        info!(
            count = self.current_token_ids.len(),
            "📡 Connecting to Polymarket WebSocket"
        );

        // Clone data for the WebSocket task
        let token_ids = self.current_token_ids.clone();
        let token_map = self.current_token_map.clone();
        let tracker = self.tracker.clone();
        let share_prices = self.share_prices.clone();
        let ws_url = self.ws_url.clone();

        // Spawn WebSocket connection in a separate task
        tokio::spawn(async move {
            if let Err(e) =
                run_websocket_connection(ws_url, token_ids, token_map, tracker, share_prices).await
            {
                error!(error = %e, "WebSocket connection error");
            }
        });

        Ok(())
    }
}

/// Run a single WebSocket connection
async fn run_websocket_connection(
    ws_url: String,
    token_ids: Vec<String>,
    token_map: HashMap<String, (Asset, Timeframe, Direction)>,
    tracker: Arc<Mutex<OrderbookImbalanceTracker>>,
    share_prices: Arc<PolymarketSharePrices>,
) -> anyhow::Result<()> {
    let (event_tx, mut event_rx) = mpsc::channel::<WsEvent>(100);
    let (subscribe_tx, subscribe_rx) = mpsc::channel::<Vec<String>>(1);
    let (_shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);

    // Send subscription immediately
    subscribe_tx.send(token_ids.clone()).await?;

    // Spawn WebSocket client
    let mut client = MarketFeedClient::new(&ws_url, event_tx.clone());
    tokio::spawn(async move {
        if let Err(e) = client.run(token_ids, subscribe_rx, shutdown_rx).await {
            error!(error = %e, "Market feed client error");
        }
    });

    info!("📊 WebSocket connected, processing events...");

    // Process events
    while let Some(event) = event_rx.recv().await {
        match event {
            WsEvent::BookUpdate(book) => {
                let midpoint = book.mid_price().unwrap_or(0.0);
                let spread = book.spread().unwrap_or(0.0);
                let spread_bps = if midpoint > 0.0 {
                    spread / midpoint * 10000.0
                } else {
                    0.0
                };

                // Update orderbook tracker
                if let Ok(mut t) = tracker.lock() {
                    if let Some(&(asset, tf, direction)) = token_map.get(&book.token_id) {
                        t.update_orderbook(&book, asset, tf);

                        // Update share prices
                        if midpoint > 0.0 {
                            let bid = book.best_bid().map(|b| b.price).unwrap_or(midpoint);
                            let ask = book.best_ask().map(|a| a.price).unwrap_or(midpoint);
                            let bid_size = book.best_bid().map(|b| b.size).unwrap_or(0.0);
                            let ask_size = book.best_ask().map(|a| a.size).unwrap_or(0.0);
                            let depth_top5 = book.bids.iter().take(5).map(|b| b.size).sum::<f64>()
                                + book.asks.iter().take(5).map(|a| a.size).sum::<f64>();

                            let direction_str = match direction {
                                Direction::Up => "UP",
                                Direction::Down => "DOWN",
                            };

                            share_prices.update_quote_with_depth(
                                asset,
                                tf,
                                direction_str,
                                bid,
                                ask,
                                midpoint,
                                bid_size,
                                ask_size,
                                depth_top5,
                            );

                            tracing::debug!(
                                token_id = %book.token_id,
                                asset = ?asset,
                                timeframe = ?tf,
                                bid = bid,
                                ask = ask,
                                mid = midpoint,
                                spread_bps = spread_bps,
                                "📊 Orderbook update"
                            );
                        }
                    }
                }
            }
            WsEvent::MarketUpdate(data) => {
                if let Some(book) = &data.orderbook {
                    let midpoint = book.mid_price().unwrap_or(0.0);
                    if let Ok(mut t) = tracker.lock() {
                        if let Some(&(asset, tf, direction)) = token_map.get(&data.token_id) {
                            t.update_orderbook(book, asset, tf);

                            if midpoint > 0.0 {
                                let bid = book.best_bid().map(|b| b.price).unwrap_or(midpoint);
                                let ask = book.best_ask().map(|a| a.price).unwrap_or(midpoint);
                                let bid_size = book.best_bid().map(|b| b.size).unwrap_or(0.0);
                                let ask_size = book.best_ask().map(|a| a.size).unwrap_or(0.0);
                                let depth_top5 =
                                    book.bids.iter().take(5).map(|b| b.size).sum::<f64>()
                                        + book.asks.iter().take(5).map(|a| a.size).sum::<f64>();

                                let direction_str = match direction {
                                    Direction::Up => "UP",
                                    Direction::Down => "DOWN",
                                };

                                share_prices.update_quote_with_depth(
                                    asset,
                                    tf,
                                    direction_str,
                                    bid,
                                    ask,
                                    midpoint,
                                    bid_size,
                                    ask_size,
                                    depth_top5,
                                );
                            }
                        }
                    }
                }
            }
            WsEvent::Error(e) => {
                error!(error = %e, "WebSocket error");
            }
            WsEvent::Disconnected => {
                info!("WebSocket disconnected");
                break;
            }
            WsEvent::Connected => {
                info!("WebSocket connected");
            }
            _ => {
                // Handle other events (OrderUpdate, Trade, etc.)
            }
        }
    }

    Ok(())
}
