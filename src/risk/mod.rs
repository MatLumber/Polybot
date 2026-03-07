//! Risk Manager - Position sizing and risk controls
//!
//! Implements:
//! - Confidence-weighted position sizing
//! - Maximum position limits
//! - Daily loss limits (properly enforced in evaluate)
//! - Exposure tracking
//! - Trailing stop-loss
//! - Take-profit targets
//! - Time-based position expiry

use anyhow::Result;
use chrono::{DateTime, Datelike, Utc};
use std::collections::HashMap;
use std::sync::RwLock;

use crate::types::{Asset, ClosedTrade, Direction, Signal, Timeframe, Trade, TradeResult};

/// Risk configuration
#[derive(Debug, Clone)]
pub struct RiskConfig {
    /// Maximum position size per trade (USDC)
    pub max_position_size: f64,
    /// Minimum position size per trade (USDC)
    pub min_position_size: f64,
    /// Percentage of available balance to risk per trade (e.g., 0.05 = 5%)
    pub balance_risk_pct: f64,
    /// Maximum total exposure (USDC)
    pub max_total_exposure: f64,
    /// Maximum daily loss (USDC)
    pub max_daily_loss: f64,
    /// Minimum confidence to trade
    pub min_confidence: f64,
    /// Confidence scaling thresholds
    pub confidence_scale: ConfidenceScale,
    /// Maximum trades per day
    pub max_trades_per_day: usize,
    /// Trailing stop-loss percentage (e.g., 0.03 = 3%)
    pub trailing_stop_pct: f64,
    /// Take-profit percentage (e.g., 0.05 = 5%)
    pub take_profit_pct: f64,
    /// Maximum hold duration in milliseconds (e.g., 2h = 7_200_000)
    pub max_hold_duration_ms: i64,
    /// Whether the kill switch is enabled
    pub kill_switch_enabled: bool,
    /// ROI that arms checkpoint profit protection
    pub checkpoint_arm_roi: f64,
    /// Initial ROI floor locked once checkpoint arms
    pub checkpoint_initial_floor_roi: f64,
    /// Minimum gap between the peak ROI and the protected floor
    pub checkpoint_trail_gap_roi: f64,
    /// Hard stop expressed as ROI (negative value)
    pub hard_stop_roi: f64,
}

#[derive(Debug, Clone)]
pub struct ConfidenceScale {
    /// At this confidence, use this fraction of max position
    pub low: (f64, f64), // (confidence, fraction)
    pub medium: (f64, f64),
    pub high: (f64, f64),
}

impl Default for ConfidenceScale {
    fn default() -> Self {
        Self {
            low: (0.72, 0.4),     // 72% conf -> 40% of risk budget ($3.4 on $85)
            medium: (0.80, 0.65), // 80% conf -> 65% of risk budget ($5.5 on $85)
            high: (0.88, 1.0),    // 88% conf -> 100% of risk budget ($8 on $85)
        }
    }
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_position_size: 8.0,   // $8 max per trade (~10% of $85)
            min_position_size: 2.0,   // $2 min per trade
            balance_risk_pct: 0.05,   // 5% of balance per trade (Kelly-ish for binary bets)
            max_total_exposure: 25.0, // $25 max total (~30% of $85)
            max_daily_loss: 20.0,     // $20 max daily loss (~24% of capital)
            min_confidence: 0.72,     // Match strategy min_confidence
            confidence_scale: ConfidenceScale::default(),
            max_trades_per_day: 10,          // Fewer, higher quality trades
            trailing_stop_pct: 0.15,         // 15% trailing stop
            take_profit_pct: 0.20,           // 20% take-profit
            max_hold_duration_ms: 7_200_000, // 2 hours
            kill_switch_enabled: true,
            checkpoint_arm_roi: 0.05,
            checkpoint_initial_floor_roi: 0.05,
            checkpoint_trail_gap_roi: 0.03,
            hard_stop_roi: -0.25,
        }
    }
}

/// Exit reason for a position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    TrailingStop,
    TakeProfit,
    CheckpointTakeProfit,
    HardStop,
    TimeExpiry,
    MarketExpiry,
    Manual,
    Signal,
}

