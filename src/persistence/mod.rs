//! CSV Persistence Module
//!
//! Handles storage of price data, signals, and trades for backtesting and analysis

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use csv::{ReaderBuilder, WriterBuilder};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{info, warn};

/// Price tick record for CSV storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceRecord {
    pub timestamp: i64,
    pub asset: String,
    pub price: f64,
    pub source: String,
    pub volume: Option<f64>,
}

/// Signal record for CSV storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalRecord {
    pub timestamp: i64,
    pub market_id: String,
    pub direction: String,
    pub confidence: f64,
    pub entry_price: f64,
    pub features_hash: String,
    #[serde(default)]
    pub token_id: Option<String>,
    #[serde(default)]
    pub market_slug: Option<String>,
    #[serde(default)]
    pub quote_bid: Option<f64>,
    #[serde(default)]
    pub quote_ask: Option<f64>,
    #[serde(default)]
    pub quote_mid: Option<f64>,
    #[serde(default)]
    pub quote_depth_top5: Option<f64>,
    #[serde(default)]
    pub spread: Option<f64>,
    #[serde(default)]
    pub edge_net: Option<f64>,
    #[serde(default)]
    pub rejection_reason: Option<String>,
}

/// Rejection record for execution diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectionRecord {
    pub timestamp: i64,
    pub signal_id: String,
    pub asset: String,
    pub timeframe: String,
    pub market_slug: String,
    pub token_id: String,
    pub reason: String,
    pub spread: f64,
    pub depth_top5: f64,
    pub p_market: f64,
    pub p_model: f64,
    pub edge_net: f64,
}

/// Trade record for CSV storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub timestamp: i64,
    pub market_id: String,
    pub token_id: String,
    pub side: String,
    pub price: f64,
    pub size: f64,
    pub outcome: Option<String>,
    pub pnl: Option<f64>,
    #[serde(default)]
    pub entry_bid: Option<f64>,
    #[serde(default)]
    pub entry_ask: Option<f64>,
    #[serde(default)]
    pub entry_mid: Option<f64>,
    #[serde(default)]
    pub exit_bid: Option<f64>,
    #[serde(default)]
    pub exit_ask: Option<f64>,
    #[serde(default)]
    pub exit_mid: Option<f64>,
    #[serde(default)]
    pub fee_open: Option<f64>,
    #[serde(default)]
    pub fee_close: Option<f64>,
    #[serde(default)]
    pub slippage_open: Option<f64>,
    #[serde(default)]
    pub slippage_close: Option<f64>,
    #[serde(default)]
    pub p_market: Option<f64>,
    #[serde(default)]
    pub p_model: Option<f64>,
    #[serde(default)]
    pub edge_net: Option<f64>,
    #[serde(default)]
    pub kelly_raw: Option<f64>,
    #[serde(default)]
    pub kelly_applied: Option<f64>,
    #[serde(default)]
    pub exit_reason_detail: Option<String>,
}

/// Performance metrics record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRecord {
    pub timestamp: i64,
    pub total_trades: u64,
    pub winning_trades: u64,
    pub total_pnl: f64,
    pub win_rate: f64,
    pub avg_pnl: f64,
    pub sharpe_ratio: Option<f64>,
}

/// Balance snapshot record for tracking balance over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceRecord {
    pub timestamp: i64,
    pub balance_usdc: f64,
    pub available_usdc: f64,
    pub locked_in_positions: f64,
    pub unrealized_pnl: f64,
    pub total_equity: f64,
}

/// Win/Loss record for internal winrate tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WinLossRecord {
    pub timestamp: i64,
    pub market_slug: String,
    pub token_id: String,
    pub entry_price: f64,
    pub exit_price: f64,
    pub size: f64,
    pub pnl: f64,
    /// Internal classification: "WIN" or "LOSS"
    pub internal_result: String,
    /// Why we exited: TAKE_PROFIT, STOP_LOSS, TIME_EXPIRY, MARKET_EXPIRY, MANUAL
    pub exit_reason: String,
    /// Polymarket's official result (only set when market resolves)
    pub official_result: Option<String>,
}

/// Periodic P&L summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnlSummaryRecord {
    pub timestamp: i64,
    pub period: String, // "hourly", "daily", "weekly", "monthly"
    pub period_start: i64,
    pub period_end: i64,
    pub starting_balance: f64,
    pub ending_balance: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub trades_count: u32,
    pub wins: u32,
    pub losses: u32,
    pub win_rate: f64,
}

/// Detailed paper trade analytics for strategy optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperAnalyticsRecord {
    pub timestamp: i64,
    pub trade_id: String,
    pub asset: String,
    pub timeframe: String,
    pub direction: String,
    pub confidence: f64,
    // Market window awareness
    pub market_open_ts: i64,
    pub market_close_ts: i64,
    pub entered_at_ts: i64,
    pub exited_at_ts: i64,
    pub time_in_market_secs: i64,
    pub time_remaining_at_entry_secs: i64,
    pub pct_of_window_used: f64,
    // Price data
    pub entry_price: f64,
    pub exit_price: f64,
    pub price_at_market_open: f64,
    pub peak_price: f64,
    pub trough_price: f64,
    pub price_move_pct: f64,
    // Technical indicators at entry
    pub rsi_at_entry: f64,
    pub macd_hist_at_entry: f64,
    pub bb_position_at_entry: f64,
    pub adx_at_entry: f64,
    pub stoch_rsi_at_entry: f64,
    pub volatility_at_entry: f64,
    pub regime_at_entry: String,
    pub momentum_at_entry: f64,
    pub relative_volume_at_entry: f64,
    // Trade outcome
    pub size_usdc: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub fee_paid: f64,
    pub result: String,
    pub exit_reason: String,
    // Running totals
    pub balance_after: f64,
    pub total_trades: u32,
    pub winrate_pct: f64,
    pub cumulative_pnl: f64,
    pub max_drawdown_pct: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub profit_factor: f64,
}

/// Options for destructive data reset.
#[derive(Debug, Clone, Copy)]
pub struct HardResetOptions {
    pub no_backup: bool,
    pub delete_prices: bool,
    pub delete_learning_state: bool,
    pub delete_paper_state: bool,
}

impl Default for HardResetOptions {
    fn default() -> Self {
        Self {
            no_backup: true,
            delete_prices: true,
            delete_learning_state: true,
            delete_paper_state: true,
        }
    }
}

