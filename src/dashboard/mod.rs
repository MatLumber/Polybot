//! Dashboard Module
//!
//! Provides HTTP/WebSocket API for real-time monitoring of PolyBot.
//! Only compiled when the `dashboard` feature is enabled.

mod api;
mod types;
mod websocket;

pub use api::create_router;
pub use types::*;
pub use websocket::WebSocketBroadcaster;

use crate::types::{Asset, PriceSource};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

const MAX_PRICE_HISTORY_POINTS_PER_ASSET: usize = 86_400;
const DEFAULT_PRICE_HISTORY_WINDOW_SECS: i64 = 3600;
const DEFAULT_PRICE_HISTORY_BUCKET_MS: i64 = 1000;
const MIN_PRICE_HISTORY_BUCKET_MS: i64 = 250;
const MAX_PRICE_HISTORY_BUCKET_MS: i64 = 60_000;
const PRICE_HISTORY_STORAGE_BUCKET_MS: i64 = 1000;
const MARKET_LEARNING_TARGET_SAMPLES: u32 = 30;
const CALIBRATION_ECE_TARGET: f64 = 0.08;

/// In-memory state for the dashboard API
#[derive(Debug)]
pub struct DashboardMemory {
    /// Paper trading stats
    pub paper_stats: RwLock<PaperStatsResponse>,
    /// Paper trading balance
    pub paper_balance: RwLock<f64>,
    /// Paper trading locked amount
    pub paper_locked: RwLock<f64>,
    /// Paper trading unrealized P&L
    pub paper_unrealized_pnl: RwLock<f64>,
    /// Paper trading peak balance
    pub paper_peak_balance: RwLock<f64>,
    /// Open positions (paper)
    pub paper_positions: RwLock<Vec<PositionResponse>>,
    /// Recent trades (paper)
    pub paper_trades: RwLock<Vec<TradeResponse>>,
    /// Asset stats (paper)
    pub paper_asset_stats: RwLock<std::collections::HashMap<String, AssetStatsResponse>>,
    /// Current prices by asset
    pub prices: RwLock<std::collections::HashMap<Asset, AssetPriceResponse>>,
    /// Price history for charting
    pub price_history: RwLock<HashMap<String, VecDeque<PriceHistoryPointResponse>>>,
    /// Session open prices (first price seen for each asset) - for calculating change_24h
    pub session_open_prices: RwLock<std::collections::HashMap<Asset, f64>>,
    /// Recent signals
    pub signals: RwLock<Vec<SignalResponse>>,
    /// Live trading state
    pub live_balance: RwLock<f64>,
    pub live_locked: RwLock<f64>,
    pub live_unrealized_pnl: RwLock<f64>,
    pub live_daily_pnl: RwLock<f64>,
    pub live_daily_trades: RwLock<u32>,
    pub live_kill_switch: RwLock<bool>,
    pub live_positions: RwLock<Vec<PositionResponse>>,
    /// Indicator calibration statistics
    pub indicator_stats: RwLock<Vec<crate::strategy::IndicatorStats>>,
    /// Market-aware calibrator snapshot (market_key -> indicator stats list)
    pub market_learning_stats: RwLock<HashMap<String, Vec<crate::strategy::IndicatorStats>>>,
    /// Probability calibration quality by market (ECE/Brier).
    pub calibration_quality_stats:
        RwLock<HashMap<String, crate::strategy::CalibrationQualitySnapshot>>,
    /// Signal execution diagnostics (accept/reject reasons).
    pub execution_diagnostics: RwLock<ExecutionDiagnosticsResponse>,
    /// Data feed health snapshot for /api/health.
    pub health: RwLock<HealthResponse>,
}