impl std::fmt::Display for ExitReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExitReason::TrailingStop => write!(f, "TRAILING_STOP"),
            ExitReason::TakeProfit => write!(f, "TAKE_PROFIT"),
            ExitReason::CheckpointTakeProfit => write!(f, "CHECKPOINT_TAKE_PROFIT"),
            ExitReason::HardStop => write!(f, "HARD_STOP"),
            ExitReason::TimeExpiry => write!(f, "TIME_EXPIRY"),
            ExitReason::MarketExpiry => write!(f, "MARKET_EXPIRY"),
            ExitReason::Manual => write!(f, "MANUAL"),
            ExitReason::Signal => write!(f, "SIGNAL"),
        }
    }
}

/// Current position tracking
#[derive(Debug, Clone)]
pub struct Position {
    pub asset: Asset,
    pub direction: Direction,
    pub size: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub opened_at: i64,
    /// When the market expires (Unix timestamp in milliseconds)
    pub expires_at: i64,
    /// Market identifier (e.g., "btc-15m")
    pub market_slug: String,
    /// Token ID for the position
    pub token_id: String,
    /// Peak price since open (highest for longs, lowest for shorts)
    pub peak_price: f64,
    /// Trailing stop trigger price
    pub stop_price: f64,
    /// Take-profit target price
    pub take_profit_price: f64,
    /// Checkpoint profit state
    pub checkpoint_armed: bool,
    pub checkpoint_peak_roi: f64,
    pub checkpoint_floor_roi: f64,
    pub checkpoint_breach_ticks: u32,
    pub hard_stop_breach_ticks: u32,
    pub dynamic_hard_stop_roi: f64,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            asset: Asset::BTC,
            direction: Direction::Up,
            size: 0.0,
            entry_price: 0.0,
            current_price: 0.0,
            pnl: 0.0,
            pnl_pct: 0.0,
            opened_at: 0,
            expires_at: 0,
            market_slug: String::new(),
            token_id: String::new(),
            peak_price: 0.0,
            stop_price: 0.0,
            take_profit_price: 0.0,
            checkpoint_armed: false,
            checkpoint_peak_roi: 0.0,
            checkpoint_floor_roi: 0.0,
            checkpoint_breach_ticks: 0,
            hard_stop_breach_ticks: 0,
            dynamic_hard_stop_roi: -0.07,
        }
    }
}

/// Daily statistics
#[derive(Debug, Clone, Default)]
pub struct DailyStats {
    pub date: String,
    pub trades: usize,
    pub wins: usize,
    pub losses: usize,
    pub pnl: f64,
    pub total_volume: f64,
}

/// Risk manager for position sizing and limits
pub struct RiskManager {
    config: RiskConfig,
    /// Current positions keyed by token_id when available, otherwise by asset.
    positions: RwLock<HashMap<String, Position>>,
    /// Daily stats (RwLock for interior mutability so evaluate() can check without &mut self)
    daily_stats: RwLock<HashMap<String, DailyStats>>,
    /// Current date key
    current_date: RwLock<String>,
    /// Current available balance (USDC)
    current_balance: RwLock<f64>,
}

impl RiskManager {
    fn position_key(asset: Asset, token_id: &str) -> String {
        let token_id = token_id.trim();
        if token_id.is_empty() {
            format!("asset::{asset}")
        } else {
            format!("token::{token_id}")
        }
    }

    fn position_key_for_token(token_id: &str) -> Option<String> {
        let token_id = token_id.trim();
        if token_id.is_empty() {
            None
        } else {
            Some(format!("token::{token_id}"))
        }
    }

    pub fn new(config: RiskConfig) -> Self {
        let current_date = Self::get_date_key(Utc::now());
        Self {
            config,
            positions: RwLock::new(HashMap::new()),
            daily_stats: RwLock::new(HashMap::new()),
            current_date: RwLock::new(current_date),
            current_balance: RwLock::new(0.0),
        }
    }

    /// Set current balance (called from main when balance is fetched)
    pub fn set_balance(&self, balance: f64) {
        if let Ok(mut b) = self.current_balance.write() {
            *b = balance;
        }
    }

    /// Get current balance
    pub fn get_balance(&self) -> f64 {
        self.current_balance.read().map(|b| *b).unwrap_or(0.0)
    }

    fn checkpoint_floor_from_peak(&self, peak_roi: f64) -> f64 {
        Self::checkpoint_floor_from_peak_inner(&self.config, peak_roi)
    }