/// CSV persistence manager
pub struct CsvPersistence {
    data_dir: PathBuf,
    price_writer: Arc<AsyncRwLock<csv::Writer<std::fs::File>>>,
    signal_writer: Arc<AsyncRwLock<csv::Writer<std::fs::File>>>,
    trade_writer: Arc<AsyncRwLock<csv::Writer<std::fs::File>>>,
    performance_writer: Arc<AsyncRwLock<csv::Writer<std::fs::File>>>,
    balance_writer: Arc<AsyncRwLock<csv::Writer<std::fs::File>>>,
    winloss_writer: Arc<AsyncRwLock<csv::Writer<std::fs::File>>>,
    pnl_summary_writer: Arc<AsyncRwLock<csv::Writer<std::fs::File>>>,
    paper_analytics_writer: Arc<AsyncRwLock<csv::Writer<std::fs::File>>>,
    rejection_writer: Arc<AsyncRwLock<csv::Writer<std::fs::File>>>,
}

impl CsvPersistence {
    /// Destructive reset of historical state and CSV data.
    /// If `no_backup = false`, a timestamped backup snapshot is created first.
    pub fn hard_reset_all_history(data_dir: &str, no_backup: bool) -> Result<()> {
        let options = HardResetOptions {
            no_backup,
            ..HardResetOptions::default()
        };
        Self::hard_reset_with_options(data_dir, options)
    }

    /// Destructive reset with granular switches for startup-controlled resets.
    pub fn hard_reset_with_options(data_dir: &str, options: HardResetOptions) -> Result<()> {
        let base = PathBuf::from(data_dir);
        fs::create_dir_all(&base).context("Failed to create data directory for hard reset")?;

        let mut backed_up_files = 0usize;
        let mut backed_up_csv = 0usize;
        let mut backup_dir_used: Option<PathBuf> = None;
        if !options.no_backup {
            let backup_dir = base.join("_backups").join(format!(
                "hard_reset_{}",
                Utc::now().format("%Y%m%d_%H%M%S_%3f")
            ));
            fs::create_dir_all(&backup_dir)
                .with_context(|| format!("Failed creating backup dir {}", backup_dir.display()))?;

            if options.delete_learning_state {
                for file_name in [
                    "calibrator_state.json",
                    "calibrator_state_v2.json",
                    ".market_learning_v2_reset.done",
                ] {
                    let src = base.join(file_name);
                    if src.exists() {
                        let dst = backup_dir.join(file_name);
                        fs::copy(&src, &dst).with_context(|| {
                            format!("Failed copying {} to {}", src.display(), dst.display())
                        })?;
                        backed_up_files += 1;
                    }
                }
            }
            if options.delete_paper_state {
                let src = base.join("paper_trading_state.json");
                if src.exists() {
                    let dst = backup_dir.join("paper_trading_state.json");
                    fs::copy(&src, &dst).with_context(|| {
                        format!("Failed copying {} to {}", src.display(), dst.display())
                    })?;
                    backed_up_files += 1;
                }
            }

            for (folder, enabled) in [
                ("prices", options.delete_prices),
                ("signals", options.delete_paper_state),
                ("trades", options.delete_paper_state),
                ("winloss", options.delete_paper_state),
                ("paper_analytics", options.delete_paper_state),
                ("performance", options.delete_paper_state),
                ("balance", options.delete_paper_state),
                ("pnl_summary", options.delete_paper_state),
                ("rejections", options.delete_paper_state),
            ] {
                if !enabled {
                    continue;
                }
                let src_dir = base.join(folder);
                if !src_dir.exists() {
                    continue;
                }
                let dst_dir = backup_dir.join(folder);
                fs::create_dir_all(&dst_dir).with_context(|| {
                    format!("Failed creating backup subdir {}", dst_dir.display())
                })?;

                for entry in fs::read_dir(&src_dir)
                    .with_context(|| format!("Failed reading {}", src_dir.display()))?
                {
                    let entry = entry?;
                    let src_path = entry.path();
                    if src_path.is_file()
                        && src_path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.eq_ignore_ascii_case("csv"))
                            .unwrap_or(false)
                    {
                        let dst_path = dst_dir.join(entry.file_name());
                        fs::copy(&src_path, &dst_path).with_context(|| {
                            format!(
                                "Failed copying {} to {}",
                                src_path.display(),
                                dst_path.display()
                            )
                        })?;
                        backed_up_csv += 1;
                    }
                }
            }

            backup_dir_used = Some(backup_dir);
        }

        // Root state files
        let mut deleted_files = 0usize;
        if options.delete_learning_state {
            for file_name in [
                "calibrator_state.json",
                "calibrator_state_v2.json",
                ".market_learning_v2_reset.done",
            ] {
                let path = base.join(file_name);
                if path.exists() {
                    fs::remove_file(&path)
                        .with_context(|| format!("Failed removing {}", path.display()))?;
                    deleted_files += 1;
                }
            }
        }
        if options.delete_paper_state {
            let path = base.join("paper_trading_state.json");
            if path.exists() {
                fs::remove_file(&path)
                    .with_context(|| format!("Failed removing {}", path.display()))?;
                deleted_files += 1;
            }
        }

        // CSV folders
        let mut cleaned_csv = 0usize;
        let targets = [
            ("prices", options.delete_prices),
            ("signals", options.delete_paper_state),
            ("trades", options.delete_paper_state),
            ("winloss", options.delete_paper_state),
            ("paper_analytics", options.delete_paper_state),
            ("performance", options.delete_paper_state),
            ("balance", options.delete_paper_state),
            ("pnl_summary", options.delete_paper_state),
            ("rejections", options.delete_paper_state),
        ];