impl Default for DashboardMemory {
    fn default() -> Self {
        Self {
            paper_stats: RwLock::new(PaperStatsResponse::default()),
            paper_balance: RwLock::new(1000.0),
            paper_locked: RwLock::new(0.0),
            paper_unrealized_pnl: RwLock::new(0.0),
            paper_peak_balance: RwLock::new(1000.0),
            paper_positions: RwLock::new(Vec::new()),
            paper_trades: RwLock::new(Vec::new()),
            paper_asset_stats: RwLock::new(std::collections::HashMap::new()),
            prices: RwLock::new(std::collections::HashMap::new()),
            price_history: RwLock::new(HashMap::new()),
            session_open_prices: RwLock::new(std::collections::HashMap::new()),
            signals: RwLock::new(Vec::new()),
            live_balance: RwLock::new(0.0),
            live_locked: RwLock::new(0.0),
            live_unrealized_pnl: RwLock::new(0.0),
            live_daily_pnl: RwLock::new(0.0),
            live_daily_trades: RwLock::new(0),
            live_kill_switch: RwLock::new(false),
            live_positions: RwLock::new(Vec::new()),
            indicator_stats: RwLock::new(Vec::new()),
            market_learning_stats: RwLock::new(HashMap::new()),
            calibration_quality_stats: RwLock::new(HashMap::new()),
            execution_diagnostics: RwLock::new(ExecutionDiagnosticsResponse::default()),
            health: RwLock::new(HealthResponse {
                stale_threshold_ms: 20_000,
                ..HealthResponse::default()
            }),
        }
    }
}

impl DashboardMemory {
    pub fn new(initial_balance: f64) -> Self {
        Self {
            paper_balance: RwLock::new(initial_balance),
            paper_peak_balance: RwLock::new(initial_balance),
            ..Default::default()
        }
    }

    /// Set paper trades from historical data (called on startup)
    pub async fn set_paper_trades(&self, trades: Vec<TradeResponse>) {
        {
            let mut current = self.paper_trades.write().await;
            *current = trades;
        } // Release write lock before calling recalculate_paper_stats to avoid deadlock
          // Recalculate stats after setting trades
        self.recalculate_paper_stats().await;
    }

