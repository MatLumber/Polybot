//! Core types used throughout PolyBot
//!
//! Defines common data structures for prices, signals, orders, etc.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Supported trading assets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Asset {
    BTC,
    ETH,
    SOL,
    XRP,
}

impl Default for Asset {
    fn default() -> Self {
        Asset::BTC
    }
}

impl Asset {
    /// Get the symbol for CEX APIs (lowercase)
    pub fn cex_symbol(&self) -> &'static str {
        match self {
            Asset::BTC => "btc",
            Asset::ETH => "eth",
            Asset::SOL => "sol",
            Asset::XRP => "xrp",
        }
    }

    /// Get the trading pair for CEX APIs (e.g., "btcusdt")
    pub fn trading_pair(&self) -> &'static str {
        match self {
            Asset::BTC => "BTCUSDT",
            Asset::ETH => "ETHUSDT",
            Asset::SOL => "SOLUSDT",
            Asset::XRP => "XRPUSDT",
        }
    }

    /// Get the trading pair for Coinbase (e.g., "BTC-USD")
    pub fn coinbase_pair(&self) -> &'static str {
        match self {
            Asset::BTC => "BTC-USD",
            Asset::ETH => "ETH-USD",
            Asset::SOL => "SOL-USD",
            Asset::XRP => "XRP-USD",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "BTC" => Some(Asset::BTC),
            "ETH" => Some(Asset::ETH),
            "SOL" => Some(Asset::SOL),
            "XRP" => Some(Asset::XRP),
            _ => None,
        }
    }
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Asset::BTC => write!(f, "BTC"),
            Asset::ETH => write!(f, "ETH"),
            Asset::SOL => write!(f, "SOL"),
            Asset::XRP => write!(f, "XRP"),
        }
    }
}

/// Supported timeframes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    Min15,
    Hour1,
}

impl Default for Timeframe {
    fn default() -> Self {
        Timeframe::Min15
    }
}

impl Timeframe {
    /// Get duration in seconds
    pub fn duration_secs(&self) -> u64 {
        match self {
            Timeframe::Min15 => 15 * 60,
            Timeframe::Hour1 => 60 * 60,
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "15m" | "15min" => Some(Timeframe::Min15),
            "1h" | "1hour" => Some(Timeframe::Hour1),
            _ => None,
        }
    }
}

impl fmt::Display for Timeframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Timeframe::Min15 => write!(f, "15m"),
            Timeframe::Hour1 => write!(f, "1h"),
        }
    }
}

/// Trading direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Up,
    Down,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::Up
    }
}

impl Direction {
    /// Convert to Polymarket outcome index
    pub fn outcome_index(&self) -> u8 {
        match self {
            Direction::Up => 0,
            Direction::Down => 1,
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Direction::Up => write!(f, "UP"),
            Direction::Down => write!(f, "DOWN"),
        }
    }
}

/// Price source identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PriceSource {
    Binance,
    Bybit,
    Coinbase,
    RTDS,
    /// Chainlink oracle via RTDS - this is the primary source Polymarket uses for resolution
    RtdsChainlink,
}

impl fmt::Display for PriceSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PriceSource::Binance => write!(f, "Binance"),
            PriceSource::Bybit => write!(f, "Bybit"),
            PriceSource::Coinbase => write!(f, "Coinbase"),
            PriceSource::RTDS => write!(f, "RTDS"),
            PriceSource::RtdsChainlink => write!(f, "RTDS-Chainlink"),
        }
    }
}

/// Normalized price tick from any source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceTick {
    /// Timestamp in milliseconds (exchange time)
    pub exchange_ts: i64,
    /// Timestamp when we received it (local time)
    pub local_ts: i64,
    /// Asset being priced
    pub asset: Asset,
    /// Best bid price
    pub bid: f64,
    /// Best ask price
    pub ask: f64,
    /// Mid price
    pub mid: f64,
    /// Source of this price
    pub source: PriceSource,
    /// Latency from exchange to us in milliseconds
    pub latency_ms: u64,
}

/// Aggregated oracle price
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OraclePrice {
    /// Timestamp in milliseconds
    pub ts: i64,
    /// Asset
    pub asset: Asset,
    /// Aggregated mid price
    pub mid: f64,
    /// Best bid across sources
    pub bid: f64,
    /// Best ask across sources
    pub ask: f64,
    /// Confidence score (0.0 - 1.0) based on source agreement
    pub confidence: f64,
    /// Number of active sources
    pub source_count: usize,
    /// Individual source prices
    pub sources: Vec<(PriceSource, f64)>,
}

/// Candlestick data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    /// Open time (start of period)
    pub open_time: i64,
    /// Close time (end of period)
    pub close_time: i64,
    /// Asset
    pub asset: Asset,
    /// Timeframe
    pub timeframe: Timeframe,
    /// Open price
    pub open: f64,
    /// High price
    pub high: f64,
    /// Low price
    pub low: f64,
    /// Close price
    pub close: f64,
    /// Volume in base currency
    pub volume: f64,
    /// Number of trades
    pub trades: u64,
}