        for (folder, enabled) in targets {
            let dir = base.join(folder);
            if !dir.exists() {
                fs::create_dir_all(&dir)
                    .with_context(|| format!("Failed creating {}", dir.display()))?;
                continue;
            }
            if !enabled {
                continue;
            }

            for entry in
                fs::read_dir(&dir).with_context(|| format!("Failed reading {}", dir.display()))?
            {
                let entry = entry?;
                let path = entry.path();
                if path.is_file()
                    && path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case("csv"))
                        .unwrap_or(false)
                {
                    fs::remove_file(&path)
                        .with_context(|| format!("Failed removing {}", path.display()))?;
                    cleaned_csv += 1;
                }
            }
        }

        // Always keep directory structure ready for immediate writes after reset.
        for folder in [
            "prices",
            "signals",
            "trades",
            "performance",
            "balance",
            "winloss",
            "pnl_summary",
            "paper_analytics",
            "rejections",
        ] {
            fs::create_dir_all(base.join(folder))
                .with_context(|| format!("Failed ensuring {} exists", folder))?;
        }

        info!(
            data_dir = %base.display(),
            backup_dir = backup_dir_used
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "none".to_string()),
            backed_up_files,
            backed_up_csv,
            deleted_files,
            cleaned_csv,
            "Hard reset completed"
        );
        Ok(())
    }

    /// Create a new CSV persistence manager
    pub fn new(data_dir: &str) -> Result<Self> {
        let data_dir = PathBuf::from(data_dir);

        // Create directory if it doesn't exist
        fs::create_dir_all(&data_dir).context("Failed to create data directory")?;

        // Create subdirectories
        fs::create_dir_all(data_dir.join("prices"))?;
        fs::create_dir_all(data_dir.join("signals"))?;
        fs::create_dir_all(data_dir.join("trades"))?;
        fs::create_dir_all(data_dir.join("performance"))?;
        fs::create_dir_all(data_dir.join("balance"))?;
        fs::create_dir_all(data_dir.join("winloss"))?;
        fs::create_dir_all(data_dir.join("pnl_summary"))?;
        fs::create_dir_all(data_dir.join("paper_analytics"))?;
        fs::create_dir_all(data_dir.join("rejections"))?;

        // Get current date for filenames
        let today = Utc::now().format("%Y-%m-%d");

        // Create writers
        let price_writer =
            Self::create_writer(&data_dir.join("prices"), &format!("prices_{}.csv", today))?;
        let signal_writer =
            Self::create_writer(&data_dir.join("signals"), &format!("signals_{}.csv", today))?;
        let trade_writer =
            Self::create_writer(&data_dir.join("trades"), &format!("trades_{}.csv", today))?;
        let performance_writer = Self::create_writer(
            &data_dir.join("performance"),
            &format!("performance_{}.csv", today),
        )?;
        let balance_writer =
            Self::create_writer(&data_dir.join("balance"), &format!("balance_{}.csv", today))?;
        let winloss_writer =
            Self::create_writer(&data_dir.join("winloss"), &format!("winloss_{}.csv", today))?;
        let pnl_summary_writer = Self::create_writer(
            &data_dir.join("pnl_summary"),
            &format!("pnl_summary_{}.csv", today),
        )?;
        let paper_analytics_writer = Self::create_writer(
            &data_dir.join("paper_analytics"),
            &format!("paper_analytics_{}.csv", today),
        )?;
        let rejection_writer = Self::create_writer(
            &data_dir.join("rejections"),
            &format!("rejections_{}.csv", today),
        )?;

        Ok(Self {
            data_dir,
            price_writer: Arc::new(AsyncRwLock::new(price_writer)),
            signal_writer: Arc::new(AsyncRwLock::new(signal_writer)),
            trade_writer: Arc::new(AsyncRwLock::new(trade_writer)),
            performance_writer: Arc::new(AsyncRwLock::new(performance_writer)),
            balance_writer: Arc::new(AsyncRwLock::new(balance_writer)),
            winloss_writer: Arc::new(AsyncRwLock::new(winloss_writer)),
            pnl_summary_writer: Arc::new(AsyncRwLock::new(pnl_summary_writer)),
            paper_analytics_writer: Arc::new(AsyncRwLock::new(paper_analytics_writer)),
            rejection_writer: Arc::new(AsyncRwLock::new(rejection_writer)),
        })
    }

    fn create_writer(dir: &Path, filename: &str) -> Result<csv::Writer<std::fs::File>> {
        let path = dir.join(filename);
        let file_has_data =
            path.exists() && fs::metadata(&path).map(|m| m.len() > 0).unwrap_or(false);

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(&path)
            .context("Failed to open CSV file")?;

        let writer = WriterBuilder::new()
            .has_headers(!file_has_data)
            .from_writer(file);

        Ok(writer)
    }

    /// Save a price tick
    pub async fn save_price(&self, record: PriceRecord) -> Result<()> {
        let mut writer = self.price_writer.write().await;
        writer
            .serialize(&record)
            .context("Failed to write price record")?;
        writer.flush().context("Failed to flush price writer")?;
        Ok(())
    }

    /// Save a signal
    pub async fn save_signal(&self, record: SignalRecord) -> Result<()> {
        let mut writer = self.signal_writer.write().await;
        writer
            .serialize(&record)
            .context("Failed to write signal record")?;
        writer.flush().context("Failed to flush signal writer")?;
        Ok(())
    }

    /// Save a rejection diagnostics row.
    pub async fn save_rejection(&self, record: RejectionRecord) -> Result<()> {
        let mut writer = self.rejection_writer.write().await;
        writer
            .serialize(&record)
            .context("Failed to write rejection record")?;
        writer.flush().context("Failed to flush rejection writer")?;
        Ok(())
    }

    /// Save a trade
    pub async fn save_trade(&self, record: TradeRecord) -> Result<()> {
        let mut writer = self.trade_writer.write().await;
        writer
            .serialize(&record)
            .context("Failed to write trade record")?;
        writer.flush().context("Failed to flush trade writer")?;
        Ok(())
    }

    /// Save performance metrics
    pub async fn save_performance(&self, record: PerformanceRecord) -> Result<()> {
        let mut writer = self.performance_writer.write().await;
        writer
            .serialize(&record)
            .context("Failed to write performance record")?;
        writer
            .flush()
            .context("Failed to flush performance writer")?;
        Ok(())
    }

    /// Save balance snapshot
    pub async fn save_balance(&self, record: BalanceRecord) -> Result<()> {
        let mut writer = self.balance_writer.write().await;
        writer
            .serialize(&record)
            .context("Failed to write balance record")?;
        writer.flush().context("Failed to flush balance writer")?;
        Ok(())
    }

    /// Save win/loss record for internal winrate tracking
    pub async fn save_winloss(&self, record: WinLossRecord) -> Result<()> {
        let mut writer = self.winloss_writer.write().await;
        writer
            .serialize(&record)
            .context("Failed to write winloss record")?;
        writer.flush().context("Failed to flush winloss writer")?;
        Ok(())
    }

    /// Save P&L summary
    pub async fn save_pnl_summary(&self, record: PnlSummaryRecord) -> Result<()> {
        let mut writer = self.pnl_summary_writer.write().await;
        writer
            .serialize(&record)
            .context("Failed to write pnl_summary record")?;
        writer
            .flush()
            .context("Failed to flush pnl_summary writer")?;
        Ok(())
    }

    /// Save paper trading analytics (detailed record for strategy optimization)
    pub async fn save_paper_analytics(&self, record: PaperAnalyticsRecord) -> Result<()> {
        let mut writer = self.paper_analytics_writer.write().await;
        writer
            .serialize(&record)
            .context("Failed to write paper_analytics record")?;
        writer
            .flush()
            .context("Failed to flush paper_analytics writer")?;
        Ok(())
    }

    /// Load price history from CSV
    pub fn load_price_history(&self, asset: &str, days: u32) -> Result<Vec<PriceRecord>> {
        let mut records = Vec::new();

        for i in 0..days {
            let date = Utc::now() - chrono::Duration::days(i as i64);
            let filename = format!("prices_{}.csv", date.format("%Y-%m-%d"));
            let path = self.data_dir.join("prices").join(&filename);

            if path.exists() {
                let file = std::fs::File::open(&path).context("Failed to open price file")?;
                let mut reader = ReaderBuilder::new().has_headers(true).from_reader(file);

                for result in reader.deserialize() {
                    let record: PriceRecord =
                        result.context("Failed to deserialize price record")?;
                    if record.asset == asset {
                        records.push(record);
                    }
                }
            }
        }

        // Sort by timestamp
        records.sort_by_key(|r| r.timestamp);

        Ok(records)
    }

    /// Load trade history from CSV
    pub fn load_trade_history(&self, days: u32) -> Result<Vec<TradeRecord>> {
        let mut records = Vec::new();

        for i in 0..days {
            let date = Utc::now() - chrono::Duration::days(i as i64);
            let filename = format!("trades_{}.csv", date.format("%Y-%m-%d"));
            let path = self.data_dir.join("trades").join(&filename);

            if path.exists() {
                let file = std::fs::File::open(&path).context("Failed to open trade file")?;
                let mut reader = ReaderBuilder::new().has_headers(true).from_reader(file);

                for result in reader.deserialize() {
                    let record: TradeRecord =
                        result.context("Failed to deserialize trade record")?;
                    records.push(record);
                }
            }
        }

        records.sort_by_key(|r| r.timestamp);
        Ok(records)
    }

    /// Load signal history from CSV
    pub fn load_signal_history(&self, market_id: &str, days: u32) -> Result<Vec<SignalRecord>> {
        let mut records = Vec::new();

        for i in 0..days {
            let date = Utc::now() - chrono::Duration::days(i as i64);
            let filename = format!("signals_{}.csv", date.format("%Y-%m-%d"));
            let path = self.data_dir.join("signals").join(&filename);

            if path.exists() {
                let file = std::fs::File::open(&path).context("Failed to open signal file")?;
                let mut reader = ReaderBuilder::new().has_headers(true).from_reader(file);

                for result in reader.deserialize() {
                    let record: SignalRecord =
                        result.context("Failed to deserialize signal record")?;
                    if record.market_id == market_id {
                        records.push(record);
                    }
                }
            }
        }

        records.sort_by_key(|r| r.timestamp);
        Ok(records)
    }

    /// Calculate performance metrics from trade history
    pub fn calculate_performance(&self, days: u32) -> Result<PerformanceRecord> {
        let trades = self.load_trade_history(days)?;

        let total_trades = trades.len() as u64;
        let winning_trades = trades
            .iter()
            .filter(|t| t.pnl.map(|p| p > 0.0).unwrap_or(false))
            .count() as u64;

        let total_pnl: f64 = trades.iter().filter_map(|t| t.pnl).sum();

        let win_rate = if total_trades > 0 {
            winning_trades as f64 / total_trades as f64
        } else {
            0.0
        };

        let avg_pnl = if total_trades > 0 {
            total_pnl / total_trades as f64
        } else {
            0.0
        };

        // Calculate Sharpe ratio (simplified)
        let sharpe_ratio = None; // Would need more data points

        Ok(PerformanceRecord {
            timestamp: Utc::now().timestamp(),
            total_trades,
            winning_trades,
            total_pnl,
            win_rate,
            avg_pnl,
            sharpe_ratio,
        })
    }

    /// Export trades for analysis
    pub fn export_trades(&self, output_path: &str) -> Result<()> {
        let trades = self.load_trade_history(30)?;

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(output_path)
            .context("Failed to create export file")?;

        let mut writer = WriterBuilder::new().has_headers(true).from_writer(file);

        for trade in &trades {
            writer.serialize(&trade)?;
        }

        writer.flush()?;
        info!("Exported {} trades to {}", trades.len(), output_path);

        Ok(())
    }

    /// Load recent paper trades from paper_analytics CSV files
    /// Returns trades formatted for dashboard display
    #[cfg(feature = "dashboard")]
    pub fn load_recent_paper_trades(
        &self,
        limit: usize,
    ) -> Result<Vec<crate::dashboard::TradeResponse>> {
        use crate::dashboard::TradeResponse;

        let mut trades = Vec::new();
        for path in self.collect_dashboard_csv_files("paper_analytics", "paper_analytics_") {
            let file = std::fs::File::open(&path)
                .with_context(|| format!("Failed to open {}", path.display()))?;
            let mut reader = ReaderBuilder::new().has_headers(false).from_reader(file);

            for result in reader.deserialize() {
                match result {
                    Ok(record) => {
                        let analytics: PaperAnalyticsRecord = record;
                        trades.push(TradeResponse {
                            timestamp: analytics.timestamp,
                            trade_id: analytics.trade_id,
                            asset: analytics.asset,
                            timeframe: analytics.timeframe,
                            direction: analytics.direction,
                            confidence: analytics.confidence,
                            entry_price: analytics.entry_price,
                            exit_price: analytics.exit_price,
                            size_usdc: analytics.size_usdc,
                            pnl: analytics.pnl,
                            pnl_pct: analytics.pnl_pct,
                            result: analytics.result,
                            exit_reason: analytics.exit_reason,
                            hold_duration_secs: analytics.time_in_market_secs,
                            balance_after: analytics.balance_after,
                            rsi_at_entry: Some(analytics.rsi_at_entry),
                            macd_hist_at_entry: Some(analytics.macd_hist_at_entry),
                            bb_position_at_entry: Some(analytics.bb_position_at_entry),
                            adx_at_entry: Some(analytics.adx_at_entry),
                            volatility_at_entry: Some(analytics.volatility_at_entry),
                        });
                    }
                    Err(e) => {
                        warn!("Failed to deserialize paper_analytics record: {}", e);
                    }
                }
            }
        }

        // Fallback: if paper_analytics is empty, rebuild from winloss + trades CSV.
        if trades.is_empty() {
            trades = self.load_recent_trades_fallback_session()?;
        }

        // Sort by timestamp descending (most recent first)
        trades.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Dedupe by trade_id (keep newest row)
        let mut seen = std::collections::HashSet::new();
        trades.retain(|trade| seen.insert(trade.trade_id.clone()));

        // Limit to N most recent trades
        trades.truncate(limit);

        info!(
            "Loaded {} recent paper trades for dashboard session",
            trades.len()
        );
        Ok(trades)
    }

    #[cfg(feature = "dashboard")]
    fn load_recent_trades_fallback_session(&self) -> Result<Vec<crate::dashboard::TradeResponse>> {
        use crate::dashboard::TradeResponse;

        let mut trades = Vec::new();
        let mut trade_meta: std::collections::HashMap<
            String,
            (i64, String, String, f64, f64, String, f64),
        > = std::collections::HashMap::new();

        for path in self.collect_dashboard_csv_files("trades", "trades_") {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            for line in content.lines() {
                let cols: Vec<&str> = line.split(',').map(str::trim).collect();
                if cols.len() < 8 {
                    continue;
                }
                let timestamp = match cols[0].parse::<i64>() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let market_slug = cols[1].to_string();
                let trade_id = cols[2].to_string();
                let direction = Self::normalize_direction(cols[3]);
                let price = cols[4].parse::<f64>().unwrap_or(0.0);
                let size_usdc = cols[5].parse::<f64>().unwrap_or(0.0);
                let result = cols[6].to_string();
                let pnl = cols[7].parse::<f64>().unwrap_or(0.0);
                trade_meta.insert(
                    trade_id,
                    (
                        timestamp,
                        direction,
                        market_slug,
                        price,
                        size_usdc,
                        result,
                        pnl,
                    ),
                );
            }
        }

        // Prefer winloss (has entry/exit/exit_reason), enrich with direction from trades.
        for path in self.collect_dashboard_csv_files("winloss", "winloss_") {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            for line in content.lines() {
                let cols: Vec<&str> = line.split(',').map(str::trim).collect();
                if cols.len() < 9 {
                    continue;
                }

                let ts_secs = match cols[0].parse::<i64>() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let fallback_ts = ts_secs * 1000;

                let market_slug = cols[1].to_string();
                let trade_id = cols[2].to_string();
                let entry_price = match cols[3].parse::<f64>() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let exit_price = match cols[4].parse::<f64>() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let size_usdc = match cols[5].parse::<f64>() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let pnl = match cols[6].parse::<f64>() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let result = cols[7].to_string();
                let exit_reason = cols[8].to_string();

                let (asset, timeframe) = Self::parse_market_slug(&market_slug);
                let (timestamp, direction) = trade_meta
                    .get(&trade_id)
                    .map(|(ts, dir, _, _, _, _, _)| (*ts, dir.clone()))
                    .unwrap_or((fallback_ts, "Unknown".to_string()));

                let pnl_pct = if size_usdc.abs() > f64::EPSILON {
                    (pnl / size_usdc) * 100.0
                } else {
                    0.0
                };

                trades.push(TradeResponse {
                    timestamp,
                    trade_id,
                    asset,
                    timeframe,
                    direction,
                    confidence: 0.0,
                    entry_price,
                    exit_price,
                    size_usdc,
                    pnl,
                    pnl_pct,
                    result,
                    exit_reason,
                    hold_duration_secs: 0,
                    balance_after: 0.0,
                    rsi_at_entry: None,
                    macd_hist_at_entry: None,
                    bb_position_at_entry: None,
                    adx_at_entry: None,
                    volatility_at_entry: None,
                });
            }
        }

        // If winloss is empty, fallback to bare trades rows.
        if trades.is_empty() {
            for (trade_id, (timestamp, direction, market_slug, price, size_usdc, result, pnl)) in
                trade_meta
            {
                let (asset, timeframe) = Self::parse_market_slug(&market_slug);
                let pnl_pct = if size_usdc.abs() > f64::EPSILON {
                    (pnl / size_usdc) * 100.0
                } else {
                    0.0
                };
                trades.push(TradeResponse {
                    timestamp,
                    trade_id: trade_id.clone(),
                    asset: asset.clone(),
                    timeframe: timeframe.clone(),
                    direction: direction.clone(),
                    confidence: 0.0,
                    entry_price: price,
                    exit_price: price,
                    size_usdc,
                    pnl,
                    pnl_pct,
                    result,
                    exit_reason: "UNKNOWN".to_string(),
                    hold_duration_secs: 0,
                    balance_after: 0.0,
                    rsi_at_entry: None,
                    macd_hist_at_entry: None,
                    bb_position_at_entry: None,
                    adx_at_entry: None,
                    volatility_at_entry: None,
                });
            }
        }

        Ok(trades)
    }

    #[cfg(feature = "dashboard")]
    fn collect_dashboard_csv_files(&self, folder: &str, prefix: &str) -> Vec<PathBuf> {
        let dir = self.data_dir.join(folder);
        let mut files: Vec<PathBuf> = Vec::new();
        if !dir.exists() {
            return files;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            return files;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.starts_with(prefix) || !name.ends_with(".csv") {
                continue;
            }
            files.push(path);
        }
        files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
        files
    }
    #[cfg(feature = "dashboard")]
    fn parse_market_slug(raw: &str) -> (String, String) {
        let lower = raw.trim().to_ascii_lowercase();
        let parts: Vec<&str> = lower.split('-').collect();

        let asset = if lower.contains("btc") || lower.contains("bitcoin") {
            "BTC".to_string()
        } else if lower.contains("eth") || lower.contains("ethereum") {
            "ETH".to_string()
        } else if lower.contains("sol") || lower.contains("solana") {
            "SOL".to_string()
        } else if lower.contains("xrp") || lower.contains("ripple") {
            "XRP".to_string()
        } else {
            parts
                .first()
                .copied()
                .unwrap_or("unknown")
                .to_ascii_uppercase()
        };

        // Match Polymarket-native cadence encodings first.
        let timeframe = if lower.contains("15m")
            || lower.contains("15min")
            || lower.contains("15-min")
            || lower.contains("15_min")
            || lower.contains("15 minute")
            || lower.contains("15-minute")
            || lower.contains("updown-15m")
        {
            "15m".to_string()
        } else if lower.contains("1h")
            || lower.contains("hour1")
            || lower.contains("1-hour")
            || lower.contains("1_hour")
            || lower.contains("60m")
            || lower.contains("60 min")
            || lower.contains("1 hour")
            || lower.contains("updown-1h")
            || Self::looks_like_hourly_updown_slug(&lower)
        {
            "1h".to_string()
        } else {
            parts.get(1).copied().unwrap_or("unknown").to_string()
        };

        (asset, timeframe)
    }

    #[cfg(feature = "dashboard")]
    fn looks_like_hourly_updown_slug(lower: &str) -> bool {
        let is_updown = lower.contains("up-or-down") || lower.contains("up or down");
        if !is_updown {
            return false;
        }

        if lower.contains("15m")
            || lower.contains("15min")
            || lower.contains("updown-15m")
            || lower.contains("5m")
            || lower.contains("updown-5m")
            || lower.contains("4h")
            || lower.contains("updown-4h")
        {
            return false;
        }

        // Current Polymarket hourly slugs/titles often encode only a clock marker.
        lower.contains("am-et")
            || lower.contains("pm-et")
            || lower.contains("am et")
            || lower.contains("pm et")
    }

    #[cfg(feature = "dashboard")]
    fn normalize_direction(raw: &str) -> String {
        match raw.trim().to_lowercase().as_str() {
            "up" | "buy" => "Up".to_string(),
            "down" | "sell" => "Down".to_string(),
            _ => "Unknown".to_string(),
        }
    }

    /// Load recent price history from CSV files for dashboard chart bootstrap.
    /// Supports files with or without headers.
    #[cfg(feature = "dashboard")]
    pub fn load_recent_price_history(
        &self,
        assets: &[crate::types::Asset],
        window_secs: i64,
    ) -> Result<Vec<(crate::types::Asset, i64, f64, String)>> {
        let mut rows: Vec<(crate::types::Asset, i64, f64, String)> = Vec::new();
        let now = Utc::now();
        let start_ts = now.timestamp_millis() - (window_secs.max(60) * 1000);
        let requested_assets: std::collections::HashSet<String> =
            assets.iter().map(|asset| asset.to_string()).collect();

        // Load up to the last 2 calendar days to safely span midnight.
        for day_offset in 0..=2 {
            let date = now - chrono::Duration::days(day_offset);
            let filename = format!("prices_{}.csv", date.format("%Y-%m-%d"));
            let path = self.data_dir.join("prices").join(&filename);

            if !path.exists() {
                continue;
            }

            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read price history file {}", path.display()))?;

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let cols: Vec<&str> = trimmed.split(',').map(str::trim).collect();
                if cols.len() < 4 {
                    continue;
                }

                let timestamp = match cols[0].parse::<i64>() {
                    Ok(ts) => ts,
                    Err(_) => continue, // header or malformed line
                };

                if timestamp < start_ts {
                    continue;
                }

                let asset_symbol = cols[1].to_uppercase();
                if !requested_assets.contains(&asset_symbol) {
                    continue;
                }
                let asset = match crate::types::Asset::from_str(&asset_symbol) {
                    Some(asset) => asset,
                    None => continue,
                };

                let price = match cols[2].parse::<f64>() {
                    Ok(price) => price,
                    Err(_) => continue,
                };

                let source = cols
                    .get(3)
                    .map(|value| value.trim().trim_matches('"'))
                    .filter(|value| !value.is_empty())
                    .unwrap_or("Unknown")
                    .to_string();

                rows.push((asset, timestamp, price, source));
            }
        }

        rows.sort_by(|a, b| a.1.cmp(&b.1));
        Ok(rows)
    }
}

