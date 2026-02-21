//! CLOB Types - Data structures for Polymarket CLOB API

use ethers::types::{Address, H256, U256};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Order side (buy/sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Buy,
    Sell,
}

impl Default for Side {
    fn default() -> Self {
        Side::Buy
    }
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Side::Buy => write!(f, "BUY"),
            Side::Sell => write!(f, "SELL"),
        }
    }
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Expired,
}

impl Default for OrderStatus {
    fn default() -> Self {
        OrderStatus::Pending
    }
}

/// Polymarket order
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Order {
    /// Unique order ID
    #[serde(default)]
    pub id: H256,
    /// Token ID being traded
    pub token_id: String,
    /// Order side (BUY/SELL)
    pub side: Side,
    /// Price (0.0 to 1.0)
    pub price: f64,
    /// Size in USDC
    pub size: f64,
    /// Current status
    #[serde(default)]
    pub status: OrderStatus,
    /// Filled size
    #[serde(default)]
    pub filled_size: f64,
    /// Average fill price
    #[serde(default)]
    pub avg_fill_price: f64,
    /// Creation timestamp
    pub created_at: i64,
    /// Expiration timestamp
    #[serde(default)]
    pub expires_at: i64,
    /// EIP-712 signature type
    #[serde(default)]
    pub signature_type: u8,
    /// Raw signature
    #[serde(default)]
    pub signature: Option<String>,
    /// Maker address
    #[serde(default)]
    pub maker: Option<Address>,
    /// Salt for uniqueness
    #[serde(default)]
    pub salt: U256,
    /// Nonce for onchain cancellation and order uniqueness
    #[serde(default)]
    pub nonce: u64,
    /// Expiration in seconds
    #[serde(default)]
    pub expiration: u64,
}

impl Order {
    /// Create a new order
    pub fn new(token_id: String, side: Side, price: f64, size: f64) -> Self {
        Self {
            id: H256::random(),
            token_id,
            side,
            price,
            size,
            status: OrderStatus::Pending,
            filled_size: 0.0,
            avg_fill_price: 0.0,
            created_at: chrono::Utc::now().timestamp_millis(),
            expires_at: 0,
            signature_type: 0, // EOA
            signature: None,
            maker: None,
            salt: U256::from(rand::random::<u64>()),
            nonce: rand::random::<u64>(),
            expiration: 0,
        }
    }

    /// Calculate total order value
    pub fn total_value(&self) -> f64 {
        self.price * self.size
    }

    /// Calculate remaining size
    pub fn remaining_size(&self) -> f64 {
        self.size - self.filled_size
    }

    /// Check if order is active
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Pending | OrderStatus::Open | OrderStatus::PartiallyFilled
        )
    }

    /// Check if order is complete
    pub fn is_complete(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Filled | OrderStatus::Cancelled | OrderStatus::Expired
        )
    }
}

/// Order book level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookLevel {
    pub price: f64,
    pub size: f64,
}

/// Order book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub token_id: String,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
    pub timestamp: i64,
}