    fn checkpoint_floor_from_peak_inner(config: &RiskConfig, peak_roi: f64) -> f64 {
        let dynamic_gap = (peak_roi * 0.25_f64).max(config.checkpoint_trail_gap_roi.max(0.0));
        (peak_roi - dynamic_gap).max(config.checkpoint_initial_floor_roi.max(0.0))
    }

    fn roi_from_stop_price(direction: Direction, entry_price: f64, stop_price: f64) -> f64 {
        if entry_price <= 0.0 {
            return 0.0;
        }

        match direction {
            Direction::Up => (stop_price - entry_price) / entry_price,
            Direction::Down => (entry_price - stop_price) / entry_price,
        }
    }

    /// Calculate position size based on confidence and available balance
    pub fn calculate_position_size(&self, signal: &Signal) -> f64 {
        let confidence = signal.confidence;

        // Scale based on confidence
        let fraction = if confidence >= self.config.confidence_scale.high.0 {
            self.config.confidence_scale.high.1
        } else if confidence >= self.config.confidence_scale.medium.0 {
            self.config.confidence_scale.medium.1
        } else if confidence >= self.config.confidence_scale.low.0 {
            self.config.confidence_scale.low.1
        } else {
            0.0 // Below minimum confidence
        };

        // Get current balance (default to max_position_size if not set)
        let current_balance = self.get_balance();
        let base_size = if current_balance > 0.0 {
            // Use percentage of balance
            current_balance * self.config.balance_risk_pct
        } else {
            // Fallback to max position size if balance not set
            self.config.max_position_size
        };

        // Apply confidence fraction
        let size = base_size * fraction;

        // Clamp between min and max
        size.max(self.config.min_position_size)
            .min(self.config.max_position_size)
    }

    /// Calculate position size based only on confidence value
    pub fn calculate_size_from_confidence(&self, confidence: f64) -> f64 {
        // Scale based on confidence
        let fraction = if confidence >= self.config.confidence_scale.high.0 {
            self.config.confidence_scale.high.1
        } else if confidence >= self.config.confidence_scale.medium.0 {
            self.config.confidence_scale.medium.1
        } else if confidence >= self.config.confidence_scale.low.0 {
            self.config.confidence_scale.low.1
        } else {
            0.0 // Below minimum confidence
        };

        // Get current balance (default to max_position_size if not set)
        let current_balance = self.get_balance();
        let base_size = if current_balance > 0.0 {
            // Use percentage of balance
            current_balance * self.config.balance_risk_pct
        } else {
            // Fallback to max position size if balance not set
            self.config.max_position_size
        };

        // Apply confidence fraction
        let size = base_size * fraction;

        // Clamp between min and max
        size.max(self.config.min_position_size)
            .min(self.config.max_position_size)
    }

    /// Evaluate if a signal should be approved - NOW includes ALL safety checks
    pub fn evaluate(&self, signal: &Signal) -> Result<bool, String> {
        // Check minimum confidence
        if signal.confidence < self.config.min_confidence {
            return Ok(false);
        }

        // Check if we already have a position in this asset
        {
            let positions = self.positions.read().map_err(|e| e.to_string())?;
            if positions.values().any(|position| position.asset == signal.asset) {
                return Ok(false);
            }
        }

        // Check position size bounds
        let size = self.calculate_position_size(signal);
        if size < self.config.min_position_size || size > self.config.max_position_size {
            return Ok(false);
        }

        // Check daily loss limit (kill switch)
        if self.config.kill_switch_enabled {
            let today_stats = self.get_today_stats_readonly();
            if let Some(stats) = today_stats {
                if stats.pnl < -self.config.max_daily_loss {
                    return Err(format!(
                        "Kill switch: daily loss limit reached ${:.2} (max -${:.2})",
                        stats.pnl, self.config.max_daily_loss
                    ));
                }
            }
        }

        // Check max trades per day
        {
            let today_stats = self.get_today_stats_readonly();
            if let Some(stats) = today_stats {
                if stats.trades >= self.config.max_trades_per_day {
                    return Err(format!(
                        "Max trades per day reached: {} (max {})",
                        stats.trades, self.config.max_trades_per_day
                    ));
                }
            }
        }

        // Check total exposure
        {
            let positions = self.positions.read().map_err(|e| e.to_string())?;
            let current_exposure: f64 = positions.values().map(|p| p.size).sum();
            if current_exposure + size > self.config.max_total_exposure {
                return Err(format!(
                    "Would exceed max exposure: ${:.2} + ${:.2} > ${:.2}",
                    current_exposure, size, self.config.max_total_exposure
                ));
            }
        }

        Ok(true)
    }

