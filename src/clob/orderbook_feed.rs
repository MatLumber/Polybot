//! Orderbook Feed Orchestrator
//!
//! Coordinates REST and WebSocket connections to provide real-time orderbook data:
//! - Fetches initial orderbook snapshots via REST
//! - Subscribes to market channel via WebSocket for real-time updates
//! - Integrates with OrderbookImbalanceTracker for microstructure analysis

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, error, info, warn};

use super::rest::RestClient;
use super::types::{OrderBook, Side, Trade};
use super::websocket::{MarketData, MarketFeedClient, WsEvent};
use crate::types::{Asset, Timeframe};

/// Market identifier (asset + timeframe + direction)
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct MarketId {
    pub asset: Asset,
    pub timeframe: Timeframe,
    pub direction: MarketDirection,
}

impl MarketId {
    pub fn new(asset: Asset, timeframe: Timeframe, direction: MarketDirection) -> Self {
        Self {
            asset,
            timeframe,
            direction,
        }
    }
}

/// Market direction (UP or DOWN prediction)
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum MarketDirection {
    Up,
    Down,
}

impl std::fmt::Display for MarketDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketDirection::Up => write!(f, "UP"),
            MarketDirection::Down => write!(f, "DOWN"),
        }
    }
}

/// Market subscription info
#[derive(Debug, Clone)]
pub struct MarketSubscription {
    pub market_id: MarketId,
    pub token_id: String,
    pub condition_id: String,
    pub slug: String,
}

/// Orderbook update callback type
pub type OrderbookCallback = Box<dyn Fn(&MarketId, &OrderBook) + Send + Sync>;

/// Trade callback type
pub type TradeCallback = Box<dyn Fn(&MarketId, &Trade) + Send + Sync>;

/// Orderbook feed configuration
#[derive(Debug, Clone)]
pub struct OrderbookFeedConfig {
    pub ws_url: String,
    pub rest_url: String,
    pub refresh_interval_ms: u64,
    pub max_reconnect_attempts: u32,
}

impl Default for OrderbookFeedConfig {
    fn default() -> Self {
        Self {
            ws_url: "wss://ws-subscriptions-clob.polymarket.com/ws".to_string(),
            rest_url: "https://clob.polymarket.com".to_string(),
            refresh_interval_ms: 5000,
            max_reconnect_attempts: 5,
        }
    }
}

/// Latest orderbook data for a market
#[derive(Debug, Clone)]
pub struct MarketOrderbook {
    pub market_id: MarketId,
    pub orderbook: OrderBook,
    pub last_update: i64,
    pub last_price: Option<f64>,
}

/// Orderbook feed orchestrator
pub struct OrderbookFeed {
    config: OrderbookFeedConfig,
    rest_client: RestClient,
    /// Market subscriptions (token_id -> subscription)
    subscriptions: Arc<RwLock<HashMap<String, MarketSubscription>>>,
    /// Token to market mapping
    token_to_market: Arc<RwLock<HashMap<String, MarketId>>>,
    /// Latest orderbooks by market
    orderbooks: Arc<RwLock<HashMap<MarketId, MarketOrderbook>>>,
    /// Callbacks for orderbook updates
    orderbook_callbacks: Arc<Mutex<Vec<OrderbookCallback>>>,
    /// Callbacks for trade updates
    trade_callbacks: Arc<Mutex<Vec<TradeCallback>>>,
}