impl OrderBook {
    /// Normalize raw book levels from REST/WS snapshots:
    /// - keep only finite positive price/size
    /// - sort bids descending (best first)
    /// - sort asks ascending (best first)
    pub fn normalize_levels(&mut self) {
        self.bids.retain(|level| {
            level.price.is_finite()
                && level.size.is_finite()
                && level.price > 0.0
                && level.size > 0.0
        });
        self.asks.retain(|level| {
            level.price.is_finite()
                && level.size.is_finite()
                && level.price > 0.0
                && level.size > 0.0
        });

        self.bids
            .sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap_or(Ordering::Equal));
        self.asks
            .sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(Ordering::Equal));
    }

    /// Get best bid
    pub fn best_bid(&self) -> Option<&BookLevel> {
        self.bids
            .iter()
            .filter(|level| {
                level.price.is_finite()
                    && level.size.is_finite()
                    && level.price > 0.0
                    && level.size > 0.0
            })
            .max_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(Ordering::Equal))
    }

    /// Get best ask
    pub fn best_ask(&self) -> Option<&BookLevel> {
        self.asks
            .iter()
            .filter(|level| {
                level.price.is_finite()
                    && level.size.is_finite()
                    && level.price > 0.0
                    && level.size > 0.0
            })
            .min_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(Ordering::Equal))
    }

    /// Get mid price
    pub fn mid_price(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some((bid.price + ask.price) / 2.0),
            _ => None,
        }
    }

    /// Get spread
    pub fn spread(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask.price - bid.price),
            _ => None,
        }
    }

    /// Compute orderbook imbalance: (bid_volume - ask_volume) / (bid_volume + ask_volume)
    /// Returns a value between -1.0 (all asks) and 1.0 (all bids)
    /// Positive = more bid pressure (bullish), Negative = more ask pressure (bearish)
    pub fn imbalance(&self, levels: usize) -> f64 {
        let bid_volume: f64 = self.bids.iter().take(levels).map(|b| b.size).sum();
        let ask_volume: f64 = self.asks.iter().take(levels).map(|a| a.size).sum();
        let total = bid_volume + ask_volume;
        if total > 0.0 {
            (bid_volume - ask_volume) / total
        } else {
            0.0
        }
    }

    /// Compute weighted bid pressure (closer levels have more weight)
    pub fn weighted_bid_pressure(&self, levels: usize) -> f64 {
        self.bids
            .iter()
            .take(levels)
            .enumerate()
            .map(|(i, b)| {
                let weight = 1.0 / (1.0 + i as f64); // Decreasing weight
                b.size * weight
            })
            .sum()
    }

    /// Compute weighted ask pressure (closer levels have more weight)
    pub fn weighted_ask_pressure(&self, levels: usize) -> f64 {
        self.asks
            .iter()
            .take(levels)
            .enumerate()
            .map(|(i, a)| {
                let weight = 1.0 / (1.0 + i as f64); // Decreasing weight
                a.size * weight
            })
            .sum()
    }
}

/// Trade execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub order_id: H256,
    pub token_id: String,
    pub side: Side,
    pub price: f64,
    pub size: f64,
    pub timestamp: i64,
    pub taker: Address,
    pub maker: Address,
}

/// Position on Polymarket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub token_id: String,
    pub size: f64,
    pub avg_price: f64,
    pub current_price: f64,
    pub pnl: f64,
}

/// API response wrapper
#[derive(Debug, Clone, Deserialize)]
pub struct ApiResponse<T> {
    pub data: Option<T>,
    pub error: Option<String>,
}

/// Market from Gamma API - matches actual Polymarket response format
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketResponse {
    pub id: String,
    pub condition_id: String,
    pub slug: Option<String>,
    pub question: String,
    pub description: Option<String>,
    /// JSON string like "[\"Yes\", \"No\"]" - deserialized manually
    #[serde(deserialize_with = "deserialize_json_string")]
    pub outcomes: Vec<String>,
    /// JSON string with token IDs like "[\"123\", \"456\"]"
    #[serde(default, deserialize_with = "deserialize_json_string_opt")]
    pub clob_token_ids: Vec<String>,
    /// JSON string with outcome prices like "[\"0.12\", \"0.88\"]"
    #[serde(default, deserialize_with = "deserialize_json_string_opt")]
    pub outcome_prices: Vec<String>,
    pub active: bool,
    pub closed: Option<bool>,
    /// UMA resolution status when available (e.g. "resolved", "proposed")
    #[serde(default)]
    pub uma_resolution_status: Option<String>,
    pub image: Option<String>,
    pub icon: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    /// Market expiry time in ISO 8601 format (e.g., "2024-01-15T15:45:00Z")
    #[serde(default)]
    pub end_date_iso: Option<String>,
    #[serde(default)]
    pub accepting_orders: Option<bool>,
    #[serde(default)]
    pub enable_order_book: Option<bool>,
    #[serde(default)]
    pub liquidity_num: Option<f64>,
    #[serde(default)]
    pub volume_num: Option<f64>,
    #[serde(default)]
    pub volume_24hr: Option<f64>,
    #[serde(default)]
    pub best_bid: Option<f64>,
    #[serde(default)]
    pub best_ask: Option<f64>,
}

/// Helper to deserialize JSON string arrays
fn deserialize_json_string<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;
    serde_json::from_str(&s).map_err(serde::de::Error::custom)
}

/// Helper to deserialize optional JSON string arrays
fn deserialize_json_string_opt<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        Some(s) => serde_json::from_str(&s).map_err(serde::de::Error::custom),
        None => Ok(Vec::new()),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenResponse {
    pub token_id: String,
    pub outcome: String,
    pub price: Option<f64>,
}

