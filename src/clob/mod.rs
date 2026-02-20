//! Polymarket CLOB Client
//!
//! Implements the Polymarket CLOB API for order management:
//! - EIP-712 signing for gasless authentication
//! - Order creation, cancellation, and status tracking
//! - Market discovery and price fetching
//! - WebSocket for real-time updates
//! - Orderbook feed for microstructure analysis
//! - Taker fee calculation based on Polymarket fee curve

pub mod dynamic_orderbook_feed;
pub mod fees;
pub mod market_discovery;
pub mod orderbook_feed;
pub mod rest;
pub mod signing;
pub mod types;
pub mod websocket;

pub use dynamic_orderbook_feed::*;
pub use fees::*;
pub use market_discovery::*;
pub use orderbook_feed::*;
pub use rest::*;
pub use signing::*;
pub use types::*;
pub use websocket::*;

use anyhow::{bail, Context, Result};
use ethers::contract::abigen;
use ethers::middleware::SignerMiddleware;
use ethers::providers::{Http, Provider};
use ethers::signers::{LocalWallet, Signer};
use ethers::types::{Address, H256, U256};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::ExecutionConfig;
use crate::types::{Asset, Direction, Timeframe};

abigen!(
    ConditionalTokensContract,
    r#"[
        function redeemPositions(address collateralToken, bytes32 parentCollectionId, bytes32 conditionId, uint256[] indexSets)
    ]"#
);

fn infer_winning_outcome_index(outcome_prices: &[String]) -> Option<usize> {
    let parsed_prices: Vec<f64> = outcome_prices
        .iter()
        .filter_map(|p| p.parse::<f64>().ok())
        .collect();
    if parsed_prices.len() < 2 {
        return None;
    }
    parsed_prices
        .iter()
        .copied()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(idx, _)| idx)
}

/// Polymarket CLOB client configuration
#[derive(Debug, Clone)]
pub struct ClobConfig {
    /// Polygon chain ID (137)
    pub chain_id: u64,
    /// CLOB REST API endpoint
    pub rest_url: String,
    /// CLOB WebSocket endpoint
    pub ws_url: String,
    /// Gamma API endpoint
    pub gamma_url: String,
    /// Private key for signing
    pub private_key: Option<String>,
    /// Wallet address
    pub address: Option<Address>,
    /// API key (derived from private key)
    pub api_key: Option<String>,
    /// API secret (derived from private key)
    pub api_secret: Option<String>,
    /// API passphrase (Level-2 auth)
    pub api_passphrase: Option<String>,
    /// Order timeout in seconds
    pub order_timeout: u64,
    /// Maximum retries
    pub max_retries: usize,
    /// Signature type
    pub signature_type: u8,
}

impl Default for ClobConfig {
    fn default() -> Self {
        Self {
            chain_id: 137, // Polygon
            rest_url: "https://clob.polymarket.com".to_string(),
            ws_url: "wss://ws-subscriptions-clob.polymarket.com/ws".to_string(),
            gamma_url: "https://gamma-api.polymarket.com".to_string(),
            private_key: None,
            address: None,
            api_key: None,
            api_secret: None,
            api_passphrase: None,
            order_timeout: 30,
            max_retries: 3,
            signature_type: 0,
        }
    }
}

impl From<ExecutionConfig> for ClobConfig {
    fn from(exec: ExecutionConfig) -> Self {
        let private_key = std::env::var("PRIVATE_KEY").ok();
        let address = std::env::var("POLYMARKET_ADDRESS")
            .ok()
            .and_then(|s| s.parse().ok());
        let api_key = std::env::var("POLY_API_KEY")
            .ok()
            .or_else(|| std::env::var("API_KEY").ok());
        let api_secret = std::env::var("POLY_API_SECRET")
            .ok()
            .or_else(|| std::env::var("API_SECRET").ok());
        let api_passphrase = std::env::var("POLY_API_PASSPHRASE")
            .ok()
            .or_else(|| std::env::var("API_PASSPHRASE").ok());

        Self {
            chain_id: exec.chain_id,
            rest_url: exec.clob_url,
            ws_url: "wss://ws-subscriptions-clob.polymarket.com/ws".to_string(),
            gamma_url: exec.gamma_url,
            private_key,
            address,
            api_key,
            api_secret,
            api_passphrase,
            order_timeout: exec.order_timeout_ms / 1000,
            max_retries: exec.max_retries,
            signature_type: exec.signature_type,
        }
    }
}

impl ClobConfig {
    pub fn from_env() -> Result<Self> {
        let private_key = std::env::var("PRIVATE_KEY").ok();
        let address = std::env::var("POLYMARKET_ADDRESS")
            .ok()
            .and_then(|s| s.parse().ok());
        let api_key = std::env::var("POLY_API_KEY")
            .ok()
            .or_else(|| std::env::var("API_KEY").ok());
        let api_secret = std::env::var("POLY_API_SECRET")
            .ok()
            .or_else(|| std::env::var("API_SECRET").ok());
        let api_passphrase = std::env::var("POLY_API_PASSPHRASE")
            .ok()
            .or_else(|| std::env::var("API_PASSPHRASE").ok());

        Ok(Self {
            private_key,
            address,
            api_key,
            api_secret,
            api_passphrase,
            ..Default::default()
        })
    }
}

/// Main CLOB client
pub struct ClobClient {
    config: ClobConfig,
    rest: RestClient,
    /// Market cache (condition_id -> MarketInfo)
    markets: RwLock<HashMap<String, MarketInfo>>,
    /// Token ID cache (market_slug#direction -> token_id)
    token_map: RwLock<HashMap<String, String>>,
    /// Order tracking
    orders: RwLock<HashMap<H256, Order>>,
    /// Dry run mode (no real orders)
    dry_run: bool,
    /// Data API URL for positions
    data_api_url: String,
}

/// Market information from Polymarket
#[derive(Debug, Clone)]
pub struct MarketInfo {
    pub condition_id: String,
    pub question: String,
    pub outcomes: Vec<String>,
    pub tokens: Vec<TokenInfo>,
    pub active: bool,
    pub min_tick: f64,
    pub max_tick: f64,
    /// Market slug (e.g., "btc-15m")
    pub slug: Option<String>,
    /// Market expiry time in ISO 8601 format with time (preferred).
    pub end_date: Option<String>,
    /// Market expiry time in ISO 8601 format
    pub end_date_iso: Option<String>,
    /// Whether market currently accepts orders.
    pub accepting_orders: bool,
    /// Whether market has CLOB orderbook enabled.
    pub enable_order_book: bool,
    /// Market liquidity (Gamma snapshot).
    pub liquidity_num: f64,
    /// Best bid snapshot from Gamma (if available).
    pub best_bid: f64,
    /// Best ask snapshot from Gamma (if available).
    pub best_ask: f64,
}

