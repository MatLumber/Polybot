//! Price source implementations (Binance, Bybit, Coinbase, RTDS)

mod binance;
mod bybit;
mod coinbase;
mod rtds;

pub use binance::BinanceClient;
pub use bybit::BybitClient;
pub use coinbase::CoinbaseClient;
pub use rtds::RtdsClient;

use crate::oracle::{NormalizedTick, QuoteData, TradeData};
use crate::types::Asset;
use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

/// Trait for price source clients
#[async_trait]
pub trait PriceSource: Send + Sync {
    /// Get the source name
    fn name(&self) -> &'static str;

    /// Connect to the source and start streaming
    async fn connect(&mut self, tx: Sender<SourceEvent>) -> Result<()>;

    /// Subscribe to assets
    async fn subscribe(&mut self, assets: &[Asset]) -> Result<()>;

    /// Disconnect from the source
    async fn disconnect(&mut self) -> Result<()>;

    /// Check if connected
    fn is_connected(&self) -> bool;
}

/// Events from price sources
#[derive(Debug, Clone)]
pub enum SourceEvent {
    /// New trade received
    Trade(TradeData),
    /// New quote received
    Quote(QuoteData),
    /// Normalized tick ready
    Tick(NormalizedTick),
    /// RTDS comments stream event (optional sentiment context).
    Comment(CommentEvent),
    /// Connection status changed
    Connected(String),
    Disconnected(String),
    /// Error occurred
    Error(String, String),
}

#[derive(Debug, Clone)]
pub struct CommentEvent {
    pub topic: String,
    pub symbol: Option<String>,
    pub body: Option<String>,
    pub username: Option<String>,
    pub timestamp: i64,
}