impl OrderbookFeed {
    pub fn new(config: OrderbookFeedConfig) -> Self {
        let rest_client = RestClient::new(&config.rest_url, None, None, None, None);

        Self {
            config,
            rest_client,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            token_to_market: Arc::new(RwLock::new(HashMap::new())),
            orderbooks: Arc::new(RwLock::new(HashMap::new())),
            orderbook_callbacks: Arc::new(Mutex::new(Vec::new())),
            trade_callbacks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Subscribe to a market
    pub async fn subscribe(&self, subscription: MarketSubscription) {
        let token_id = subscription.token_id.clone();
        let market_id = subscription.market_id.clone();

        info!("Subscribed to market: {:?}", market_id);

        {
            let mut subs = self.subscriptions.write().await;
            subs.insert(token_id.clone(), subscription);
        }

        {
            let mut mapping = self.token_to_market.write().await;
            mapping.insert(token_id, market_id);
        }
    }

    /// Unsubscribe from a market
    pub async fn unsubscribe(&self, market_id: &MarketId) {
        let token_to_remove = {
            let subs = self.subscriptions.read().await;
            subs.values()
                .find(|s| &s.market_id == market_id)
                .map(|s| s.token_id.clone())
        };

        if let Some(token_id) = token_to_remove {
            let mut subs = self.subscriptions.write().await;
            subs.remove(&token_id);

            let mut mapping = self.token_to_market.write().await;
            mapping.remove(&token_id);

            let mut books = self.orderbooks.write().await;
            books.remove(market_id);

            info!("Unsubscribed from market: {:?}", market_id);
        }
    }

    /// Add callback for orderbook updates
    pub async fn on_orderbook_update(&self, callback: OrderbookCallback) {
        let mut callbacks = self.orderbook_callbacks.lock().await;
        callbacks.push(callback);
    }

    /// Add callback for trade updates
    pub async fn on_trade(&self, callback: TradeCallback) {
        let mut callbacks = self.trade_callbacks.lock().await;
        callbacks.push(callback);
    }

    /// Get latest orderbook for a market
    pub async fn get_orderbook(&self, market_id: &MarketId) -> Option<MarketOrderbook> {
        let books = self.orderbooks.read().await;
        books.get(market_id).cloned()
    }

    /// Get all subscribed token IDs
    pub async fn get_subscribed_tokens(&self) -> Vec<String> {
        let subs = self.subscriptions.read().await;
        subs.keys().cloned().collect()
    }

    /// Fetch initial orderbook snapshots via REST
    async fn fetch_initial_snapshots(&self) -> Result<()> {
        let tokens: Vec<String> = self.get_subscribed_tokens().await;

        for token_id in tokens {
            match self.rest_client.get_order_book(&token_id).await {
                Ok(book) => {
                    let mapping = self.token_to_market.read().await;
                    if let Some(market_id) = mapping.get(&token_id) {
                        let market_ob = MarketOrderbook {
                            market_id: market_id.clone(),
                            orderbook: book.clone(),
                            last_update: chrono::Utc::now().timestamp_millis(),
                            last_price: book.mid_price(),
                        };

                        {
                            let mut books = self.orderbooks.write().await;
                            books.insert(market_id.clone(), market_ob);
                        }

                        self.notify_orderbook_update(market_id, &book).await;
                    }
                }
                Err(e) => {
                    warn!("Failed to fetch orderbook for {}: {}", token_id, e);
                }
            }
        }

        Ok(())
    }

    /// Handle WebSocket event
    async fn handle_ws_event(&self, event: WsEvent) {
        match event {
            WsEvent::BookUpdate(book) => {
                self.handle_book_update(book).await;
            }
            WsEvent::MarketUpdate(data) => {
                self.handle_market_update(data).await;
            }
            WsEvent::Trade(trade) => {
                self.handle_trade(trade).await;
            }
            WsEvent::Connected => {
                info!("Orderbook feed WebSocket connected");
            }
            WsEvent::Disconnected => {
                warn!("Orderbook feed WebSocket disconnected");
            }
            WsEvent::Error(e) => {
                error!("Orderbook feed WebSocket error: {}", e);
            }
            _ => {}
        }
    }

    async fn handle_book_update(&self, book: OrderBook) {
        let mapping = self.token_to_market.read().await;
        if let Some(market_id) = mapping.get(&book.token_id) {
            let market_ob = MarketOrderbook {
                market_id: market_id.clone(),
                orderbook: book.clone(),
                last_update: chrono::Utc::now().timestamp_millis(),
                last_price: book.mid_price(),
            };

            {
                let mut books = self.orderbooks.write().await;
                books.insert(market_id.clone(), market_ob);
            }

            self.notify_orderbook_update(market_id, &book).await;
        }
    }

    async fn handle_market_update(&self, data: MarketData) {
        let mapping = self.token_to_market.read().await;
        if let Some(market_id) = mapping.get(&data.token_id) {
            if let Some(book) = &data.orderbook {
                let market_ob = MarketOrderbook {
                    market_id: market_id.clone(),
                    orderbook: book.clone(),
                    last_update: data.timestamp,
                    last_price: data.last_price.or_else(|| book.mid_price()),
                };

                {
                    let mut books = self.orderbooks.write().await;
                    books.insert(market_id.clone(), market_ob);
                }

                self.notify_orderbook_update(market_id, book).await;
            } else if let Some(price) = data.last_price {
                let mut books = self.orderbooks.write().await;
                if let Some(existing) = books.get_mut(market_id) {
                    existing.last_price = Some(price);
                    existing.last_update = data.timestamp;
                }
            }
        }
    }

    async fn handle_trade(&self, trade: Trade) {
        let mapping = self.token_to_market.read().await;
        if let Some(market_id) = mapping.get(&trade.token_id) {
            self.notify_trade(market_id, &trade).await;
        }
    }

    async fn notify_orderbook_update(&self, market_id: &MarketId, book: &OrderBook) {
        let callbacks = self.orderbook_callbacks.lock().await;
        for callback in callbacks.iter() {
            callback(market_id, book);
        }
    }

    async fn notify_trade(&self, market_id: &MarketId, trade: &Trade) {
        let callbacks = self.trade_callbacks.lock().await;
        for callback in callbacks.iter() {
            callback(market_id, trade);
        }
    }

    /// Run the orderbook feed
    pub async fn run(
        self,
        initial_tokens: Vec<String>,
        mut subscribe_rx: mpsc::Receiver<Vec<String>>,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) -> Result<()> {
        let (event_tx, mut event_rx) = mpsc::channel::<WsEvent>(100);

        for token in &initial_tokens {
            let mapping = self.token_to_market.read().await;
            if !mapping.contains_key(token) {
                debug!("Initial token {} not in subscription mapping", token);
            }
        }

        self.fetch_initial_snapshots().await?;

        let feed_client = MarketFeedClient::new(&self.config.ws_url, event_tx);

        let feed_handle = {
            let tokens = initial_tokens.clone();
            tokio::spawn(async move { feed_client.run(tokens, subscribe_rx, shutdown_rx).await })
        };

        loop {
            tokio::select! {
                event = event_rx.recv() => {
                    match event {
                        Some(e) => {
                            self.handle_ws_event(e).await;
                        }
                        None => {
                            warn!("Event channel closed");
                            break;
                        }
                    }
                }

                _ = tokio::time::sleep(tokio::time::Duration::from_millis(self.config.refresh_interval_ms)) => {
                    if let Err(e) = self.fetch_initial_snapshots().await {
                        warn!("Failed to refresh orderbooks: {}", e);
                    }
                }
            }
        }

        let _ = feed_handle.await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_id_creation() {
        let id = MarketId::new(Asset::BTC, Timeframe::Min15, MarketDirection::Up);
        assert_eq!(id.asset, Asset::BTC);
        assert_eq!(id.timeframe, Timeframe::Min15);
        assert_eq!(id.direction, MarketDirection::Up);
    }

    #[test]
    fn test_market_id_equality() {
        let id1 = MarketId::new(Asset::BTC, Timeframe::Min15, MarketDirection::Up);
        let id2 = MarketId::new(Asset::BTC, Timeframe::Min15, MarketDirection::Up);
        let id3 = MarketId::new(Asset::ETH, Timeframe::Min15, MarketDirection::Up);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_orderbook_feed_config_default() {
        let config = OrderbookFeedConfig::default();
        assert!(config.ws_url.contains("polymarket.com"));
        assert!(config.rest_url.contains("polymarket.com"));
    }
}