/// Convert crate types to CSV record types
impl From<crate::types::PriceTick> for PriceRecord {
    fn from(tick: crate::types::PriceTick) -> Self {
        Self {
            timestamp: tick.exchange_ts,
            asset: tick.asset.to_string(),
            price: tick.mid,
            source: tick.source.to_string(),
            volume: None,
        }
    }
}

impl From<crate::types::Signal> for SignalRecord {
    fn from(signal: crate::types::Signal) -> Self {
        let market_id = if !signal.condition_id.trim().is_empty() {
            signal.condition_id.clone()
        } else {
            signal.market_slug.clone()
        };
        Self {
            timestamp: signal.ts,
            market_id,
            direction: signal.direction.to_string(),
            confidence: signal.confidence,
            entry_price: signal.features.vwap, // Use VWAP as proxy
            features_hash: signal.id.clone(),
            token_id: if signal.token_id.trim().is_empty() {
                None
            } else {
                Some(signal.token_id.clone())
            },
            market_slug: if signal.market_slug.trim().is_empty() {
                None
            } else {
                Some(signal.market_slug.clone())
            },
            quote_bid: Some(signal.quote_bid),
            quote_ask: Some(signal.quote_ask),
            quote_mid: Some(signal.quote_mid),
            quote_depth_top5: Some(signal.quote_depth_top5),
            spread: if signal.quote_bid > 0.0 && signal.quote_ask > 0.0 {
                Some((signal.quote_ask - signal.quote_bid).max(0.0))
            } else {
                None
            },
            edge_net: None,
            rejection_reason: None,
        }
    }
}

