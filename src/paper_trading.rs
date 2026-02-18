//! Paper Trading Engine
//!
//! Simulates LIVE order execution using real-time RTDS prices
//! (prioritizing Chainlink - what Polymarket uses for market resolution).
//!
//! Key features:
//! - **Market-aware expiry**: Positions close when the Polymarket market window ends
//!   (e.g., BTC-15m market 15:00-15:15 → position must close at 15:15)
//! - **Chainlink-first pricing**: Uses the same price feed Polymarket resolves against
//! - **Rich analytics CSV**: Every trade logs indicators, market window, timing data
//! - **Real-time dashboard**: Winrate, P&L, per-asset breakdown, streaks
//! - **$85 balance**: Sized for your actual balance with proportional positioning
//! - **State persistence**: Saves/loads state to JSON file for recovery on restart

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use tracing::{error, info, warn};

use crate::persistence::{CsvPersistence, PaperAnalyticsRecord, TradeRecord, WinLossRecord};
use crate::polymarket::{compute_fractional_kelly, estimate_expected_value, fee_rate_from_price};
use crate::types::{Asset, Direction, FeatureSet, PriceSource, Signal, Timeframe};

// ─────────────────────────────────────────────────────────────────
// Persistent State (for restart recovery)
// ─────────────────────────────────────────────────────────────────

/// Serializable state for paper trading engine persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperTradingState {
    /// Current balance (USDC)
    pub balance: f64,
    /// Open positions (keyed by market string, e.g. BTC-15m)
    pub positions: HashMap<String, SerializablePosition>,
    /// Global stats
    pub stats: SerializableStats,
    /// Per-asset stats (keyed by asset string)
    pub asset_stats: HashMap<String, SerializableAssetStats>,
    /// Session open prices for change_24h calculation
    pub session_open_prices: HashMap<String, f64>,
    /// Timestamp of last save
    pub saved_at: i64,
}

/// Serializable position (Asset enum can't be used as key in JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializablePosition {
    pub id: String,
    pub asset: String,
    pub timeframe: String,
    pub direction: String,
    pub entry_price: f64,
    pub current_price: f64,
    pub size_usdc: f64,
    pub shares: f64,
    pub fee_paid: f64,
    pub opened_at: i64,
    pub market_slug: String,
    pub signal_id: String,
    pub confidence: f64,
    pub market_open_ts: i64,
    pub market_close_ts: i64,
    pub price_at_market_open: f64,
    pub peak_price: f64,
    pub trough_price: f64,
    pub stop_price: f64,
    pub take_profit_price: f64,
    pub unrealized_pnl: f64,
    pub trail_pct: f64,
    pub share_price: f64,
    pub peak_share_price: f64,
    pub is_winning: bool,
    pub checkpoint_armed: bool,
    pub checkpoint_peak_roi: f64,
    pub checkpoint_floor_roi: f64,
    pub checkpoint_breach_ticks: u8,
    #[serde(default)]
    pub hard_stop_breach_ticks: u8,
    #[serde(default)]
    pub dynamic_hard_stop_roi: f64,
    #[serde(default)]
    pub indicators_used: Vec<String>,
    #[serde(default)]
    pub token_id: String,
    #[serde(default)]
    pub outcome: String,
    #[serde(default)]
    pub entry_bid: f64,
    #[serde(default)]
    pub entry_ask: f64,
    #[serde(default)]
    pub entry_mid: f64,
    #[serde(default)]
    pub p_market: f64,
    #[serde(default)]
    pub p_model: f64,
    #[serde(default)]
    pub edge_net: f64,
    #[serde(default)]
    pub kelly_raw: f64,
    #[serde(default)]
    pub kelly_applied: f64,
}

/// Serializable stats
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SerializableStats {
    pub total_trades: u32,
    pub wins: u32,
    pub losses: u32,
    pub total_pnl: f64,
    pub total_fees: f64,
    pub largest_win: f64,
    pub largest_loss: f64,
    pub max_drawdown: f64,
    pub peak_balance: f64,
    pub current_streak: i32,
    pub best_streak: i32,
    pub worst_streak: i32,
    pub sum_win_pnl: f64,
    pub sum_loss_pnl: f64,
    pub gross_profit: f64,
    pub gross_loss: f64,
    pub exits_trailing_stop: u32,
    pub exits_take_profit: u32,
    pub exits_market_expiry: u32,
    pub exits_time_expiry: u32,
}

/// Serializable asset stats
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SerializableAssetStats {
    pub trades: u32,
    pub wins: u32,
    pub losses: u32,
    pub pnl: f64,
    pub avg_confidence_win: f64,
    pub avg_confidence_loss: f64,
    pub sum_confidence_win: f64,
    pub sum_confidence_loss: f64,
}

impl Default for PaperTradingState {
    fn default() -> Self {
        Self {
            balance: 1000.0,
            positions: HashMap::new(),
            stats: SerializableStats::default(),
            asset_stats: HashMap::new(),
            session_open_prices: HashMap::new(),
            saved_at: 0,
        }
    }
}

/// Helper function to parse asset string back to enum
fn parse_asset(s: &str) -> Result<Asset> {
    match s.trim_matches('"') {
        "BTC" | "Btc" | "btc" => Ok(Asset::BTC),
        "ETH" | "Eth" | "eth" => Ok(Asset::ETH),
        "SOL" | "Sol" | "sol" => Ok(Asset::SOL),
        "XRP" | "Xrp" | "xrp" => Ok(Asset::XRP),
        _ => anyhow::bail!("Unknown asset: {}", s),
    }
}

fn parse_timeframe_label(s: &str) -> Result<Timeframe> {
    match s.trim_matches('"').to_ascii_lowercase().as_str() {
        "15m" | "15min" | "min15" | "m15" => Ok(Timeframe::Min15),
        "1h" | "hour1" | "h1" | "60m" => Ok(Timeframe::Hour1),
        _ => anyhow::bail!("Unknown timeframe: {}", s),
    }
}

fn parse_direction_label(s: &str) -> Result<Direction> {
    match s.trim_matches('"').to_ascii_lowercase().as_str() {
        "up" | "buy" => Ok(Direction::Up),
        "down" | "sell" => Ok(Direction::Down),
        _ => anyhow::bail!("Unknown direction: {}", s),
    }
}

// ─────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PaperTradingConfig {
    /// Starting virtual balance (USDC)
    pub initial_balance: f64,
    /// Simulated slippage in basis points (e.g., 5 = 0.05%)
    pub slippage_bps: f64,
    /// Simulated fee in basis points (e.g., 10 = 0.10%)
    pub fee_bps: f64,
    /// Trailing stop percentage (e.g., 0.03 = 3%)
    pub trailing_stop_pct: f64,
    /// Take-profit percentage (e.g., 0.05 = 5%)
    pub take_profit_pct: f64,
    /// Fallback max hold (only used if no market expiry is known)
    pub max_hold_duration_ms: i64,
    /// Dashboard log interval in seconds
    pub dashboard_interval_secs: u64,
    /// Prefer Chainlink prices for fills (matches Polymarket resolution)
    pub prefer_chainlink: bool,
    /// Enforce Polymarket-native price sources only.
    pub native_only: bool,
    /// Checkpoint arm ROI baseline (dynamic logic may tighten/lower this intratrade).
    pub checkpoint_arm_roi: f64,
    /// Initial checkpoint floor ROI baseline.
    pub checkpoint_initial_floor_roi: f64,
    /// Baseline trailing gap from peak ROI.
    pub checkpoint_trail_gap_roi: f64,
    /// Hard stop ROI baseline.
    pub hard_stop_roi: f64,
    /// Time-stop threshold in seconds to expiry
    pub time_stop_seconds_to_expiry: i64,
    /// Kelly sizing toggle and parameters
    pub kelly_enabled: bool,
    pub kelly_fraction_15m: f64,
    pub kelly_fraction_1h: f64,
    pub kelly_cap_15m: f64,
    pub kelly_cap_1h: f64,
    /// Minimum edge threshold for entry. v2.0: Increased to 0.05 (5%)
    pub min_edge_net: f64,
    // NUEVO v2.0: Stops adaptativos basados en ATR
    pub hard_stop_atr_multiplier: f64,
    pub adaptive_stops_enabled: bool,
    // NUEVO v2.0: ETH sizing multiplier (más conservador)
    pub eth_size_multiplier: f64,
    // NUEVO v2.0: ATR period for adaptive stops
    pub atr_period: usize,
}

impl Default for PaperTradingConfig {
    fn default() -> Self {
        Self {
            initial_balance: 1000.0,
            slippage_bps: 5.0,
            fee_bps: 10.0,
            trailing_stop_pct: 0.005,
            take_profit_pct: 0.008,
            max_hold_duration_ms: 7_200_000,
            dashboard_interval_secs: 30,
            prefer_chainlink: true,
            native_only: false,
            checkpoint_arm_roi: 0.05,
            checkpoint_initial_floor_roi: 0.022,
            checkpoint_trail_gap_roi: 0.012,
            hard_stop_roi: -0.07,
            time_stop_seconds_to_expiry: 90,
            kelly_enabled: true,
            kelly_fraction_15m: 0.25,
            kelly_fraction_1h: 0.50,
            kelly_cap_15m: 0.05,
            kelly_cap_1h: 0.10,
            min_edge_net: 0.05,  // v2.0: 5% edge mínimo (era 0.0)
            // NUEVO v2.0
            hard_stop_atr_multiplier: 1.5,
            adaptive_stops_enabled: true,
            eth_size_multiplier: 0.8,  // Reducir sizing ETH 20%
            atr_period: 14,
        }
    }
}

// ─────────────────────────────────────────────────────────────────
// Paper Position (market-window aware)
// ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PaperPosition {
    pub id: String,
    pub asset: Asset,
    pub timeframe: Timeframe,
    pub direction: Direction,
    pub entry_price: f64,
    pub current_price: f64,
    pub size_usdc: f64,
    pub shares: f64,
    pub fee_paid: f64,
    pub opened_at: i64,
    pub market_slug: String,
    pub signal_id: String,
    pub confidence: f64,
    /// Features snapshot at entry (for analytics CSV)
    pub features_at_entry: Option<FeatureSet>,
    // ── Market window awareness ──
    /// When the Polymarket market window STARTED (e.g., 15:00:00 for a 15m market)
    pub market_open_ts: i64,
    /// When the Polymarket market window ENDS (e.g., 15:15:00) - position MUST close here
    pub market_close_ts: i64,
    /// Price when this market window opened (for reference)
    pub price_at_market_open: f64,
    // ── Tracking ──
    pub peak_price: f64,
    pub trough_price: f64,
    pub stop_price: f64,
    pub take_profit_price: f64,
    pub unrealized_pnl: f64,
    /// Trailing stop distance as percentage (set from ATR at entry)
    pub trail_pct: f64,
    // ── Prediction Market Fields ──
    /// Share price at entry (probability-based, $0.01-$0.99)
    pub share_price: f64,
    /// Highest share price seen since entry (for trailing take profit)
    pub peak_share_price: f64,
    /// Whether the position is currently winning (price moved in correct direction)
    pub is_winning: bool,
    /// Checkpoint trailing state (dynamic arm/floor/gap).
    pub checkpoint_armed: bool,
    pub checkpoint_peak_roi: f64,
    pub checkpoint_floor_roi: f64,
    pub checkpoint_breach_ticks: u8,
    /// Dynamic hard-stop consecutive breach counter (noise filter).
    pub hard_stop_breach_ticks: u8,
    /// Last computed dynamic hard-stop ROI threshold.
    pub dynamic_hard_stop_roi: f64,
    /// Indicators that contributed to this trade (for calibration)
    pub indicators_used: Vec<String>,
    pub token_id: String,
    pub outcome: String,
    pub entry_bid: f64,
    pub entry_ask: f64,
    pub entry_mid: f64,
    pub p_market: f64,
    pub p_model: f64,
    pub edge_net: f64,
    pub kelly_raw: f64,
    pub kelly_applied: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperExitReason {
    HardStop,
    CheckpointTakeProfit,
    TimeStop,
    MarketExpiry,
    TimeExpiry,
    Manual,
}