#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub token_id: String,
    pub outcome: String,
    pub price: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct TokenQuote {
    pub bid: f64,
    pub ask: f64,
    pub mid: f64,
    pub spread: f64,
    pub bid_size: f64,
    pub ask_size: f64,
    pub depth_top5: f64,
}

impl ClobClient {
    pub fn new(config: impl Into<ClobConfig>) -> Self {
        Self::with_dry_run(config, true)
    }

    /// Create client with explicit dry_run setting
    pub fn with_dry_run(config: impl Into<ClobConfig>, dry_run: bool) -> Self {
        let config = config.into();
        let rest = RestClient::new(
            &config.rest_url,
            config.address.map(|a| format!("0x{:x}", a)),
            config.api_key.clone(),
            config.api_secret.clone(),
            config.api_passphrase.clone(),
        );
        Self {
            config,
            rest,
            markets: RwLock::new(HashMap::new()),
            token_map: RwLock::new(HashMap::new()),
            orders: RwLock::new(HashMap::new()),
            dry_run,
            data_api_url: "https://data-api.polymarket.com".to_string(),
        }
    }

    /// Run the client (for spawning as a task) - receives orders from channel
    pub async fn run(&self, mut order_rx: tokio::sync::mpsc::Receiver<Order>) -> Result<()> {
        self.initialize().await?;

        loop {
            tokio::select! {
                // Process incoming orders
                Some(order) = order_rx.recv() => {
                    if self.dry_run {
                        tracing::info!(
                            order_id = ?order.id,
                            token_id = %order.token_id,
                            side = ?order.side,
                            price = %order.price,
                            size = %order.size,
                            "🧪 [DRY_RUN] Simulated order (not submitted)"
                        );
                        // Simulate order tracking
                        let mut orders = self.orders.write().await;
                        orders.insert(order.id, order);
                    } else {
                        match self.execute_order_with_policy(&order).await {
                            Ok(order_id) => {
                                tracing::info!(
                                    order_id = %order_id,
                                    "✅ Order submitted successfully"
                                );
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "❌ Failed to submit order");
                            }
                        }
                    }
                }

                // Periodic market refresh
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(60)) => {
                    if let Err(e) = self.refresh_markets().await {
                        tracing::warn!(error = %e, "Failed to refresh markets");
                    }
                }
            }
        }
    }

    /// Submit an order to the CLOB
    pub async fn submit_order(&self, order: &Order) -> Result<String> {
        if self.dry_run {
            tracing::info!(
                order_id = ?order.id,
                "🧪 [DRY_RUN] Would submit order"
            );
            return Ok(format!("dry_run_{:?}", order.id));
        }

        let mut to_submit = order.clone();
        if to_submit.signature.is_none() {
            let private_key = self
                .config
                .private_key
                .as_deref()
                .context("PRIVATE_KEY is required to sign CLOB orders")?;
            sign_order(&mut to_submit, private_key, self.config.chain_id).await?;
        }
        if to_submit.maker.is_none() {
            to_submit.maker = self.config.address;
        }

        let order_id = self.rest.create_order(&to_submit).await?;

        // Track order
        let mut orders = self.orders.write().await;
        orders.insert(to_submit.id, to_submit);

        Ok(order_id)
    }

    /// Execute order using maker-first policy when post-only/GTD intent is enabled.
    async fn execute_order_with_policy(&self, order: &Order) -> Result<String> {
        // Non post-only path.
        if order.expiration == 0 {
            return self.submit_order(order).await;
        }

        let maker_attempts = self.config.max_retries.max(1);
        let ttl_secs = self.config.order_timeout.max(1).min(30);

        for attempt in 0..maker_attempts {
            let mut maker_order = order.clone();
            if let Ok(quote) = self.quote_token(&order.token_id).await {
                let tick = 0.001;
                let maker_price = (quote.bid + tick)
                    .min((quote.ask - tick).max(0.01))
                    .clamp(0.01, 0.99);
                maker_order.price = maker_price;
            }

            let order_id = self.submit_order(&maker_order).await?;
            tracing::info!(
                token_id = %order.token_id,
                attempt = attempt + 1,
                maker_attempts = maker_attempts,
                ttl_secs = ttl_secs,
                order_id = %order_id,
                "Placed maker-first order attempt"
            );

            tokio::time::sleep(tokio::time::Duration::from_secs(ttl_secs)).await;

            // Best effort status check: if filled, keep it.
            let filled = match self.rest.get_order(&order_id).await {
                Ok(remote) => matches!(remote.status, OrderStatus::Filled),
                Err(_) => false,
            };
            if filled {
                return Ok(order_id);
            }

            // Cancel and reprice for next attempt.
            let _ = self.rest.cancel_order(&order_id).await;
        }

        // Fallback taker execution.
        let mut taker_order = order.clone();
        taker_order.expiration = 0;
        if let Ok(quote) = self.quote_token(&order.token_id).await {
            taker_order.price = quote.ask.clamp(0.01, 0.99);
        }
        tracing::warn!(
            token_id = %order.token_id,
            "Maker attempts exhausted; executing taker fallback"
        );
        self.submit_order(&taker_order).await
    }

    /// Initialize the client (derive API keys if needed)
    pub async fn initialize(&self) -> Result<()> {
        if !self.dry_run {
            let missing_l2 = self.config.api_key.is_none()
                || self.config.api_secret.is_none()
                || self.config.api_passphrase.is_none();

            if missing_l2 {
                let private_key = self
                    .config
                    .private_key
                    .clone()
                    .or_else(|| std::env::var("PRIVATE_KEY").ok());
                let address = self.config.address.or_else(|| {
                    std::env::var("POLYMARKET_ADDRESS")
                        .ok()
                        .and_then(|s| s.parse().ok())
                });

                if let (Some(private_key), Some(address)) = (private_key, address) {
                    match self
                        .rest
                        .create_or_derive_api_credentials(
                            &private_key,
                            self.config.chain_id,
                            address,
                        )
                        .await
                    {
                        Ok((api_key, api_secret, api_passphrase)) => {
                            std::env::set_var("POLY_API_KEY", &api_key);
                            std::env::set_var("POLY_API_SECRET", &api_secret);
                            std::env::set_var("POLY_API_PASSPHRASE", &api_passphrase);
                            tracing::info!(
                                address = %format!("{:#x}", address),
                                "Derived CLOB API credentials via /auth endpoints"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                "Failed to derive CLOB API credentials automatically"
                            );
                        }
                    }
                } else {
                    tracing::warn!(
                        "Missing PRIVATE_KEY or POLYMARKET_ADDRESS; cannot derive CLOB API credentials"
                    );
                }
            }

            let has_l2 = std::env::var("POLY_API_KEY").ok().is_some()
                && std::env::var("POLY_API_SECRET").ok().is_some()
                && std::env::var("POLY_API_PASSPHRASE").ok().is_some();
            if !has_l2
                && (self.config.api_key.is_none()
                    || self.config.api_secret.is_none()
                    || self.config.api_passphrase.is_none())
            {
                bail!(
                    "Live CLOB mode requires POLY_API_KEY, POLY_API_SECRET, and POLY_API_PASSPHRASE"
                );
            }
        }

        // Fetch and cache markets
        self.refresh_markets().await?;

        Ok(())
    }

    /// Refresh market cache
    pub async fn refresh_markets(&self) -> Result<()> {
        const PAGE_SIZE: usize = 500;
        // Gamma's active market universe is large and not strictly ordered by relevance.
        // Scan deeper to avoid missing intraday BTC/ETH markets (updown-15m/1h/5m),
        // which can appear far beyond the first pages.
        const MAX_PAGES: usize = 80;

        let mut all_markets: Vec<MarketResponse> = Vec::new();
        let mut scanned_pages = 0usize;
        for page in 0..MAX_PAGES {
            let offset = page * PAGE_SIZE;
            let page_markets = self
                .rest
                .get_markets_page(&self.config.gamma_url, None, PAGE_SIZE, offset)
                .await?;
            let page_len = page_markets.len();
            all_markets.extend(page_markets);
            scanned_pages += 1;
            if page_len < PAGE_SIZE {
                break;
            }
        }

        let mut cache = self.markets.write().await;
        cache.clear();
        for market in all_markets {
            // Convert MarketResponse to MarketInfo and cache it
            let info = MarketInfo::from(market);
            cache.insert(info.condition_id.clone(), info);
        }
        tracing::info!(
            cached_markets = cache.len(),
            scanned_pages = scanned_pages,
            page_size = PAGE_SIZE,
            "Cached markets from Gamma API"
        );
        Ok(())
    }

    /// Get market info by condition ID
    pub async fn get_market(&self, condition_id: &str) -> Option<MarketInfo> {
        let cache = self.markets.read().await;
        cache.get(condition_id).cloned()
    }

    /// Get current USDC balance from Polymarket
    pub async fn get_balance(&self) -> Result<f64> {
        self.rest.get_balance().await
    }

    /// Check if a market is resolved (closed and has winning outcome)
    /// Returns Some(winning_outcome_index) if resolved, None otherwise
    pub async fn check_market_resolution(&self, condition_id: &str) -> Option<usize> {
        // Fetch market from Gamma API to check resolution status
        match self
            .rest
            .get_market(&self.config.gamma_url, condition_id)
            .await
        {
            Ok(market) => {
                let closed = market.closed.unwrap_or(false);
                let uma_resolved = market
                    .uma_resolution_status
                    .as_deref()
                    .map(|s| s.eq_ignore_ascii_case("resolved"))
                    .unwrap_or(false);

                if !closed && !uma_resolved {
                    return None;
                }

                if let Some(winner_idx) = infer_winning_outcome_index(&market.outcome_prices) {
                    let parsed_prices: Vec<f64> = market
                        .outcome_prices
                        .iter()
                        .filter_map(|p| p.parse::<f64>().ok())
                        .collect();
                    let winner_price = parsed_prices.get(winner_idx).copied().unwrap_or_default();

                    // Resolution prices should collapse near {1, 0}; still accept winner by max.
                    tracing::info!(
                        condition_id = %condition_id,
                        winner_idx = winner_idx,
                        winner_price = winner_price,
                        prices = ?parsed_prices,
                        closed = closed,
                        uma_status = ?market.uma_resolution_status,
                        "🏁 Market resolution detected from outcome prices"
                    );
                    return Some(winner_idx);
                }

                tracing::warn!(
                    condition_id = %condition_id,
                    closed = closed,
                    uma_status = ?market.uma_resolution_status,
                    "Market appears closed/resolved but has no parsable outcome prices"
                );
                None
            }
            Err(e) => {
                tracing::warn!(error = %e, condition_id = %condition_id, "Failed to check market resolution");
                None
            }
        }
    }

    /// Resolve official result for a token once market resolution is known.
    /// Returns Some("WIN"/"LOSS") only after the market has resolved.
    pub async fn official_result_for_token(
        &self,
        condition_id: &str,
        token_id: &str,
    ) -> Option<String> {
        match self
            .rest
            .get_market(&self.config.gamma_url, condition_id)
            .await
        {
            Ok(market) => {
                let closed = market.closed.unwrap_or(false);
                let uma_resolved = market
                    .uma_resolution_status
                    .as_deref()
                    .map(|s| s.eq_ignore_ascii_case("resolved"))
                    .unwrap_or(false);
                if !closed && !uma_resolved {
                    return None;
                }

                let winner_idx = infer_winning_outcome_index(&market.outcome_prices)?;
                let winner_token = market.clob_token_ids.get(winner_idx)?;
                if winner_token == token_id {
                    Some("WIN".to_string())
                } else {
                    Some("LOSS".to_string())
                }
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    condition_id = %condition_id,
                    "Failed to determine official result for token"
                );
                None
            }
        }
    }

    /// Redeem winning tokens for a resolved market
    pub async fn redeem_winning_tokens(
        &self,
        condition_id: &str,
        token_id: &str,
        size: f64,
    ) -> Result<()> {
        if size <= 0.0 {
            tracing::info!(
                condition_id = %condition_id,
                token_id = %token_id,
                size = size,
                "Skipping redemption for non-positive size"
            );
            return Ok(());
        }

        let winning_outcome = self.check_market_resolution(condition_id).await;

        match winning_outcome {
            Some(outcome_idx) => {
                let market = self
                    .rest
                    .get_market(&self.config.gamma_url, condition_id)
                    .await
                    .context("Failed to fetch market details for redemption")?;
                let winner_token_id = market
                    .clob_token_ids
                    .get(outcome_idx)
                    .cloned()
                    .context("Resolved market missing winner token id")?;
                if winner_token_id != token_id {
                    tracing::info!(
                        condition_id = %condition_id,
                        token_id = %token_id,
                        winner_token_id = %winner_token_id,
                        "Skipping redemption: token is not the winning outcome"
                    );
                    return Ok(());
                }

                let private_key = self
                    .config
                    .private_key
                    .clone()
                    .or_else(|| std::env::var("PRIVATE_KEY").ok())
                    .context("PRIVATE_KEY is required for redemption transaction")?;
                let rpc_url = std::env::var("POLYGON_RPC_URL")
                    .unwrap_or_else(|_| "https://polygon-rpc.com".to_string());
                let ctf_contract = std::env::var("POLY_CTF_CONTRACT")
                    .unwrap_or_else(|_| "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045".to_string());
                let collateral_token = std::env::var("POLY_CTF_COLLATERAL_TOKEN")
                    .unwrap_or_else(|_| "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_string());

                let provider = Provider::<Http>::try_from(rpc_url.clone())
                    .with_context(|| format!("Invalid POLYGON_RPC_URL '{}'", rpc_url))?;
                let wallet: LocalWallet = private_key
                    .parse()
                    .context("Invalid PRIVATE_KEY for redemption")?;
                let signer = wallet.with_chain_id(self.config.chain_id);
                let client = Arc::new(SignerMiddleware::new(provider, signer));

                let contract_address: Address = ctf_contract
                    .parse()
                    .with_context(|| format!("Invalid POLY_CTF_CONTRACT '{}'", ctf_contract))?;
                let collateral_address: Address = collateral_token.parse().with_context(|| {
                    format!("Invalid POLY_CTF_COLLATERAL_TOKEN '{}'", collateral_token)
                })?;
                let condition_hash: H256 = condition_id
                    .parse()
                    .with_context(|| format!("Invalid condition id '{}'", condition_id))?;
                let index_set = U256::from(1u64 << outcome_idx);

                let contract = ConditionalTokensContract::new(contract_address, client);
                tracing::info!(
                    condition_id = %condition_id,
                    token_id = %token_id,
                    size = size,
                    winning_outcome = outcome_idx,
                    contract = %ctf_contract,
                    collateral = %collateral_token,
                    "Submitting redeemPositions transaction"
                );

                let call = contract.redeem_positions(
                    collateral_address,
                    [0u8; 32],
                    condition_hash.to_fixed_bytes(),
                    vec![index_set],
                );
                let pending = call
                    .send()
                    .await
                    .context("Failed to submit redeemPositions transaction")?;
                let tx_hash = pending.tx_hash();
                let receipt = pending
                    .await
                    .context("Redeem transaction dropped before confirmation")?;

                tracing::info!(
                    condition_id = %condition_id,
                    token_id = %token_id,
                    tx_hash = %format!("{:#x}", tx_hash),
                    block = ?receipt.and_then(|r| r.block_number),
                    "Redeem transaction confirmed"
                );

                Ok(())
            }
            None => {
                tracing::info!(
                    condition_id = %condition_id,
                    "Market not yet resolved or not found"
                );
                Ok(())
            }
        }
    }

    /// Find markets by keyword (e.g., "BTC", "ETH")
    pub async fn find_markets(&self, keyword: &str) -> Vec<MarketInfo> {
        let cache = self.markets.read().await;
        cache
            .values()
            .filter(|m| m.question.to_lowercase().contains(&keyword.to_lowercase()))
            .cloned()
            .collect()
    }

    /// Find the best currently-tradeable market for an asset/timeframe.
    ///
    /// Selection is strict on asset keyword boundaries and market validity
    /// (active, accepting orders, not expired, has tokens).
    pub async fn find_tradeable_market_for_signal(
        &self,
        asset: Asset,
        timeframe: Timeframe,
    ) -> Option<MarketInfo> {
        let cache_len = { self.markets.read().await.len() };
        if cache_len == 0 {
            if let Err(e) = self.refresh_markets().await {
                tracing::warn!(
                    error = %e,
                    asset = ?asset,
                    timeframe = ?timeframe,
                    "Failed to refresh markets for tradeable market lookup"
                );
                return None;
            }
        }

        let now_ms = chrono::Utc::now().timestamp_millis();
        let cache = self.markets.read().await;
        let mut candidates: Vec<(i32, i64, MarketInfo)> = Vec::new();
        let mut total = 0usize;
        let mut active_with_tokens = 0usize;
        let mut asset_matched = 0usize;

        for market in cache.values() {
            total += 1;
            if !market.active
                || !market.accepting_orders
                || !market.enable_order_book
                || market.tokens.len() < 2
            {
                continue;
            }
            // Require unambiguous directional token mapping (no implicit index fallback).
            if select_token_for_direction(market, Direction::Up).is_none()
                || select_token_for_direction(market, Direction::Down).is_none()
            {
                continue;
            }
            active_with_tokens += 1;

            let expiry = market_expiry_timestamp_ms(market);
            if expiry > 0 && expiry <= now_ms + 30_000 {
                continue;
            }
            if expiry == i64::MAX {
                continue;
            }
            let minutes_to_expiry = ((expiry - now_ms) / 60_000).max(0);
            let window_ms = timeframe.duration_secs() as i64 * 1000;
            let market_open_ts = expiry.saturating_sub(window_ms);
            // Do not route signals to windows that have not started yet.
            if market_open_ts > now_ms + 15_000 {
                continue;
            }

            let text = market_market_text(market);
            let asset_match = market_matches_asset(asset, &text);
            if !asset_match {
                continue;
            }
            asset_matched += 1;

            let timeframe_hint = match timeframe {
                Timeframe::Min15 => TimeframeHint::Min15,
                Timeframe::Hour1 => TimeframeHint::Hour1,
            };
            let timeframe_match = text_matches_timeframe_hint(&text, timeframe_hint);
            if !timeframe_match {
                continue;
            }

            let mut score = 0i32;
            score += 45;
            if text.contains("up or down") || text.contains("updown") {
                score += 55;
            }

            let cadence = infer_market_cadence(&text);
            match timeframe {
                Timeframe::Min15 => match cadence {
                    MarketCadence::Min15 => score += 60,
                    MarketCadence::Min5 => score -= 15,
                    MarketCadence::Hour1 => score -= 50,
                    MarketCadence::Unknown => score += 0,
                },
                Timeframe::Hour1 => match cadence {
                    MarketCadence::Hour1 => score += 60,
                    MarketCadence::Min15 => score -= 30,
                    MarketCadence::Min5 => score -= 50,
                    MarketCadence::Unknown => score += 0,
                },
            }

            if market.best_bid > 0.0 && market.best_ask > market.best_bid && market.best_ask <= 1.0
            {
                score += 25;
                score -= (((market.best_ask - market.best_bid).max(0.0) * 1000.0) as i32).min(250);
            } else {
                score -= 30;
            }

            if market.liquidity_num > 0.0 {
                score += (market.liquidity_num.log10().max(0.0) * 12.0) as i32;
            }

            // Strict intraday horizon gating:
            // - 15m lane: must be near 15m expiry window
            // - 1h lane: must be near 1h expiry window
            if !matches_timeframe_expiry_window(timeframe, minutes_to_expiry) {
                continue;
            }
            score += 70;

            candidates.push((score, expiry, market.clone()));
        }

        // Fallback keeps strict semantic, token mapping and expiry-window guarantees.
        if candidates.is_empty() {
            let mut fallback_asset_matched = 0usize;
            for market in cache.values() {
                if !market.active
                    || !market.accepting_orders
                    || !market.enable_order_book
                    || market.tokens.len() < 2
                {
                    continue;
                }
                if select_token_for_direction(market, Direction::Up).is_none()
                    || select_token_for_direction(market, Direction::Down).is_none()
                {
                    continue;
                }

                let text = market_market_text(market);
                if !market_matches_asset(asset, &text) {
                    continue;
                }
                let timeframe_hint = match timeframe {
                    Timeframe::Min15 => TimeframeHint::Min15,
                    Timeframe::Hour1 => TimeframeHint::Hour1,
                };
                if !text_matches_timeframe_hint(&text, timeframe_hint) {
                    continue;
                }
                fallback_asset_matched += 1;

                let expiry = market_expiry_timestamp_ms(market);
                if expiry > 0 && expiry <= now_ms + 30_000 {
                    continue;
                }
                if expiry == i64::MAX {
                    continue;
                }
                let minutes_to_expiry = ((expiry - now_ms) / 60_000).max(0);
                let window_ms = timeframe.duration_secs() as i64 * 1000;
                let market_open_ts = expiry.saturating_sub(window_ms);
                if market_open_ts > now_ms + 15_000 {
                    continue;
                }
                if !matches_timeframe_expiry_window(timeframe, minutes_to_expiry) {
                    continue;
                }

                let mut score = 0i32;
                score += 35;
                if market.best_bid > 0.0 && market.best_ask > market.best_bid {
                    score += 20;
                }
                if market.liquidity_num > 0.0 {
                    score += (market.liquidity_num.log10().max(0.0) * 10.0) as i32;
                }

                candidates.push((score, expiry, market.clone()));
            }

            if candidates.is_empty() {
                tracing::warn!(
                    asset = ?asset,
                    timeframe = ?timeframe,
                    cache_total = total,
                    strict_active_with_tokens = active_with_tokens,
                    strict_asset_matches = asset_matched,
                    fallback_asset_matches = fallback_asset_matched,
                    "No tradeable market candidate found after strict + fallback selection"
                );
            }
        }

        candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        candidates.into_iter().next().map(|(_, _, market)| market)
    }

    /// Resolve token id for a direction directly from an already selected market.
    pub fn resolve_token_id_for_direction(
        market: &MarketInfo,
        direction: Direction,
    ) -> Option<String> {
        select_token_for_direction(market, direction)
    }

    /// Find active market by slug pattern (e.g., "btc-15m", "eth-1h")
    /// Returns the first active market that matches the pattern
    pub async fn find_market_by_slug(&self, slug_pattern: &str) -> Option<MarketInfo> {
        let cache = self.markets.read().await;
        let slug_lower = slug_pattern.to_ascii_lowercase();
        let asset_keywords = extract_asset_keywords(slug_pattern);
        let timeframe_hint = infer_timeframe_hint(slug_pattern);
        let now_ms = chrono::Utc::now().timestamp_millis();

        let mut candidates: Vec<(i32, i64, MarketInfo)> = cache
            .values()
            .filter(|m| m.active && m.accepting_orders)
            .filter_map(|market| {
                let slug = market
                    .slug
                    .as_ref()
                    .map(|s| s.to_ascii_lowercase())
                    .unwrap_or_default();
                let question = market.question.to_ascii_lowercase();
                let text = format!("{} {}", slug, question);

                let exact_slug =
                    !slug.is_empty() && (slug.contains(&slug_lower) || slug_lower.contains(&slug));
                let keyword_match = asset_keywords.iter().any(|k| text.contains(k));
                if !exact_slug && !keyword_match {
                    return None;
                }

                let timeframe_match = timeframe_hint
                    .map(|hint| text_matches_timeframe_hint(&text, hint))
                    .unwrap_or(true);

                let mut score = 0i32;
                if exact_slug {
                    score += 120;
                }
                if keyword_match {
                    score += 30;
                }
                if timeframe_match {
                    score += 25;
                }
                if !slug.is_empty() && slug.starts_with(&slug_lower) {
                    score += 10;
                }

                let expiry = market_expiry_timestamp_ms(market);
                let expiry_penalty = if expiry >= now_ms {
                    ((expiry - now_ms) / 1000 / 60).min(5000) as i32
                } else {
                    5000
                };
                score -= expiry_penalty / 200; // Prefer markets expiring sooner.

                Some((score, expiry, market.clone()))
            })
            .collect();

        // Prefer markets that explicitly match timeframe hint if one exists.
        if let Some(hint) = timeframe_hint {
            let mut strict: Vec<(i32, i64, MarketInfo)> = candidates
                .iter()
                .cloned()
                .filter(|(_, _, m)| {
                    let slug = m.slug.as_deref().unwrap_or_default().to_ascii_lowercase();
                    let question = m.question.to_ascii_lowercase();
                    let text = format!("{} {}", slug, question);
                    text_matches_timeframe_hint(&text, hint)
                })
                .collect();
            if !strict.is_empty() {
                strict.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
                return strict.into_iter().next().map(|(_, _, market)| market);
            }
        }

        candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        candidates.into_iter().next().map(|(_, _, market)| market)
    }

    /// Parse ISO 8601 date string to Unix timestamp in milliseconds
    pub fn parse_expiry_to_timestamp(iso_date: &str) -> Option<i64> {
        // Try parsing common ISO 8601 formats
        // Format: "2024-01-15T15:45:00Z" or "2024-01-15T15:45:00.000Z"
        use chrono::{DateTime, NaiveDateTime, Utc};

        // Try with Z suffix
        if let Ok(dt) = DateTime::parse_from_rfc3339(iso_date) {
            return Some(dt.timestamp_millis());
        }

        // Try without timezone
        if let Ok(ndt) = NaiveDateTime::parse_from_str(iso_date, "%Y-%m-%dT%H:%M:%S") {
            return Some(ndt.and_utc().timestamp_millis());
        }

        // Try with milliseconds
        if let Ok(ndt) = NaiveDateTime::parse_from_str(iso_date, "%Y-%m-%dT%H:%M:%S%.3f") {
            return Some(ndt.and_utc().timestamp_millis());
        }

        // Try date only
        if let Ok(ndt) = chrono::NaiveDate::parse_from_str(iso_date, "%Y-%m-%d") {
            return Some(ndt.and_hms_opt(23, 59, 59)?.and_utc().timestamp_millis());
        }

        None
    }

    /// Create and submit an order
    pub async fn create_order(
        &self,
        market: &MarketInfo,
        outcome_index: usize,
        side: Side,
        price: f64,
        size: f64,
    ) -> Result<Order> {
        let token = market
            .tokens
            .get(outcome_index)
            .context("Invalid outcome index")?;

        // Create order
        let mut order = Order {
            id: H256::random(),
            token_id: token.token_id.clone(),
            side,
            price,
            size,
            status: OrderStatus::Pending,
            created_at: chrono::Utc::now().timestamp_millis(),
            ..Default::default()
        };

        // Submit order
        let order_id = self.rest.create_order(&order).await?;
        order.id =
            H256::from_slice(&hex::decode(order_id.trim_start_matches("0x")).unwrap_or_default());

        // Track order
        let mut orders = self.orders.write().await;
        orders.insert(order.id, order.clone());

        Ok(order)
    }

    /// Cancel an order
    pub async fn cancel_order(&self, order_id: H256) -> Result<()> {
        let order_id_str = format!("0x{:x}", order_id);
        self.rest.cancel_order(&order_id_str).await?;

        let mut orders = self.orders.write().await;
        if let Some(order) = orders.get_mut(&order_id) {
            order.status = OrderStatus::Cancelled;
        }

        Ok(())
    }

    /// Get order status
    pub async fn get_order(&self, order_id: H256) -> Option<Order> {
        let orders = self.orders.read().await;
        orders.get(&order_id).cloned()
    }

    /// Get all active orders
    pub async fn active_orders(&self) -> Vec<Order> {
        let orders = self.orders.read().await;
        orders
            .values()
            .filter(|o| {
                matches!(
                    o.status,
                    OrderStatus::Pending | OrderStatus::Open | OrderStatus::PartiallyFilled
                )
            })
            .cloned()
            .collect()
    }

    /// Find token ID for a market slug (e.g., "btc-15m" -> token_id for YES)
    /// Returns the YES outcome token ID by default
    pub async fn find_token_id(&self, market_slug: &str) -> Option<String> {
        self.find_token_id_for_direction(market_slug, Direction::Up)
            .await
    }

    /// Find token for the requested direction.
    /// UP -> YES token, DOWN -> NO token.
    pub async fn find_token_id_for_direction(
        &self,
        market_slug: &str,
        direction: Direction,
    ) -> Option<String> {
        let cache_key = format!("{}#{}", market_slug.to_ascii_lowercase(), direction);

        // Check cache first
        {
            let cache = self.token_map.read().await;
            if let Some(token_id) = cache.get(&cache_key) {
                return Some(token_id.clone());
            }
        }

        // Search in markets - find token first, then cache after releasing lock
        let found_token: Option<String> = {
            let markets = self.markets.read().await;
            let slug_lower = market_slug.to_lowercase();
            let asset_keywords = extract_asset_keywords(market_slug);
            let timeframe_hint = infer_timeframe_hint(market_slug);
            let now_ms = chrono::Utc::now().timestamp_millis();
            let mut best: Option<(i32, i64, String)> = None;

            for market in markets.values() {
                if !market.active || !market.accepting_orders {
                    continue;
                }
                let expiry = market_expiry_timestamp_ms(market);
                if expiry > 0 && expiry <= now_ms + 30_000 {
                    continue;
                }
                let slug_match = market
                    .slug
                    .as_ref()
                    .map(|s| {
                        let s = s.to_ascii_lowercase();
                        s.contains(&slug_lower) || slug_lower.contains(&s)
                    })
                    .unwrap_or(false);
                let question = market.question.to_ascii_lowercase();
                let text = format!(
                    "{} {}",
                    market
                        .slug
                        .as_deref()
                        .unwrap_or_default()
                        .to_ascii_lowercase(),
                    question
                );
                let keyword_match = asset_keywords.iter().any(|k| text.contains(k));
                if !slug_match && !keyword_match {
                    continue;
                }
                let timeframe_match = timeframe_hint
                    .map(|hint| text_matches_timeframe_hint(&text, hint))
                    .unwrap_or(true);
                if !timeframe_match && !slug_match {
                    continue;
                }
                let Some(token_id) = select_token_for_direction(market, direction) else {
                    continue;
                };
                let mut score = 0i32;
                if slug_match {
                    score += 100;
                }
                if keyword_match {
                    score += 40;
                }
                if timeframe_match {
                    score += 30;
                }
                if market.enable_order_book {
                    score += 20;
                }
                if market.best_bid > 0.0
                    && market.best_ask > market.best_bid
                    && market.best_ask <= 1.0
                {
                    score += 20;
                    score -=
                        (((market.best_ask - market.best_bid).max(0.0) * 1000.0) as i32).min(200);
                }
                if market.liquidity_num > 0.0 {
                    score += (market.liquidity_num.log10().max(0.0) * 10.0) as i32;
                }
                match &best {
                    Some((best_score, best_expiry, _))
                        if score < *best_score
                            || (score == *best_score && expiry >= *best_expiry) => {}
                    _ => {
                        best = Some((score, expiry, token_id));
                    }
                }
            }

            best.map(|(_, _, token_id)| token_id)
        };

        // Cache the result if found
        if let Some(ref token_id) = found_token {
            let mut cache = self.token_map.write().await;
            cache.insert(cache_key, token_id.clone());
        }

        found_token
    }

    /// Get best bid/ask/mid quote for a token using orderbook endpoint.
    pub async fn quote_token(&self, token_id: &str) -> Result<TokenQuote> {
        let book = self.rest.get_order_book(token_id).await?;
        let mut bid = book.best_bid().map(|b| b.price).unwrap_or(0.0);
        let mut ask = book.best_ask().map(|a| a.price).unwrap_or(0.0);
        let mut bid_size = book.best_bid().map(|b| b.size).unwrap_or(0.0);
        let mut ask_size = book.best_ask().map(|a| a.size).unwrap_or(0.0);
        let depth_top5 = book.bids.iter().take(5).map(|b| b.size).sum::<f64>()
            + book.asks.iter().take(5).map(|a| a.size).sum::<f64>();

        let book_valid = bid > 0.0 && ask > 0.0 && ask > bid;
        let (mid, spread) = if book_valid {
            ((bid + ask) / 2.0, (ask - bid).max(0.0))
        } else {
            let mid = self.rest.get_midpoint(token_id).await.unwrap_or(0.0);
            let spread = self.rest.get_spread(token_id).await.unwrap_or(0.0).max(0.0);
            if mid > 0.0 {
                let half = spread / 2.0;
                bid = (mid - half).clamp(0.01, 0.99);
                ask = (mid + half).clamp(0.01, 0.99);
                if ask <= bid {
                    ask = (bid + 0.001).min(0.99);
                }
                if bid_size <= 0.0 {
                    bid_size = 0.0;
                }
                if ask_size <= 0.0 {
                    ask_size = 0.0;
                }
            }
            (mid, spread)
        };

        Ok(TokenQuote {
            bid,
            ask,
            mid,
            spread,
            bid_size,
            ask_size,
            depth_top5,
        })
    }

    /// Get historical traded prices for a token.
    pub async fn get_token_price_history(
        &self,
        token_id: &str,
        interval: PriceHistoryInterval,
        start_ts: Option<i64>,
        end_ts: Option<i64>,
        fidelity: Option<i32>,
    ) -> Result<Vec<MarketPrice>> {
        self.rest
            .get_price_history(token_id, interval, start_ts, end_ts, fidelity)
            .await
    }

    /// Fetch wallet positions from Data API
    pub async fn fetch_wallet_positions(&self, address: &str) -> Result<Vec<WalletPosition>> {
        let url = format!("{}/positions?user={}", self.data_api_url, address);

        let response = reqwest::Client::new()
            .get(&url)
            .send()
            .await
            .context("Failed to fetch positions from Data API")?;

        if !response.status().is_success() {
            bail!("Data API returned: {}", response.status());
        }

        let positions: Vec<WalletPosition> = response
            .json()
            .await
            .context("Failed to parse positions response")?;

        tracing::info!(address = %address, count = positions.len(), "📊 Fetched wallet positions");
        Ok(positions)
    }

    /// Check if dry run mode is enabled
    pub fn is_dry_run(&self) -> bool {
        self.dry_run
    }
}

