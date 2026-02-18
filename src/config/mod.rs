//! Configuration management for PolyBot
//!
//! Loads from YAML files + environment variables via .env

mod types;

pub use types::*;

use anyhow::{bail, Context, Result};
use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Main application configuration
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub bot: BotConfig,
    pub oracle: OracleConfig,
    pub polymarket: PolymarketConfig,
    pub features: FeaturesConfig,
    pub strategy: StrategyConfig,
    pub risk: RiskConfig,
    pub execution: ExecutionConfig,
    pub kelly: KellyConfig,
    pub persistence: PersistenceConfig,
    pub paper_trading: PaperTradingCfg,
    pub reset: ResetConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BotConfig {
    /// Bot version tag for logging and CSV
    pub tag: String,
    /// Trading pairs to monitor
    pub assets: Vec<String>,
    /// Timeframes to trade (15m, 1h)
    pub timeframes: Vec<String>,
    /// Dry run mode (no real orders)
    pub dry_run: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OracleConfig {
    /// Enable Binance price feed
    pub binance_enabled: bool,
    /// Enable Bybit price feed
    pub bybit_enabled: bool,
    /// Enable Coinbase price feed
    pub coinbase_enabled: bool,
    /// Enable RTDS (Polymarket) price feed
    pub rtds_enabled: bool,
    /// Minimum sources required for confidence
    pub min_sources: usize,
    /// Price staleness threshold in milliseconds
    pub staleness_ms: u64,
    /// Reconnect delay in milliseconds
    pub reconnect_delay_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeaturesConfig {
    /// RSI period
    pub rsi_period: usize,
    /// MACD fast period
    pub macd_fast: usize,
    /// MACD slow period
    pub macd_slow: usize,
    /// MACD signal period
    pub macd_signal: usize,
    /// Bollinger Bands period
    pub bb_period: usize,
    /// ATR period
    pub atr_period: usize,
    /// VWAP reset interval in seconds
    pub vwap_reset_secs: u64,
    /// Momentum lookback in seconds
    pub momentum_lookback_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StrategyConfig {
    /// Minimum confidence to trade (0.0 - 1.0)
    pub min_confidence: f64,
    /// Entry delay after market open in seconds
    pub entry_delay_secs: u64,
    /// Minimum time to expiry before no-trade window in seconds
    pub min_time_to_expiry_secs: u64,
    /// Enable rolling analysis (update prediction throughout period)
    pub rolling_analysis: bool,
    /// Re-evaluation interval in seconds
    pub reeval_interval_secs: u64,
    /// Minimum calibration samples required before market-specific weighting
    pub calibration_min_samples_per_market: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RiskConfig {
    /// Maximum position size in USDC
    pub max_position_usdc: f64,
    /// Maximum daily loss in USDC (kill switch)
    pub max_daily_loss_usdc: f64,
    /// Maximum open positions
    pub max_open_positions: usize,
    /// Maximum drawdown percentage
    pub max_drawdown_pct: f64,
    /// Kill switch enabled
    pub kill_switch_enabled: bool,
    /// Minimum oracle confidence to trade
    pub min_oracle_confidence: f64,
    /// Checkpoint arm ROI (e.g. 0.10 = +10%)
    pub checkpoint_arm_roi: f64,
    /// Initial floor when checkpoint arms (e.g. 0.08 = +8%)
    pub checkpoint_initial_floor_roi: f64,
    /// Trailing gap from peak ROI (e.g. 0.02 = 2pp)
    pub checkpoint_trail_gap_roi: f64,
    /// Hard stop ROI
    pub hard_stop_roi: f64,
    /// Time-stop threshold to expiry
    pub time_stop_seconds_to_expiry: i64,
    /// Exposure limits
    pub max_open_exposure_total: f64,
    pub max_open_exposure_asset: f64,
    pub max_open_exposure_market: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExecutionConfig {
    /// CLOB API endpoint
    pub clob_url: String,
    /// Gamma API endpoint
    pub gamma_url: String,
    /// Polygon chain ID (137)
    pub chain_id: u64,
    /// Signature type (0=EOA, 1=Proxy, 2=Safe)
    pub signature_type: u8,
    /// Order timeout in milliseconds
    pub order_timeout_ms: u64,
    /// Maximum retries
    pub max_retries: usize,
    /// Market refresh interval in seconds
    pub market_refresh_secs: u64,
    /// Prefer maker entries first
    pub maker_first: bool,
    /// Post-only behavior for maker orders
    pub post_only: bool,
    /// Fallback to taker near expiry
    pub fallback_taker_seconds_to_expiry: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PersistenceConfig {
    /// Data directory
    pub data_dir: String,
    /// Enable CSV logging
    pub csv_enabled: bool,
    /// Enable feature recording (for ML training)
    pub record_features: bool,
    /// Feature recording interval in seconds
    pub feature_record_interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaperTradingCfg {
    /// Enable paper trading mode (overrides dry_run)
    pub enabled: bool,
    /// Starting virtual balance in USDC
    pub initial_balance: f64,
    /// Simulated slippage in basis points
    pub slippage_bps: f64,
    /// Simulated fee in basis points
    pub fee_bps: f64,
    /// Trailing stop percentage (e.g., 0.03 = 3%)
    pub trailing_stop_pct: f64,
    /// Take-profit percentage (e.g., 0.05 = 5%)
    pub take_profit_pct: f64,
    /// Max hold duration in milliseconds
    pub max_hold_duration_ms: i64,
    /// Dashboard log interval in seconds
    pub dashboard_interval_secs: u64,
    /// Prefer Chainlink prices (matches Polymarket resolution)
    pub prefer_chainlink: bool,
    /// Minimum net edge required in paper mode.
    /// Negative values allow controlled exploration during learning.
    pub min_edge_net: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PolymarketConfig {
    /// "native_only" enforces Polymarket sources only.
    pub data_mode: String,
    pub rtds: PolymarketRtdsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PolymarketRtdsConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KellyConfig {
    pub enabled: bool,
    pub fraction_15m: f64,
    pub fraction_1h: f64,
    pub max_bankroll_fraction_15m: f64,
    pub max_bankroll_fraction_1h: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResetConfig {
    /// Run reset automatically at startup
    pub enabled_on_start: bool,
    /// Reset mode (supported: hard_all_history)
    pub mode: String,
    /// If false, backup may be created before cleaning (currently unsupported)
    pub no_backup: bool,
    /// Delete historical prices CSVs
    pub delete_prices: bool,
    /// Delete calibrator/learning state files
    pub delete_learning_state: bool,
    /// Delete paper state and paper-related CSVs
    pub delete_paper_state: bool,
}

impl AppConfig {
    /// Load configuration from file and environment
    pub fn load() -> Result<Self> {
        // Load .env file first
        dotenvy::dotenv().ok();

        let config = Config::builder()
            // Load default config
            .set_default("bot.tag", env!("CARGO_PKG_VERSION"))?
            .set_default("bot.dry_run", true)?
            .set_default("bot.assets", vec!["BTC", "ETH"])?
            .set_default("bot.timeframes", vec!["15m", "1h"])?
            // Oracle defaults
            .set_default("oracle.binance_enabled", false)?
            .set_default("oracle.bybit_enabled", false)?
            .set_default("oracle.coinbase_enabled", false)?
            .set_default("oracle.rtds_enabled", true)?
            .set_default("oracle.min_sources", 2)?
            .set_default("oracle.staleness_ms", 20000)?
            .set_default("oracle.reconnect_delay_ms", 5000)?
            // Polymarket defaults
            .set_default("polymarket.data_mode", "native_only")?
            .set_default("polymarket.rtds.enabled", true)?
            // Features defaults
            .set_default("features.rsi_period", 14)?
            .set_default("features.macd_fast", 12)?
            .set_default("features.macd_slow", 26)?
            .set_default("features.macd_signal", 9)?
            .set_default("features.bb_period", 20)?
            .set_default("features.atr_period", 14)?
            .set_default("features.vwap_reset_secs", 86400)?
            .set_default("features.momentum_lookback_secs", 60)?
            // Strategy defaults
            .set_default("strategy.min_confidence", 0.60)?
            .set_default("strategy.entry_delay_secs", 30)?
            .set_default("strategy.min_time_to_expiry_secs", 30)?
            .set_default("strategy.rolling_analysis", true)?
            .set_default("strategy.reeval_interval_secs", 10)?
            .set_default("strategy.calibration_min_samples_per_market", 30)?
            // Risk defaults
            .set_default("risk.max_position_usdc", 100.0)?
            .set_default("risk.max_daily_loss_usdc", 500.0)?
            .set_default("risk.max_open_positions", 5)?
            .set_default("risk.max_drawdown_pct", 10.0)?
            .set_default("risk.kill_switch_enabled", true)?
            .set_default("risk.min_oracle_confidence", 0.7)?
            .set_default("risk.checkpoint_arm_roi", 0.05)?
            .set_default("risk.checkpoint_initial_floor_roi", 0.022)?
            .set_default("risk.checkpoint_trail_gap_roi", 0.012)?
            .set_default("risk.hard_stop_roi", -0.07)?
            .set_default("risk.time_stop_seconds_to_expiry", 90)?
            .set_default("risk.max_open_exposure_total", 0.12)?
            .set_default("risk.max_open_exposure_asset", 0.04)?
            .set_default("risk.max_open_exposure_market", 0.015)?
            // Execution defaults
            .set_default("execution.clob_url", "https://clob.polymarket.com")?
            .set_default("execution.gamma_url", "https://gamma-api.polymarket.com")?
            .set_default("execution.chain_id", 137)?
            .set_default("execution.signature_type", 0)?
            .set_default("execution.order_timeout_ms", 5000)?
            .set_default("execution.max_retries", 3)?
            .set_default("execution.market_refresh_secs", 300)?
            .set_default("execution.maker_first", true)?
            .set_default("execution.post_only", true)?
            .set_default("execution.fallback_taker_seconds_to_expiry", 120)?
            // Kelly defaults
            .set_default("kelly.enabled", true)?
            .set_default("kelly.fraction_15m", 0.25)?
            .set_default("kelly.fraction_1h", 0.50)?
            .set_default("kelly.max_bankroll_fraction_15m", 0.0035)?
            .set_default("kelly.max_bankroll_fraction_1h", 0.0075)?
            // Persistence defaults
            .set_default("persistence.data_dir", "./data")?
            .set_default("persistence.csv_enabled", true)?
            .set_default("persistence.record_features", false)?
            .set_default("persistence.feature_record_interval_secs", 10)?
            // Paper trading defaults
            .set_default("paper_trading.enabled", true)?
            .set_default("paper_trading.initial_balance", 1000.0)?
            .set_default("paper_trading.slippage_bps", 5.0)?
            .set_default("paper_trading.fee_bps", 10.0)?
            .set_default("paper_trading.trailing_stop_pct", 0.005)?
            .set_default("paper_trading.take_profit_pct", 0.008)?
            .set_default("paper_trading.max_hold_duration_ms", 7_200_000)?
            .set_default("paper_trading.dashboard_interval_secs", 30)?
            .set_default("paper_trading.prefer_chainlink", true)?
            .set_default("paper_trading.min_edge_net", 0.0)?
            // Reset defaults
            .set_default("reset.enabled_on_start", false)?
            .set_default("reset.mode", "hard_all_history")?
            .set_default("reset.no_backup", true)?
            .set_default("reset.delete_prices", true)?
            .set_default("reset.delete_learning_state", true)?
            .set_default("reset.delete_paper_state", true)?
            // Load config file if exists
            .add_source(File::with_name("config/default").required(false))
            .add_source(File::with_name("config/local").required(false))
            // Override with environment variables (POLYBOT_*)
            .add_source(Environment::with_prefix("POLYBOT").separator("__"))
            .build()
            .context("Failed to build configuration")?;

        let app_config: AppConfig = config
            .try_deserialize()
            .context("Failed to deserialize configuration")?;

        Ok(app_config)
    }

    /// Generate a digest of the config (without secrets) for logging
    pub fn digest(&self) -> String {
        format!(
            "bot={} assets={:?} timeframes={:?} dry_run={} min_conf={:.2}",
            self.bot.tag,
            self.bot.assets,
            self.bot.timeframes,
            self.bot.dry_run,
            self.strategy.min_confidence
        )
    }

    /// Validate required environment variables
    pub fn validate_env(&self) -> Result<()> {
        let required = vec!["PRIVATE_KEY", "POLYMARKET_ADDRESS"];

        for var in required {
            if std::env::var(var).is_err() {
                bail!("Required environment variable {} is not set", var);
            }
        }

        // Validate private key format
        let pk = std::env::var("PRIVATE_KEY")?;
        if !pk.starts_with("0x") || pk.len() != 66 {
            bail!("PRIVATE_KEY must be a hex string with 0x prefix (66 chars total)");
        }

        Ok(())
    }
}

impl std::fmt::Display for AppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.digest())
    }
}