impl std::fmt::Display for PaperExitReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaperExitReason::HardStop => write!(f, "HARD_STOP"),
            PaperExitReason::CheckpointTakeProfit => write!(f, "CHECKPOINT_TAKE_PROFIT"),
            PaperExitReason::TimeStop => write!(f, "TIME_STOP"),
            PaperExitReason::MarketExpiry => write!(f, "MARKET_EXPIRY"),
            PaperExitReason::TimeExpiry => write!(f, "TIME_EXPIRY"),
            PaperExitReason::Manual => write!(f, "MANUAL"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperTradeRecord {
    pub timestamp: i64,
    pub trade_id: String,
    pub signal_id: String,
    pub asset: String,
    pub timeframe: String,
    pub direction: String,
    pub confidence: f64,
    pub entry_price: f64,
    pub exit_price: f64,
    pub size_usdc: f64,
    pub shares: f64,
    pub fee_paid: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub result: String,
    pub exit_reason: String,
    pub hold_duration_ms: i64,
    pub balance_after: f64,
    // Market window data
    pub market_open_ts: i64,
    pub market_close_ts: i64,
    pub time_remaining_at_entry_secs: i64,
    // Indicators that contributed to this trade (for calibration)
    #[serde(default)]
    pub indicators_used: Vec<String>,
    /// Extended execution and EV telemetry
    #[serde(default)]
    pub market_id: String,
    #[serde(default)]
    pub token_id: String,
    #[serde(default)]
    pub outcome: String,
    #[serde(default)]
    pub entry_bid: f64,
    #[serde(default)]
    pub entry_ask: f64,
    #[serde(default)]
    pub entry_mid: f64,
    #[serde(default)]
    pub exit_bid: f64,
    #[serde(default)]
    pub exit_ask: f64,
    #[serde(default)]
    pub exit_mid: f64,
    #[serde(default)]
    pub fee_open: f64,
    #[serde(default)]
    pub fee_close: f64,
    #[serde(default)]
    pub slippage_open: f64,
    #[serde(default)]
    pub slippage_close: f64,
    #[serde(default)]
    pub p_market: f64,
    #[serde(default)]
    pub p_model: f64,
    #[serde(default)]
    pub edge_net: f64,
    #[serde(default)]
    pub kelly_raw: f64,
    #[serde(default)]
    pub kelly_applied: f64,
    #[serde(default)]
    pub exit_reason_detail: String,
}

// ─────────────────────────────────────────────────────────────────
// Per-asset stats (for optimization)
// ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct AssetStats {
    pub trades: u32,
    pub wins: u32,
    pub losses: u32,
    pub pnl: f64,
    pub avg_confidence_win: f64,
    pub avg_confidence_loss: f64,
    pub sum_confidence_win: f64,
    pub sum_confidence_loss: f64,
}

// ─────────────────────────────────────────────────────────────────
// Calibration callback type
// ─────────────────────────────────────────────────────────────────

/// Callback type for trade result calibration
/// Parameters: (asset, timeframe, indicators_used, is_win, p_model)
pub type CalibrationCallback = Box<dyn Fn(Asset, Timeframe, &[String], bool, f64) + Send + Sync>;

// ─────────────────────────────────────────────────────────────────
// Polymarket Share Price Data (from real orderbook)
// ─────────────────────────────────────────────────────────────────

/// Real share price data from Polymarket orderbook
/// Key: (asset, timeframe, direction) -> share price
#[derive(Debug, Clone, Default)]
pub struct PolymarketSharePrices {
    /// Map: (asset, timeframe, "UP") -> YES share quote
    /// Map: (asset, timeframe, "DOWN") -> NO share quote
    prices: std::sync::Arc<RwLock<HashMap<(Asset, Timeframe, String), ShareQuote>>>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ShareQuote {
    pub bid: f64,
    pub ask: f64,
    pub mid: f64,
    pub bid_size: f64,
    pub ask_size: f64,
    pub depth_top5: f64,
    pub updated_at: i64,
}

impl PolymarketSharePrices {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update share price from orderbook midpoint
    pub fn update(&self, asset: Asset, timeframe: Timeframe, direction: &str, price: f64) {
        self.update_quote(asset, timeframe, direction, price, price, price);
    }

    /// Update full share quote from orderbook.
    pub fn update_quote(
        &self,
        asset: Asset,
        timeframe: Timeframe,
        direction: &str,
        bid: f64,
        ask: f64,
        mid: f64,
    ) {
        self.update_quote_with_depth(asset, timeframe, direction, bid, ask, mid, 0.0, 0.0, 0.0);
    }

    /// Update full share quote with top-of-book sizes and aggregated top-5 depth.
    pub fn update_quote_with_depth(
        &self,
        asset: Asset,
        timeframe: Timeframe,
        direction: &str,
        bid: f64,
        ask: f64,
        mid: f64,
        bid_size: f64,
        ask_size: f64,
        depth_top5: f64,
    ) {
        let key = (asset, timeframe, direction.to_uppercase());
        if let Ok(mut prices) = self.prices.write() {
            let b = bid.clamp(0.01, 0.99);
            let a = ask.clamp(0.01, 0.99);
            let m = mid.clamp(0.01, 0.99);
            prices.insert(
                key,
                ShareQuote {
                    bid: b,
                    ask: a,
                    mid: m,
                    bid_size: bid_size.max(0.0),
                    ask_size: ask_size.max(0.0),
                    depth_top5: depth_top5.max(0.0),
                    updated_at: Utc::now().timestamp_millis(),
                },
            );
        }
    }

    /// Get share price for a specific market
    pub fn get(&self, asset: Asset, timeframe: Timeframe, direction: &str) -> Option<f64> {
        let key = (asset, timeframe, direction.to_uppercase());
        self.prices.read().ok()?.get(&key).map(|q| q.mid)
    }

    pub fn get_quote(
        &self,
        asset: Asset,
        timeframe: Timeframe,
        direction: &str,
    ) -> Option<ShareQuote> {
        let key = (asset, timeframe, direction.to_uppercase());
        self.prices.read().ok()?.get(&key).copied()
    }

    pub fn get_bid(&self, asset: Asset, timeframe: Timeframe, direction: &str) -> Option<f64> {
        self.get_quote(asset, timeframe, direction).map(|q| q.bid)
    }

    pub fn get_ask(&self, asset: Asset, timeframe: Timeframe, direction: &str) -> Option<f64> {
        self.get_quote(asset, timeframe, direction).map(|q| q.ask)
    }

    pub fn get_spread(&self, asset: Asset, timeframe: Timeframe, direction: &str) -> Option<f64> {
        self.get_quote(asset, timeframe, direction)
            .map(|q| (q.ask - q.bid).max(0.0))
    }
}

// ─────────────────────────────────────────────────────────────────
// Paper Trading Engine
// ─────────────────────────────────────────────────────────────────

pub struct PaperTradingEngine {
    config: PaperTradingConfig,
    balance: RwLock<f64>,
    positions: RwLock<HashMap<(Asset, Timeframe), PaperPosition>>,
    latest_prices: RwLock<HashMap<(Asset, PriceSource), f64>>,
    trade_history: RwLock<Vec<PaperTradeRecord>>,
    stats: RwLock<PaperStats>,
    asset_stats: RwLock<HashMap<Asset, AssetStats>>,
    last_dashboard: RwLock<i64>,
    persistence: Option<std::sync::Arc<CsvPersistence>>,
    /// Path to save state file
    state_file: Option<PathBuf>,
    /// Optional callback for indicator calibration when trades close
    calibration_callback: Option<std::sync::Arc<CalibrationCallback>>,
    /// Real share prices from Polymarket orderbook (optional)
    /// When set, paper trading uses REAL share prices from Polymarket
    /// instead of simulated model
    polymarket_share_prices: Option<std::sync::Arc<PolymarketSharePrices>>,
    /// Enforce one directional bias per market window.
    window_bias: RwLock<HashMap<(Asset, Timeframe, i64), Direction>>,
    /// Windows where a hard stop has occurred — block re-entry for the remainder.
    stopped_windows: RwLock<std::collections::HashSet<(Asset, Timeframe, i64)>>,
}

#[derive(Debug, Clone, Default)]
pub struct PaperStats {
    pub total_trades: u32,
    pub wins: u32,
    pub losses: u32,
    pub total_pnl: f64,
    pub total_fees: f64,
    pub largest_win: f64,
    pub largest_loss: f64,
    pub max_drawdown: f64,
    pub peak_balance: f64,
    pub current_streak: i32,
    pub best_streak: i32,
    pub worst_streak: i32,
    // For averages
    pub sum_win_pnl: f64,
    pub sum_loss_pnl: f64,
    // For profit factor
    pub gross_profit: f64,
    pub gross_loss: f64,
    // Exit reason counts (for analyzing strategy behavior)
    pub exits_trailing_stop: u32,
    pub exits_take_profit: u32,
    pub exits_market_expiry: u32,
    pub exits_time_expiry: u32,
}

impl PaperTradingEngine {
    pub fn new(config: PaperTradingConfig) -> Self {
        let initial_balance = config.initial_balance;
        Self {
            config,
            balance: RwLock::new(initial_balance),
            positions: RwLock::new(HashMap::new()),
            latest_prices: RwLock::new(HashMap::new()),
            trade_history: RwLock::new(Vec::new()),
            stats: RwLock::new(PaperStats {
                peak_balance: initial_balance,
                ..Default::default()
            }),
            asset_stats: RwLock::new(HashMap::new()),
            last_dashboard: RwLock::new(0),
            persistence: None,
            state_file: None,
            calibration_callback: None,
            polymarket_share_prices: None,
            window_bias: RwLock::new(HashMap::new()),
            stopped_windows: RwLock::new(std::collections::HashSet::new()),
        }
    }

    pub fn with_persistence(mut self, persistence: std::sync::Arc<CsvPersistence>) -> Self {
        self.persistence = Some(persistence);
        self
    }

    /// Set the calibration callback for indicator weight adjustment
    pub fn with_calibration_callback(
        mut self,
        callback: std::sync::Arc<CalibrationCallback>,
    ) -> Self {
        self.calibration_callback = Some(callback);
        self
    }

    /// Connect to real Polymarket share prices from orderbook
    /// When connected, paper trading uses REAL share prices instead of simulation
    pub fn with_polymarket_share_prices(
        mut self,
        prices: std::sync::Arc<PolymarketSharePrices>,
    ) -> Self {
        self.polymarket_share_prices = Some(prices);
        self
    }

    /// Set the state file path for persistence
    pub fn with_state_file(mut self, path: PathBuf) -> Self {
        self.state_file = Some(path);
        self
    }

    /// Save current state to JSON file
    pub fn save_state(&self) -> Result<()> {
        let state_file = match &self.state_file {
            Some(p) => p.clone(),
            None => return Ok(()), // No state file configured, skip saving
        };

        let state = self.export_state();
        let json = serde_json::to_string_pretty(&state)?;
        fs::write(&state_file, json)?;
        info!(path = %state_file.display(), "💾 [PAPER] State saved");
        Ok(())
    }

    /// Calculate simulated share price when real orderbook data is not available
    /// This models how share price moves based on underlying price movement
    fn calculate_simulated_share_price(&self, pos: &PaperPosition, current_price: f64) -> f64 {
        let price_diff = match pos.direction {
            Direction::Up => current_price - pos.entry_price,
            Direction::Down => pos.entry_price - current_price,
        };
        let price_move_pct = price_diff / pos.entry_price;

        // Time factor: closer to expiry = more extreme share price movements
        let now = Utc::now().timestamp_millis();
        let window_duration = (pos.market_close_ts - pos.market_open_ts).max(1) as f64;
        let time_elapsed = (now - pos.opened_at).max(0) as f64;
        let time_factor = 1.0 + (time_elapsed / window_duration).min(1.0) * 2.0;

        // Sensitivity based on timeframe
        let sensitivity = match pos.timeframe {
            Timeframe::Min15 => 15.0,
            Timeframe::Hour1 => 8.0,
        };

        let share_price_delta = price_move_pct * sensitivity * time_factor;
        (pos.share_price + share_price_delta).clamp(0.02, 0.98)
    }

    /// Load state from JSON file
    pub fn load_state(&self) -> Result<()> {
        let state_file = match &self.state_file {
            Some(p) => p.clone(),
            None => return Ok(()), // No state file configured
        };

        if !state_file.exists() {
            info!(path = %state_file.display(), "💾 [PAPER] No state file found, starting fresh");
            return Ok(());
        }

        let json = fs::read_to_string(&state_file)?;
        let state: PaperTradingState = serde_json::from_str(&json)?;

        self.import_state(&state);
        info!(
            path = %state_file.display(),
            balance = %format!("${:.2}", state.balance),
            positions = state.positions.len(),
            trades = state.stats.total_trades,
            "💾 [PAPER] State loaded"
        );
        Ok(())
    }

