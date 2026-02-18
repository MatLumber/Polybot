//! Backtesting Module
//!
//! Provides tools for testing strategy performance on historical data:
//! - Historical trade simulation
//! - Performance metrics calculation
//! - Win rate, profit factor, max drawdown analysis

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::features::{FeatureEngine, Features};
use crate::strategy::{GeneratedSignal, StrategyConfig, StrategyEngine, TradeResult};
use crate::types::{Asset, Candle, Direction, Timeframe};

/// Historical trade record for backtesting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestTrade {
    /// Entry timestamp
    pub entry_ts: i64,
    /// Exit timestamp
    pub exit_ts: i64,
    /// Asset traded
    pub asset: Asset,
    /// Timeframe used
    pub timeframe: Timeframe,
    /// Direction of trade
    pub direction: Direction,
    /// Entry price
    pub entry_price: f64,
    /// Exit price
    pub exit_price: f64,
    /// Signal confidence
    pub confidence: f64,
    /// PnL (profit/loss)
    pub pnl: f64,
    /// Is this a winning trade?
    pub is_win: bool,
    /// Indicators that contributed
    pub indicators_used: Vec<String>,
}

/// Backtest performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestMetrics {
    /// Total number of trades
    pub total_trades: usize,
    /// Number of winning trades
    pub wins: usize,
    /// Number of losing trades
    pub losses: usize,
    /// Win rate (0.0 to 1.0)
    pub win_rate: f64,
    /// Profit factor (gross profit / gross loss)
    pub profit_factor: f64,
    /// Maximum drawdown
    pub max_drawdown: f64,
    /// Sharpe ratio (simplified)
    pub sharpe_ratio: f64,
    /// Average win amount
    pub avg_win: f64,
    /// Average loss amount
    pub avg_loss: f64,
    /// Expectancy (average profit per trade)
    pub expectancy: f64,
    /// Largest win
    pub largest_win: f64,
    /// Largest loss
    pub largest_loss: f64,
    /// Average confidence
    pub avg_confidence: f64,
}

impl Default for BacktestMetrics {
    fn default() -> Self {
        Self {
            total_trades: 0,
            wins: 0,
            losses: 0,
            win_rate: 0.0,
            profit_factor: 0.0,
            max_drawdown: 0.0,
            sharpe_ratio: 0.0,
            avg_win: 0.0,
            avg_loss: 0.0,
            expectancy: 0.0,
            largest_win: 0.0,
            largest_loss: 0.0,
            avg_confidence: 0.0,
        }
    }
}

/// Backtest configuration
#[derive(Debug, Clone)]
pub struct BacktestConfig {
    /// Initial capital
    pub initial_capital: f64,
    /// Position size per trade (fixed)
    pub position_size: f64,
    /// Commission per trade (as fraction)
    pub commission: f64,
    /// Slippage per trade (as fraction)
    pub slippage: f64,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            initial_capital: 1000.0,
            position_size: 8.0,
            commission: 0.0,
            slippage: 0.0,
        }
    }
}

/// Backtester
pub struct Backtester {
    config: BacktestConfig,
    strategy_config: StrategyConfig,
    /// Historical candles by (asset, timeframe)
    historical_data: Vec<Candle>,
    /// Executed trades
    trades: Vec<BacktestTrade>,
    /// Equity curve
    equity_curve: Vec<f64>,
    /// Current equity
    current_equity: f64,
}

impl Backtester {
    pub fn new(config: BacktestConfig, strategy_config: StrategyConfig) -> Self {
        Self {
            config,
            strategy_config,
            historical_data: Vec::new(),
            trades: Vec::new(),
            equity_curve: Vec::new(),
            current_equity: 0.0,
        }
    }

    /// Load historical candles
    pub fn load_data(&mut self, candles: Vec<Candle>) {
        self.historical_data = candles;
    }