    /// Get today's stats in read-only mode (for evaluate)
    fn get_today_stats_readonly(&self) -> Option<DailyStats> {
        let today = Self::get_date_key(Utc::now());
        let stats = self.daily_stats.read().ok()?;
        stats.get(&today).cloned()
    }

    /// Record a new position with stop-loss and take-profit
    pub fn open_position(&self, signal: &Signal, size: f64, price: f64) {
        let take_profit_price = price * (1.0 + self.config.take_profit_pct);
        let stop_price = price * (1.0 - self.config.trailing_stop_pct);
        let peak_price = price;

        let position = Position {
            asset: signal.asset,
            direction: signal.direction,
            size,
            entry_price: price,
            current_price: price,
            pnl: 0.0,
            pnl_pct: 0.0,
            opened_at: signal.ts,
            expires_at: signal.expires_at,
            market_slug: signal.market_slug.clone(),
            token_id: signal.token_id.clone(),
            peak_price,
            stop_price,
            take_profit_price,
            checkpoint_armed: false,
            checkpoint_peak_roi: 0.0,
            checkpoint_floor_roi: 0.0,
            checkpoint_breach_ticks: 0,
            hard_stop_breach_ticks: 0,
            dynamic_hard_stop_roi: self.config.hard_stop_roi,
        };

        if let Ok(mut positions) = self.positions.write() {
            let key = Self::position_key(signal.asset, &signal.token_id);
            positions.insert(key, position);
        }

        // Update daily stats
        if let Ok(mut stats_map) = self.daily_stats.write() {
            let today = Self::get_date_key(Utc::now());
            let stats = stats_map
                .entry(today.clone())
                .or_insert_with(|| DailyStats {
                    date: today,
                    ..Default::default()
                });
            stats.trades += 1;
            stats.total_volume += size;
        }
    }

    /// Restore a live position after a restart/crash so dynamic risk rules keep working.
    pub fn restore_position(
        &self,
        asset: Asset,
        direction: Direction,
        size: f64,
        entry_price: f64,
        current_price: f64,
        opened_at: i64,
        expires_at: i64,
        market_slug: String,
        token_id: String,
    ) {
        if entry_price <= 0.0 || size <= 0.0 {
            return;
        }

        if self.has_position_token(&token_id) {
            return;
        }

        let take_profit_price = entry_price * (1.0 + self.config.take_profit_pct);
        let stop_price = entry_price * (1.0 - self.config.trailing_stop_pct);
        let peak_price = entry_price.max(current_price);

        let key = Self::position_key(asset, &token_id);

        let position = Position {
            asset,
            direction,
            size,
            entry_price,
            current_price,
            pnl: 0.0,
            pnl_pct: 0.0,
            opened_at,
            expires_at,
            market_slug,
            token_id,
            peak_price,
            stop_price,
            take_profit_price,
            checkpoint_armed: false,
            checkpoint_peak_roi: 0.0,
            checkpoint_floor_roi: 0.0,
            checkpoint_breach_ticks: 0,
            hard_stop_breach_ticks: 0,
            dynamic_hard_stop_roi: self.config.hard_stop_roi,
        };

        if let Ok(mut positions) = self.positions.write() {
            positions.insert(key, position);
        }
    }

    /// Update position with current price and return exit reason if position should close
    pub fn update_position(&self, asset: Asset, current_price: f64) -> Option<ExitReason> {
        let mut positions = self.positions.write().ok()?;
        let position = positions.values_mut().find(|position| position.asset == asset)?;

        Self::update_position_inner(position, current_price, &self.config)
    }

    /// Update a specific live position with its token_id.
    pub fn update_position_by_token_id(
        &self,
        token_id: &str,
        current_price: f64,
    ) -> Option<ExitReason> {
        let mut positions = self.positions.write().ok()?;
        let key = Self::position_key_for_token(token_id)?;
        let position = positions.get_mut(&key)?;

        Self::update_position_inner(position, current_price, &self.config)
    }