    /// Recalculate paper trading statistics based on current trades
    pub async fn recalculate_paper_stats(&self) {
        let trades = self.paper_trades.read().await;
        let current_balance = *self.paper_balance.read().await;
        let mut resolved_peak_balance = current_balance;
        let mut resolved_latest_balance: Option<f64> = None;

        let mut stats = self.paper_stats.write().await;
        let mut asset_stats_map = self.paper_asset_stats.write().await;

        // Reset stats
        *stats = PaperStatsResponse::default();
        asset_stats_map.clear();
        if trades.is_empty() {
            stats.peak_balance = current_balance;
        } else {
            // Calculate global stats
            let mut total_pnl = 0.0;
            let mut total_fees = 0.0;
            let mut wins = 0;
            let mut losses = 0;
            let mut largest_win = 0.0;
            let mut largest_loss = 0.0;
            let mut sum_win_pnl = 0.0;
            let mut sum_loss_pnl = 0.0;
            let mut gross_profit = 0.0;
            let mut gross_loss = 0.0;
            let mut exits_trailing_stop = 0;
            let mut exits_take_profit = 0;
            let mut exits_market_expiry = 0;
            let mut exits_time_expiry = 0;
            let mut balance_points: Vec<(i64, f64)> = Vec::new();
            let mut latest_trade_balance: Option<(i64, f64)> = None;

            // Calculate per-asset stats
            let mut asset_stats: HashMap<String, (u32, u32, f64, f64, f64)> = HashMap::new();

            for trade in trades.iter() {
                let pnl = trade.pnl;
                total_pnl += pnl;
                total_fees += trade.size_usdc * 0.001; // Assume 0.1% fee

                if pnl >= 0.0 {
                    wins += 1;
                    sum_win_pnl += pnl;
                    gross_profit += pnl;
                    if pnl > largest_win {
                        largest_win = pnl;
                    }
                } else {
                    losses += 1;
                    sum_loss_pnl += pnl;
                    gross_loss += pnl.abs();
                    if pnl < largest_loss {
                        largest_loss = pnl;
                    }
                }

                // Count exit reasons
                match trade.exit_reason.as_str() {
                    "TRAILING_STOP" => exits_trailing_stop += 1,
                    "TAKE_PROFIT" => exits_take_profit += 1,
                    "MARKET_EXPIRY" => exits_market_expiry += 1,
                    "TIME_EXPIRY" => exits_time_expiry += 1,
                    _ => {}
                }

                // Update per-asset stats
                let entry = asset_stats
                    .entry(trade.asset.clone())
                    .or_insert((0, 0, 0.0, 0.0, 0.0));
                entry.0 += 1; // total trades
                if pnl >= 0.0 {
                    entry.1 += 1; // wins
                    entry.2 += trade.confidence;
                    entry.3 += pnl;
                } else {
                    entry.2 += trade.confidence;
                    entry.4 += pnl;
                }

                if trade.balance_after.is_finite() && trade.balance_after > 0.0 {
                    balance_points.push((trade.timestamp, trade.balance_after));
                    match latest_trade_balance {
                        Some((ts, _)) if ts >= trade.timestamp => {}
                        _ => latest_trade_balance = Some((trade.timestamp, trade.balance_after)),
                    }
                }
            }

            // Update stats
            stats.total_trades = (wins + losses) as u32;
            stats.wins = wins as u32;
            stats.losses = losses as u32;
            stats.total_pnl = total_pnl;
            stats.total_fees = total_fees;
            stats.largest_win = largest_win;
            stats.largest_loss = largest_loss;
            stats.avg_win = if wins > 0 {
                sum_win_pnl / wins as f64
            } else {
                0.0
            };
            stats.avg_loss = if losses > 0 {
                sum_loss_pnl / losses as f64
            } else {
                0.0
            };
            stats.exits_trailing_stop = exits_trailing_stop;
            stats.exits_take_profit = exits_take_profit;
            stats.exits_market_expiry = exits_market_expiry;
            stats.exits_time_expiry = exits_time_expiry;

            // Calculate win rate
            stats.win_rate = if stats.total_trades > 0 {
                (stats.wins as f64 / stats.total_trades as f64) * 100.0
            } else {
                0.0
            };

            // Calculate profit factor
            stats.profit_factor = if gross_loss > 0.0 {
                gross_profit / gross_loss
            } else if gross_profit > 0.0 {
                f64::INFINITY
            } else {
                0.0
            };

            // Drawdown/peak from balance curve when available.
            if !balance_points.is_empty() {
                balance_points.sort_by_key(|(ts, _)| *ts);
                let mut peak = balance_points
                    .first()
                    .map(|(_, b)| *b)
                    .unwrap_or(current_balance)
                    .max(0.0);
                let mut max_drawdown = 0.0;
                let mut last_balance = peak;
                for (_, balance_after) in &balance_points {
                    let bal = *balance_after;
                    if bal > peak {
                        peak = bal;
                    }
                    if peak > 0.0 {
                        let drawdown = ((peak - bal) / peak * 100.0).max(0.0);
                        if drawdown > max_drawdown {
                            max_drawdown = drawdown;
                        }
                    }
                    last_balance = bal;
                }
                stats.peak_balance = peak;
                stats.max_drawdown = max_drawdown;
                stats.current_drawdown = if peak > 0.0 {
                    ((peak - last_balance) / peak * 100.0).max(0.0)
                } else {
                    0.0
                };
                resolved_peak_balance = peak;
                resolved_latest_balance = latest_trade_balance.map(|(_, b)| b);
            } else {
                stats.peak_balance = current_balance;
                stats.max_drawdown = 0.0;
                stats.current_drawdown = 0.0;
                resolved_peak_balance = current_balance;
            }

            // Update asset stats
            for (asset, (total, asset_wins, total_confidence, total_win_pnl, total_loss_pnl)) in
                asset_stats
            {
                let asset_win_rate = if total > 0 {
                    (asset_wins as f64 / total as f64) * 100.0
                } else {
                    0.0
                };
                let asset_stat = AssetStatsResponse {
                    asset: asset.clone(),
                    trades: total,
                    wins: asset_wins,
                    losses: total - asset_wins,
                    win_rate: asset_win_rate,
                    pnl: total_win_pnl + total_loss_pnl,
                    avg_confidence: if total > 0 {
                        total_confidence / total as f64
                    } else {
                        0.0
                    },
                };
                asset_stats_map.insert(asset, asset_stat);
            }
        }
        drop(asset_stats_map);
        drop(stats);
        *self.paper_peak_balance.write().await = resolved_peak_balance;
        if let Some(balance_from_trades) = resolved_latest_balance {
            *self.paper_balance.write().await = balance_from_trades;
        }
    }

    /// Get current paper trading state
    pub async fn get_paper_state(&self) -> PaperDashboard {
        let balance = *self.paper_balance.read().await;
        let locked = *self.paper_locked.read().await;
        let unrealized = *self.paper_unrealized_pnl.read().await;
        let stats = self.paper_stats.read().await.clone();
        let positions = self.paper_positions.read().await.clone();
        let trades = self.paper_trades.read().await.clone();
        let asset_stats = self.paper_asset_stats.read().await.clone();

        PaperDashboard {
            balance,
            available: balance,
            locked,
            total_equity: balance + locked + unrealized,
            unrealized_pnl: unrealized,
            stats,
            open_positions: positions,
            recent_trades: trades,
            asset_stats,
        }
    }