fn market_market_text(market: &MarketInfo) -> String {
    format!(
        "{} {} {}",
        market
            .slug
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase(),
        market.question.to_ascii_lowercase(),
        market.outcomes.join(" ").to_ascii_lowercase()
    )
}

fn tokenized_words(raw: &str) -> Vec<String> {
    raw.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn market_matches_asset(asset: Asset, text: &str) -> bool {
    let words = tokenized_words(text);
    match asset {
        Asset::BTC => words.iter().any(|w| w == "btc" || w == "bitcoin"),
        Asset::ETH => words.iter().any(|w| w == "eth" || w == "ethereum"),
        Asset::SOL => words.iter().any(|w| w == "sol" || w == "solana"),
        Asset::XRP => words.iter().any(|w| w == "xrp" || w == "ripple"),
    }
}

fn market_expiry_timestamp_ms(market: &MarketInfo) -> i64 {
    if let Some(end_date) = market.end_date.as_deref() {
        if let Some(ts) = ClobClient::parse_expiry_to_timestamp(end_date) {
            return ts;
        }
    }
    if let Some(end_date_iso) = market.end_date_iso.as_deref() {
        if let Some(ts) = ClobClient::parse_expiry_to_timestamp(end_date_iso) {
            return ts;
        }
    }
    i64::MAX
}

fn matches_timeframe_expiry_window(timeframe: Timeframe, minutes_to_expiry: i64) -> bool {
    match timeframe {
        // Allow a small tolerance around nominal 15m cadence due to API/clock skew.
        Timeframe::Min15 => (1..=16).contains(&minutes_to_expiry),
        // Allow a small tolerance around nominal 1h cadence.
        Timeframe::Hour1 => (1..=65).contains(&minutes_to_expiry),
    }
}

/// Extract asset keywords from market slug (e.g., "btc-15m" -> ["btc", "bitcoin"])
fn extract_asset_keywords(slug: &str) -> Vec<String> {
    let mut keywords = Vec::new();
    let words = tokenized_words(slug);

    if words.iter().any(|w| w == "btc" || w == "bitcoin") {
        keywords.push("btc".to_string());
        keywords.push("bitcoin".to_string());
    }
    if words.iter().any(|w| w == "eth" || w == "ethereum") {
        keywords.push("eth".to_string());
        keywords.push("ethereum".to_string());
    }
    if words.iter().any(|w| w == "sol" || w == "solana") {
        keywords.push("sol".to_string());
        keywords.push("solana".to_string());
    }
    if words.iter().any(|w| w == "xrp" || w == "ripple") {
        keywords.push("xrp".to_string());
        keywords.push("ripple".to_string());
    }

    for part in words {
        if part.len() >= 2 {
            keywords.push(part);
        }
    }

    keywords
}

#[derive(Debug, Clone, Copy)]
enum TimeframeHint {
    Min15,
    Hour1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarketCadence {
    Min5,
    Min15,
    Hour1,
    Unknown,
}

fn infer_market_cadence(raw: &str) -> MarketCadence {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("5m")
        || lower.contains("5 min")
        || lower.contains("5-minute")
        || lower.contains("5 minute")
        || lower.contains("updown-5m")
    {
        return MarketCadence::Min5;
    }
    if lower.contains("15m")
        || lower.contains("15 min")
        || lower.contains("15-minute")
        || lower.contains("15 minute")
        || lower.contains("updown-15m")
    {
        return MarketCadence::Min15;
    }
    if lower.contains("1h")
        || lower.contains("1 hour")
        || lower.contains("60m")
        || lower.contains("60 min")
        || lower.contains("updown-1h")
        || looks_like_hourly_updown_market_text(&lower)
    {
        return MarketCadence::Hour1;
    }
    MarketCadence::Unknown
}

fn infer_timeframe_hint(raw: &str) -> Option<TimeframeHint> {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("15m")
        || lower.contains("15min")
        || lower.contains("15-min")
        || lower.contains("15_min")
    {
        return Some(TimeframeHint::Min15);
    }
    if lower.contains("1h")
        || lower.contains("hour1")
        || lower.contains("1-hour")
        || lower.contains("1_hour")
        || looks_like_hourly_updown_market_text(&lower)
    {
        return Some(TimeframeHint::Hour1);
    }
    None
}

fn text_matches_timeframe_hint(text: &str, hint: TimeframeHint) -> bool {
    let lower = text.to_ascii_lowercase();
    match hint {
        TimeframeHint::Min15 => {
            lower.contains("15m")
                || lower.contains("15 min")
                || lower.contains("15-minute")
                || lower.contains("15 minute")
        }
        TimeframeHint::Hour1 => {
            lower.contains("1h")
                || lower.contains("1 hour")
                || lower.contains("60m")
                || lower.contains("60 min")
                || lower.contains("hour")
                || looks_like_hourly_updown_market_text(&lower)
        }
    }
}

/// Detects Polymarket hourly crypto up/down markets that do not encode explicit
/// `1h` tokens in slug/title (e.g. `bitcoin-up-or-down-february-18-1am-et`).
fn looks_like_hourly_updown_market_text(lower: &str) -> bool {
    let is_updown = lower.contains("bitcoin up or down")
        || lower.contains("ethereum up or down")
        || lower.contains("bitcoin-up-or-down-")
        || lower.contains("ethereum-up-or-down-")
        || lower.contains("btc-updown-")
        || lower.contains("eth-updown-");
    if !is_updown {
        return false;
    }

    // Keep explicit intraday cadences out of Hour1 matching.
    if lower.contains("updown-5m")
        || lower.contains("updown-15m")
        || lower.contains("updown-4h")
        || lower.contains("5m")
        || lower.contains("15m")
        || lower.contains("4h")
    {
        return false;
    }

    // Hourly markets commonly appear as `...-11pm-et` / `...-1am-et`.
    if lower.contains("am-et") || lower.contains("pm-et") {
        return true;
    }

    // Question format: `Bitcoin Up or Down - February 18, 1AM ET` (no range/colon).
    let has_ampm_et = lower.contains("am et") || lower.contains("pm et");
    has_ampm_et && !lower.contains(':')
}

fn select_token_for_direction(market: &MarketInfo, direction: Direction) -> Option<String> {
    for token in &market.tokens {
        if let Some(mapped) = outcome_to_direction(&token.outcome) {
            if mapped == direction {
                return Some(token.token_id.clone());
            }
        }
    }
    None
}

fn outcome_to_direction(outcome: &str) -> Option<Direction> {
    let words = tokenized_words(outcome);
    let has_up = words
        .iter()
        .any(|word| matches!(word.as_str(), "yes" | "up" | "higher" | "above" | "true"));
    let has_down = words
        .iter()
        .any(|word| matches!(word.as_str(), "no" | "down" | "lower" | "below" | "false"));

    match (has_up, has_down) {
        (true, false) => Some(Direction::Up),
        (false, true) => Some(Direction::Down),
        _ => None,
    }
}

/// Wallet position from Data API
#[derive(Debug, Clone, Deserialize)]
pub struct WalletPosition {
    #[serde(alias = "market")]
    pub asset: String,
    #[serde(deserialize_with = "de_string_or_number")]
    pub size: String,
    #[serde(alias = "avgPrice", deserialize_with = "de_string_or_number")]
    pub avg_price: String,
    #[serde(
        default,
        alias = "currentPrice",
        deserialize_with = "de_opt_string_or_number"
    )]
    pub current_price: Option<String>,
    #[serde(
        default,
        alias = "realizedPnl",
        deserialize_with = "de_opt_string_or_number"
    )]
    pub realized_pnl: Option<String>,
    #[serde(default, alias = "conditionId")]
    pub condition_id: Option<String>,
    #[serde(default, alias = "tokenId")]
    pub token_id: Option<String>,
    #[serde(default)]
    pub outcome: Option<String>,
}