    /// Export current state to serializable format
    fn export_state(&self) -> PaperTradingState {
        let positions: HashMap<String, SerializablePosition> = self
            .positions
            .read()
            .unwrap()
            .iter()
            .map(|((asset, timeframe), pos)| {
                (
                    format!("{:?}-{}", asset, timeframe),
                    SerializablePosition {
                        id: pos.id.clone(),
                        asset: format!("{:?}", pos.asset),
                        timeframe: format!("{}", pos.timeframe),
                        direction: format!("{:?}", pos.direction),
                        entry_price: pos.entry_price,
                        current_price: pos.current_price,
                        size_usdc: pos.size_usdc,
                        shares: pos.shares,
                        fee_paid: pos.fee_paid,
                        opened_at: pos.opened_at,
                        market_slug: pos.market_slug.clone(),
                        signal_id: pos.signal_id.clone(),
                        confidence: pos.confidence,
                        market_open_ts: pos.market_open_ts,
                        market_close_ts: pos.market_close_ts,
                        price_at_market_open: pos.price_at_market_open,
                        peak_price: pos.peak_price,
                        trough_price: pos.trough_price,
                        stop_price: pos.stop_price,
                        take_profit_price: pos.take_profit_price,
                        unrealized_pnl: pos.unrealized_pnl,
                        trail_pct: pos.trail_pct,
                        share_price: pos.share_price,
                        peak_share_price: pos.peak_share_price,
                        is_winning: pos.is_winning,
                        checkpoint_armed: pos.checkpoint_armed,
                        checkpoint_peak_roi: pos.checkpoint_peak_roi,
                        checkpoint_floor_roi: pos.checkpoint_floor_roi,
                        checkpoint_breach_ticks: pos.checkpoint_breach_ticks,
                        hard_stop_breach_ticks: pos.hard_stop_breach_ticks,
                        dynamic_hard_stop_roi: pos.dynamic_hard_stop_roi,
                        indicators_used: pos.indicators_used.clone(),
                        token_id: pos.token_id.clone(),
                        outcome: pos.outcome.clone(),
                        entry_bid: pos.entry_bid,
                        entry_ask: pos.entry_ask,
                        entry_mid: pos.entry_mid,
                        p_market: pos.p_market,
                        p_model: pos.p_model,
                        edge_net: pos.edge_net,
                        kelly_raw: pos.kelly_raw,
                        kelly_applied: pos.kelly_applied,
                    },
                )
            })
            .collect();

        let stats = self.stats.read().unwrap().clone();
        let serializable_stats = SerializableStats {
            total_trades: stats.total_trades,
            wins: stats.wins,
            losses: stats.losses,
            total_pnl: stats.total_pnl,
            total_fees: stats.total_fees,
            largest_win: stats.largest_win,
            largest_loss: stats.largest_loss,
            max_drawdown: stats.max_drawdown,
            peak_balance: stats.peak_balance,
            current_streak: stats.current_streak,
            best_streak: stats.best_streak,
            worst_streak: stats.worst_streak,
            sum_win_pnl: stats.sum_win_pnl,
            sum_loss_pnl: stats.sum_loss_pnl,
            gross_profit: stats.gross_profit,
            gross_loss: stats.gross_loss,
            exits_trailing_stop: stats.exits_trailing_stop,
            exits_take_profit: stats.exits_take_profit,
            exits_market_expiry: stats.exits_market_expiry,
            exits_time_expiry: stats.exits_time_expiry,
        };

        let asset_stats: HashMap<String, SerializableAssetStats> = self
            .asset_stats
            .read()
            .unwrap()
            .iter()
            .map(|(asset, a)| {
                (
                    format!("{:?}", asset),
                    SerializableAssetStats {
                        trades: a.trades,
                        wins: a.wins,
                        losses: a.losses,
                        pnl: a.pnl,
                        avg_confidence_win: a.avg_confidence_win,
                        avg_confidence_loss: a.avg_confidence_loss,
                        sum_confidence_win: a.sum_confidence_win,
                        sum_confidence_loss: a.sum_confidence_loss,
                    },
                )
            })
            .collect();

        PaperTradingState {
            balance: self.get_balance(),
            positions,
            stats: serializable_stats,
            asset_stats,
            session_open_prices: HashMap::new(), // Will be populated by dashboard memory
            saved_at: Utc::now().timestamp_millis(),
        }
    }

