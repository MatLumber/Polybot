//! CLOB REST API Client
//!
//! Handles HTTP communication with Polymarket CLOB API
//! Endpoints documented at: https://docs.polymarket.com/developers/CLOB/clients/methods-public

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use ethers::types::Address;
use hmac::{Hmac, Mac};
use reqwest::{
    header::{HeaderMap, HeaderValue, CONTENT_TYPE},
    Client,
};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::time::Duration;

use super::types::{BookLevel, MarketResponse, Order, OrderBook, Side};

fn parse_book_level(price: &str, size: &str) -> Option<BookLevel> {
    let price = price.parse::<f64>().ok()?;
    let size = size.parse::<f64>().ok()?;
    if !price.is_finite() || !size.is_finite() || price <= 0.0 || size <= 0.0 {
        return None;
    }
    Some(BookLevel { price, size })
}

fn build_normalized_order_book(
    token_id: String,
    bids: Vec<(String, String)>,
    asks: Vec<(String, String)>,
    timestamp: i64,
) -> OrderBook {
    let mut book = OrderBook {
        token_id,
        bids: bids
            .into_iter()
            .filter_map(|(price, size)| parse_book_level(&price, &size))
            .collect(),
        asks: asks
            .into_iter()
            .filter_map(|(price, size)| parse_book_level(&price, &size))
            .collect(),
        timestamp,
    };
    book.normalize_levels();
    book
}

/// REST API client for Polymarket CLOB
pub struct RestClient {
    client: Client,
    base_url: String,
    address: Option<String>,
    api_key: Option<String>,
    api_secret: Option<String>,
    api_passphrase: Option<String>,
}