fn de_string_or_number<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Value {
        Str(String),
        F64(f64),
        I64(i64),
        U64(u64),
    }

    let value = Value::deserialize(deserializer)?;
    Ok(match value {
        Value::Str(s) => s,
        Value::F64(n) => n.to_string(),
        Value::I64(n) => n.to_string(),
        Value::U64(n) => n.to_string(),
    })
}

fn de_opt_string_or_number<'de, D>(deserializer: D) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Value {
        Str(String),
        F64(f64),
        I64(i64),
        U64(u64),
        Null(Option<()>),
    }

    let value = Value::deserialize(deserializer)?;
    Ok(match value {
        Value::Str(s) => Some(s),
        Value::F64(n) => Some(n.to_string()),
        Value::I64(n) => Some(n.to_string()),
        Value::U64(n) => Some(n.to_string()),
        Value::Null(_) => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = ClobConfig::default();
        assert_eq!(config.chain_id, 137);
        assert!(config.rest_url.contains("clob.polymarket.com"));
    }

    #[test]
    fn infer_winner_from_outcome_prices() {
        let prices = vec!["0".to_string(), "1".to_string()];
        assert_eq!(infer_winning_outcome_index(&prices), Some(1));

        let prices = vec!["0.998".to_string(), "0.002".to_string()];
        assert_eq!(infer_winning_outcome_index(&prices), Some(0));
    }

    #[test]
    fn select_token_requires_unambiguous_outcome_mapping() {
        let market = MarketInfo {
            condition_id: "cond".to_string(),
            question: "BTC up?".to_string(),
            outcomes: vec!["Yes".to_string(), "No".to_string()],
            tokens: vec![
                TokenInfo {
                    token_id: "a".to_string(),
                    outcome: "YES/NO".to_string(),
                    price: 0.5,
                },
                TokenInfo {
                    token_id: "b".to_string(),
                    outcome: "MAYBE".to_string(),
                    price: 0.5,
                },
            ],
            active: true,
            min_tick: 0.01,
            max_tick: 0.99,
            slug: Some("btc-15m".to_string()),
            end_date: Some("2030-01-01T00:00:00Z".to_string()),
            end_date_iso: Some("2030-01-01T00:00:00Z".to_string()),
            accepting_orders: true,
            enable_order_book: true,
            liquidity_num: 1000.0,
            best_bid: 0.49,
            best_ask: 0.51,
        };

        assert!(select_token_for_direction(&market, Direction::Up).is_none());
        assert!(select_token_for_direction(&market, Direction::Down).is_none());
    }

    #[test]
    fn timeframe_expiry_window_guard_is_strict() {
        assert!(matches_timeframe_expiry_window(Timeframe::Min15, 15));
        assert!(matches_timeframe_expiry_window(Timeframe::Min15, 1));
        assert!(!matches_timeframe_expiry_window(Timeframe::Min15, 20));

        assert!(matches_timeframe_expiry_window(Timeframe::Hour1, 60));
        assert!(matches_timeframe_expiry_window(Timeframe::Hour1, 5));
        assert!(!matches_timeframe_expiry_window(Timeframe::Hour1, 90));
    }

    #[test]
    fn hourly_slug_without_1h_token_is_detected() {
        let text = "bitcoin-up-or-down-february-18-1am-et bitcoin up or down - february 18, 1am et up down";
        assert!(text_matches_timeframe_hint(text, TimeframeHint::Hour1));
        assert_eq!(infer_market_cadence(text), MarketCadence::Hour1);
    }

    #[test]
    fn intraday_15m_slug_is_not_misclassified_as_hour1() {
        let text = "btc-updown-15m-1771387200 bitcoin up or down - february 17, 11:00pm-11:15pm et up down";
        assert!(text_matches_timeframe_hint(text, TimeframeHint::Min15));
        assert!(!text_matches_timeframe_hint(text, TimeframeHint::Hour1));
    }

    #[tokio::test]
    #[ignore = "Live network smoke test against Gamma/CLOB APIs"]
    async fn smoke_find_tradeable_market_live() {
        let client = ClobClient::new(ClobConfig::default());
        client
            .refresh_markets()
            .await
            .expect("refresh_markets should succeed");

        let cached = client.markets.read().await.len();
        assert!(cached > 0, "expected non-empty market cache");

        let lanes = [
            (Asset::BTC, Timeframe::Min15),
            (Asset::BTC, Timeframe::Hour1),
            (Asset::ETH, Timeframe::Min15),
            (Asset::ETH, Timeframe::Hour1),
        ];

        for (asset, timeframe) in lanes {
            let market = client
                .find_tradeable_market_for_signal(asset, timeframe)
                .await;
            assert!(
                market.is_some(),
                "expected tradeable market for {:?} {:?}, cache={}",
                asset,
                timeframe,
                cached
            );
        }
    }
}