impl From<crate::types::Trade> for TradeRecord {
    fn from(trade: crate::types::Trade) -> Self {
        Self {
            timestamp: trade.ts_open,
            market_id: trade.market_slug.clone(),
            token_id: trade.id.clone(), // Use trade ID as token_id field
            side: trade.side.to_string(),
            price: trade.entry_px,
            size: trade.size,
            outcome: Some(trade.result.to_string()),
            pnl: Some(trade.pnl_usdc),
            entry_bid: None,
            entry_ask: None,
            entry_mid: None,
            exit_bid: None,
            exit_ask: None,
            exit_mid: None,
            fee_open: None,
            fee_close: None,
            slippage_open: None,
            slippage_close: None,
            p_market: None,
            p_model: None,
            edge_net: None,
            kelly_raw: None,
            kelly_applied: None,
            exit_reason_detail: None,
        }
    }
}

// ============================================================================
// BALANCE TRACKER
// ============================================================================

/// Balance tracker for tracking balance over time and calculating P&L
pub struct BalanceTracker {
    /// Initial balance when tracking started
    initial_balance: RwLock<f64>,
    /// Current available balance
    current_balance: RwLock<f64>,
    /// Balance locked in positions
    locked_balance: RwLock<f64>,
    /// Last snapshot timestamp
    last_snapshot: RwLock<i64>,
    /// Win/loss history for internal winrate
    winloss_history: RwLock<Vec<WinLossRecord>>,
    /// Balance history snapshots
    balance_history: RwLock<Vec<BalanceRecord>>,
    /// Starting timestamp
    started_at: RwLock<i64>,
}