    fn update_position_inner(
        position: &mut Position,
        current_price: f64,
        config: &RiskConfig,
    ) -> Option<ExitReason> {

        position.current_price = current_price;

        // Calculate PnL regardless of direction because we hold the Polymarket share
        let price_diff = current_price - position.entry_price;
        position.pnl = (price_diff / position.entry_price) * position.size;
        position.pnl_pct = price_diff / position.entry_price;

        if position.pnl_pct >= config.take_profit_pct {
            return Some(ExitReason::TakeProfit);
        }

        if !position.checkpoint_armed && position.pnl_pct >= config.checkpoint_arm_roi {
            position.checkpoint_armed = true;
            position.checkpoint_peak_roi = position.pnl_pct;
            position.checkpoint_floor_roi =
                Self::checkpoint_floor_from_peak_inner(config, position.checkpoint_peak_roi);
            position.checkpoint_breach_ticks = 0;
        }

        // Update peak price and trailing stop (applies the same for holding YES or NO token)
        if current_price > position.peak_price {
            position.peak_price = current_price;
            position.stop_price = current_price * (1.0 - config.trailing_stop_pct);
        }
        // Check trailing stop (price dropped below stop)
        if current_price <= position.stop_price {
            return Some(ExitReason::TrailingStop);
        }

        // We compute trailing stop ROI as a fallback
        let trailing_stop_roi = (position.stop_price - position.entry_price) / position.entry_price;
        position.dynamic_hard_stop_roi = trailing_stop_roi.min(0.0).max(config.hard_stop_roi);

        if position.checkpoint_armed {
            if position.pnl_pct > position.checkpoint_peak_roi {
                position.checkpoint_peak_roi = position.pnl_pct;
            }

            let desired_floor =
                Self::checkpoint_floor_from_peak_inner(config, position.checkpoint_peak_roi);
            if desired_floor > position.checkpoint_floor_roi {
                position.checkpoint_floor_roi = desired_floor;
            }

            if position.pnl_pct <= position.checkpoint_floor_roi {
                position.checkpoint_breach_ticks =
                    position.checkpoint_breach_ticks.saturating_add(1);
            } else {
                position.checkpoint_breach_ticks = 0;
            }

            if position.checkpoint_breach_ticks >= 1 {
                return Some(ExitReason::CheckpointTakeProfit);
            }
        }

        if position.pnl_pct <= config.hard_stop_roi {
            position.hard_stop_breach_ticks = position.hard_stop_breach_ticks.saturating_add(1);
        } else {
            position.hard_stop_breach_ticks = 0;
        }

        // Check time-based expiry BEFORE price stops — expired markets must always exit
        let now = Utc::now().timestamp_millis();
        if position.expires_at > 0 && now >= position.expires_at {
            return Some(ExitReason::MarketExpiry);
        }
        if now - position.opened_at > config.max_hold_duration_ms {
            return Some(ExitReason::TimeExpiry);
        }

        if position.hard_stop_breach_ticks >= 1 {
            return Some(ExitReason::HardStop);
        }

        None
    }

    /// Close a position and record result
    pub fn close_position(
        &self,
        asset: Asset,
        close_price: f64,
        reason: ExitReason,
    ) -> Option<ClosedTrade> {
        let position = {
            let mut positions = self.positions.write().ok()?;
            let key = positions
                .iter()
                .find(|(_, position)| position.asset == asset)
                .map(|(key, _)| key.clone())?;
            positions.remove(&key)?
        };

        self.close_position_inner(position, asset, close_price, reason)
    }

    pub fn close_position_by_token_id(
        &self,
        token_id: &str,
        close_price: f64,
        reason: ExitReason,
    ) -> Option<ClosedTrade> {
        let position = {
            let mut positions = self.positions.write().ok()?;
            let key = Self::position_key_for_token(token_id)?;
            positions.remove(&key)?
        };

        let asset = position.asset;
        self.close_position_inner(position, asset, close_price, reason)
    }