impl RestClient {
    /// Create a new REST client
    pub fn new(
        base_url: &str,
        address: Option<String>,
        api_key: Option<String>,
        api_secret: Option<String>,
        api_passphrase: Option<String>,
    ) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .default_headers(headers)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            address,
            api_key,
            api_secret,
            api_passphrase,
        }
    }

    fn resolve_env(var_names: &[&str]) -> Option<String> {
        for var in var_names {
            if let Ok(value) = std::env::var(var) {
                if !value.trim().is_empty() {
                    return Some(value);
                }
            }
        }
        None
    }

    fn auth_tuple(&self) -> Result<(String, String, String, String)> {
        let address = self
            .address
            .clone()
            .or_else(|| Self::resolve_env(&["POLYMARKET_ADDRESS"]))
            .context("POLYMARKET_ADDRESS not configured for authenticated CLOB requests")?;
        let api_key = self
            .api_key
            .clone()
            .or_else(|| Self::resolve_env(&["POLY_API_KEY", "API_KEY", "POLYMARKET_API_KEY"]))
            .context("POLY_API_KEY not configured for authenticated CLOB requests")?;
        let api_secret = self
            .api_secret
            .clone()
            .or_else(|| {
                Self::resolve_env(&["POLY_API_SECRET", "API_SECRET", "POLYMARKET_API_SECRET"])
            })
            .context("POLY_API_SECRET not configured for authenticated CLOB requests")?;
        let api_passphrase = self
            .api_passphrase
            .clone()
            .or_else(|| {
                Self::resolve_env(&[
                    "POLY_API_PASSPHRASE",
                    "API_PASSPHRASE",
                    "POLYMARKET_API_PASSPHRASE",
                ])
            })
            .context("POLY_API_PASSPHRASE not configured for authenticated CLOB requests")?;
        Ok((address, api_key, api_secret, api_passphrase))
    }

    fn build_l2_headers(&self, method: &str, request_path: &str, body: &str) -> Result<HeaderMap> {
        let (address, api_key, api_secret, api_passphrase) = self.auth_tuple()?;

        let timestamp = Utc::now().timestamp().to_string();
        let message = format!(
            "{}{}{}{}",
            timestamp,
            method.to_uppercase(),
            request_path,
            body
        );

        let secret_bytes = general_purpose::URL_SAFE_NO_PAD
            .decode(&api_secret)
            .or_else(|_| general_purpose::URL_SAFE.decode(&api_secret))
            .context("Failed to decode POLY_API_SECRET as url-safe base64")?;

        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(&secret_bytes)
            .context("Failed to initialize HMAC for CLOB signature")?;
        mac.update(message.as_bytes());
        let signature = general_purpose::URL_SAFE.encode(mac.finalize().into_bytes());

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "POLY_ADDRESS",
            HeaderValue::from_str(&address).context("Invalid POLY_ADDRESS header value")?,
        );
        headers.insert(
            "POLY_SIGNATURE",
            HeaderValue::from_str(&signature).context("Invalid POLY_SIGNATURE header value")?,
        );
        headers.insert(
            "POLY_TIMESTAMP",
            HeaderValue::from_str(&timestamp).context("Invalid POLY_TIMESTAMP header value")?,
        );
        headers.insert(
            "POLY_API_KEY",
            HeaderValue::from_str(&api_key).context("Invalid POLY_API_KEY header value")?,
        );
        headers.insert(
            "POLY_PASSPHRASE",
            HeaderValue::from_str(&api_passphrase)
                .context("Invalid POLY_PASSPHRASE header value")?,
        );
        Ok(headers)
    }

    fn build_l1_headers(
        address: Address,
        signature: &str,
        timestamp: i64,
        nonce: u64,
    ) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "POLY_ADDRESS",
            HeaderValue::from_str(&format!("{:#x}", address))
                .context("Invalid POLY_ADDRESS for L1 auth")?,
        );
        headers.insert(
            "POLY_SIGNATURE",
            HeaderValue::from_str(signature).context("Invalid POLY_SIGNATURE for L1 auth")?,
        );
        headers.insert(
            "POLY_TIMESTAMP",
            HeaderValue::from_str(&timestamp.to_string())
                .context("Invalid POLY_TIMESTAMP for L1 auth")?,
        );
        headers.insert(
            "POLY_NONCE",
            HeaderValue::from_str(&nonce.to_string()).context("Invalid POLY_NONCE for L1 auth")?,
        );
        Ok(headers)
    }

    fn extract_l2_credentials(raw: &serde_json::Value) -> Result<(String, String, String)> {
        fn pick(value: &serde_json::Value, candidates: &[&str]) -> Option<String> {
            for key in candidates {
                if let Some(v) = value.get(*key).and_then(|v| v.as_str()) {
                    if !v.trim().is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
            None
        }

        let data = raw.get("data").unwrap_or(raw);
        let api_key = pick(data, &["apiKey", "api_key", "key", "id"])
            .context("Missing api key in auth response")?;
        let api_secret = pick(data, &["secret", "apiSecret", "api_secret"])
            .context("Missing api secret in auth response")?;
        let passphrase = pick(data, &["passphrase", "apiPassphrase", "api_passphrase"])
            .context("Missing passphrase in auth response")?;
        Ok((api_key, api_secret, passphrase))
    }

    /// Create or derive L2 API credentials using L1 signed auth endpoints.
    /// Flow:
    /// 1) POST /auth/api-key
    /// 2) fallback GET /auth/derive-api-key
    pub async fn create_or_derive_api_credentials(
        &self,
        private_key: &str,
        chain_id: u64,
        address: Address,
    ) -> Result<(String, String, String)> {
        let timestamp = Utc::now().timestamp();
        let nonce = rand::random::<u64>();
        let signature =
            super::signing::create_l1_signature(private_key, chain_id, timestamp, nonce).await?;
        let headers = Self::build_l1_headers(address, &signature, timestamp, nonce)?;

        let create_url = format!("{}/auth/api-key", self.base_url);
        let create_resp = self
            .client
            .post(&create_url)
            .headers(headers.clone())
            .body("{}")
            .send()
            .await
            .context("Failed POST /auth/api-key")?;

        if create_resp.status().is_success() {
            let raw: serde_json::Value = create_resp
                .json()
                .await
                .context("Failed parsing /auth/api-key response")?;
            return Self::extract_l2_credentials(&raw);
        }

        let derive_url = format!("{}/auth/derive-api-key", self.base_url);
        let derive_resp = self
            .client
            .get(&derive_url)
            .headers(headers)
            .send()
            .await
            .context("Failed GET /auth/derive-api-key")?;

        if !derive_resp.status().is_success() {
            let create_status = create_resp.status();
            let create_body = create_resp.text().await.unwrap_or_default();
            let derive_status = derive_resp.status();
            let derive_body = derive_resp.text().await.unwrap_or_default();
            bail!(
                "L1 auth endpoints failed. create: {} [{}], derive: {} [{}]",
                create_status,
                create_body,
                derive_status,
                derive_body
            );
        }

        let raw: serde_json::Value = derive_resp
            .json()
            .await
            .context("Failed parsing /auth/derive-api-key response")?;
        Self::extract_l2_credentials(&raw)
    }

    /// Get markets from Gamma API
    pub async fn get_markets(
        &self,
        gamma_url: &str,
        tag: Option<&str>,
    ) -> Result<Vec<MarketResponse>> {
        self.get_markets_page(gamma_url, tag, 500, 0).await
    }

    /// Get a single page of markets from Gamma API.
    pub async fn get_markets_page(
        &self,
        gamma_url: &str,
        tag: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MarketResponse>> {
        let base = format!(
            "{}/markets?closed=false&active=true&limit={}&offset={}",
            gamma_url.trim_end_matches('/'),
            limit.max(1),
            offset
        );
        let url = match tag {
            Some(t) => format!("{base}&tag={t}"),
            None => base,
        };

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch markets")?;

        if !response.status().is_success() {
            bail!("Failed to get markets: {}", response.status());
        }

        let markets: Vec<MarketResponse> = response
            .json()
            .await
            .context("Failed to parse markets response")?;

        Ok(markets)
    }

    /// Get market by condition ID
    pub async fn get_market(&self, gamma_url: &str, condition_id: &str) -> Result<MarketResponse> {
        let url = format!(
            "{}/markets?condition_id={}",
            gamma_url.trim_end_matches('/'),
            condition_id
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch market")?;

        if !response.status().is_success() {
            bail!("Failed to get market: {}", response.status());
        }

        let mut markets: Vec<MarketResponse> = response
            .json()
            .await
            .context("Failed to parse market response")?;

        markets.pop().context("No market found for condition ID")
    }

    /// Create a new order
    pub async fn create_order(&self, order: &Order) -> Result<String> {
        let request_path = "/order";
        let url = format!("{}{}", self.base_url, request_path);

        let (address, api_key, _, _) = self.auth_tuple()?;
        let maker = order
            .maker
            .map(|a| format!("{:#x}", a))
            .unwrap_or_else(|| address);
        let signature = order
            .signature
            .clone()
            .context("Order signature missing for CLOB POST /order")?;

        let shares_scaled = (order.size.max(0.0) * 1_000_000.0).round() as u128;
        let usdc_scaled =
            (order.size.max(0.0) * order.price.max(0.0) * 1_000_000.0).round() as u128;
        let (maker_amount, taker_amount) = match order.side {
            Side::Buy => (usdc_scaled, shares_scaled),
            Side::Sell => (shares_scaled, usdc_scaled),
        };

        let payload = serde_json::json!({
            "order": {
                "salt": order.salt.to_string(),
                "maker": maker,
                "signer": maker,
                "taker": "0x0000000000000000000000000000000000000000",
                "tokenId": order.token_id,
                "makerAmount": maker_amount.to_string(),
                "takerAmount": taker_amount.to_string(),
                "expiration": order.expiration.to_string(),
                "nonce": order.nonce.to_string(),
                "feeRateBps": "0",
                "side": match order.side { Side::Buy => "BUY", Side::Sell => "SELL" },
                "signatureType": order.signature_type,
                "signature": signature
            },
            "owner": api_key,
            "orderType": if order.expiration > 0 { "GTD" } else { "GTC" },
            "postOnly": order.expiration > 0
        });

        let body =
            serde_json::to_string(&payload).context("Failed to serialize create order payload")?;
        let headers = self.build_l2_headers("POST", request_path, &body)?;

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .body(body)
            .send()
            .await
            .context("Failed to create order")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            bail!("Failed to create order: {} - {}", status, text);
        }

        let raw: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse create order response")?;

        if let Some(id) = raw
            .get("orderID")
            .and_then(|v| v.as_str())
            .or_else(|| raw.get("order_id").and_then(|v| v.as_str()))
            .or_else(|| raw.get("orderId").and_then(|v| v.as_str()))
            .or_else(|| raw.get("id").and_then(|v| v.as_str()))
        {
            return Ok(id.to_string());
        }

        bail!("Missing order id in create order response: {}", raw)
    }

    /// Cancel an order
    pub async fn cancel_order(&self, order_id: &str) -> Result<bool> {
        let request_path = format!("/order/{}", order_id);
        let url = format!("{}{}", self.base_url, request_path);
        let headers = self.build_l2_headers("DELETE", &request_path, "")?;

        let response = self
            .client
            .delete(&url)
            .headers(headers)
            .send()
            .await
            .context("Failed to cancel order")?;

        Ok(response.status().is_success())
    }

    /// Get order book for a token
    pub async fn get_order_book(&self, token_id: &str) -> Result<OrderBook> {
        let url = format!("{}/book?token_id={}", self.base_url, token_id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch order book")?;

        if !response.status().is_success() {
            bail!("Failed to get order book: {}", response.status());
        }

        #[derive(Deserialize)]
        struct BookResponse {
            market: String,
            asset_id: String,
            bids: Vec<BookEntry>,
            asks: Vec<BookEntry>,
            hash: String,
            timestamp: String,
        }

        #[derive(Deserialize)]
        struct BookEntry {
            price: String,
            size: String,
        }

        let resp: BookResponse = response
            .json()
            .await
            .context("Failed to parse order book response")?;

        Ok(build_normalized_order_book(
            resp.asset_id,
            resp.bids
                .into_iter()
                .map(|entry| (entry.price, entry.size))
                .collect(),
            resp.asks
                .into_iter()
                .map(|entry| (entry.price, entry.size))
                .collect(),
            chrono::Utc::now().timestamp(),
        ))
    }

    /// Get order by ID
    pub async fn get_order(&self, order_id: &str) -> Result<Order> {
        let request_path = format!("/data/order/{}", order_id);
        let url = format!("{}{}", self.base_url, request_path);
        let headers = self.build_l2_headers("GET", &request_path, "")?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Failed to fetch order")?;

        if !response.status().is_success() {
            bail!("Failed to get order: {}", response.status());
        }

        let order: Order = response
            .json()
            .await
            .context("Failed to parse order response")?;

        Ok(order)
    }

    /// Get all open orders
    pub async fn get_open_orders(&self) -> Result<Vec<Order>> {
        let request_path = "/data/orders";
        let url = format!("{}{}", self.base_url, request_path);
        let headers = self.build_l2_headers("GET", request_path, "")?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Failed to fetch open orders")?;

        if !response.status().is_success() {
            bail!("Failed to get open orders: {}", response.status());
        }

        let orders: Vec<Order> = response
            .json()
            .await
            .context("Failed to parse open orders response")?;

        Ok(orders)
    }

    /// Get current positions
    pub async fn get_positions(&self) -> Result<Vec<super::types::Position>> {
        let request_path = "/positions";
        let url = format!("{}{}", self.base_url, request_path);
        let headers = self.build_l2_headers("GET", request_path, "")?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Failed to fetch positions")?;

        if !response.status().is_success() {
            bail!("Failed to get positions: {}", response.status());
        }

        let positions: Vec<super::types::Position> = response
            .json()
            .await
            .context("Failed to parse positions response")?;

        Ok(positions)
    }

    /// Get USDC balance for the authenticated wallet
    pub async fn get_balance(&self) -> Result<f64> {
        let request_path = "/balance-allowance?asset_type=COLLATERAL&signature_type=0";
        let url = format!("{}{}", self.base_url, request_path);
        let headers = self.build_l2_headers("GET", request_path, "")?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Failed to fetch balance")?;

        if !response.status().is_success() {
            bail!("Failed to get balance: {}", response.status());
        }

        let raw: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse balance response")?;

        let balance_str = raw
            .get("balance")
            .and_then(|v| v.as_str())
            .or_else(|| raw.get("available").and_then(|v| v.as_str()))
            .or_else(|| raw.get("amount").and_then(|v| v.as_str()))
            .context("balance field missing in balance-allowance response")?;

        balance_str.parse().context("Failed to parse balance value")
    }

    /// Get midpoint price for a token
    /// Endpoint: GET /midpoint?token_id={id}
    pub async fn get_midpoint(&self, token_id: &str) -> Result<f64> {
        let url = format!("{}/midpoint?token_id={}", self.base_url, token_id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch midpoint")?;

        if !response.status().is_success() {
            bail!("Failed to get midpoint: {}", response.status());
        }

        #[derive(Deserialize)]
        struct MidpointResponse {
            mid: String,
        }

        let resp: MidpointResponse = response
            .json()
            .await
            .context("Failed to parse midpoint response")?;

        resp.mid.parse().context("Failed to parse midpoint value")
    }

    /// Get current best price for a token
    /// Endpoint: GET /price?token_id={id}&side={BUY|SELL}
    pub async fn get_price(&self, token_id: &str, side: Side) -> Result<f64> {
        let side_str = match side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        };
        let url = format!(
            "{}/price?token_id={}&side={}",
            self.base_url, token_id, side_str
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch price")?;

        if !response.status().is_success() {
            bail!("Failed to get price: {}", response.status());
        }

        #[derive(Deserialize)]
        struct PriceResponse {
            price: String,
        }

        let resp: PriceResponse = response
            .json()
            .await
            .context("Failed to parse price response")?;

        resp.price.parse().context("Failed to parse price value")
    }

    /// Get last trade price for a token
    /// Endpoint: GET /last_trade_price?token_id={id}
    pub async fn get_last_trade_price(&self, token_id: &str) -> Result<LastTradePrice> {
        let url = format!("{}/last_trade_price?token_id={}", self.base_url, token_id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch last trade price")?;

        if !response.status().is_success() {
            bail!("Failed to get last trade price: {}", response.status());
        }

        let resp: LastTradePrice = response
            .json()
            .await
            .context("Failed to parse last trade price response")?;

        Ok(resp)
    }

    /// Get spread for a token
    /// Endpoint: GET /spread?token_id={id}
    pub async fn get_spread(&self, token_id: &str) -> Result<f64> {
        let url = format!("{}/spread?token_id={}", self.base_url, token_id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch spread")?;

        if !response.status().is_success() {
            bail!("Failed to get spread: {}", response.status());
        }

        #[derive(Deserialize)]
        struct SpreadResponse {
            spread: String,
        }

        let resp: SpreadResponse = response
            .json()
            .await
            .context("Failed to parse spread response")?;

        resp.spread.parse().context("Failed to parse spread value")
    }

    /// Get prices for multiple tokens
    /// Endpoint: GET /prices with token_ids as query params
    pub async fn get_prices(&self, token_ids: &[&str]) -> Result<HashMap<String, TokenPrices>> {
        let ids_param = token_ids.join(",");
        let url = format!("{}/prices?token_ids={}", self.base_url, ids_param);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch prices")?;

        if !response.status().is_success() {
            bail!("Failed to get prices: {}", response.status());
        }

        let resp: HashMap<String, TokenPrices> = response
            .json()
            .await
            .context("Failed to parse prices response")?;

        Ok(resp)
    }

    /// Get price history for a token
    /// Endpoint: GET /prices-history
    pub async fn get_price_history(
        &self,
        token_id: &str,
        interval: PriceHistoryInterval,
        start_ts: Option<i64>,
        end_ts: Option<i64>,
        fidelity: Option<i32>,
    ) -> Result<Vec<MarketPrice>> {
        let mut url = format!(
            "{}/prices-history?market={}&interval={}",
            self.base_url,
            token_id,
            interval.as_str()
        );

        if let Some(start) = start_ts {
            url.push_str(&format!("&startTs={}", start));
        }
        if let Some(end) = end_ts {
            url.push_str(&format!("&endTs={}", end));
        }
        if let Some(f) = fidelity {
            url.push_str(&format!("&fidelity={}", f));
        }

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch price history")?;

        if !response.status().is_success() {
            bail!("Failed to get price history: {}", response.status());
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum PriceHistoryPayload {
            Direct(Vec<MarketPrice>),
            Wrapped { history: Vec<MarketPrice> },
        }

        let payload: PriceHistoryPayload = response
            .json()
            .await
            .context("Failed to parse price history response")?;

        let mut points = match payload {
            PriceHistoryPayload::Direct(points) => points,
            PriceHistoryPayload::Wrapped { history } => history,
        };
        points.sort_by_key(|point| point.t);

        Ok(points)
    }
}

/// Last trade price response
#[derive(Debug, Clone, Deserialize)]
pub struct LastTradePrice {
    pub price: String,
    pub side: String,
}

/// Token prices (buy and sell)
#[derive(Debug, Clone, Deserialize)]
pub struct TokenPrices {
    #[serde(rename = "BUY")]
    pub buy: Option<String>,
    #[serde(rename = "SELL")]
    pub sell: Option<String>,
}

/// Market price point for price history
#[derive(Debug, Clone, Deserialize)]
pub struct MarketPrice {
    pub t: i64,
    pub p: f64,
}

/// Price history interval
#[derive(Debug, Clone, Copy)]
pub enum PriceHistoryInterval {
    Max,
    OneWeek,
    OneDay,
    SixHours,
    OneHour,
}

impl PriceHistoryInterval {
    pub fn as_str(&self) -> &'static str {
        match self {
            PriceHistoryInterval::Max => "max",
            PriceHistoryInterval::OneWeek => "1w",
            PriceHistoryInterval::OneDay => "1d",
            PriceHistoryInterval::SixHours => "6h",
            PriceHistoryInterval::OneHour => "1h",
        }
    }
}

/// Fetch crypto markets from Gamma API
pub async fn fetch_crypto_markets(gamma_url: &str) -> Result<Vec<MarketResponse>> {
    let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

    // Fetch markets tagged with crypto
    let url = format!(
        "{}/markets?tag=crypto&closed=false",
        gamma_url.trim_end_matches('/')
    );

    let response = client
        .get(&url)
        .send()
        .await
        .context("Failed to fetch crypto markets")?;

    if !response.status().is_success() {
        bail!("Failed to get crypto markets: {}", response.status());
    }

    let markets: Vec<MarketResponse> = response
        .json()
        .await
        .context("Failed to parse crypto markets response")?;

    Ok(markets)
}

#[cfg(test)]
mod tests {
    use super::build_normalized_order_book;

    #[test]
    fn build_normalized_order_book_sorts_and_filters_levels() {
        let book = build_normalized_order_book(
            "token".to_string(),
            vec![
                ("0.45".to_string(), "3".to_string()),
                ("0.60".to_string(), "0".to_string()),
                ("0.55".to_string(), "1.5".to_string()),
            ],
            vec![
                ("0.70".to_string(), "1".to_string()),
                ("0.61".to_string(), "2".to_string()),
                ("0.59".to_string(), "-5".to_string()),
            ],
            123,
        );

        assert_eq!(book.bids.len(), 2);
        assert_eq!(book.bids[0].price, 0.55);
        assert_eq!(book.bids[1].price, 0.45);
        assert_eq!(book.asks.len(), 2);
        assert_eq!(book.asks[0].price, 0.61);
        assert_eq!(book.asks[1].price, 0.70);
    }
}