    /// Import state from serializable format
    fn import_state(&self, state: &PaperTradingState) {
        // Restore balance
        {
            let mut bal = self.balance.write().unwrap();
            *bal = state.balance;
        }

        // Restore stats
        {
            let mut stats = self.stats.write().unwrap();
            stats.total_trades = state.stats.total_trades;
            stats.wins = state.stats.wins;
            stats.losses = state.stats.losses;
            stats.total_pnl = state.stats.total_pnl;
            stats.total_fees = state.stats.total_fees;
            stats.largest_win = state.stats.largest_win;
            stats.largest_loss = state.stats.largest_loss;
            stats.max_drawdown = state.stats.max_drawdown;
            stats.peak_balance = state.stats.peak_balance;
            stats.current_streak = state.stats.current_streak;
            stats.best_streak = state.stats.best_streak;
            stats.worst_streak = state.stats.worst_streak;
            stats.sum_win_pnl = state.stats.sum_win_pnl;
            stats.sum_loss_pnl = state.stats.sum_loss_pnl;
            stats.gross_profit = state.stats.gross_profit;
            stats.gross_loss = state.stats.gross_loss;
            stats.exits_trailing_stop = state.stats.exits_trailing_stop;
            stats.exits_take_profit = state.stats.exits_take_profit;
            stats.exits_market_expiry = state.stats.exits_market_expiry;
            stats.exits_time_expiry = state.stats.exits_time_expiry;
        }

        // Restore asset stats
        {
            let mut asset_map = self.asset_stats.write().unwrap();
            for (asset_str, a) in &state.asset_stats {
                if let Ok(asset) = parse_asset(asset_str) {
                    asset_map.insert(
                        asset,
                        AssetStats {
                            trades: a.trades,
                            wins: a.wins,
                            losses: a.losses,
                            pnl: a.pnl,
                            avg_confidence_win: a.avg_confidence_win,
                            avg_confidence_loss: a.avg_confidence_loss,
                            sum_confidence_win: a.sum_confidence_win,
                            sum_confidence_loss: a.sum_confidence_loss,
                        },
                    );
                }
            }
        }

        // Restore only active positions (ignore already expired windows).
        let now_ms = Utc::now().timestamp_millis();
        let mut restored_positions: Vec<PaperPosition> = Vec::new();
        let mut skipped_positions = 0usize;
        {
            let mut positions_map = self.positions.write().unwrap();
            positions_map.clear();

            for serialized in state.positions.values() {
                if serialized.market_close_ts > 0 && serialized.market_close_ts <= now_ms {
                    skipped_positions += 1;
                    continue;
                }

                let asset = match parse_asset(&serialized.asset) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!(error = %e, value = %serialized.asset, "Skipping persisted position with invalid asset");
                        skipped_positions += 1;
                        continue;
                    }
                };
                let timeframe = match parse_timeframe_label(&serialized.timeframe) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!(error = %e, value = %serialized.timeframe, "Skipping persisted position with invalid timeframe");
                        skipped_positions += 1;
                        continue;
                    }
                };
                let direction = match parse_direction_label(&serialized.direction) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!(error = %e, value = %serialized.direction, "Skipping persisted position with invalid direction");
                        skipped_positions += 1;
                        continue;
                    }
                };

                let restored = PaperPosition {
                    id: serialized.id.clone(),
                    asset,
                    timeframe,
                    direction,
                    entry_price: serialized.entry_price,
                    current_price: serialized.current_price,
                    size_usdc: serialized.size_usdc,
                    shares: serialized.shares,
                    fee_paid: serialized.fee_paid,
                    opened_at: serialized.opened_at,
                    market_slug: serialized.market_slug.clone(),
                    signal_id: serialized.signal_id.clone(),
                    confidence: serialized.confidence,
                    features_at_entry: None,
                    market_open_ts: serialized.market_open_ts,
                    market_close_ts: serialized.market_close_ts,
                    price_at_market_open: serialized.price_at_market_open,
                    peak_price: serialized.peak_price,
                    trough_price: serialized.trough_price,
                    stop_price: serialized.stop_price,
                    take_profit_price: serialized.take_profit_price,
                    unrealized_pnl: serialized.unrealized_pnl,
                    trail_pct: serialized.trail_pct,
                    share_price: serialized.share_price,
                    peak_share_price: serialized.peak_share_price,
                    is_winning: serialized.is_winning,
                    checkpoint_armed: serialized.checkpoint_armed,
                    checkpoint_peak_roi: serialized.checkpoint_peak_roi,
                    checkpoint_floor_roi: serialized.checkpoint_floor_roi,
                    checkpoint_breach_ticks: serialized.checkpoint_breach_ticks,
                    hard_stop_breach_ticks: serialized.hard_stop_breach_ticks,
                    dynamic_hard_stop_roi: serialized.dynamic_hard_stop_roi,
                    indicators_used: serialized.indicators_used.clone(),
                    token_id: serialized.token_id.clone(),
                    outcome: serialized.outcome.clone(),
                    entry_bid: serialized.entry_bid,
                    entry_ask: serialized.entry_ask,
                    entry_mid: serialized.entry_mid,
                    p_market: serialized.p_market,
                    p_model: serialized.p_model,
                    edge_net: serialized.edge_net,
                    kelly_raw: serialized.kelly_raw,
                    kelly_applied: serialized.kelly_applied,
                };

                positions_map.insert((asset, timeframe), restored.clone());
                restored_positions.push(restored);
            }
        }

        // Rebuild per-window direction bias from restored open positions.
        {
            let mut bias_map = self.window_bias.write().unwrap();
            bias_map.clear();
            for pos in &restored_positions {
                bias_map.insert((pos.asset, pos.timeframe, pos.market_open_ts), pos.direction);
            }
        }

        // Hard-stop windows are runtime-only safeguards; start clean on reboot.
        self.stopped_windows.write().unwrap().clear();

        info!(
            restored_positions = restored_positions.len(),
            skipped_positions,
            "💾 [PAPER] Positions restored from persisted state"
        );
    }

    // ── Balance ─────────────────────────────────────────────────

    pub fn get_balance(&self) -> f64 {
        *self.balance.read().unwrap()
    }

    pub fn get_locked_balance(&self) -> f64 {
        self.positions
            .read()
            .unwrap()
            .values()
            .map(|p| p.size_usdc)
            .sum()
    }

    pub fn get_total_equity(&self) -> f64 {
        let balance = self.get_balance();
        // Use unrealized P&L from share price model for accurate equity
        // Each position's current value = size_usdc + unrealized_pnl
        let position_value: f64 = self
            .positions
            .read()
            .unwrap()
            .values()
            .map(|p| (p.size_usdc + p.unrealized_pnl).max(0.0))
            .sum();
        balance + position_value
    }

    // ── Market window calculation ───────────────────────────────

    /// Calculate the market window (open, close) for a given timeframe.
    /// Polymarket crypto markets run on fixed windows:
    ///   - 15m: :00-:15, :15-:30, :30-:45, :45-:00
    ///   - 1h:  :00-:00 (full hours)
    ///
    /// If the signal has a valid `expires_at`, use that as close.
    /// Otherwise, calculate from current time + timeframe.
    fn calculate_market_window(timeframe: Timeframe, signal_expires_at: i64) -> (i64, i64) {
        let now_ms = Utc::now().timestamp_millis();

        // If signal already has a valid market expiry, use it
        if signal_expires_at > now_ms {
            let duration_ms = timeframe.duration_secs() as i64 * 1000;
            let market_open = signal_expires_at - duration_ms;
            return (market_open, signal_expires_at);
        }

        // Calculate from current time - snap to the current window
        let now_secs = now_ms / 1000;
        let window_secs = timeframe.duration_secs() as i64;
        let window_start_secs = (now_secs / window_secs) * window_secs;
        let window_end_secs = window_start_secs + window_secs;

        (window_start_secs * 1000, window_end_secs * 1000)
    }

    /// Progress within the market window [0.0, 1.0].
    fn market_progress_ratio(pos: &PaperPosition, now_ms: i64) -> f64 {
        if pos.market_close_ts > pos.market_open_ts {
            let span = (pos.market_close_ts - pos.market_open_ts) as f64;
            return ((now_ms - pos.market_open_ts) as f64 / span).clamp(0.0, 1.0);
        }
        if pos.market_close_ts > pos.opened_at {
            let span = (pos.market_close_ts - pos.opened_at) as f64;
            return ((now_ms - pos.opened_at) as f64 / span).clamp(0.0, 1.0);
        }
        0.5
    }

    /// Dynamic hard-stop threshold.
    ///
    /// Base comes from `config.hard_stop_roi`, then adapts using:
    /// - real-time spread (wider spread => wider stop to avoid microstructure noise)
    /// - intratrade share volatility (more noise => wider stop)
    /// - low-volatility tightening (tiny moves => cut losers earlier)
    /// - time-to-expiry (near expiry => tighter stop)
    fn dynamic_hard_stop_roi(
        &self,
        pos: &PaperPosition,
        now_ms: i64,
        bid_share: f64,
        ask_share: f64,
    ) -> f64 {
        let base = self.config.hard_stop_roi.min(-0.01).max(-0.50);
        let progress = Self::market_progress_ratio(pos, now_ms);
        let early_phase = (1.0 - progress).clamp(0.0, 1.0);

        let bid = bid_share.clamp(0.01, 0.99);
        let ask = ask_share.clamp(0.01, 0.99);
        let spread = (ask - bid).max(0.0);
        let entry = pos.share_price.max(0.01);

        // Range relative to entry approximates local volatility in probability space.
        let excursion = ((pos.peak_share_price - pos.share_price).abs() / entry).clamp(0.0, 0.30);
        let retrace = ((pos.peak_share_price - bid).max(0.0) / entry).clamp(0.0, 0.30);
        let local_range = excursion.max(retrace);

        let (
            vol_scale,
            spread_scale,
            low_vol_ref,
            low_vol_tighten,
            late_tighten,
            min_floor,
            max_floor,
        ) = match pos.timeframe {
            Timeframe::Min15 => (0.45, 0.40, 0.06, 0.030, 0.03, -0.16, -0.04),
            Timeframe::Hour1 => (0.30, 0.25, 0.08, 0.022, 0.02, -0.20, -0.05),
        };

        let widen_vol = (local_range * vol_scale * early_phase).clamp(0.0, 0.06);
        let widen_spread = (spread * spread_scale).clamp(0.0, 0.03);
        let low_vol_factor = ((low_vol_ref - local_range) / low_vol_ref).clamp(0.0, 1.0);
        let tighten_low_vol =
            (low_vol_factor * low_vol_tighten * (0.5 + progress * 0.5)).clamp(0.0, 0.035);

        let mut dynamic = base - widen_vol - widen_spread;
        dynamic += tighten_low_vol;

        // As expiry approaches, cut losers faster.
        if progress >= 0.80 {
            dynamic += late_tighten;
        } else if progress >= 0.60 {
            dynamic += late_tighten * 0.5;
        }

        dynamic.clamp(min_floor, max_floor)
    }

    /// Dynamic checkpoint parameters for take-profit.
    ///
    /// Keeps the checkpoint mechanism, but adapts arm/floor/gap to:
    /// - low-volatility markets (arms earlier)
    /// - wide spreads (slightly less sensitive to avoid churn)
    /// - late phase of market window (locks gains sooner)
    fn dynamic_checkpoint_params(
        &self,
        pos: &PaperPosition,
        now_ms: i64,
        bid_share: f64,
        ask_share: f64,
    ) -> (f64, f64, f64) {
        let progress = Self::market_progress_ratio(pos, now_ms);
        let bid = bid_share.clamp(0.01, 0.99);
        let ask = ask_share.clamp(0.01, 0.99);
        let spread = (ask - bid).max(0.0).clamp(0.0, 0.10);
        let entry = pos.share_price.max(0.01);

        let favorable_move = ((pos.peak_share_price - entry).max(0.0) / entry).clamp(0.0, 0.40);
        let retrace = ((pos.peak_share_price - bid).max(0.0) / entry).clamp(0.0, 0.40);

        let base_arm = self.config.checkpoint_arm_roi.max(0.005);
        let base_floor = self.config.checkpoint_initial_floor_roi.max(0.003);
        let base_gap = self.config.checkpoint_trail_gap_roi.max(0.002);

        let (
            abs_arm_min,
            abs_floor_min,
            abs_gap_min,
            low_vol_ref,
            low_vol_scale,
            late_tighten,
            spread_relax,
        ) = match pos.timeframe {
            Timeframe::Min15 => (0.020, 0.009, 0.005, 0.07, 0.45, 0.25, 0.08),
            Timeframe::Hour1 => (0.028, 0.012, 0.007, 0.09, 0.35, 0.20, 0.06),
        };

        let arm_min = (base_arm * 0.45).max(abs_arm_min);
        let floor_min = (base_floor * 0.40).max(abs_floor_min);
        let gap_min = (base_gap * 0.55).max(abs_gap_min);

        let low_vol_factor = ((low_vol_ref - favorable_move) / low_vol_ref).clamp(0.0, 1.0);
        let late_factor = if progress >= 0.85 {
            1.0
        } else if progress >= 0.60 {
            ((progress - 0.60) / 0.25).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let mut arm = base_arm * (1.0 - low_vol_scale * low_vol_factor);
        let mut floor = base_floor * (1.0 - (low_vol_scale * 0.90) * low_vol_factor);
        let mut gap = base_gap * (1.0 - (low_vol_scale * 0.55) * low_vol_factor);

        arm *= 1.0 - late_tighten * late_factor;
        floor *= 1.0 - (late_tighten * 0.85) * late_factor;
        gap *= 1.0 - (late_tighten * 0.65) * late_factor;

        // Protect gains faster when we've retraced from the local peak.
        let retrace_factor = (retrace / 0.05).clamp(0.0, 1.0);
        floor *= 1.0 - 0.15 * retrace_factor;
        gap *= 1.0 - 0.20 * retrace_factor;

        // Wide spread => slightly relax thresholds to reduce microstructure churn.
        arm += spread * spread_relax;
        floor += spread * spread_relax * 0.8;
        gap += spread * spread_relax * 0.5;

        arm = arm.clamp(arm_min, base_arm.max(arm_min));
        floor = floor.clamp(floor_min, base_floor.max(floor_min));
        gap = gap.clamp(gap_min, base_gap.max(gap_min));

        // Keep floor safely below arm.
        let floor_cap = (arm - gap_min * 0.5).max(floor_min);
        if floor > floor_cap {
            floor = floor_cap;
        }

        (arm, floor, gap)
    }

    // ── Price updates ──────────────────────────────────────────

    pub fn update_price(
        &self,
        asset: Asset,
        price: f64,
        source: PriceSource,
    ) -> Vec<((Asset, Timeframe), PaperExitReason)> {
        // Store the latest price
        if let Ok(mut prices) = self.latest_prices.write() {
            prices.insert((asset, source), price);
        }

        let mut exits = Vec::new();

        // Prefer Chainlink for position updates (matches Polymarket resolution),
        // but fallback to any source if Chainlink hasn't sent data for this asset.
        if self.config.prefer_chainlink && source != PriceSource::RtdsChainlink {
            let has_chainlink = self
                .latest_prices
                .read()
                .map(|p| p.contains_key(&(asset, PriceSource::RtdsChainlink)))
                .unwrap_or(false);
            if has_chainlink {
                return exits;
            }
        }

        if let Ok(mut positions) = self.positions.write() {
            for ((pos_asset, pos_timeframe), pos) in positions.iter_mut() {
                if *pos_asset != asset {
                    continue;
                }

                pos.current_price = price;
                if price > pos.peak_price {
                    pos.peak_price = price;
                }
                if price < pos.trough_price {
                    pos.trough_price = price;
                }

                let now = Utc::now().timestamp_millis();

                // Resolution close has highest priority.
                if pos.market_close_ts > 0 && now >= pos.market_close_ts {
                    exits.push(((*pos_asset, *pos_timeframe), PaperExitReason::MarketExpiry));
                    continue;
                }

                let direction_str = match pos.direction {
                    Direction::Up => "UP",
                    Direction::Down => "DOWN",
                };

                // Use bid as liquidation price (sell-early executable), mid for edge estimates.
                let (bid_share, ask_share, mid_share) = if let Some(ref share_prices) =
                    self.polymarket_share_prices
                {
                    if let Some(q) = share_prices.get_quote(pos.asset, pos.timeframe, direction_str)
                    {
                        (q.bid, q.ask, q.mid)
                    } else {
                        let sim = self.calculate_simulated_share_price(pos, price);
                        (sim, sim, sim)
                    }
                } else {
                    let sim = self.calculate_simulated_share_price(pos, price);
                    (sim, sim, sim)
                };

                let sell_share_price = bid_share.clamp(0.01, 0.99);
                pos.is_winning = sell_share_price > pos.share_price;

                if sell_share_price > pos.peak_share_price {
                    pos.peak_share_price = sell_share_price;
                }

                // Net liquidation value if sold now, including estimated close fee.
                let gross_return = pos.shares * sell_share_price;
                let close_fee = gross_return * fee_rate_from_price(sell_share_price);
                let net_return = (gross_return - close_fee).max(0.0);
                pos.unrealized_pnl = net_return - pos.size_usdc;

                let roi = if pos.size_usdc > 0.0 {
                    pos.unrealized_pnl / pos.size_usdc
                } else {
                    0.0
                };

                // Dynamic checkpoint trailing: adapts arm/floor/gap to volatility + time.
                let (arm_roi, floor_base, trail_gap) =
                    self.dynamic_checkpoint_params(pos, now, sell_share_price, ask_share);

                if !pos.checkpoint_armed && roi >= arm_roi {
                    pos.checkpoint_armed = true;
                    pos.checkpoint_peak_roi = roi;
                    pos.checkpoint_floor_roi = floor_base;
                    pos.checkpoint_breach_ticks = 0;
                }

                if pos.checkpoint_armed {
                    if roi > pos.checkpoint_peak_roi {
                        pos.checkpoint_peak_roi = roi;
                        let dynamic_floor = (pos.checkpoint_peak_roi - trail_gap).max(floor_base);
                        if dynamic_floor > pos.checkpoint_floor_roi {
                            pos.checkpoint_floor_roi = dynamic_floor;
                        }
                    }

                    if roi <= pos.checkpoint_floor_roi {
                        pos.checkpoint_breach_ticks = pos.checkpoint_breach_ticks.saturating_add(1);
                    } else {
                        pos.checkpoint_breach_ticks = 0;
                    }

                    // Two consecutive breaches to avoid noise exits.
                    if pos.checkpoint_breach_ticks >= 2 {
                        exits.push((
                            (*pos_asset, *pos_timeframe),
                            PaperExitReason::CheckpointTakeProfit,
                        ));
                        continue;
                    }
                }

                let dynamic_hard_stop_roi =
                    self.dynamic_hard_stop_roi(pos, now, sell_share_price, ask_share);
                pos.dynamic_hard_stop_roi = dynamic_hard_stop_roi;
                if roi <= dynamic_hard_stop_roi {
                    pos.hard_stop_breach_ticks = pos.hard_stop_breach_ticks.saturating_add(1);
                } else {
                    pos.hard_stop_breach_ticks = 0;
                }

                // Require two consecutive breaches to avoid stop-outs on one noisy tick.
                if pos.hard_stop_breach_ticks >= 2 {
                    exits.push(((*pos_asset, *pos_timeframe), PaperExitReason::HardStop));
                    continue;
                }

                if pos.market_close_ts > 0 {
                    let secs_to_expiry = ((pos.market_close_ts - now) / 1000).max(0);
                    if secs_to_expiry <= self.config.time_stop_seconds_to_expiry {
                        let p_market = mid_share.clamp(0.01, 0.99);
                        let p_model = pos.confidence.clamp(0.01, 0.99);
                        let spread = (ask_share - bid_share).max(0.0);
                        let fee_rate = fee_rate_from_price(p_market);
                        let ev = estimate_expected_value(
                            p_market, p_model, p_market, fee_rate, spread, 0.005,
                        );
                        if ev.edge_net <= 0.0 {
                            exits.push(((*pos_asset, *pos_timeframe), PaperExitReason::TimeStop));
                            continue;
                        }
                    }
                }

                // Fallback max hold when market expiry is unknown.
                if pos.market_close_ts == 0
                    && now - pos.opened_at > self.config.max_hold_duration_ms
                {
                    exits.push(((*pos_asset, *pos_timeframe), PaperExitReason::TimeExpiry));
                }
            }
        }

        exits
    }

    // Execute signal
    pub fn execute_signal(&self, signal: &Signal) -> Result<bool> {
        if signal.token_id.trim().is_empty() {
            warn!(
                asset = %signal.asset,
                timeframe = ?signal.timeframe,
                market_slug = %signal.market_slug,
                "[PAPER] Rejecting signal without token_id"
            );
            return Ok(false);
        }
        if signal.market_slug.trim().is_empty() {
            warn!(
                asset = %signal.asset,
                timeframe = ?signal.timeframe,
                token_id = %signal.token_id,
                "[PAPER] Rejecting signal without market_slug"
            );
            return Ok(false);
        }

        let balance = self.get_balance();
        let now = Utc::now().timestamp_millis();

        // Safety guard: reject mismatched expiry horizons (e.g. a 15m lane carrying a 20m+ expiry).
        if signal.expires_at > now {
            let secs_to_expiry = ((signal.expires_at - now) / 1000).max(0);
            let max_allowed_secs = match signal.timeframe {
                Timeframe::Min15 => (15 * 60) + 90,
                Timeframe::Hour1 => (60 * 60) + 300,
            };
            if secs_to_expiry > max_allowed_secs {
                warn!(
                    asset = %signal.asset,
                    timeframe = ?signal.timeframe,
                    expires_in_secs = secs_to_expiry,
                    max_allowed_secs = max_allowed_secs,
                    market_slug = %signal.market_slug,
                    "[PAPER] Rejecting signal with expiry horizon inconsistent with timeframe"
                );
                return Ok(false);
            }
        }

        // Market window for conflict control and timing checks.
        let (market_open_ts, market_close_ts) =
            Self::calculate_market_window(signal.timeframe, signal.expires_at);
        // Do not open positions for future windows; 15m/1h lane must trade the current active window.
        if market_open_ts > now + 15_000 {
            warn!(
                asset = %signal.asset,
                timeframe = ?signal.timeframe,
                market_slug = %signal.market_slug,
                market_open_ts = market_open_ts,
                now_ts = now,
                "[PAPER] Rejecting signal for a market window that has not started yet"
            );
            return Ok(false);
        }

        // Remove stale window bias entries.
        if let Ok(mut bias_map) = self.window_bias.write() {
            let cutoff = now - (Timeframe::Hour1.duration_secs() as i64 * 1000 * 2);
            bias_map.retain(|(_, _, window_start), _| *window_start >= cutoff);
        }

        let bias_key = (signal.asset, signal.timeframe, market_open_ts);
        {
            let bias_map = self.window_bias.read().unwrap();
            if let Some(existing) = bias_map.get(&bias_key) {
                if *existing != signal.direction {
                    warn!(
                        asset = %signal.asset,
                        timeframe = ?signal.timeframe,
                        market_open_ts = market_open_ts,
                        existing = ?existing,
                        incoming = ?signal.direction,
                        "[PAPER] Opposite direction blocked for same market window"
                    );
                    return Ok(false);
                }
            }
        }

        // Block re-entry after a hard stop in the same window.
        {
            if let Ok(stopped) = self.stopped_windows.read() {
                if stopped.contains(&bias_key) {
                    warn!(
                        asset = %signal.asset,
                        timeframe = ?signal.timeframe,
                        market_open_ts = market_open_ts,
                        "[PAPER] Re-entry blocked: hard stop already occurred in this window"
                    );
                    return Ok(false);
                }
            }
        }

        // Check duplicate open position in same market key.
        {
            let positions = self.positions.read().unwrap();
            let market_key = (signal.asset, signal.timeframe);
            if positions.contains_key(&market_key) {
                info!(
                    asset = %signal.asset,
                    timeframe = ?signal.timeframe,
                    "[PAPER] Already have position for market, skipping"
                );
                return Ok(false);
            }
        }

        // Available balance accounts for locked capital.
        let locked: f64 = self
            .positions
            .read()
            .unwrap()
            .values()
            .map(|p| p.size_usdc)
            .sum();
        let available_balance = (balance - locked).max(0.0);
        if available_balance < 1.0 {
            warn!(
                balance = balance,
                locked = locked,
                available = available_balance,
                "[PAPER] Insufficient available balance"
            );
            return Ok(false);
        }

        let base_price = self.get_best_price(signal.asset).unwrap_or(0.0);
        if base_price <= 0.0 {
            warn!(asset = %signal.asset, "[PAPER] No price available");
            return Ok(false);
        }

        let slippage_mult = self.config.slippage_bps / 10_000.0;
        let fill_price = match signal.direction {
            Direction::Up => base_price * (1.0 + slippage_mult),
            Direction::Down => base_price * (1.0 - slippage_mult),
        };

        let direction_str = match signal.direction {
            Direction::Up => "UP",
            Direction::Down => "DOWN",
        };

        // Entry quote: buy on ask, use mid for market probability.
        // Polymarket-native guardrail: never infer entry price from confidence.
        let (
            entry_bid,
            entry_ask,
            entry_mid,
            entry_bid_size,
            entry_ask_size,
            entry_depth_top5,
            quote_source,
        ): (f64, f64, f64, f64, f64, f64, String) = {
            if let Some(ref share_prices) = self.polymarket_share_prices {
                if let Some(q) =
                    share_prices.get_quote(signal.asset, signal.timeframe, direction_str)
                {
                    (
                        q.bid.clamp(0.01, 0.99),
                        q.ask.clamp(0.01, 0.99),
                        q.mid.clamp(0.01, 0.99),
                        q.bid_size.max(0.0),
                        q.ask_size.max(0.0),
                        q.depth_top5.max(0.0),
                        "orderbook".to_string(),
                    )
                } else if signal.quote_bid > 0.0
                    && signal.quote_ask > 0.0
                    && signal.quote_mid > 0.0
                {
                    (
                        signal.quote_bid.clamp(0.01, 0.99),
                        signal.quote_ask.clamp(0.01, 0.99),
                        signal.quote_mid.clamp(0.01, 0.99),
                        0.0,
                        0.0,
                        signal.quote_depth_top5.max(0.0),
                        "signal_quote_hint".to_string(),
                    )
                } else {
                    warn!(
                        asset = %signal.asset,
                        timeframe = ?signal.timeframe,
                        token_id = %signal.token_id,
                        direction = direction_str,
                        "[PAPER] Missing orderbook quote and no quote hint; skipping signal"
                    );
                    return Ok(false);
                }
            } else if signal.quote_bid > 0.0 && signal.quote_ask > 0.0 && signal.quote_mid > 0.0 {
                (
                    signal.quote_bid.clamp(0.01, 0.99),
                    signal.quote_ask.clamp(0.01, 0.99),
                    signal.quote_mid.clamp(0.01, 0.99),
                    0.0,
                    0.0,
                    signal.quote_depth_top5.max(0.0),
                    "signal_quote_hint".to_string(),
                )
            } else {
                warn!(
                    asset = %signal.asset,
                    timeframe = ?signal.timeframe,
                    token_id = %signal.token_id,
                    "[PAPER] Missing share price provider and no quote hint; skipping signal"
                );
                return Ok(false);
            }
        };

        let share_price = entry_ask.clamp(0.01, 0.99);
        let p_market = entry_mid.clamp(0.01, 0.99);
        let p_model = signal.confidence.clamp(0.01, 0.99);
        let spread = (entry_ask - entry_bid).max(0.0);
        let max_spread = match signal.timeframe {
            Timeframe::Min15 => 0.15,
            Timeframe::Hour1 => 0.25,
        };
        if spread > max_spread {
            info!(
                asset = %signal.asset,
                timeframe = ?signal.timeframe,
                token_id = %signal.token_id,
                spread = spread,
                max_spread = max_spread,
                "[PAPER] Spread too wide, skipping signal"
            );
            return Ok(false);
        }

        let min_depth_top5 = 0.0;
        let top_of_book_depth = entry_bid_size + entry_ask_size;
        let usable_depth = if entry_depth_top5 > 0.0 {
            entry_depth_top5
        } else {
            top_of_book_depth
        };
        if usable_depth > 0.0 && usable_depth < min_depth_top5 {
            info!(
                asset = %signal.asset,
                timeframe = ?signal.timeframe,
                token_id = %signal.token_id,
                depth = usable_depth,
                min_depth_top5 = min_depth_top5,
                "[PAPER] Depth too low, skipping signal"
            );
            return Ok(false);
        }

        let fee_rate = fee_rate_from_price(share_price);
        let ev = estimate_expected_value(p_market, p_model, share_price, fee_rate, spread, 0.005);
        if ev.edge_net <= self.config.min_edge_net {
            info!(
                asset = %signal.asset,
                timeframe = ?signal.timeframe,
                edge_net = ev.edge_net,
                min_edge_net = self.config.min_edge_net,
                p_market = p_market,
                p_model = p_model,
                "[PAPER] Signal skipped due to edge below configured floor"
            );
            return Ok(false);
        }

        // Kelly sizing with timeframe-specific fractions/caps.
        let (kelly_fraction, kelly_cap) = match signal.timeframe {
            Timeframe::Min15 => (self.config.kelly_fraction_15m, self.config.kelly_cap_15m),
            Timeframe::Hour1 => (self.config.kelly_fraction_1h, self.config.kelly_cap_1h),
        };
        let kelly_quote =
            compute_fractional_kelly(p_model, 0.05, share_price, kelly_fraction, kelly_cap);

        let max_per_trade = available_balance * 0.15;
        let max_total = available_balance * 0.50;
        let kelly_size = available_balance * kelly_quote.f_fractional;

        // Kelly-only sizing: if Kelly says 0 (negative edge after uncertainty), reject.
        if self.config.kelly_enabled && kelly_quote.f_fractional <= 0.0 {
            info!(
                asset = %signal.asset,
                timeframe = ?signal.timeframe,
                p_model = p_model,
                p_market = p_market,
                kelly_raw = kelly_quote.f_raw,
                kelly_adj_p = kelly_quote.p_adj,
                "[PAPER] Kelly fraction is zero — no positive expected value; rejecting trade"
            );
            return Ok(false);
        }

        let sized = if self.config.kelly_enabled && kelly_size >= 1.0 {
            kelly_size
        } else if !self.config.kelly_enabled {
            // Fallback only when Kelly is disabled entirely
            signal.suggested_size_usdc.max(0.0)
        } else {
            // Kelly is enabled but size < $1 — too small, reject
            info!(
                asset = %signal.asset,
                timeframe = ?signal.timeframe,
                kelly_size = kelly_size,
                kelly_fraction = kelly_quote.f_fractional,
                bankroll = available_balance,
                "[PAPER] Kelly size below $1 minimum; rejecting trade"
            );
            return Ok(false);
        };
        let size_usdc = sized.min(max_per_trade).min(max_total);

        if size_usdc < 1.0 {
            warn!(
                available = available_balance,
                kelly_size = kelly_size,
                chosen = size_usdc,
                "[PAPER] Position size below minimum"
            );
            return Ok(false);
        }

        let fee_open = size_usdc * fee_rate_from_price(share_price);
        let effective_size = (size_usdc - fee_open).max(0.0);
        if effective_size <= 0.0 {
            warn!(
                size_usdc = size_usdc,
                fee_open = fee_open,
                "[PAPER] Effective size is zero after fee"
            );
            return Ok(false);
        }

        let shares = effective_size / share_price;
        if shares <= 0.0 {
            warn!("[PAPER] Computed shares <= 0, skipping");
            return Ok(false);
        }

        let time_remaining_secs = (market_close_ts - now) / 1000;
        let window_duration_secs = match signal.timeframe {
            Timeframe::Min15 => 900,
            Timeframe::Hour1 => 3600,
        };
        let min_remaining = 15;
        if time_remaining_secs < min_remaining {
            warn!(
                asset = %signal.asset,
                timeframe = ?signal.timeframe,
                remaining_secs = time_remaining_secs,
                min_required = min_remaining,
                window_duration_secs = window_duration_secs,
                "[PAPER] Skipping due to very low time remaining in market window"
            );
            return Ok(false);
        }

        let stop_price = match signal.direction {
            Direction::Up => fill_price * (1.0 + self.config.hard_stop_roi),
            Direction::Down => fill_price * (1.0 - self.config.hard_stop_roi),
        };

        let trade_id = format!("PT-{}-{}", signal.asset, now);
        let position = PaperPosition {
            id: trade_id.clone(),
            asset: signal.asset,
            timeframe: signal.timeframe,
            direction: signal.direction,
            entry_price: fill_price,
            current_price: fill_price,
            size_usdc: effective_size,
            shares,
            fee_paid: fee_open,
            opened_at: now,
            market_slug: signal.market_slug.clone(),
            signal_id: signal.id.clone(),
            confidence: signal.confidence,
            features_at_entry: Some(signal.features.clone()),
            market_open_ts,
            market_close_ts,
            price_at_market_open: base_price,
            peak_price: fill_price,
            trough_price: fill_price,
            stop_price,
            take_profit_price: fill_price,
            unrealized_pnl: 0.0,
            trail_pct: self.config.checkpoint_trail_gap_roi,
            share_price,
            peak_share_price: share_price,
            is_winning: false,
            checkpoint_armed: false,
            checkpoint_peak_roi: 0.0,
            checkpoint_floor_roi: 0.0,
            checkpoint_breach_ticks: 0,
            hard_stop_breach_ticks: 0,
            dynamic_hard_stop_roi: self.config.hard_stop_roi,
            indicators_used: signal.indicators_used.clone(),
            token_id: signal.token_id.clone(),
            outcome: direction_str.to_string(),
            entry_bid,
            entry_ask,
            entry_mid,
            p_market,
            p_model,
            edge_net: ev.edge_net,
            kelly_raw: kelly_quote.f_raw,
            kelly_applied: kelly_quote.f_fractional,
        };

        // Deduct committed funds.
        {
            let mut bal = self.balance.write().unwrap();
            *bal -= size_usdc;
        }

        {
            let mut positions = self.positions.write().unwrap();
            positions.insert((signal.asset, signal.timeframe), position);
        }

        {
            let mut bias_map = self.window_bias.write().unwrap();
            bias_map.insert(bias_key, signal.direction);
        }

        info!(
            trade_id = %trade_id,
            asset = %signal.asset,
            direction = ?signal.direction,
            quote_source = quote_source,
            asset_price = %format!("${:.2}", fill_price),
            share_bid = %format!("${:.3}", entry_bid),
            share_ask = %format!("${:.3}", entry_ask),
            share_mid = %format!("${:.3}", entry_mid),
            shares = %format!("{:.2}", shares),
            investment = %format!("${:.2}", effective_size),
            fee_open = %format!("${:.4}", fee_open),
            edge_net = ev.edge_net,
            kelly_fraction = kelly_quote.f_fractional,
            market_closes_in = %format!("{}s", time_remaining_secs),
            balance = %format!("${:.2}", self.get_balance()),
            "[PAPER] POSITION OPENED"
        );

        if let Err(e) = self.save_state() {
            warn!(error = %e, "Failed to save paper trading state");
        }

        Ok(true)
    }

    // Close position
    pub fn close_position(
        &self,
        asset: Asset,
        timeframe: Timeframe,
        reason: PaperExitReason,
    ) -> Option<PaperTradeRecord> {
        let position = {
            let mut positions = self.positions.write().ok()?;
            positions.remove(&(asset, timeframe))?
        };

        let now = Utc::now().timestamp_millis();
        let exit_price = position.current_price;

        let direction_str = match position.direction {
            Direction::Up => "UP",
            Direction::Down => "DOWN",
        };

        let (exit_bid, exit_ask, exit_mid) =
            if let Some(ref share_prices) = self.polymarket_share_prices {
                if let Some(q) =
                    share_prices.get_quote(position.asset, position.timeframe, direction_str)
                {
                    (
                        q.bid.clamp(0.01, 0.99),
                        q.ask.clamp(0.01, 0.99),
                        q.mid.clamp(0.01, 0.99),
                    )
                } else {
                    let sim = self
                        .calculate_simulated_share_price(&position, exit_price)
                        .clamp(0.01, 0.99);
                    (sim, sim, sim)
                }
            } else {
                let sim = self
                    .calculate_simulated_share_price(&position, exit_price)
                    .clamp(0.01, 0.99);
                (sim, sim, sim)
            };

        let sell_share_price = exit_bid.clamp(0.01, 0.99);
        let gross_sell = position.shares * sell_share_price;
        let estimated_fee_close = gross_sell * fee_rate_from_price(sell_share_price);

        let price_diff = match position.direction {
            Direction::Up => exit_price - position.entry_price,
            Direction::Down => position.entry_price - exit_price,
        };

        let (return_amount, pnl, fee_close, exit_reason_detail) = match reason {
            PaperExitReason::MarketExpiry => {
                // Binary payout at resolution.
                let is_win = price_diff > 0.0;
                if is_win {
                    let ret = position.shares;
                    (
                        ret,
                        ret - position.size_usdc,
                        0.0,
                        "market_expiry_resolution_win".to_string(),
                    )
                } else {
                    (
                        0.0,
                        -position.size_usdc,
                        0.0,
                        "market_expiry_resolution_loss".to_string(),
                    )
                }
            }
            PaperExitReason::CheckpointTakeProfit => {
                let ret = (gross_sell - estimated_fee_close).max(0.0);
                (
                    ret,
                    ret - position.size_usdc,
                    estimated_fee_close,
                    format!(
                        "checkpoint_floor_breach>=2 floor_roi={:.4} peak_roi={:.4}",
                        position.checkpoint_floor_roi, position.checkpoint_peak_roi
                    ),
                )
            }
            PaperExitReason::HardStop => {
                let ret = (gross_sell - estimated_fee_close).max(0.0);
                (
                    ret,
                    ret - position.size_usdc,
                    estimated_fee_close,
                    format!(
                        "dynamic_hard_stop_roi<={:.4} breaches={}",
                        position.dynamic_hard_stop_roi, position.hard_stop_breach_ticks
                    ),
                )
            }
            PaperExitReason::TimeStop => {
                let ret = (gross_sell - estimated_fee_close).max(0.0);
                (
                    ret,
                    ret - position.size_usdc,
                    estimated_fee_close,
                    format!(
                        "time_stop_tte<={}s edge_residual<=0",
                        self.config.time_stop_seconds_to_expiry
                    ),
                )
            }
            PaperExitReason::TimeExpiry | PaperExitReason::Manual => {
                let ret = (gross_sell - estimated_fee_close).max(0.0);
                (
                    ret,
                    ret - position.size_usdc,
                    estimated_fee_close,
                    "early_sell_no_resolution".to_string(),
                )
            }
        };

        let is_win = pnl >= 0.0;
        let pnl_pct = if position.size_usdc > 0.0 {
            (pnl / position.size_usdc) * 100.0
        } else {
            0.0
        };
        let result_str = if is_win { "WIN" } else { "LOSS" };

        // Record hard-stop windows to block re-entry in the same window.
        if matches!(reason, PaperExitReason::HardStop) {
            if let Ok(mut stopped) = self.stopped_windows.write() {
                stopped.insert((asset, timeframe, position.market_open_ts));
                // Clean up old entries (keep only recent 2 hours).
                let cutoff = now - (Timeframe::Hour1.duration_secs() as i64 * 1000 * 2);
                stopped.retain(|(_, _, ws)| *ws >= cutoff);
            }
        }

        {
            let mut bal = self.balance.write().unwrap();
            *bal += return_amount;
        }

        let hold_duration = now - position.opened_at;
        let balance_after = self.get_balance();

        // Update global stats.
        {
            let mut stats = self.stats.write().unwrap();
            stats.total_trades += 1;
            stats.total_pnl += pnl;
            stats.total_fees += position.fee_paid + fee_close;

            match reason {
                PaperExitReason::HardStop => stats.exits_trailing_stop += 1,
                PaperExitReason::CheckpointTakeProfit => stats.exits_take_profit += 1,
                PaperExitReason::MarketExpiry => stats.exits_market_expiry += 1,
                PaperExitReason::TimeStop | PaperExitReason::TimeExpiry => {
                    stats.exits_time_expiry += 1
                }
                PaperExitReason::Manual => {}
            }

            if pnl >= 0.0 {
                stats.wins += 1;
                stats.sum_win_pnl += pnl;
                stats.gross_profit += pnl;
                if pnl > stats.largest_win {
                    stats.largest_win = pnl;
                }
                stats.current_streak = if stats.current_streak >= 0 {
                    stats.current_streak + 1
                } else {
                    1
                };
                if stats.current_streak > stats.best_streak {
                    stats.best_streak = stats.current_streak;
                }
            } else {
                stats.losses += 1;
                stats.sum_loss_pnl += pnl;
                stats.gross_loss += pnl.abs();
                if pnl < stats.largest_loss {
                    stats.largest_loss = pnl;
                }
                stats.current_streak = if stats.current_streak <= 0 {
                    stats.current_streak - 1
                } else {
                    -1
                };
                if stats.current_streak < stats.worst_streak {
                    stats.worst_streak = stats.current_streak;
                }
            }

            if balance_after > stats.peak_balance {
                stats.peak_balance = balance_after;
            }
            let drawdown = if stats.peak_balance > 0.0 {
                (stats.peak_balance - balance_after) / stats.peak_balance * 100.0
            } else {
                0.0
            };
            if drawdown > stats.max_drawdown {
                stats.max_drawdown = drawdown;
            }
        }

        // Update per-asset stats.
        {
            let mut asset_map = self.asset_stats.write().unwrap();
            let a = asset_map
                .entry(position.asset)
                .or_insert_with(AssetStats::default);
            a.trades += 1;
            a.pnl += pnl;
            if pnl >= 0.0 {
                a.wins += 1;
                a.sum_confidence_win += position.confidence;
                a.avg_confidence_win = a.sum_confidence_win / a.wins as f64;
            } else {
                a.losses += 1;
                a.sum_confidence_loss += position.confidence;
                a.avg_confidence_loss = a.sum_confidence_loss / a.losses as f64;
            }
        }

        let time_remaining_at_entry = (position.market_close_ts - position.opened_at) / 1000;
        let market_id = if position.market_slug.is_empty() {
            format!("{:?}-{}", position.asset, position.timeframe)
        } else {
            position.market_slug.clone()
        };

        let record = PaperTradeRecord {
            timestamp: now,
            trade_id: position.id.clone(),
            signal_id: position.signal_id.clone(),
            asset: format!("{:?}", position.asset),
            timeframe: format!("{}", position.timeframe),
            direction: format!("{:?}", position.direction),
            confidence: position.confidence,
            entry_price: position.entry_price,
            exit_price,
            size_usdc: position.size_usdc,
            shares: position.shares,
            fee_paid: position.fee_paid + fee_close,
            pnl,
            pnl_pct,
            result: result_str.to_string(),
            exit_reason: reason.to_string(),
            hold_duration_ms: hold_duration,
            balance_after,
            market_open_ts: position.market_open_ts,
            market_close_ts: position.market_close_ts,
            time_remaining_at_entry_secs: time_remaining_at_entry,
            indicators_used: position.indicators_used.clone(),
            market_id,
            token_id: if position.token_id.is_empty() {
                position.id.clone()
            } else {
                position.token_id.clone()
            },
            outcome: position.outcome.clone(),
            entry_bid: position.entry_bid,
            entry_ask: position.entry_ask,
            entry_mid: position.entry_mid,
            exit_bid,
            exit_ask,
            exit_mid,
            fee_open: position.fee_paid,
            fee_close,
            slippage_open: (position.entry_ask - position.entry_mid).max(0.0),
            slippage_close: (exit_mid - exit_bid).max(0.0),
            p_market: position.p_market,
            p_model: position.p_model,
            edge_net: position.edge_net,
            kelly_raw: position.kelly_raw,
            kelly_applied: position.kelly_applied,
            exit_reason_detail,
        };

        {
            let mut history = self.trade_history.write().unwrap();
            history.push(record.clone());
        }

        let emoji = if pnl >= 0.0 { "OK" } else { "X" };
        let stats = self.stats.read().unwrap();
        let wr = if stats.total_trades > 0 {
            (stats.wins as f64 / stats.total_trades as f64) * 100.0
        } else {
            0.0
        };

        info!(
            trade_id = %position.id,
            asset = %position.asset,
            direction = ?position.direction,
            entry_price = %format!("${:.2}", position.entry_price),
            exit_price = %format!("${:.2}", exit_price),
            shares = %format!("{:.2}", position.shares),
            entry_share_mid = %format!("${:.3}", position.entry_mid),
            exit_share_bid = %format!("${:.3}", exit_bid),
            return_amount = %format!("${:.2}", return_amount),
            pnl = %format!("${:+.2}", pnl),
            pnl_pct = %format!("{:+.1}%", pnl_pct),
            result = %result_str,
            reason = %reason,
            hold_secs = hold_duration / 1000,
            balance = %format!("${:.2}", balance_after),
            winrate = %format!("{:.1}%", wr),
            total_trades = stats.total_trades,
            "[PAPER] {} {} | PnL ${:+.2} ({:+.1}%) | WR {:.1}% ({}/{})",
            emoji,
            result_str,
            pnl,
            pnl_pct,
            wr,
            stats.wins,
            stats.total_trades
        );

        if let Some(ref callback) = self.calibration_callback {
            let is_win = pnl >= 0.0;
            callback(
                position.asset,
                position.timeframe,
                &position.indicators_used,
                is_win,
                position.p_model,
            );
            info!(
                asset = %position.asset,
                is_win = is_win,
                indicators = ?position.indicators_used,
                "[CALIBRATION] Trade result recorded for indicator calibration"
            );
        }

        drop(stats);
        if let Err(e) = self.save_state() {
            warn!(error = %e, "Failed to save paper trading state");
        }

        Some(record)
    }
    // ── Price helpers ────────────────────────────────────────────

    pub fn get_best_price(&self, asset: Asset) -> Option<f64> {
        let prices = self.latest_prices.read().ok()?;
        if self.config.prefer_chainlink {
            if let Some(&p) = prices.get(&(asset, PriceSource::RtdsChainlink)) {
                return Some(p);
            }
        }
        if let Some(&p) = prices.get(&(asset, PriceSource::RTDS)) {
            return Some(p);
        }
        if self.config.native_only {
            return None;
        }
        if let Some(&p) = prices.get(&(asset, PriceSource::Binance)) {
            return Some(p);
        }
        if let Some(&p) = prices.get(&(asset, PriceSource::Bybit)) {
            return Some(p);
        }
        if let Some(&p) = prices.get(&(asset, PriceSource::Coinbase)) {
            return Some(p);
        }
        None
    }

    // ── Dashboard ───────────────────────────────────────────────

    pub fn maybe_print_dashboard(&self) -> bool {
        let now = Utc::now().timestamp_millis();
        let interval_ms = (self.config.dashboard_interval_secs * 1000) as i64;
        let should_print = {
            let last = *self.last_dashboard.read().unwrap();
            now - last >= interval_ms
        };
        if should_print {
            self.print_dashboard();
            if let Ok(mut last) = self.last_dashboard.write() {
                *last = now;
            }
            true
        } else {
            false
        }
    }

    pub fn print_dashboard(&self) {
        let balance = self.get_balance();
        let locked = self.get_locked_balance();
        let equity = self.get_total_equity();
        let stats = self.stats.read().unwrap().clone();
        let positions = self.positions.read().unwrap();
        let asset_stats = self.asset_stats.read().unwrap();

        let wr = if stats.total_trades > 0 {
            (stats.wins as f64 / stats.total_trades as f64) * 100.0
        } else {
            0.0
        };
        let initial = self.config.initial_balance;
        let total_return = ((equity - initial) / initial) * 100.0;
        let avg_win = if stats.wins > 0 {
            stats.sum_win_pnl / stats.wins as f64
        } else {
            0.0
        };
        let avg_loss = if stats.losses > 0 {
            stats.sum_loss_pnl / stats.losses as f64
        } else {
            0.0
        };
        let profit_factor = if stats.gross_loss > 0.0 {
            stats.gross_profit / stats.gross_loss
        } else {
            0.0
        };

        // Calculate expected value: EV = (winrate * avg_win) - ((1-winrate) * avg_loss_abs)
        let avg_loss_abs = if stats.losses > 0 {
            stats.sum_loss_pnl.abs() / stats.losses as f64
        } else {
            0.0
        };
        let wr_dec = wr / 100.0;
        let ev = if stats.total_trades > 0 {
            (wr_dec * avg_win) - ((1.0 - wr_dec) * avg_loss_abs)
        } else {
            0.0
        };

        info!("╔══════════════════════════════════════════════════════════════════════════╗");
        info!("║  📋 POLYMARKET PAPER TRADING DASHBOARD                                  ║");
        info!("║  Binary Prediction Model: BTC/ETH only | 15m + 1h windows               ║");
        info!("╠══════════════════════════════════════════════════════════════════════════╣");
        info!(
            "║  💰 Balance: ${:.2} | Locked: ${:.2} | Equity: ${:.2} (initial: ${:.2})",
            balance, locked, equity, initial
        );
        info!(
            "║  📈 P&L: ${:+.2} ({:+.2}%) | Fees: ${:.4}",
            stats.total_pnl, total_return, stats.total_fees
        );
        info!(
            "║  🎯 WINRATE: {:.1}% ({} W / {} L / {} total) | Streak: {} | EV/trade: ${:+.4}",
            wr, stats.wins, stats.losses, stats.total_trades, stats.current_streak, ev
        );
        info!(
            "║  📊 Avg Win: ${:+.4} | Avg Loss: ${:+.4} | Profit Factor: {:.2}",
            avg_win, avg_loss, profit_factor
        );
        info!(
            "║  📊 Best: ${:+.4} | Worst: ${:+.4} | Max DD: {:.2}%",
            stats.largest_win, stats.largest_loss, stats.max_drawdown
        );
        info!(
            "║  🚪 Exits → Market: {} | TP: {} | SL: {} | Time: {}",
            stats.exits_market_expiry,
            stats.exits_take_profit,
            stats.exits_trailing_stop,
            stats.exits_time_expiry
        );

        // Per-asset breakdown
        if !asset_stats.is_empty() {
            info!("║  ─── Per-Asset Breakdown ───");
            for (asset, a) in asset_stats.iter() {
                let awr = if a.trades > 0 {
                    (a.wins as f64 / a.trades as f64) * 100.0
                } else {
                    0.0
                };
                info!(
                    "║  {:>4}: {:.1}% WR ({}/{}) P&L: ${:+.4} | AvgConfWin: {:.2} AvgConfLoss: {:.2}",
                    asset, awr, a.wins, a.trades, a.pnl, a.avg_confidence_win, a.avg_confidence_loss
                );
            }
        }

        // Open positions
        if !positions.is_empty() {
            info!("║  ─── Open Positions ───");
            let now = Utc::now().timestamp_millis();
            for ((asset, _), pos) in positions.iter() {
                let time_left = (pos.market_close_ts - now) / 1000;
                let time_left_str = if time_left > 0 {
                    format!("{}m{}s", time_left / 60, time_left % 60)
                } else {
                    "RESOLVING".to_string()
                };
                let status = if pos.is_winning {
                    "✓ WIN"
                } else {
                    "✗ LOSE"
                };
                let unrealized = pos.unrealized_pnl;
                let unrealized_pct = if pos.size_usdc > 0.0 {
                    (unrealized / pos.size_usdc) * 100.0
                } else {
                    0.0
                };

                let tp_status = if pos.checkpoint_armed {
                    format!("CP floor:{:.1}%", pos.checkpoint_floor_roi * 100.0)
                } else {
                    format!("CP arm:{:.0}%", self.config.checkpoint_arm_roi * 100.0)
                };
                let sl_th = pos.dynamic_hard_stop_roi.abs();

                info!(
                    "║  {:>4} {:?} {} | ${:.2} @{:.2}/sh | {} P&L:{:+.2}({:+.1}%) | {} SL:{:.0}% | {}",
                    asset, pos.direction, pos.timeframe, pos.size_usdc, pos.share_price,
                    status, unrealized, unrealized_pct, tp_status, sl_th * 100.0, time_left_str
                );
            }
        } else {
            info!("║  📌 No open positions");
        }

        info!("╚══════════════════════════════════════════════════════════════════════╝");
    }

    // ── Getters ─────────────────────────────────────────────────

    pub fn get_stats(&self) -> PaperStats {
        self.stats.read().unwrap().clone()
    }

    pub fn get_positions(&self) -> Vec<PaperPosition> {
        self.positions.read().unwrap().values().cloned().collect()
    }

    pub fn get_trade_history(&self) -> Vec<PaperTradeRecord> {
        self.trade_history.read().unwrap().clone()
    }

    pub fn has_position(&self, asset: Asset) -> bool {
        self.positions
            .read()
            .unwrap()
            .keys()
            .any(|(pos_asset, _)| *pos_asset == asset)
    }

    pub fn has_market_position(&self, asset: Asset, timeframe: Timeframe) -> bool {
        self.positions
            .read()
            .unwrap()
            .contains_key(&(asset, timeframe))
    }

    pub fn open_position_count(&self) -> usize {
        self.positions.read().unwrap().len()
    }

    pub fn get_expired_positions(&self) -> Vec<(Asset, Timeframe)> {
        let now = Utc::now().timestamp_millis();
        self.positions
            .read()
            .unwrap()
            .iter()
            .filter(|(_, pos)| pos.market_close_ts > 0 && now >= pos.market_close_ts)
            .map(|((asset, timeframe), _)| (*asset, *timeframe))
            .collect()
    }

    // ── CSV persistence ─────────────────────────────────────────

    /// Save trade to all relevant CSVs (trades, winloss, AND detailed analytics)
    pub async fn save_trade_to_csv(&self, record: &PaperTradeRecord) {
        if let Some(ref persistence) = self.persistence {
            // Basic trade record
            let trade = TradeRecord {
                timestamp: record.timestamp,
                market_id: if record.market_id.is_empty() {
                    format!("{}-{}", record.asset, record.timeframe)
                } else {
                    record.market_id.clone()
                },
                token_id: if record.token_id.is_empty() {
                    record.trade_id.clone()
                } else {
                    record.token_id.clone()
                },
                side: record.direction.clone(),
                price: record.entry_price,
                size: record.size_usdc,
                outcome: Some(record.result.clone()),
                pnl: Some(record.pnl),
                entry_bid: Some(record.entry_bid),
                entry_ask: Some(record.entry_ask),
                entry_mid: Some(record.entry_mid),
                exit_bid: Some(record.exit_bid),
                exit_ask: Some(record.exit_ask),
                exit_mid: Some(record.exit_mid),
                fee_open: Some(record.fee_open),
                fee_close: Some(record.fee_close),
                slippage_open: Some(record.slippage_open),
                slippage_close: Some(record.slippage_close),
                p_market: Some(record.p_market),
                p_model: Some(record.p_model),
                edge_net: Some(record.edge_net),
                kelly_raw: Some(record.kelly_raw),
                kelly_applied: Some(record.kelly_applied),
                exit_reason_detail: Some(record.exit_reason_detail.clone()),
            };
            if let Err(e) = persistence.save_trade(trade).await {
                warn!(error = %e, "Failed to save paper trade to CSV");
            }

            // Win/loss record
            let winloss = WinLossRecord {
                timestamp: record.timestamp / 1000,
                market_slug: if record.market_id.is_empty() {
                    format!("{}-{}", record.asset, record.timeframe)
                } else {
                    record.market_id.clone()
                },
                token_id: if record.token_id.is_empty() {
                    record.trade_id.clone()
                } else {
                    record.token_id.clone()
                },
                entry_price: record.entry_price,
                exit_price: record.exit_price,
                size: record.size_usdc,
                pnl: record.pnl,
                internal_result: record.result.clone(),
                exit_reason: record.exit_reason.clone(),
                official_result: Some("PAPER".to_string()),
            };
            if let Err(e) = persistence.save_winloss(winloss).await {
                warn!(error = %e, "Failed to save paper winloss to CSV");
            }

            // Detailed analytics record (the gold mine for optimization)
            let stats = self.stats.read().unwrap().clone();
            let wr = if stats.total_trades > 0 {
                (stats.wins as f64 / stats.total_trades as f64) * 100.0
            } else {
                0.0
            };
            let avg_win = if stats.wins > 0 {
                stats.sum_win_pnl / stats.wins as f64
            } else {
                0.0
            };
            let avg_loss = if stats.losses > 0 {
                stats.sum_loss_pnl / stats.losses as f64
            } else {
                0.0
            };
            let pf = if stats.gross_loss > 0.0 {
                stats.gross_profit / stats.gross_loss
            } else {
                0.0
            };

            let time_in_market = record.hold_duration_ms / 1000;
            let window_duration = (record.market_close_ts - record.market_open_ts) / 1000;
            let pct_used = if window_duration > 0 {
                (time_in_market as f64 / window_duration as f64) * 100.0
            } else {
                0.0
            };
            let price_move = if record.entry_price > 0.0 {
                ((record.exit_price - record.entry_price) / record.entry_price) * 100.0
            } else {
                0.0
            };

            let analytics = PaperAnalyticsRecord {
                timestamp: record.timestamp,
                trade_id: record.trade_id.clone(),
                asset: record.asset.clone(),
                timeframe: record.timeframe.clone(),
                direction: record.direction.clone(),
                confidence: record.confidence,
                market_open_ts: record.market_open_ts,
                market_close_ts: record.market_close_ts,
                entered_at_ts: record.timestamp - record.hold_duration_ms,
                exited_at_ts: record.timestamp,
                time_in_market_secs: time_in_market,
                time_remaining_at_entry_secs: record.time_remaining_at_entry_secs,
                pct_of_window_used: pct_used,
                entry_price: record.entry_price,
                exit_price: record.exit_price,
                price_at_market_open: 0.0, // Would need to track
                peak_price: 0.0,
                trough_price: 0.0,
                price_move_pct: price_move,
                rsi_at_entry: 0.0,
                macd_hist_at_entry: 0.0,
                bb_position_at_entry: 0.0,
                adx_at_entry: 0.0,
                stoch_rsi_at_entry: 0.0,
                volatility_at_entry: 0.0,
                regime_at_entry: String::new(),
                momentum_at_entry: 0.0,
                relative_volume_at_entry: 0.0,
                size_usdc: record.size_usdc,
                pnl: record.pnl,
                pnl_pct: record.pnl_pct,
                fee_paid: record.fee_paid,
                result: record.result.clone(),
                exit_reason: record.exit_reason.clone(),
                balance_after: record.balance_after,
                total_trades: stats.total_trades,
                winrate_pct: wr,
                cumulative_pnl: stats.total_pnl,
                max_drawdown_pct: stats.max_drawdown,
                avg_win,
                avg_loss,
                profit_factor: pf,
            };
            if let Err(e) = persistence.save_paper_analytics(analytics).await {
                warn!(error = %e, "Failed to save paper analytics to CSV");
            }
        }
    }

    /// Save analytics WITH features snapshot from position (call before closing)
    pub async fn save_trade_with_features(
        &self,
        record: &PaperTradeRecord,
        features: &FeatureSet,
        peak: f64,
        trough: f64,
        price_at_open: f64,
    ) {
        if let Some(ref persistence) = self.persistence {
            // Save basic trade record
            let trade = TradeRecord {
                timestamp: record.timestamp,
                market_id: if record.market_id.is_empty() {
                    format!("{}-{}", record.asset, record.timeframe)
                } else {
                    record.market_id.clone()
                },
                token_id: if record.token_id.is_empty() {
                    record.trade_id.clone()
                } else {
                    record.token_id.clone()
                },
                side: record.direction.clone(),
                price: record.entry_price,
                size: record.size_usdc,
                outcome: Some(record.result.clone()),
                pnl: Some(record.pnl),
                entry_bid: Some(record.entry_bid),
                entry_ask: Some(record.entry_ask),
                entry_mid: Some(record.entry_mid),
                exit_bid: Some(record.exit_bid),
                exit_ask: Some(record.exit_ask),
                exit_mid: Some(record.exit_mid),
                fee_open: Some(record.fee_open),
                fee_close: Some(record.fee_close),
                slippage_open: Some(record.slippage_open),
                slippage_close: Some(record.slippage_close),
                p_market: Some(record.p_market),
                p_model: Some(record.p_model),
                edge_net: Some(record.edge_net),
                kelly_raw: Some(record.kelly_raw),
                kelly_applied: Some(record.kelly_applied),
                exit_reason_detail: Some(record.exit_reason_detail.clone()),
            };
            if let Err(e) = persistence.save_trade(trade).await {
                warn!(error = %e, "Failed to save paper trade to CSV");
            }

            // Save winloss record
            let winloss = WinLossRecord {
                timestamp: record.timestamp / 1000,
                market_slug: if record.market_id.is_empty() {
                    format!("{}-{}", record.asset, record.timeframe)
                } else {
                    record.market_id.clone()
                },
                token_id: if record.token_id.is_empty() {
                    record.trade_id.clone()
                } else {
                    record.token_id.clone()
                },
                entry_price: record.entry_price,
                exit_price: record.exit_price,
                size: record.size_usdc,
                pnl: record.pnl,
                internal_result: record.result.clone(),
                exit_reason: record.exit_reason.clone(),
                official_result: Some("PAPER".to_string()),
            };
            if let Err(e) = persistence.save_winloss(winloss).await {
                warn!(error = %e, "Failed to save paper winloss to CSV");
            }

            // Now overwrite analytics with feature-enriched version
            let stats = self.stats.read().unwrap().clone();
            let wr = if stats.total_trades > 0 {
                (stats.wins as f64 / stats.total_trades as f64) * 100.0
            } else {
                0.0
            };
            let avg_win = if stats.wins > 0 {
                stats.sum_win_pnl / stats.wins as f64
            } else {
                0.0
            };
            let avg_loss = if stats.losses > 0 {
                stats.sum_loss_pnl / stats.losses as f64
            } else {
                0.0
            };
            let pf = if stats.gross_loss > 0.0 {
                stats.gross_profit / stats.gross_loss
            } else {
                0.0
            };

            let time_in_market = record.hold_duration_ms / 1000;
            let window_duration = (record.market_close_ts - record.market_open_ts) / 1000;
            let pct_used = if window_duration > 0 {
                (time_in_market as f64 / window_duration as f64) * 100.0
            } else {
                0.0
            };
            let price_move = if record.entry_price > 0.0 {
                ((record.exit_price - record.entry_price) / record.entry_price) * 100.0
            } else {
                0.0
            };

            let regime_str = match features.regime {
                1 => "TRENDING",
                0 => "RANGING",
                -1 => "VOLATILE",
                _ => "UNKNOWN",
            };

            // BB position (where price is between bands)
            let bb_pos = if features.bb_upper > features.bb_lower && features.bb_upper > 0.0 {
                (record.entry_price - features.bb_lower) / (features.bb_upper - features.bb_lower)
            } else {
                0.5
            };

            let analytics = PaperAnalyticsRecord {
                timestamp: record.timestamp,
                trade_id: record.trade_id.clone(),
                asset: record.asset.clone(),
                timeframe: record.timeframe.clone(),
                direction: record.direction.clone(),
                confidence: record.confidence,
                market_open_ts: record.market_open_ts,
                market_close_ts: record.market_close_ts,
                entered_at_ts: record.timestamp - record.hold_duration_ms,
                exited_at_ts: record.timestamp,
                time_in_market_secs: time_in_market,
                time_remaining_at_entry_secs: record.time_remaining_at_entry_secs,
                pct_of_window_used: pct_used,
                entry_price: record.entry_price,
                exit_price: record.exit_price,
                price_at_market_open: price_at_open,
                peak_price: peak,
                trough_price: trough,
                price_move_pct: price_move,
                rsi_at_entry: features.rsi,
                macd_hist_at_entry: features.macd_hist,
                bb_position_at_entry: bb_pos,
                adx_at_entry: features.adx,
                stoch_rsi_at_entry: features.stoch_rsi,
                volatility_at_entry: features.atr,
                regime_at_entry: regime_str.to_string(),
                momentum_at_entry: features.momentum,
                relative_volume_at_entry: features.relative_volume,
                size_usdc: record.size_usdc,
                pnl: record.pnl,
                pnl_pct: record.pnl_pct,
                fee_paid: record.fee_paid,
                result: record.result.clone(),
                exit_reason: record.exit_reason.clone(),
                balance_after: record.balance_after,
                total_trades: stats.total_trades,
                winrate_pct: wr,
                cumulative_pnl: stats.total_pnl,
                max_drawdown_pct: stats.max_drawdown,
                avg_win,
                avg_loss,
                profit_factor: pf,
            };
            if let Err(e) = persistence.save_paper_analytics(analytics).await {
                warn!(error = %e, "Failed to save enriched paper analytics to CSV");
            }
        }
    }

    /// Close position and save with full feature context
    pub async fn close_and_save(
        &self,
        asset: Asset,
        timeframe: Timeframe,
        reason: PaperExitReason,
    ) -> Option<PaperTradeRecord> {
        // Grab features, peak, trough before closing
        let (features_opt, peak, trough, price_at_open) = {
            let positions = self.positions.read().unwrap();
            if let Some(pos) = positions.get(&(asset, timeframe)) {
                (
                    pos.features_at_entry.clone(),
                    pos.peak_price,
                    pos.trough_price,
                    pos.price_at_market_open,
                )
            } else {
                (None, 0.0, 0.0, 0.0)
            }
        };

        if let Some(record) = self.close_position(asset, timeframe, reason) {
            if let Some(ref features) = features_opt {
                self.save_trade_with_features(&record, features, peak, trough, price_at_open)
                    .await;
            } else {
                self.save_trade_to_csv(&record).await;
            }
            Some(record)
        } else {
            None
        }
    }

    pub fn summary_string(&self) -> String {
        let stats = self.stats.read().unwrap();
        let balance = self.get_balance();
        let wr = if stats.total_trades > 0 {
            (stats.wins as f64 / stats.total_trades as f64) * 100.0
        } else {
            0.0
        };
        let pf = if stats.gross_loss > 0.0 {
            stats.gross_profit / stats.gross_loss
        } else {
            0.0
        };
        let open = self.positions.read().unwrap().len();
        format!(
            "📋 Paper: ${:.2} bal | {}/{} ({:.0}% WR) | P&L: ${:+.2} | PF: {:.2} | DD: {:.1}% | {} open",
            balance, stats.wins, stats.total_trades, wr, stats.total_pnl, pf, stats.max_drawdown, open
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn sample_features(asset: Asset, timeframe: Timeframe, ts: i64) -> FeatureSet {
        FeatureSet {
            ts,
            asset,
            timeframe,
            rsi: 50.0,
            macd_line: 0.0,
            macd_signal: 0.0,
            macd_hist: 0.0,
            vwap: 0.0,
            bb_upper: 0.0,
            bb_lower: 0.0,
            atr: 0.0,
            momentum: 0.0,
            momentum_accel: 0.0,
            book_imbalance: 0.0,
            spread_bps: 0.0,
            trade_intensity: 0.0,
            ha_close: 0.0,
            ha_trend: 0,
            oracle_confidence: 1.0,
            adx: 0.0,
            stoch_rsi: 0.5,
            obv: 0.0,
            relative_volume: 1.0,
            regime: 0,
        }
    }

    fn sample_signal(direction: Direction, expires_at: i64) -> Signal {
        let ts = Utc::now().timestamp_millis();
        Signal {
            id: format!("sig-{}-{}", ts, direction),
            ts,
            asset: Asset::BTC,
            timeframe: Timeframe::Min15,
            direction,
            confidence: 0.62,
            features: sample_features(Asset::BTC, Timeframe::Min15, ts),
            strategy_id: "test".to_string(),
            market_slug: "btc-15m".to_string(),
            condition_id: "cond-test".to_string(),
            token_id: format!("token-{}", direction),
            expires_at,
            suggested_size_usdc: 100.0,
            quote_bid: 0.50,
            quote_ask: 0.50,
            quote_mid: 0.50,
            quote_depth_top5: 100.0,
            indicators_used: vec!["rsi_extreme".to_string()],
        }
    }

    fn new_engine_with_quotes() -> (PaperTradingEngine, std::sync::Arc<PolymarketSharePrices>) {
        let mut cfg = PaperTradingConfig::default();
        cfg.native_only = true;
        cfg.prefer_chainlink = true;
        cfg.kelly_enabled = false;
        cfg.checkpoint_arm_roi = 0.10;
        cfg.checkpoint_initial_floor_roi = 0.08;
        cfg.checkpoint_trail_gap_roi = 0.02;
        cfg.hard_stop_roi = -0.08;

        let share_prices = std::sync::Arc::new(PolymarketSharePrices::new());
        share_prices.update_quote(Asset::BTC, Timeframe::Min15, "UP", 0.50, 0.50, 0.50);
        share_prices.update_quote(Asset::BTC, Timeframe::Min15, "DOWN", 0.50, 0.50, 0.50);

        let engine =
            PaperTradingEngine::new(cfg).with_polymarket_share_prices(share_prices.clone());
        (engine, share_prices)
    }

    fn new_engine_without_quotes() -> PaperTradingEngine {
        let mut cfg = PaperTradingConfig::default();
        cfg.native_only = true;
        cfg.prefer_chainlink = true;
        cfg.kelly_enabled = false;
        PaperTradingEngine::new(cfg)
    }

    #[test]
    fn checkpoint_take_profit_triggers_after_two_breaches() {
        let (engine, share_prices) = new_engine_with_quotes();
        let now = Utc::now().timestamp_millis();
        let expires_at = now + 15 * 60 * 1000;

        // Seed base price source for entry.
        engine.update_price(Asset::BTC, 1000.0, PriceSource::RtdsChainlink);
        let signal = sample_signal(Direction::Up, expires_at);
        assert!(engine.execute_signal(&signal).unwrap());

        // Arm checkpoint (>10% ROI net).
        share_prices.update_quote(Asset::BTC, Timeframe::Min15, "UP", 0.56, 0.57, 0.565);
        let exits = engine.update_price(Asset::BTC, 1010.0, PriceSource::RtdsChainlink);
        assert!(exits.is_empty());

        // First breach below dynamic floor.
        share_prices.update_quote(Asset::BTC, Timeframe::Min15, "UP", 0.53, 0.54, 0.535);
        let exits = engine.update_price(Asset::BTC, 1008.0, PriceSource::RtdsChainlink);
        assert!(exits.is_empty());

        // Second consecutive breach should trigger checkpoint TP.
        let exits = engine.update_price(Asset::BTC, 1007.0, PriceSource::RtdsChainlink);
        assert_eq!(exits.len(), 1);
        assert_eq!(exits[0].1, PaperExitReason::CheckpointTakeProfit);
    }

    #[test]
    fn hard_stop_triggers_on_deep_negative_roi() {
        let (engine, share_prices) = new_engine_with_quotes();
        let now = Utc::now().timestamp_millis();
        let expires_at = now + 15 * 60 * 1000;

        engine.update_price(Asset::BTC, 1000.0, PriceSource::RtdsChainlink);
        let signal = sample_signal(Direction::Up, expires_at);
        assert!(engine.execute_signal(&signal).unwrap());

        // Deep drawdown in sellable bid should trigger hard stop.
        share_prices.update_quote(Asset::BTC, Timeframe::Min15, "UP", 0.42, 0.43, 0.425);
        let exits = engine.update_price(Asset::BTC, 995.0, PriceSource::RtdsChainlink);
        assert!(exits.is_empty());

        // Second consecutive breach confirms hard stop.
        let exits = engine.update_price(Asset::BTC, 994.8, PriceSource::RtdsChainlink);
        assert_eq!(exits.len(), 1);
        assert_eq!(exits[0].1, PaperExitReason::HardStop);
    }

    #[test]
    fn window_bias_blocks_opposite_direction_same_window() {
        let (engine, _share_prices) = new_engine_with_quotes();
        let now = Utc::now().timestamp_millis();
        let expires_at = now + 15 * 60 * 1000;

        engine.update_price(Asset::BTC, 1000.0, PriceSource::RtdsChainlink);
        let up = sample_signal(Direction::Up, expires_at);
        assert!(engine.execute_signal(&up).unwrap());

        // Close manually but keep the window bias active.
        let closed = engine.close_position(Asset::BTC, Timeframe::Min15, PaperExitReason::Manual);
        assert!(closed.is_some());

        let down = sample_signal(Direction::Down, expires_at);
        assert!(!engine.execute_signal(&down).unwrap());
    }

    #[test]
    fn rejects_signal_without_token_id() {
        let (engine, _share_prices) = new_engine_with_quotes();
        let now = Utc::now().timestamp_millis();
        let expires_at = now + 15 * 60 * 1000;

        engine.update_price(Asset::BTC, 1000.0, PriceSource::RtdsChainlink);
        let mut signal = sample_signal(Direction::Up, expires_at);
        signal.token_id.clear();

        assert!(!engine.execute_signal(&signal).unwrap());
    }

    #[test]
    fn rejects_signal_without_orderbook_quote() {
        let engine = new_engine_without_quotes();
        let now = Utc::now().timestamp_millis();
        let expires_at = now + 15 * 60 * 1000;

        engine.update_price(Asset::BTC, 1000.0, PriceSource::RtdsChainlink);
        let signal = sample_signal(Direction::Up, expires_at);
        assert!(!engine.execute_signal(&signal).unwrap());
    }

    #[test]
    fn rejects_15m_signal_with_expiry_too_far_in_future() {
        let (engine, _share_prices) = new_engine_with_quotes();
        let now = Utc::now().timestamp_millis();

        engine.update_price(Asset::BTC, 1000.0, PriceSource::RtdsChainlink);

        // 20 minutes on a 15m lane should be rejected as inconsistent horizon.
        let expires_at = now + 20 * 60 * 1000;
        let signal = sample_signal(Direction::Up, expires_at);
        assert!(!engine.execute_signal(&signal).unwrap());
    }

    #[test]
    fn rejects_15m_signal_for_future_window_not_started_yet() {
        let (engine, _share_prices) = new_engine_with_quotes();
        let now = Utc::now().timestamp_millis();

        engine.update_price(Asset::BTC, 1000.0, PriceSource::RtdsChainlink);

        // 16m to expiry may pass horizon tolerance, but implies market_open is ~1m in the future.
        // The engine must reject opening a position before the window starts.
        let expires_at = now + 16 * 60 * 1000;
        let signal = sample_signal(Direction::Up, expires_at);
        assert!(!engine.execute_signal(&signal).unwrap());
    }

    #[test]
    fn load_state_missing_file_keeps_clean_boot_defaults() {
        let mut cfg = PaperTradingConfig::default();
        cfg.initial_balance = 1234.0;
        cfg.native_only = true;

        let temp_dir =
            std::env::temp_dir().join(format!("polybot_clean_boot_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        let state_path = temp_dir.join("paper_state_missing.json");

        let engine = PaperTradingEngine::new(cfg).with_state_file(state_path);
        engine.load_state().unwrap();

        assert_eq!(engine.get_balance(), 1234.0);
        assert_eq!(engine.open_position_count(), 0);
        assert_eq!(engine.get_trade_history().len(), 0);

        let _ = fs::remove_dir_all(temp_dir);
    }
}
