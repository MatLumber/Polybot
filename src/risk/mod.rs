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
            trailing_stop_pct: 0.03,         // 3% trailing stop
            take_profit_pct: 0.05,           // 5% take-profit
            max_hold_duration_ms: 7_200_000, // 2 hours
            kill_switch_enabled: true,
        }
    }
}

/// Exit reason for a position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    TrailingStop,
    TakeProfit,
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
    /// Current positions by asset
    positions: RwLock<HashMap<Asset, Position>>,
    /// Daily stats (RwLock for interior mutability so evaluate() can check without &mut self)
    daily_stats: RwLock<HashMap<String, DailyStats>>,
    /// Current date key
    current_date: RwLock<String>,
    /// Current available balance (USDC)
    current_balance: RwLock<f64>,
}

impl RiskManager {
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
            if positions.contains_key(&signal.asset) {
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
        let (stop_price, take_profit_price, peak_price) = match signal.direction {
            Direction::Up => (
                price * (1.0 - self.config.trailing_stop_pct),
                price * (1.0 + self.config.take_profit_pct),
                price,
            ),
            Direction::Down => (
                price * (1.0 + self.config.trailing_stop_pct),
                price * (1.0 - self.config.take_profit_pct),
                price,
            ),
        };

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
        };

        if let Ok(mut positions) = self.positions.write() {
            positions.insert(signal.asset, position);
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

    /// Update position with current price and return exit reason if position should close
    pub fn update_position(&self, asset: Asset, current_price: f64) -> Option<ExitReason> {
        let mut positions = self.positions.write().ok()?;
        let position = positions.get_mut(&asset)?;

        position.current_price = current_price;

        // Calculate PnL based on direction
        let price_diff = match position.direction {
            Direction::Up => current_price - position.entry_price,
            Direction::Down => position.entry_price - current_price,
        };
        position.pnl = (price_diff / position.entry_price) * position.size;
        position.pnl_pct = price_diff / position.entry_price;

        // Update peak price and trailing stop
        match position.direction {
            Direction::Up => {
                if current_price > position.peak_price {
                    position.peak_price = current_price;
                    position.stop_price = current_price * (1.0 - self.config.trailing_stop_pct);
                }
                // Check trailing stop (price dropped below stop)
                if current_price <= position.stop_price {
                    return Some(ExitReason::TrailingStop);
                }
                // Check take-profit
                if current_price >= position.take_profit_price {
                    return Some(ExitReason::TakeProfit);
                }
            }
            Direction::Down => {
                if current_price < position.peak_price {
                    position.peak_price = current_price;
                    position.stop_price = current_price * (1.0 + self.config.trailing_stop_pct);
                }
                // Check trailing stop (price rose above stop)
                if current_price >= position.stop_price {
                    return Some(ExitReason::TrailingStop);
                }
                // Check take-profit
                if current_price <= position.take_profit_price {
                    return Some(ExitReason::TakeProfit);
                }
            }
        }

        // Check time-based expiry
        let now = Utc::now().timestamp_millis();
        // Check market expiry first (if we have it)
        if position.expires_at > 0 && now >= position.expires_at {
            return Some(ExitReason::MarketExpiry);
        }
        // Fallback to max_hold_duration for backwards compatibility
        if now - position.opened_at > self.config.max_hold_duration_ms {
            return Some(ExitReason::TimeExpiry);
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
            positions.remove(&asset)?
        };

        let price_diff = match position.direction {
            Direction::Up => close_price - position.entry_price,
            Direction::Down => position.entry_price - close_price,
        };
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

        for (asset, pos) in positions.iter() {
            // Check time expiry (even if price hasn't updated)
            if now - pos.opened_at > self.config.max_hold_duration_ms {
                exits.push((*asset, ExitReason::TimeExpiry, pos.current_price));
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
            .map(|p| p.contains_key(&asset))
            .unwrap_or(false)
    }

    /// Get position for an asset
    pub fn get_position(&self, asset: Asset) -> Option<Position> {
        self.positions.read().ok()?.get(&asset).cloned()
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

        rm.open_position(&sig, 1000.0, 50000.0);

        // Price goes up - should update peak and stop
        let exit = rm.update_position(Asset::BTC, 52000.0);
        assert!(exit.is_none());

        // Price drops 3% from peak (52000) -> 50440 should trigger stop
        let exit = rm.update_position(Asset::BTC, 50440.0);
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

        rm.open_position(&sig, 1000.0, 50000.0);

        // Price goes down - good for short
        let exit = rm.update_position(Asset::BTC, 48000.0);
        assert!(exit.is_none());

        // Price rises 3% from low (48000) -> 49440+ should trigger stop
        let exit = rm.update_position(Asset::BTC, 49500.0);
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

        rm.open_position(&sig, 1000.0, 50000.0);

        // Price goes up 5% -> 52500 should trigger take-profit
        let exit = rm.update_position(Asset::BTC, 52500.0);
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
}