/// Feature set for prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureSet {
    /// Timestamp
    pub ts: i64,
    /// Asset
    pub asset: Asset,
    /// Timeframe
    pub timeframe: Timeframe,
    /// RSI (14-period, Wilder's smoothing)
    pub rsi: f64,
    /// MACD line
    pub macd_line: f64,
    /// MACD signal (proper EMA-based)
    pub macd_signal: f64,
    /// MACD histogram
    pub macd_hist: f64,
    /// VWAP
    pub vwap: f64,
    /// Bollinger Band upper
    pub bb_upper: f64,
    /// Bollinger Band lower
    pub bb_lower: f64,
    /// ATR (14-period)
    pub atr: f64,
    /// Price momentum (% per minute)
    pub momentum: f64,
    /// Momentum acceleration
    pub momentum_accel: f64,
    /// Order book imbalance (-1 to 1)
    pub book_imbalance: f64,
    /// Spread in basis points
    pub spread_bps: f64,
    /// Trade intensity (trades per second)
    pub trade_intensity: f64,
    /// Heikin Ashi close
    pub ha_close: f64,
    /// Heikin Ashi trend (1 = bullish, -1 = bearish)
    pub ha_trend: i8,
    /// Oracle confidence
    pub oracle_confidence: f64,
    /// ADX (Average Directional Index, 0-100)
    pub adx: f64,
    /// Stochastic RSI (0.0-1.0)
    pub stoch_rsi: f64,
    /// On Balance Volume
    pub obv: f64,
    /// Relative Volume (current / avg)
    pub relative_volume: f64,
    /// Market regime (1=trending, 0=ranging, -1=volatile)
    pub regime: i8,
}

/// Trading signal generated by strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    /// Unique signal ID
    pub id: String,
    /// Timestamp
    pub ts: i64,
    /// Asset
    pub asset: Asset,
    /// Timeframe
    pub timeframe: Timeframe,
    /// Predicted direction
    pub direction: Direction,
    /// Confidence level (0.5 - 1.0)
    pub confidence: f64,
    /// Features used for this prediction
    pub features: FeatureSet,
    /// Strategy that generated this signal
    pub strategy_id: String,
    /// Market slug on Polymarket
    pub market_slug: String,
    /// Condition ID
    pub condition_id: String,
    /// Token ID for the predicted outcome
    pub token_id: String,
    /// When the market expires (Unix timestamp in milliseconds)
    pub expires_at: i64,
    /// Suggested position size in USDC
    pub suggested_size_usdc: f64,
    /// Quote hints captured when the signal was formed (Polymarket token quote).
    #[serde(default)]
    pub quote_bid: f64,
    #[serde(default)]
    pub quote_ask: f64,
    #[serde(default)]
    pub quote_mid: f64,
    #[serde(default)]
    pub quote_depth_top5: f64,
    /// Indicators that contributed to this signal (for calibration)
    #[serde(default)]
    pub indicators_used: Vec<String>,
}

/// Trade record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    /// Trade ID
    pub id: String,
    /// Open timestamp
    pub ts_open: i64,
    /// Close timestamp (0 if open)
    pub ts_close: i64,
    /// Asset
    pub asset: Asset,
    /// Timeframe
    pub timeframe: Timeframe,
    /// Market slug
    pub market_slug: String,
    /// Strategy ID
    pub strategy_id: String,
    /// Direction (UP/DOWN)
    pub side: Direction,
    /// Entry price
    pub entry_px: f64,
    /// Exit price (0 if open)
    pub exit_px: f64,
    /// Position size
    pub size: f64,
    /// Fee paid in USDC
    pub fee_paid: f64,
    /// Result (Win/Loss/Pending)
    pub result: TradeResult,
    /// PnL in USDC
    pub pnl_usdc: f64,
    /// PnL in basis points
    pub pnl_bps: i64,
    /// Order submission latency in ms
    pub latency_submit_ms: u64,
    /// Order fill latency in ms
    pub latency_fill_ms: u64,
    /// Oracle mid at open
    pub oracle_mid_open: f64,
    /// Polymarket mid at open
    pub polymarket_mid_open: f64,
    /// Confidence at entry
    pub confidence: f64,
    /// Additional notes
    pub notes: serde_json::Value,
}

/// Trade result
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeResult {
    Pending,
    Win,
    Loss,
}

impl fmt::Display for TradeResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TradeResult::Pending => write!(f, "PENDING"),
            TradeResult::Win => write!(f, "WIN"),
            TradeResult::Loss => write!(f, "LOSS"),
        }
    }
}

/// Closed trade result with PnL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosedTrade {
    /// Asset traded
    pub asset: Asset,
    /// Direction of the trade
    pub direction: Direction,
    /// Position size
    pub size: f64,
    /// Entry price
    pub entry_price: f64,
    /// Exit price
    pub exit_price: f64,
    /// Profit/Loss
    pub pnl: f64,
    /// Open timestamp (milliseconds)
    pub opened_at: i64,
    /// Close timestamp (milliseconds)
    pub closed_at: i64,
}

/// Market info from Polymarket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    /// Market slug (human-readable ID)
    pub slug: String,
    /// Question/title
    pub question: String,
    /// Condition ID (used for orders)
    pub condition_id: String,
    /// Token ID for YES/UP outcome
    pub token_up: String,
    /// Token ID for NO/DOWN outcome
    pub token_down: String,
    /// End timestamp
    pub end_ts: i64,
    /// Asset (if crypto market)
    pub asset: Option<Asset>,
    /// Timeframe (if crypto market)
    pub timeframe: Option<Timeframe>,
    /// Tick size
    pub tick_size: f64,
    /// Minimum order size
    pub min_size: f64,
    /// Active status
    pub active: bool,
}

/// Order for Polymarket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Order ID
    pub id: String,
    /// Token ID
    pub token_id: String,
    /// Price (0.0 - 1.0)
    pub price: f64,
    /// Size in shares
    pub size: f64,
    /// Side (BUY/SELL)
    pub side: OrderSide,
    /// Order type
    pub order_type: OrderType,
    /// Expiration timestamp (0 = GTC)
    pub expiration: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    GTC, // Good-Til-Cancelled
    GTD, // Good-Til-Date
    FOK, // Fill-Or-Kill
}