    /// Get current live trading state
    pub async fn get_live_state(&self) -> LiveDashboard {
        let balance = *self.live_balance.read().await;
        let locked = *self.live_locked.read().await;
        let unrealized = *self.live_unrealized_pnl.read().await;

        LiveDashboard {
            balance,
            available: balance,
            locked,
            total_equity: balance + locked + unrealized,
            unrealized_pnl: unrealized,
            open_positions: self.live_positions.read().await.clone(),
            daily_pnl: *self.live_daily_pnl.read().await,
            daily_trades: *self.live_daily_trades.read().await,
            kill_switch_active: *self.live_kill_switch.read().await,
        }
    }

    /// Get current prices
    pub async fn get_prices(&self) -> PriceDashboard {
        use std::collections::HashMap;
        let prices = self.prices.read().await;
        let mapped: HashMap<String, AssetPriceResponse> = prices
            .iter()
            .map(|(k, v)| (format!("{:?}", k), v.clone()))
            .collect();

        let last_update = mapped.values().map(|p| p.timestamp).max().unwrap_or(0);

        PriceDashboard {
            prices: mapped,
            last_update,
        }
    }

    /// Get complete dashboard state
    pub async fn get_state(&self) -> DashboardState {
        DashboardState {
            paper: self.get_paper_state().await,
            live: self.get_live_state().await,
            prices: self.get_prices().await,
            execution: self.execution_diagnostics.read().await.clone(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub async fn get_health(&self) -> HealthResponse {
        let now = chrono::Utc::now().timestamp_millis();
        let mut health = self.health.write().await;
        let stale_threshold_ms = health.stale_threshold_ms;
        for asset_health in health.assets.values_mut() {
            let age = if asset_health.last_tick_ts > 0 {
                now.saturating_sub(asset_health.last_tick_ts)
            } else {
                i64::MAX
            };
            asset_health.tick_age_ms = age;
            asset_health.stale = age > stale_threshold_ms;
        }
        health.updated_at = now;
        health.clone()
    }

    pub async fn record_source_connection(&self, source: &str, connected: bool) {
        let now = chrono::Utc::now().timestamp_millis();
        let mut health = self.health.write().await;
        let mut reconnect_increment = false;

        match source.to_ascii_uppercase().as_str() {
            "RTDS" => {
                reconnect_increment = connected && !health.rtds_connected;
                health.rtds_connected = connected;
                if reconnect_increment {
                    health.rtds_reconnect_count = health.rtds_reconnect_count.saturating_add(1);
                }
            }
            "ORDERBOOK" => {
                reconnect_increment = connected && !health.orderbook_connected;
                health.orderbook_connected = connected;
                if reconnect_increment {
                    health.orderbook_reconnect_count =
                        health.orderbook_reconnect_count.saturating_add(1);
                }
            }
            _ => {}
        }
        health.updated_at = now;

        if reconnect_increment {
            let mut diagnostics = self.execution_diagnostics.write().await;
            diagnostics.reconnect_events = diagnostics.reconnect_events.saturating_add(1);
        }
    }

    pub async fn record_asset_tick(
        &self,
        asset: Asset,
        source: PriceSource,
        tick_ts: i64,
        stale_threshold_ms: i64,
    ) {
        let now = chrono::Utc::now().timestamp_millis();
        let mut health = self.health.write().await;
        health.stale_threshold_ms = stale_threshold_ms.max(1);
        let age = now.saturating_sub(tick_ts);
        let stale = age > health.stale_threshold_ms;
        health.assets.insert(
            asset.to_string(),
            AssetHealthResponse {
                asset: asset.to_string(),
                last_tick_ts: tick_ts,
                tick_age_ms: age,
                stale,
                source: source.to_string(),
            },
        );
        health.updated_at = now;

        let stale_assets: HashMap<String, bool> = health
            .assets
            .iter()
            .map(|(asset_key, row)| (asset_key.clone(), row.stale))
            .collect();
        drop(health);

        let mut diagnostics = self.execution_diagnostics.write().await;
        diagnostics.stale_assets = stale_assets;
    }

    /// Replace current market-learning snapshot.
    pub async fn set_market_learning_stats(
        &self,
        stats_by_market: HashMap<String, Vec<crate::strategy::IndicatorStats>>,
    ) {
        *self.market_learning_stats.write().await = stats_by_market;
    }

    pub async fn set_calibration_quality_stats(
        &self,
        stats: HashMap<String, crate::strategy::CalibrationQualitySnapshot>,
    ) {
        *self.calibration_quality_stats.write().await = stats;
    }

    /// Human-friendly progress summary for each market brain.
    pub async fn get_market_learning_progress(&self) -> Vec<MarketLearningProgressResponse> {
        let snapshot = self.market_learning_stats.read().await.clone();
        let mut rows: Vec<MarketLearningProgressResponse> = Vec::new();
        let mut seen = HashSet::new();

        let canonical = [
            ("BTC_15M", "BTC", "15M"),
            ("BTC_1H", "BTC", "1H"),
            ("ETH_15M", "ETH", "15M"),
            ("ETH_1H", "ETH", "1H"),
        ];

        for (market_key, asset, timeframe) in canonical {
            let indicators = snapshot.get(market_key).map(Vec::as_slice).unwrap_or(&[]);
            rows.push(build_market_learning_row(
                market_key, asset, timeframe, indicators,
            ));
            seen.insert(market_key.to_string());
        }

        for (market_key, indicators) in snapshot {
            if seen.contains(&market_key) {
                continue;
            }
            let (asset, timeframe) = parse_market_key(&market_key);
            rows.push(build_market_learning_row(
                &market_key,
                &asset,
                &timeframe,
                &indicators,
            ));
        }

        rows.sort_by(|a, b| a.market_key.cmp(&b.market_key));
        rows
    }

    pub async fn get_calibration_quality(&self) -> Vec<CalibrationQualityResponse> {
        let snapshot = self.calibration_quality_stats.read().await.clone();
        let mut rows: Vec<CalibrationQualityResponse> = Vec::new();

        let canonical = ["BTC_15M", "BTC_1H", "ETH_15M", "ETH_1H"];
        for market_key in canonical {
            let (asset, timeframe) = parse_market_key(market_key);
            if let Some(s) = snapshot.get(market_key) {
                let ece = s.ece;
                rows.push(CalibrationQualityResponse {
                    market_key: market_key.to_string(),
                    asset,
                    timeframe,
                    sample_count: s.sample_count as u32,
                    brier_score: s.brier_score,
                    ece,
                    ece_target: CALIBRATION_ECE_TARGET,
                    ece_pass: ece.map(|v| v <= CALIBRATION_ECE_TARGET).unwrap_or(false),
                });
            } else {
                rows.push(CalibrationQualityResponse {
                    market_key: market_key.to_string(),
                    asset,
                    timeframe,
                    sample_count: 0,
                    brier_score: None,
                    ece: None,
                    ece_target: CALIBRATION_ECE_TARGET,
                    ece_pass: false,
                });
            }
        }

        for (market_key, s) in snapshot {
            if rows.iter().any(|r| r.market_key == market_key) {
                continue;
            }
            let (asset, timeframe) = parse_market_key(&market_key);
            let ece = s.ece;
            rows.push(CalibrationQualityResponse {
                market_key,
                asset,
                timeframe,
                sample_count: s.sample_count as u32,
                brier_score: s.brier_score,
                ece,
                ece_target: CALIBRATION_ECE_TARGET,
                ece_pass: ece.map(|v| v <= CALIBRATION_ECE_TARGET).unwrap_or(false),
            });
        }

        rows.sort_by(|a, b| a.market_key.cmp(&b.market_key));
        rows
    }

    /// Update price for an asset
    pub async fn update_price(
        &self,
        asset: Asset,
        price: f64,
        bid: f64,
        ask: f64,
        source: PriceSource,
    ) {
        let now = chrono::Utc::now().timestamp_millis();
        self.update_price_at(asset, price, bid, ask, source, now).await;
    }

    /// Update price for an asset with explicit event timestamp.
    pub async fn update_price_at(
        &self,
        asset: Asset,
        price: f64,
        bid: f64,
        ask: f64,
        source: PriceSource,
        timestamp: i64,
    ) {
        let source_label = format!("{:?}", source);

        let mut prices = self.prices.write().await;
        let mut open_prices = self.session_open_prices.write().await;

        // Track session open price (first price we see)
        let open_price = open_prices.entry(asset).or_insert(price);

        // Calculate change from session open
        let change_24h = if *open_price > 0.0 {
            (price - *open_price) / *open_price
        } else {
            0.0
        };

        prices.insert(
            asset,
            AssetPriceResponse {
                asset: format!("{:?}", asset),
                price,
                bid,
                ask,
                source: source_label.clone(),
                timestamp,
                change_24h,
            },
        );

        drop(open_prices);
        drop(prices);

        self.push_price_history_point(
            format!("{:?}", asset),
            PriceHistoryPointResponse {
                timestamp,
                price,
                source: source_label,
            },
        )
        .await;
    }

    /// Seed historical price point (used during startup bootstrap)
    pub async fn seed_price_history_point(
        &self,
        asset: Asset,
        price: f64,
        source: impl Into<String>,
        timestamp: i64,
    ) {
        let source_label = source.into();
        let mut prices = self.prices.write().await;
        let mut open_prices = self.session_open_prices.write().await;
        let open_price = open_prices.entry(asset).or_insert(price);

        let change_24h = if *open_price > 0.0 {
            (price - *open_price) / *open_price
        } else {
            0.0
        };

        let asset_label = format!("{:?}", asset);
        prices.insert(
            asset,
            AssetPriceResponse {
                asset: asset_label.clone(),
                price,
                bid: price,
                ask: price,
                source: source_label.clone(),
                timestamp,
                change_24h,
            },
        );

        drop(open_prices);
        drop(prices);

        self.push_price_history_point(
            asset_label,
            PriceHistoryPointResponse {
                timestamp,
                price,
                source: source_label,
            },
        )
        .await;
    }

    /// Get price history for the requested assets and time window.
    pub async fn get_price_history(
        &self,
        requested_assets: &[String],
        window_secs: Option<i64>,
        bucket_ms: Option<i64>,
    ) -> HashMap<String, Vec<PriceHistoryPointResponse>> {
        let now = chrono::Utc::now().timestamp_millis();
        let window_secs = window_secs
            .unwrap_or(DEFAULT_PRICE_HISTORY_WINDOW_SECS)
            .clamp(60, 86_400);
        let bucket_ms = bucket_ms
            .unwrap_or(DEFAULT_PRICE_HISTORY_BUCKET_MS)
            .clamp(MIN_PRICE_HISTORY_BUCKET_MS, MAX_PRICE_HISTORY_BUCKET_MS);
        let start_ts = now - (window_secs * 1000);

        let requested_upper: Vec<String> = requested_assets
            .iter()
            .map(|asset| asset.trim().to_uppercase())
            .filter(|asset| !asset.is_empty())
            .collect();

        let history = self.price_history.read().await;
        let mut output: HashMap<String, Vec<PriceHistoryPointResponse>> = HashMap::new();

        for (asset, points) in history.iter() {
            if !requested_upper.is_empty()
                && !requested_upper.iter().any(|requested| requested == asset)
            {
                continue;
            }

            let mut bucketed: Vec<PriceHistoryPointResponse> = Vec::new();
            let mut last_bucket: Option<i64> = None;

            for point in points.iter().filter(|point| point.timestamp >= start_ts) {
                let bucket_id = point.timestamp / bucket_ms;
                if last_bucket == Some(bucket_id) {
                    if let Some(last_point) = bucketed.last_mut() {
                        *last_point = point.clone();
                    }
                } else {
                    bucketed.push(point.clone());
                    last_bucket = Some(bucket_id);
                }
            }

            output.insert(asset.clone(), bucketed);
        }

        for requested in requested_upper {
            output.entry(requested).or_default();
        }

        output
    }

    async fn push_price_history_point(&self, asset: String, point: PriceHistoryPointResponse) {
        let mut history = self.price_history.write().await;
        let series = history.entry(asset).or_insert_with(VecDeque::new);

        let bucketed_ts =
            (point.timestamp / PRICE_HISTORY_STORAGE_BUCKET_MS) * PRICE_HISTORY_STORAGE_BUCKET_MS;
        let mut next_point = point;
        next_point.timestamp = bucketed_ts;

        if let Some(last) = series.back_mut() {
            if bucketed_ts < last.timestamp {
                // Ignore stale out-of-order points.
                return;
            }
            if bucketed_ts == last.timestamp {
                *last = next_point;
                return;
            }
        }

        series.push_back(next_point);
        while series.len() > MAX_PRICE_HISTORY_POINTS_PER_ASSET {
            series.pop_front();
        }
    }

    /// Add a new signal
    pub async fn add_signal(&self, signal: SignalResponse) {
        let mut signals = self.signals.write().await;
        signals.insert(0, signal);
        // Keep only last 50 signals
        signals.truncate(50);
    }

    pub async fn record_strategy_evaluation(
        &self,
        generated: bool,
        filter_reason: Option<String>,
    ) {
        let mut diagnostics = self.execution_diagnostics.write().await;
        diagnostics.processed_features = diagnostics.processed_features.saturating_add(1);
        if generated {
            diagnostics.generated_signals = diagnostics.generated_signals.saturating_add(1);
        } else {
            diagnostics.filtered_features = diagnostics.filtered_features.saturating_add(1);
            if let Some(reason) = filter_reason {
                *diagnostics
                    .strategy_filter_reasons
                    .entry(reason.clone())
                    .or_insert(0) += 1;
                diagnostics.last_strategy_filter_reason = Some(reason);
                diagnostics.last_strategy_filter_ts = chrono::Utc::now().timestamp_millis();
            }
        }
    }

    pub async fn record_execution_accept(&self) {
        let mut diagnostics = self.execution_diagnostics.write().await;
        diagnostics.accepted_signals = diagnostics.accepted_signals.saturating_add(1);
    }

    pub async fn record_execution_rejection(&self, reason: impl Into<String>) {
        let reason = reason.into();
        let mut diagnostics = self.execution_diagnostics.write().await;
        diagnostics.rejected_signals = diagnostics.rejected_signals.saturating_add(1);
        *diagnostics.rejection_reasons.entry(reason.clone()).or_insert(0) += 1;
        if reason == "price_stale" {
            diagnostics.stale_rejections = diagnostics.stale_rejections.saturating_add(1);
        }
        diagnostics.last_rejection_reason = Some(reason);
        diagnostics.last_rejection_ts = chrono::Utc::now().timestamp_millis();
    }

    /// Add a completed trade
    pub async fn add_trade(&self, trade: TradeResponse) {
        {
            let mut trades = self.paper_trades.write().await;
            trades.insert(0, trade);
            // Keep only last 100 trades
            trades.truncate(100);
        }
        self.recalculate_paper_stats().await;
    }
}

fn parse_market_key(raw: &str) -> (String, String) {
    if let Some((asset, timeframe)) = raw.split_once('_') {
        return (asset.trim().to_string(), timeframe.trim().to_string());
    }
    (raw.trim().to_string(), String::from("N/A"))
}

fn build_market_learning_row(
    market_key: &str,
    asset: &str,
    timeframe: &str,
    indicators: &[crate::strategy::IndicatorStats],
) -> MarketLearningProgressResponse {
    let mut sample_count = 0_u32;
    let mut indicators_active = 0_u32;
    let mut avg_win_rate_sum = 0.0_f64;
    let mut last_updated_ts = 0_i64;

    for stat in indicators {
        sample_count = sample_count.max(stat.total_signals as u32);
        last_updated_ts = last_updated_ts.max(stat.last_updated);
        if stat.total_signals > 0 {
            indicators_active += 1;
            avg_win_rate_sum += stat.win_rate * 100.0;
        }
    }

    let avg_win_rate_pct = if indicators_active > 0 {
        avg_win_rate_sum / indicators_active as f64
    } else {
        0.0
    };

    let progress_pct = if MARKET_LEARNING_TARGET_SAMPLES > 0 {
        ((sample_count as f64 / MARKET_LEARNING_TARGET_SAMPLES as f64) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };

    let status = if sample_count >= MARKET_LEARNING_TARGET_SAMPLES {
        "ready"
    } else if sample_count == 0 {
        "idle"
    } else {
        "warming_up"
    };

    MarketLearningProgressResponse {
        market_key: market_key.to_string(),
        asset: asset.to_string(),
        timeframe: timeframe.to_string(),
        sample_count,
        target_samples: MARKET_LEARNING_TARGET_SAMPLES,
        progress_pct,
        indicators_active,
        avg_win_rate_pct,
        last_updated_ts,
        status: status.to_string(),
    }
}

/// Start the dashboard server
pub async fn start_server(
    memory: Arc<DashboardMemory>,
    broadcaster: WebSocketBroadcaster,
    port: u16,
    data_dir: String,
) -> anyhow::Result<()> {
    let app = create_router(memory, broadcaster, data_dir);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("🖥️ Dashboard API starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Asset, PriceSource};

    #[tokio::test]
    async fn health_marks_asset_stale_when_tick_is_old() {
        let memory = DashboardMemory::new(1000.0);
        let now = chrono::Utc::now().timestamp_millis();
        memory
            .record_asset_tick(Asset::BTC, PriceSource::RTDS, now - 30_000, 20_000)
            .await;

        let health = memory.get_health().await;
        let btc = health.assets.get("BTC").expect("missing BTC health row");
        assert!(btc.stale, "expected BTC to be stale");
    }

    #[tokio::test]
    async fn source_reconnect_updates_diagnostics_counter() {
        let memory = DashboardMemory::new(1000.0);
        memory.record_source_connection("RTDS", false).await;
        memory.record_source_connection("RTDS", true).await;
        memory.record_source_connection("RTDS", true).await;

        let diagnostics = memory.execution_diagnostics.read().await.clone();
        assert_eq!(diagnostics.reconnect_events, 1);
    }

    #[tokio::test]
    async fn price_stale_rejection_increments_stale_counter() {
        let memory = DashboardMemory::new(1000.0);
        memory.record_execution_rejection("price_stale").await;
        memory.record_execution_rejection("spread_too_wide").await;

        let diagnostics = memory.execution_diagnostics.read().await.clone();
        assert_eq!(diagnostics.stale_rejections, 1);
        assert_eq!(diagnostics.rejected_signals, 2);
        assert_eq!(
            diagnostics
                .rejection_reasons
                .get("price_stale")
                .copied()
                .unwrap_or_default(),
            1
        );
    }

    #[tokio::test]
    async fn health_snapshot_contains_source_flags() {
        let memory = DashboardMemory::new(1000.0);
        memory.record_source_connection("RTDS", true).await;
        memory.record_source_connection("ORDERBOOK", true).await;
        let health = memory.get_health().await;
        assert!(health.rtds_connected);
        assert!(health.orderbook_connected);
        assert!(health.rtds_reconnect_count >= 1);
        assert!(health.orderbook_reconnect_count >= 1);
    }

    #[tokio::test]
    async fn recalculate_stats_restores_balance_and_drawdown_from_trades() {
        let memory = DashboardMemory::new(1000.0);
        memory
            .set_paper_trades(vec![
                TradeResponse {
                    timestamp: 1000,
                    trade_id: "t1".to_string(),
                    asset: "BTC".to_string(),
                    timeframe: "15m".to_string(),
                    direction: "Up".to_string(),
                    confidence: 0.7,
                    entry_price: 1.0,
                    exit_price: 1.1,
                    size_usdc: 10.0,
                    pnl: 5.0,
                    pnl_pct: 50.0,
                    result: "WIN".to_string(),
                    exit_reason: "TIME_EXPIRY".to_string(),
                    hold_duration_secs: 10,
                    balance_after: 1005.0,
                    rsi_at_entry: None,
                    macd_hist_at_entry: None,
                    bb_position_at_entry: None,
                    adx_at_entry: None,
                    volatility_at_entry: None,
                },
                TradeResponse {
                    timestamp: 2000,
                    trade_id: "t2".to_string(),
                    asset: "BTC".to_string(),
                    timeframe: "15m".to_string(),
                    direction: "Down".to_string(),
                    confidence: 0.6,
                    entry_price: 1.1,
                    exit_price: 1.0,
                    size_usdc: 10.0,
                    pnl: -15.0,
                    pnl_pct: -150.0,
                    result: "LOSS".to_string(),
                    exit_reason: "HARD_STOP".to_string(),
                    hold_duration_secs: 12,
                    balance_after: 990.0,
                    rsi_at_entry: None,
                    macd_hist_at_entry: None,
                    bb_position_at_entry: None,
                    adx_at_entry: None,
                    volatility_at_entry: None,
                },
            ])
            .await;

        let paper = memory.get_paper_state().await;
        assert!((paper.balance - 990.0).abs() < f64::EPSILON);
        assert!(paper.stats.peak_balance >= 1005.0);
        assert!(paper.stats.max_drawdown > 0.0);
        assert!(paper.stats.current_drawdown > 0.0);
    }
}