impl From<MarketResponse> for super::MarketInfo {
    fn from(m: MarketResponse) -> Self {
        let tokens: Vec<super::TokenInfo> = m
            .clob_token_ids
            .into_iter()
            .zip(m.outcomes.into_iter())
            .map(|(token_id, outcome)| super::TokenInfo {
                token_id,
                outcome,
                price: 0.5, // Default price, should be fetched separately
            })
            .collect();

        Self {
            condition_id: m.condition_id,
            question: m.question,
            outcomes: vec!["Yes".to_string(), "No".to_string()], // Default outcomes
            tokens,
            active: m.active && m.closed.unwrap_or(false) == false,
            min_tick: 0.01,
            max_tick: 0.99,
            slug: m.slug,
            end_date: m.end_date,
            end_date_iso: m.end_date_iso,
            accepting_orders: m.accepting_orders.unwrap_or(true),
            enable_order_book: m.enable_order_book.unwrap_or(true),
            liquidity_num: m.liquidity_num.unwrap_or(0.0),
            volume_num: m.volume_num.unwrap_or(0.0),
            volume_24hr: m.volume_24hr.unwrap_or(0.0),
            best_bid: m.best_bid.unwrap_or(0.0),
            best_ask: m.best_ask.unwrap_or(1.0),
        }
    }
}

/// Event response from Gamma API (for discovering markets)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventResponse {
    pub id: String,
    pub slug: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub active: bool,
    pub closed: Option<bool>,
    pub end_date: Option<String>,
    #[serde(default)]
    pub markets: Vec<MarketResponse>,
}

/// Order creation request
#[derive(Debug, Clone, Serialize)]
pub struct CreateOrderRequest {
    pub token_id: String,
    pub side: Side,
    pub price: f64,
    pub size: f64,
    #[serde(rename = "feeRateBps")]
    pub fee_rate_bps: u64,
    #[serde(rename = "nonce")]
    pub nonce: u64,
    #[serde(rename = "maker")]
    pub maker: String,
    #[serde(rename = "signatureType")]
    pub signature_type: u8,
    pub signature: Option<String>,
    #[serde(rename = "expiration")]
    pub expiration: u64,
    #[serde(rename = "orderType", skip_serializing_if = "Option::is_none")]
    pub order_type: Option<String>,
    #[serde(rename = "postOnly", skip_serializing_if = "Option::is_none")]
    pub post_only: Option<bool>,
}

/// Cancel order request
#[derive(Debug, Clone, Serialize)]
pub struct CancelOrderRequest {
    pub order_id: H256,
}

#[cfg(test)]
mod tests {
    use super::{BookLevel, OrderBook};

    #[test]
    fn best_bid_best_ask_handle_unsorted_levels() {
        let book = OrderBook {
            token_id: "t".to_string(),
            bids: vec![
                BookLevel {
                    price: 0.45,
                    size: 100.0,
                },
                BookLevel {
                    price: 0.55,
                    size: 10.0,
                },
                BookLevel {
                    price: 0.50,
                    size: 50.0,
                },
            ],
            asks: vec![
                BookLevel {
                    price: 0.70,
                    size: 10.0,
                },
                BookLevel {
                    price: 0.60,
                    size: 50.0,
                },
                BookLevel {
                    price: 0.65,
                    size: 20.0,
                },
            ],
            timestamp: 0,
        };

        let best_bid = book.best_bid().expect("missing best bid");
        let best_ask = book.best_ask().expect("missing best ask");
        assert_eq!(best_bid.price, 0.55);
        assert_eq!(best_ask.price, 0.60);
    }

    #[test]
    fn normalize_levels_filters_invalid_and_sorts() {
        let mut book = OrderBook {
            token_id: "t".to_string(),
            bids: vec![
                BookLevel {
                    price: -1.0,
                    size: 1.0,
                },
                BookLevel {
                    price: 0.45,
                    size: 0.0,
                },
                BookLevel {
                    price: 0.52,
                    size: 4.0,
                },
                BookLevel {
                    price: 0.51,
                    size: 3.0,
                },
            ],
            asks: vec![
                BookLevel {
                    price: 0.70,
                    size: 0.0,
                },
                BookLevel {
                    price: 0.64,
                    size: 5.0,
                },
                BookLevel {
                    price: 0.62,
                    size: 2.0,
                },
            ],
            timestamp: 0,
        };

        book.normalize_levels();

        assert_eq!(book.bids.len(), 2);
        assert_eq!(book.bids[0].price, 0.52);
        assert_eq!(book.bids[1].price, 0.51);
        assert_eq!(book.asks.len(), 2);
        assert_eq!(book.asks[0].price, 0.62);
        assert_eq!(book.asks[1].price, 0.64);
    }
}