impl Default for BalanceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl BalanceTracker {
    pub fn new() -> Self {
        Self {
            initial_balance: RwLock::new(0.0),
            current_balance: RwLock::new(0.0),
            locked_balance: RwLock::new(0.0),
            last_snapshot: RwLock::new(0),
            winloss_history: RwLock::new(Vec::new()),
            balance_history: RwLock::new(Vec::new()),
            started_at: RwLock::new(0),
        }
    }

    /// Initialize tracker with starting balance
    pub fn initialize(&self, balance: f64) {
        let now = Utc::now().timestamp_millis();
        if let Ok(mut initial) = self.initial_balance.write() {
            *initial = balance;
        }
        if let Ok(mut current) = self.current_balance.write() {
            *current = balance;
        }
        if let Ok(mut started) = self.started_at.write() {
            *started = now;
        }
        if let Ok(mut last) = self.last_snapshot.write() {
            *last = now;
        }
        info!("📊 BalanceTracker initialized with ${:.2} USDC", balance);
    }

    /// Update current balance
    pub fn update_balance(&self, available: f64, locked_amount: f64) {
        if let Ok(mut current) = self.current_balance.write() {
            *current = available;
        }
        if let Ok(mut locked) = self.locked_balance.write() {
            *locked = locked_amount;
        }
    }