    /// Run backtest
    pub fn run(&mut self) -> BacktestMetrics {
        if self.historical_data.is_empty() {
            return BacktestMetrics::default();
        }

        self.trades.clear();
        self.equity_curve.clear();
        self.current_equity = self.config.initial_capital;

        let mut feature_engine = FeatureEngine::new();
        let mut strategy = StrategyEngine::new(self.strategy_config.clone());

        // Sort candles by time
        let mut candles = self.historical_data.clone();
        candles.sort_by_key(|c| c.open_time);

        // Group candles by (asset, timeframe)
        let mut candle_groups: std::collections::HashMap<(Asset, Timeframe), Vec<Candle>> =
            std::collections::HashMap::new();

        for candle in candles {
            let key = (candle.asset, candle.timeframe);
            candle_groups
                .entry(key)
                .or_insert_with(Vec::new)
                .push(candle);
        }

        // Process each group
        for (_key, mut group_candles) in candle_groups {
            group_candles.sort_by_key(|c| c.open_time);

            let window = group_candles[0].timeframe.duration_secs() * 1000;
            let lookback = 50; // Candles needed for indicators

            for i in lookback..group_candles.len() {
                let window_candles: Vec<Candle> = group_candles[..=i].to_vec();

                if let Some(features) = feature_engine.compute(&window_candles) {
                    if let Some(signal) = strategy.process(&features) {
                        // Simulate trade
                        let entry_candle = &group_candles[i];

                        // Find exit point (next window)
                        let exit_idx = (i + 15).min(group_candles.len() - 1);
                        let exit_candle = &group_candles[exit_idx];

                        let is_win = match signal.direction {
                            Direction::Up => exit_candle.close > entry_candle.open,
                            Direction::Down => exit_candle.close < entry_candle.open,
                        };

                        let pnl = if is_win {
                            self.config.position_size * 0.8 // Win: 80% profit on binary
                        } else {
                            -self.config.position_size // Loss: 100% loss on binary
                        };

                        // Apply commission and slippage
                        let net_pnl = pnl * (1.0 - self.config.commission - self.config.slippage);
                        self.current_equity += net_pnl;

                        let trade = BacktestTrade {
                            entry_ts: entry_candle.open_time,
                            exit_ts: exit_candle.open_time,
                            asset: signal.asset,
                            timeframe: signal.timeframe,
                            direction: signal.direction,
                            entry_price: entry_candle.open,
                            exit_price: exit_candle.close,
                            confidence: signal.confidence,
                            pnl: net_pnl,
                            is_win,
                            indicators_used: signal.indicators_used,
                        };

                        // Record trade result for calibration
                        let result = if is_win {
                            TradeResult::Win
                        } else {
                            TradeResult::Loss
                        };
                        strategy.record_trade_with_indicators(&trade.indicators_used, result);

                        self.trades.push(trade);
                        self.equity_curve.push(self.current_equity);
                    }
                }
            }
        }

        self.calculate_metrics()
    }

    /// Calculate performance metrics from trades
    pub fn calculate_metrics(&self) -> BacktestMetrics {
        if self.trades.is_empty() {
            return BacktestMetrics::default();
        }

        let wins: Vec<&BacktestTrade> = self.trades.iter().filter(|t| t.is_win).collect();
        let losses: Vec<&BacktestTrade> = self.trades.iter().filter(|t| !t.is_win).collect();

        let total_trades = self.trades.len();
        let win_count = wins.len();
        let loss_count = losses.len();
        let win_rate = win_count as f64 / total_trades as f64;

        let gross_profit: f64 = wins.iter().map(|t| t.pnl).sum();
        let gross_loss: f64 = losses.iter().map(|t| t.pnl.abs()).sum();
        let profit_factor = if gross_loss > 0.0 {
            gross_profit / gross_loss
        } else if gross_profit > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };

        let avg_win = if win_count > 0 {
            gross_profit / win_count as f64
        } else {
            0.0
        };