    fn close_position_inner(
        &self,
        position: Position,
        asset: Asset,
        close_price: f64,
        reason: ExitReason,
    ) -> Option<ClosedTrade> {
        let price_diff = close_price - position.entry_price;
        let pnl = (price_diff / position.entry_price) * position.size;

        // Update daily stats
        if let Ok(mut stats_map) = self.daily_stats.write() {
            let today = Self::get_date_key(Utc::now());
            let stats = stats_map
                .entry(today.clone())
                .or_insert_with(|| DailyStats {
                    date: today,
                    ..Default::default()
                });
            stats.pnl += pnl;
            if pnl > 0.0 {
                stats.wins += 1;
            } else {
                stats.losses += 1;
            }
        }

        let _ = reason;

        Some(ClosedTrade {
            asset,
            direction: position.direction,
            size: position.size,
            entry_price: position.entry_price,
            exit_price: close_price,
            pnl,
            opened_at: position.opened_at,
            closed_at: Utc::now().timestamp_millis(),
        })
    }

    /// Check all positions for exit conditions. Returns list of (asset, exit_reason, current_price)
    pub fn check_all_exits(&self) -> Vec<(Asset, ExitReason, f64)> {
        let positions = match self.positions.read() {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };

        let mut exits = Vec::new();
        let now = Utc::now().timestamp_millis();

        for pos in positions.values() {
            // Check time expiry (even if price hasn't updated)
            if now - pos.opened_at > self.config.max_hold_duration_ms {
                exits.push((pos.asset, ExitReason::TimeExpiry, pos.current_price));
            }
        }

        exits
    }

    /// Get current total exposure
    pub fn total_exposure(&self) -> f64 {
        self.positions
            .read()
            .map(|p| p.values().map(|pos| pos.size).sum())
            .unwrap_or(0.0)
    }

    /// Get current unrealized PnL
    pub fn unrealized_pnl(&self) -> f64 {
        self.positions
            .read()
            .map(|p| p.values().map(|pos| pos.pnl).sum())
            .unwrap_or(0.0)
    }

    /// Get today's statistics
    pub fn today_stats(&self) -> Option<DailyStats> {
        self.get_today_stats_readonly()
    }

    /// Get date key for stats grouping
    fn get_date_key(dt: DateTime<Utc>) -> String {
        format!("{}-{:02}-{:02}", dt.year(), dt.month(), dt.day())
    }

    /// Check if we have an open position for an asset
    pub fn has_position(&self, asset: Asset) -> bool {
        self.positions
            .read()
            .map(|p| p.values().any(|position| position.asset == asset))
            .unwrap_or(false)
    }

    pub fn has_position_token(&self, token_id: &str) -> bool {
        let positions = match self.positions.read() {
            Ok(positions) => positions,
            Err(_) => return false,
        };
        let Some(key) = Self::position_key_for_token(token_id) else {
            return false;
        };
        positions.contains_key(&key)
    }

    /// Get position for an asset
    pub fn get_position(&self, asset: Asset) -> Option<Position> {
        self.positions
            .read()
            .ok()?
            .values()
            .find(|position| position.asset == asset)
            .cloned()
    }

    pub fn get_position_by_token_id(&self, token_id: &str) -> Option<Position> {
        let positions = self.positions.read().ok()?;
        let key = Self::position_key_for_token(token_id)?;
        positions.get(&key).cloned()
    }

    /// Get all open positions
    pub fn all_positions(&self) -> Vec<Position> {
        self.positions
            .read()
            .map(|p| p.values().cloned().collect())
            .unwrap_or_default()
    }
}

