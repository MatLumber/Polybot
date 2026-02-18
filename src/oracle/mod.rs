//! Oracle module - Multi-source price aggregation
//!
//! Collects real-time prices from Binance, Bybit, Coinbase, and RTDS
//! and produces a unified, confidence-weighted price feed.

mod aggregator;
mod candles;
pub mod sources;

pub use aggregator::{AggregatedPrice, PriceAggregator};
pub use candles::CandleBuilder;

use crate::types::*;

/// Normalized tick from any exchange
#[derive(Debug, Clone)]
pub struct NormalizedTick {
    pub ts: i64,
    pub asset: Asset,
    pub bid: f64,
    pub ask: f64,
    pub mid: f64,
    pub source: PriceSource,
    pub latency_ms: u64,
}

/// Trade data from exchange
#[derive(Debug, Clone)]
pub struct TradeData {
    pub ts: i64,
    pub asset: Asset,
    pub price: f64,
    pub size: f64,
    pub source: PriceSource,
}

/// Quote data from exchange (top of book)
#[derive(Debug, Clone)]
pub struct QuoteData {
    pub ts: i64,
    pub asset: Asset,
    pub bid: f64,
    pub bid_size: f64,
    pub ask: f64,
    pub ask_size: f64,
    pub source: PriceSource,
}