        let avg_loss = if loss_count > 0 {
            gross_loss / loss_count as f64
        } else {
            0.0
        };

        let total_pnl: f64 = self.trades.iter().map(|t| t.pnl).sum();
        let expectancy = total_pnl / total_trades as f64;

        let largest_win = wins.iter().map(|t| t.pnl).fold(0.0, f64::max);
        let largest_loss = losses.iter().map(|t| t.pnl.abs()).fold(0.0, f64::max);

        let avg_confidence: f64 =
            self.trades.iter().map(|t| t.confidence).sum::<f64>() / total_trades as f64;

        // Calculate max drawdown
        let mut max_equity = self.config.initial_capital;
        let mut max_drawdown: f64 = 0.0;

        for &equity in &self.equity_curve {
            max_equity = max_equity.max(equity);
            let drawdown = (max_equity - equity) / max_equity;
            max_drawdown = max_drawdown.max(drawdown);
        }

        // Simplified Sharpe ratio
        let returns: Vec<f64> = self
            .trades
            .iter()
            .map(|t| t.pnl / self.config.position_size)
            .collect();
        let avg_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance: f64 = returns
            .iter()
            .map(|r| (r - avg_return).powi(2))
            .sum::<f64>()
            / returns.len() as f64;
        let std_dev = variance.sqrt();
        let sharpe_ratio = if std_dev > 0.0 {
            (avg_return * 252.0) / (std_dev * 252.0_f64.sqrt()) // Annualized
        } else {
            0.0
        };

        BacktestMetrics {
            total_trades,
            wins: win_count,
            losses: loss_count,
            win_rate,
            profit_factor,
            max_drawdown,
            sharpe_ratio,
            avg_win,
            avg_loss,
            expectancy,
            largest_win,
            largest_loss,
            avg_confidence,
        }
    }

    /// Get all trades
    pub fn get_trades(&self) -> &[BacktestTrade] {
        &self.trades
    }

    /// Get equity curve
    pub fn get_equity_curve(&self) -> &[f64] {
        &self.equity_curve
    }

    /// Export trades to CSV format
    pub fn export_trades_csv(&self) -> String {
        let mut csv = String::from("entry_ts,exit_ts,asset,timeframe,direction,entry_price,exit_price,confidence,pnl,is_win\n");

        for trade in &self.trades {
            csv.push_str(&format!(
                "{},{},{:?},{:?},{:?},{},{},{},{},{}\n",
                trade.entry_ts,
                trade.exit_ts,
                trade.asset,
                trade.timeframe,
                trade.direction,
                trade.entry_price,
                trade.exit_price,
                trade.confidence,
                trade.pnl,
                trade.is_win
            ));
        }

        csv
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candle(asset: Asset, ts: i64, open: f64, close: f64) -> Candle {
        Candle {
            open_time: ts,
            close_time: ts + 900000,
            asset,
            timeframe: Timeframe::Min15,
            open,
            high: open.max(close) + 10.0,
            low: open.min(close) - 10.0,
            close,
            volume: 1000.0,
            trades: 100,
        }
    }

    #[test]
    fn test_backtester_creation() {
        let backtester = Backtester::new(BacktestConfig::default(), StrategyConfig::default());
        assert_eq!(backtester.trades.len(), 0);
    }

    #[test]
    fn test_empty_backtest() {
        let mut backtester = Backtester::new(BacktestConfig::default(), StrategyConfig::default());
        let metrics = backtester.run();
        assert_eq!(metrics.total_trades, 0);
    }

    #[test]
    fn test_metrics_default() {
        let metrics = BacktestMetrics::default();
        assert_eq!(metrics.total_trades, 0);
        assert_eq!(metrics.win_rate, 0.0);
    }

    #[test]
    fn test_export_csv() {
        let backtester = Backtester::new(BacktestConfig::default(), StrategyConfig::default());
        let csv = backtester.export_trades_csv();
        assert!(csv.contains("entry_ts"));
    }
}