    /// Record a balance snapshot
    pub fn record_snapshot(&self) -> Option<BalanceRecord> {
        let now = Utc::now().timestamp_millis();

        let (available, locked, initial) = {
            let available = *self.current_balance.read().ok()?;
            let locked = *self.locked_balance.read().ok()?;
            let initial = *self.initial_balance.read().ok()?;
            (available, locked, initial)
        };

        let total_equity = available + locked;
        let unrealized_pnl = total_equity - initial;

        let record = BalanceRecord {
            timestamp: now,
            balance_usdc: initial,
            available_usdc: available,
            locked_in_positions: locked,
            unrealized_pnl,
            total_equity,
        };

        if let Ok(mut history) = self.balance_history.write() {
            history.push(record.clone());
            // Keep last 10000 records
            if history.len() > 10000 {
                history.remove(0);
            }
        }
        if let Ok(mut last) = self.last_snapshot.write() {
            *last = now;
        }

        Some(record)
    }

    /// Record a win/loss result
    pub fn record_winloss(&self, record: WinLossRecord) {
        if let Ok(mut history) = self.winloss_history.write() {
            history.push(record);
            // Keep last 1000 records
            if history.len() > 1000 {
                history.remove(0);
            }
        }
    }

    /// Calculate internal winrate from recorded win/losses
    pub fn calculate_winrate(&self) -> (u32, u32, f64) {
        if let Ok(history) = self.winloss_history.read() {
            let wins = history
                .iter()
                .filter(|r| r.internal_result == "WIN")
                .count() as u32;
            let losses = history
                .iter()
                .filter(|r| r.internal_result == "LOSS")
                .count() as u32;
            let total = wins + losses;
            let winrate = if total > 0 {
                wins as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            (wins, losses, winrate)
        } else {
            (0, 0, 0.0)
        }
    }

    /// Get P&L since start
    pub fn get_total_pnl(&self) -> Option<(f64, f64)> {
        let current = *self.current_balance.read().ok()?;
        let initial = *self.initial_balance.read().ok()?;
        let pnl = current - initial;
        let pnl_pct = if initial > 0.0 {
            (pnl / initial) * 100.0
        } else {
            0.0
        };
        Some((pnl, pnl_pct))
    }

    /// Calculate P&L for a time period
    pub fn calculate_period_pnl(&self, period: &str) -> Option<PnlSummaryRecord> {
        let now = Utc::now().timestamp_millis();
        let period_start = match period {
            "hourly" => now - 3_600_000,      // 1 hour
            "daily" => now - 86_400_000,      // 24 hours
            "weekly" => now - 604_800_000,    // 7 days
            "monthly" => now - 2_592_000_000, // 30 days
            _ => return None,
        };

        let history = self.balance_history.read().ok()?;

        // Find closest balance at period start
        let starting_balance = history
            .iter()
            .filter(|r| r.timestamp >= period_start)
            .min_by_key(|r| r.timestamp)
            .map(|r| r.total_equity)
            .unwrap_or_else(|| self.initial_balance.read().map(|g| *g).unwrap_or(0.0));

        let ending_balance = history
            .iter()
            .filter(|r| r.timestamp <= now)
            .max_by_key(|r| r.timestamp)
            .map(|r| r.total_equity)
            .unwrap_or_else(|| self.current_balance.read().map(|g| *g).unwrap_or(0.0));

        let pnl = ending_balance - starting_balance;
        let pnl_pct = if starting_balance > 0.0 {
            (pnl / starting_balance) * 100.0
        } else {
            0.0
        };

        // Count trades in period from winloss history
        let winloss = self.winloss_history.read().ok()?;
        let period_trades: Vec<_> = winloss
            .iter()
            .filter(|r| r.timestamp >= period_start && r.timestamp <= now)
            .collect();

        let trades_count = period_trades.len() as u32;
        let wins = period_trades
            .iter()
            .filter(|r| r.internal_result == "WIN")
            .count() as u32;
        let losses = period_trades
            .iter()
            .filter(|r| r.internal_result == "LOSS")
            .count() as u32;
        let win_rate = if trades_count > 0 {
            wins as f64 / trades_count as f64 * 100.0
        } else {
            0.0
        };

        Some(PnlSummaryRecord {
            timestamp: now,
            period: period.to_string(),
            period_start,
            period_end: now,
            starting_balance,
            ending_balance,
            pnl,
            pnl_pct,
            trades_count,
            wins,
            losses,
            win_rate,
        })
    }

    /// Get formatted stats string
    pub fn get_stats_string(&self) -> String {
        let (pnl, pnl_pct) = self.get_total_pnl().unwrap_or((0.0, 0.0));
        let (wins, losses, winrate) = self.calculate_winrate();

        let initial = self.initial_balance.read().map(|g| *g).unwrap_or(0.0);
        let current = self.current_balance.read().map(|g| *g).unwrap_or(0.0);

        format!(
            "💰 Balance: ${:.2} / ${:.2} inicial | P&L: ${:.2} ({:+.2}%) | Winrate: {:.1}% ({}/{})",
            current,
            initial,
            pnl,
            pnl_pct,
            winrate,
            wins,
            wins + losses
        )
    }

    /// Get all period summaries
    pub fn get_all_period_summaries(&self) -> Vec<PnlSummaryRecord> {
        vec![
            self.calculate_period_pnl("hourly"),
            self.calculate_period_pnl("daily"),
            self.calculate_period_pnl("weekly"),
            self.calculate_period_pnl("monthly"),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

/// Classify exit reason as WIN or LOSS based on PnL
pub fn classify_trade_result(pnl: f64) -> &'static str {
    if pnl >= 0.0 {
        "WIN"
    } else {
        "LOSS"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_data_dir(test_name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "polybot_persistence_{}_{}",
            test_name,
            uuid::Uuid::new_v4()
        ))
    }

    #[test]
    fn hard_reset_all_history_removes_state_and_csv_files() {
        let data_dir = temp_data_dir("hard_reset");
        fs::create_dir_all(&data_dir).unwrap();

        // Root state files
        for file in [
            "calibrator_state.json",
            "calibrator_state_v2.json",
            "paper_trading_state.json",
            ".market_learning_v2_reset.done",
        ] {
            fs::write(data_dir.join(file), "x").unwrap();
        }

        // CSV files across all tracked folders
        for folder in [
            "prices",
            "signals",
            "trades",
            "winloss",
            "paper_analytics",
            "performance",
            "balance",
            "pnl_summary",
            "rejections",
        ] {
            let dir = data_dir.join(folder);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("sample.csv"), "a,b\n1,2\n").unwrap();
        }

        CsvPersistence::hard_reset_all_history(data_dir.to_str().unwrap(), true).unwrap();

        for file in [
            "calibrator_state.json",
            "calibrator_state_v2.json",
            "paper_trading_state.json",
            ".market_learning_v2_reset.done",
        ] {
            assert!(
                !data_dir.join(file).exists(),
                "expected {} to be removed",
                file
            );
        }

        for folder in [
            "prices",
            "signals",
            "trades",
            "winloss",
            "paper_analytics",
            "performance",
            "balance",
            "pnl_summary",
            "rejections",
        ] {
            let dir = data_dir.join(folder);
            assert!(dir.exists(), "expected {} directory to exist", folder);
            let has_csv = fs::read_dir(&dir)
                .unwrap()
                .filter_map(|entry| entry.ok().map(|e| e.path()))
                .any(|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case("csv"))
                        .unwrap_or(false)
                });
            assert!(!has_csv, "expected no CSVs left in {}", folder);
        }

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn hard_reset_with_backup_creates_snapshot_before_cleanup() {
        let data_dir = temp_data_dir("hard_reset_backup");
        fs::create_dir_all(&data_dir).unwrap();
        fs::write(data_dir.join("calibrator_state_v2.json"), "{}").unwrap();
        fs::create_dir_all(data_dir.join("prices")).unwrap();
        fs::write(
            data_dir.join("prices").join("prices_2026-01-01.csv"),
            "a,b\n1,2\n",
        )
        .unwrap();

        CsvPersistence::hard_reset_with_options(
            data_dir.to_str().unwrap(),
            HardResetOptions {
                no_backup: false,
                delete_prices: true,
                delete_learning_state: true,
                delete_paper_state: false,
            },
        )
        .unwrap();

        let backups_root = data_dir.join("_backups");
        assert!(backups_root.exists(), "expected backups root to exist");
        let backup_dirs: Vec<PathBuf> = fs::read_dir(&backups_root)
            .unwrap()
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        assert_eq!(backup_dirs.len(), 1, "expected exactly one backup snapshot");

        let backup_dir = &backup_dirs[0];
        assert!(
            backup_dir.join("calibrator_state_v2.json").exists(),
            "expected state file in backup"
        );
        assert!(
            backup_dir
                .join("prices")
                .join("prices_2026-01-01.csv")
                .exists(),
            "expected CSV file in backup"
        );
        assert!(
            !data_dir.join("calibrator_state_v2.json").exists(),
            "expected state file removed after reset"
        );
        assert!(
            !data_dir
                .join("prices")
                .join("prices_2026-01-01.csv")
                .exists(),
            "expected CSV removed after reset"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn create_writer_adds_headers_when_file_exists_but_is_empty() {
        let data_dir = temp_data_dir("headers_on_empty");
        let signals_dir = data_dir.join("signals");
        fs::create_dir_all(&signals_dir).unwrap();

        let today = Utc::now().format("%Y-%m-%d").to_string();
        let signal_file = signals_dir.join(format!("signals_{}.csv", today));
        fs::write(&signal_file, "").unwrap();

        let persistence = CsvPersistence::new(data_dir.to_str().unwrap()).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            persistence
                .save_signal(SignalRecord {
                    timestamp: 1,
                    market_id: "BTC-15m".to_string(),
                    direction: "Up".to_string(),
                    confidence: 0.7,
                    entry_price: 0.5,
                    features_hash: "h".to_string(),
                    token_id: None,
                    market_slug: None,
                    quote_bid: None,
                    quote_ask: None,
                    quote_mid: None,
                    quote_depth_top5: None,
                    spread: None,
                    edge_net: None,
                    rejection_reason: None,
                })
                .await
                .unwrap();
        });

        let content = fs::read_to_string(&signal_file).unwrap();
        let mut lines = content.lines();
        let header = lines.next().unwrap_or_default();
        assert!(
            header
                .starts_with("timestamp,market_id,direction,confidence,entry_price,features_hash"),
            "unexpected header line: {}",
            header
        );
        assert!(lines.next().is_some(), "expected one data row after header");

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn signal_record_backward_compatible_with_legacy_rows() {
        let legacy = "timestamp,market_id,direction,confidence,entry_price,features_hash\n1,BTC-15m,Up,0.71,0.5,legacy\n";
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_reader(legacy.as_bytes());
        let row: SignalRecord = reader
            .deserialize()
            .next()
            .expect("expected one row")
            .expect("legacy row should deserialize");

        assert_eq!(row.timestamp, 1);
        assert_eq!(row.market_id, "BTC-15m");
        assert_eq!(row.token_id, None);
        assert_eq!(row.market_slug, None);
        assert_eq!(row.quote_bid, None);
        assert_eq!(row.quote_ask, None);
        assert_eq!(row.quote_mid, None);
        assert_eq!(row.quote_depth_top5, None);
        assert_eq!(row.spread, None);
        assert_eq!(row.edge_net, None);
        assert_eq!(row.rejection_reason, None);
    }

    #[test]
    fn save_rejection_creates_file_with_header_and_row() {
        let data_dir = temp_data_dir("save_rejection");
        fs::create_dir_all(&data_dir).unwrap();
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let rejection_file = data_dir
            .join("rejections")
            .join(format!("rejections_{}.csv", today));

        let persistence = CsvPersistence::new(data_dir.to_str().unwrap()).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            persistence
                .save_rejection(RejectionRecord {
                    timestamp: 1,
                    signal_id: "sig-1".to_string(),
                    asset: "BTC".to_string(),
                    timeframe: "15m".to_string(),
                    market_slug: "btc-15m".to_string(),
                    token_id: "token-1".to_string(),
                    reason: "spread_too_wide".to_string(),
                    spread: 0.12,
                    depth_top5: 150.0,
                    p_market: 0.50,
                    p_model: 0.54,
                    edge_net: 0.01,
                })
                .await
                .unwrap();
        });

        let content = fs::read_to_string(rejection_file).unwrap();
        let mut lines = content.lines();
        let header = lines.next().unwrap_or_default();
        assert!(
            header.starts_with(
                "timestamp,signal_id,asset,timeframe,market_slug,token_id,reason,spread,depth_top5,p_market,p_model,edge_net"
            ),
            "unexpected header line: {}",
            header
        );
        assert!(
            lines.next().is_some(),
            "expected one rejection row after header"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }
}