impl Default for RiskManager {
    fn default() -> Self {
        Self::new(RiskConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_signal(asset: Asset, confidence: f64, direction: Direction) -> Signal {
        Signal {
            id: "test".to_string(),
            ts: 1700000000000,
            asset,
            timeframe: Timeframe::Min15,
            direction,
            confidence,
            model_prob_up: if direction == Direction::Up {
                confidence.clamp(0.0, 1.0)
            } else {
                (1.0 - confidence).clamp(0.0, 1.0)
            },
            features: crate::types::FeatureSet {
                ts: 0,
                asset,
                timeframe: Timeframe::Min15,
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
            },
            strategy_id: "test".to_string(),
            market_slug: String::new(),
            condition_id: String::new(),
            token_id: String::new(),
            expires_at: 0,
            suggested_size_usdc: 100.0,
            quote_bid: 0.0,
            quote_ask: 0.0,
            quote_mid: 0.0,
            quote_depth_top5: 0.0,
            indicators_used: Vec::new(),
        }
    }

    #[test]
    fn test_position_sizing() {
        let rm = RiskManager::default();
        // Default: no balance set, so base_size = max_position_size = $8
        // Confidence scale: low(0.72, 0.4), medium(0.80, 0.65), high(0.88, 1.0)

        // Low confidence (0.72 threshold) → fraction = 0.4 → $8 * 0.4 = $3.2
        let sig1 = make_signal(Asset::BTC, 0.74, Direction::Up);
        let size1 = rm.calculate_position_size(&sig1);
        assert!(
            (size1 - 3.2).abs() < 0.01,
            "Expected ~$3.2, got ${:.2}",
            size1
        );

        // Medium confidence (0.80 threshold) → fraction = 0.65 → $8 * 0.65 = $5.2
        let sig2 = make_signal(Asset::ETH, 0.82, Direction::Up);
        let size2 = rm.calculate_position_size(&sig2);
        assert!(
            (size2 - 5.2).abs() < 0.01,
            "Expected ~$5.2, got ${:.2}",
            size2
        );

        // High confidence (0.88 threshold) → fraction = 1.0 → $8 * 1.0 = $8.0
        let sig3 = make_signal(Asset::BTC, 0.90, Direction::Up);
        let size3 = rm.calculate_position_size(&sig3);
        assert!(
            (size3 - 8.0).abs() < 0.01,
            "Expected ~$8.0, got ${:.2}",
            size3
        );
    }

    #[test]
    fn test_evaluate_checks_all_safety() {
        let rm = RiskManager::default();
        // min_confidence is now 0.72
        let sig = make_signal(Asset::BTC, 0.75, Direction::Up);

        // Should be allowed
        assert!(rm.evaluate(&sig).unwrap());

        // Below min confidence should fail
        let low_conf_sig = make_signal(Asset::ETH, 0.50, Direction::Up);
        assert!(!rm.evaluate(&low_conf_sig).unwrap());
    }

    #[test]
    fn test_evaluate_rejects_duplicate_position() {
        let rm = RiskManager::default();
        let sig = make_signal(Asset::BTC, 0.80, Direction::Up);
        let size = rm.calculate_position_size(&sig);

        rm.open_position(&sig, size, 50000.0);

        // Same asset should be rejected
        let sig2 = make_signal(Asset::BTC, 0.80, Direction::Up);
        assert!(!rm.evaluate(&sig2).unwrap());
    }

    #[test]
    fn test_trailing_stop_loss_long() {
        let config = RiskConfig {
            trailing_stop_pct: 0.03,        // 3% trailing stop
            max_hold_duration_ms: i64::MAX, // Prevent time expiry during test
            ..Default::default()
        };
        let rm = RiskManager::new(config);
        let sig = make_signal(Asset::BTC, 0.80, Direction::Up);

        rm.open_position(&sig, 100.0, 0.50);

        // Price goes up - should update peak and stop
        let exit = rm.update_position(Asset::BTC, 0.52);
        assert!(exit.is_none());

        // Price drops 3% from peak (0.52) -> 0.5044 should trigger stop
        let exit = rm.update_position(Asset::BTC, 0.5044);
        assert_eq!(exit, Some(ExitReason::TrailingStop));
    }

    #[test]
    fn test_trailing_stop_loss_short() {
        let config = RiskConfig {
            trailing_stop_pct: 0.03,        // 3% trailing stop
            max_hold_duration_ms: i64::MAX, // Prevent time expiry during test
            ..Default::default()
        };
        let rm = RiskManager::new(config);
        let sig = make_signal(Asset::BTC, 0.80, Direction::Down);

        // Buying NO token at 0.50
        rm.open_position(&sig, 100.0, 0.50);

        // Token price goes up - should update peak and stop
        let exit = rm.update_position(Asset::BTC, 0.52);
        assert!(exit.is_none());

        // Token price drops 3% from peak (0.52) -> 0.5044 should trigger stop
        let exit = rm.update_position(Asset::BTC, 0.5044);
        assert_eq!(exit, Some(ExitReason::TrailingStop));
    }

    #[test]
    fn test_take_profit() {
        let config = RiskConfig {
            take_profit_pct: 0.05,   // 5% take-profit
            trailing_stop_pct: 0.10, // Wide stop so it doesn't interfere
            ..Default::default()
        };
        let rm = RiskManager::new(config);
        let sig = make_signal(Asset::BTC, 0.80, Direction::Up);

        rm.open_position(&sig, 100.0, 0.50);

        // Price goes up 5% (0.50 * 1.05 = 0.525) -> should trigger take-profit
        let exit = rm.update_position(Asset::BTC, 0.525);
        assert_eq!(exit, Some(ExitReason::TakeProfit));
    }

    #[test]
    fn test_position_tracking_with_pnl_pct() {
        let rm = RiskManager::default();
        let sig = make_signal(Asset::BTC, 0.80, Direction::Up);
        let size = rm.calculate_position_size(&sig);

        rm.open_position(&sig, size, 50000.0);
        assert!(rm.has_position(Asset::BTC));

        rm.update_position(Asset::BTC, 51000.0);
        let pos = rm.get_position(Asset::BTC).unwrap();
        assert!(pos.pnl > 0.0);
        assert!((pos.pnl_pct - 0.02).abs() < 0.001); // ~2% gain

        let result = rm.close_position(Asset::BTC, 51000.0, ExitReason::Manual);
        assert!(result.is_some());
        assert!(!rm.has_position(Asset::BTC));
    }

    #[test]
    fn test_restore_position_rehydrates_live_state() {
        let rm = RiskManager::default();

        rm.restore_position(
            Asset::BTC,
            Direction::Up,
            2.5,
            0.50,
            0.51,
            1700000000000,
            1700003600000,
            "btc-test".to_string(),
            "token-1".to_string(),
        );

        assert!(rm.has_position(Asset::BTC));

        rm.update_position(Asset::BTC, 0.52);
        let position = rm.get_position(Asset::BTC).unwrap();
        assert!(position.pnl > 0.0);
        assert_eq!(position.market_slug, "btc-test");
        assert_eq!(position.token_id, "token-1");
    }

    #[test]
    fn test_restore_position_keeps_multiple_live_tokens_for_same_asset() {
        let rm = RiskManager::default();
        let now = Utc::now().timestamp_millis();

        rm.restore_position(
            Asset::BTC,
            Direction::Down,
            2.5,
            0.51,
            0.0,
            now - 1_000,
            now + 60_000,
            "btc-expired".to_string(),
            "token-expired".to_string(),
        );
        rm.restore_position(
            Asset::BTC,
            Direction::Up,
            2.55,
            0.51,
            0.52,
            now - 500,
            now + 120_000,
            "btc-active".to_string(),
            "token-active".to_string(),
        );

        assert!(rm.has_position_token("token-expired"));
        assert!(rm.has_position_token("token-active"));
        assert_eq!(rm.all_positions().len(), 2);

        let exit = rm.update_position_by_token_id("token-active", 0.52);
        assert!(exit.is_none());
        let active = rm.get_position_by_token_id("token-active").unwrap();
        assert_eq!(active.market_slug, "btc-active");
        assert!(active.pnl > 0.0);

        let expired = rm.get_position_by_token_id("token-expired").unwrap();
        assert_eq!(expired.current_price, 0.0);
    }

    #[test]
    fn test_checkpoint_take_profit_triggers_from_dynamic_floor() {
        let config = RiskConfig {
            checkpoint_arm_roi: 0.05,
            checkpoint_initial_floor_roi: 0.02,
            checkpoint_trail_gap_roi: 0.02,
            trailing_stop_pct: 0.20,
            hard_stop_roi: -0.20,
            max_hold_duration_ms: i64::MAX,
            ..Default::default()
        };
        let rm = RiskManager::new(config);
        let sig = make_signal(Asset::BTC, 0.85, Direction::Up);

        rm.open_position(&sig, 100.0, 0.50);

        let exit = rm.update_position(Asset::BTC, 0.53);
        assert!(exit.is_none()); // Checkpoint armed but not breached

        let position = rm.get_position(Asset::BTC).unwrap();
        assert!(position.checkpoint_armed);
        assert!(position.checkpoint_floor_roi >= 0.03); // Peak is 0.06 roi, floor is 0.04

        let exit = rm.update_position(Asset::BTC, 0.515); // Drops to 0.03 roi, below floor
        assert_eq!(exit, Some(ExitReason::CheckpointTakeProfit));
    }
}
