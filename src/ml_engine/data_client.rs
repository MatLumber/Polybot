//! Polymarket Historical Data Client - FUNCIONAL
//!
//! Descarga datos históricos reales de mercados cerrados de Polymarket

use crate::ml_engine::dataset::TradeSample;
use crate::ml_engine::features::MLFeatureVector;
use crate::types::{Asset, Direction, Timeframe};
use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use reqwest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Cliente para la API de Polymarket
pub struct PolymarketDataClient {
    client: reqwest::Client,
    gamma_api_url: String,
    clob_api_url: String,
    data_api_url: String,
}

/// Evento de Polymarket
#[derive(Debug, Clone, Deserialize)]
pub struct PolymarketEvent {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    #[serde(rename = "endDate")]
    pub end_date: Option<String>,
    pub active: bool,
    pub closed: bool,
    pub markets: Vec<PolymarketMarket>,
    pub tags: Vec<PolymarketTag>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PolymarketTag {
    pub id: String,
    pub label: String,
}

/// Mercado individual
#[derive(Debug, Clone, Deserialize)]
pub struct PolymarketMarket {
    pub id: String,
    pub slug: String,
    pub question: String,
    #[serde(rename = "conditionId")]
    pub condition_id: String,
    #[serde(rename = "outcomePrices")]
    pub outcome_prices: Option<Vec<String>>,
    pub outcomes: Option<Vec<String>>,
    pub tokens: Vec<MarketToken>,
    #[serde(rename = "endDate")]
    pub end_date: Option<String>,
    pub closed: bool,
    pub active: bool,
    #[serde(rename = "volume24hr")]
    pub volume_24hr: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketToken {
    #[serde(rename = "tokenId")]
    pub token_id: String,
    pub outcome: String,
    pub price: Option<f64>,
}

/// Precio histórico
#[derive(Debug, Clone, Deserialize)]
pub struct HistoricalPrice {
    pub t: i64,    // timestamp en segundos
    pub p: String, // price como string
}

/// Respuesta de precios históricos
#[derive(Debug, Clone, Deserialize)]
pub struct PriceHistoryResponse {
    pub history: Vec<HistoricalPrice>,
}

/// Trade histórico
#[derive(Debug, Clone, Deserialize)]
pub struct HistoricalTrade {
    pub side: String,
    pub size: String,
    pub price: String,
    pub timestamp: i64,
    #[serde(rename = "transactionHash")]
    pub transaction_hash: Option<String>,
}

impl PolymarketDataClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            gamma_api_url: "https://gamma-api.polymarket.com".to_string(),
            clob_api_url: "https://clob.polymarket.com".to_string(),
            data_api_url: "https://data-api.polymarket.com".to_string(),
        }
    }

    /// Buscar eventos de mercados cripto
    pub async fn fetch_crypto_markets(
        &self,
        closed: bool,
        limit: usize,
    ) -> Result<Vec<PolymarketEvent>> {
        info!(
            "Fetching {} crypto markets from Polymarket...",
            if closed { "closed" } else { "active" }
        );

        let url = format!(
            "{}/events?closed={}&active={}&limit={}&order=volume&ascending=false",
            self.gamma_api_url, closed, !closed, limit
        );

        let response = self.client.get(&url).send().await?;
        let events: Vec<PolymarketEvent> = response.json().await?;

        // Filtrar solo eventos relacionados con BTC/ETH/SOL/XRP
        let crypto_keywords = [
            "bitcoin", "btc", "ethereum", "eth", "solana", "sol", "ripple", "xrp",
        ];
        let crypto_events: Vec<_> = events
            .into_iter()
            .filter(|e| {
                let text = format!(
                    "{} {} {}",
                    e.title,
                    e.slug,
                    e.description.as_deref().unwrap_or("")
                )
                .to_lowercase();
                crypto_keywords.iter().any(|&kw| text.contains(kw))
            })
            .collect();

        info!("Found {} crypto events", crypto_events.len());
        Ok(crypto_events)
    }

    /// Descargar precios históricos de un token
    pub async fn fetch_price_history(
        &self,
        token_id: &str,
        start_ts: i64,
        end_ts: i64,
        fidelity: i64, // minutos
    ) -> Result<Vec<HistoricalPrice>> {
        let url = format!("{}/prices-history", self.clob_api_url);

        let params = [
            ("market", token_id),
            ("startTs", &start_ts.to_string()),
            ("endTs", &end_ts.to_string()),
            ("fidelity", &fidelity.to_string()),
        ];

        let response = self.client.get(&url).query(&params).send().await?;

        if response.status().is_success() {
            let data: PriceHistoryResponse = response.json().await?;
            debug!(
                "Fetched {} price points for token {}",
                data.history.len(),
                token_id
            );
            Ok(data.history)
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!("Failed to fetch price history: {} - {}", status, body);
            Ok(Vec::new())
        }
    }

    /// Descargar trades históricos
    pub async fn fetch_historical_trades(
        &self,
        condition_id: &str,
        limit: usize,
    ) -> Result<Vec<HistoricalTrade>> {
        let url = format!("{}/trades", self.data_api_url);

        let params = [("conditionId", condition_id), ("limit", &limit.to_string())];

        let response = self.client.get(&url).query(&params).send().await?;

        if response.status().is_success() {
            let trades: Vec<HistoricalTrade> = response.json().await?;
            Ok(trades)
        } else {
            warn!(
                "Failed to fetch trades for condition {}: {}",
                condition_id,
                response.status()
            );
            Ok(Vec::new())
        }
    }

    /// Parsear pregunta del mercado para determinar asset y timeframe
    pub fn parse_market_metadata(&self, question: &str) -> (Asset, Timeframe, String) {
        let q = question.to_lowercase();

        // Determinar asset
        let asset = if q.contains("bitcoin") || q.contains("btc") {
            Asset::BTC
        } else if q.contains("ethereum") || q.contains("eth") {
            Asset::ETH
        } else if q.contains("solana") || q.contains("sol") {
            Asset::SOL
        } else if q.contains("ripple") || q.contains("xrp") {
            Asset::XRP
        } else {
            Asset::BTC
        };

        // Determinar timeframe
        let timeframe = if q.contains("15 min") || q.contains("15m") || q.contains("15 minute") {
            Timeframe::Min15
        } else if q.contains("1 hour")
            || q.contains("1h")
            || q.contains("hour") && !q.contains("24")
        {
            Timeframe::Hour1
        } else {
            Timeframe::Min15
        };

        // Determinar dirección
        let direction = if q.contains("higher")
            || q.contains("above")
            || q.contains("up")
            || q.contains("over")
        {
            "UP"
        } else if q.contains("lower")
            || q.contains("below")
            || q.contains("down")
            || q.contains("under")
        {
            "DOWN"
        } else {
            "UNKNOWN"
        }
        .to_string();

        (asset, timeframe, direction)
    }

    /// Calcular features desde historial de precios
    pub fn calculate_features_from_prices(
        &self,
        prices: &[HistoricalPrice],
        asset: Asset,
        timeframe: Timeframe,
    ) -> MLFeatureVector {
        let mut features = MLFeatureVector::default();

        if prices.len() < 10 {
            return features;
        }

        // Extraer precios como f64
        let price_values: Vec<f64> = prices
            .iter()
            .filter_map(|p| p.p.parse::<f64>().ok())
            .collect();

        if price_values.len() < 10 {
            return features;
        }

        // RSI simple
        let gains: Vec<f64> = price_values
            .windows(2)
            .map(|w| if w[1] > w[0] { w[1] - w[0] } else { 0.0 })
            .collect();

        let losses: Vec<f64> = price_values
            .windows(2)
            .map(|w| if w[1] < w[0] { w[0] - w[1] } else { 0.0 })
            .collect();

        let avg_gain = if !gains.is_empty() {
            gains.iter().sum::<f64>() / gains.len() as f64
        } else {
            0.01
        };

        let avg_loss = if !losses.is_empty() {
            losses.iter().sum::<f64>() / losses.len() as f64
        } else {
            0.01
        };

        let rs = avg_gain / avg_loss.max(0.001);
        features.rsi = 100.0 - (100.0 / (1.0 + rs));
        features.rsi_normalized = (features.rsi - 50.0) / 50.0;

        // MACD simple
        let ema12 = self.calculate_ema(&price_values, 12);
        let ema26 = self.calculate_ema(&price_values, 26);
        features.macd = ema12 - ema26;
        features.macd_histogram = features.macd;

        // ADX simplificado
        let plus_dm: Vec<f64> = price_values
            .windows(2)
            .map(|w| if w[1] > w[0] { w[1] - w[0] } else { 0.0 })
            .collect();
        let minus_dm: Vec<f64> = price_values
            .windows(2)
            .map(|w| if w[1] < w[0] { w[0] - w[1] } else { 0.0 })
            .collect();

        features.plus_di = if !plus_dm.is_empty() {
            plus_dm.iter().sum::<f64>() / plus_dm.len() as f64 * 100.0
        } else {
            0.0
        };

        features.minus_di = if !minus_dm.is_empty() {
            minus_dm.iter().sum::<f64>() / minus_dm.len() as f64 * 100.0
        } else {
            0.0
        };

        features.adx = (features.plus_di + features.minus_di) / 2.0;
        features.trend_strength = features.adx * (features.plus_di - features.minus_di) / 100.0;

        // Momentum
        let first_price = price_values.first().unwrap_or(&0.5);
        let last_price = price_values.last().unwrap_or(&0.5);
        let momentum = (last_price - first_price) / first_price;
        features.price_velocity = momentum / price_values.len() as f64;

        // Volatilidad
        let mean = price_values.iter().sum::<f64>() / price_values.len() as f64;
        let variance = price_values.iter().map(|p| (p - mean).powi(2)).sum::<f64>()
            / price_values.len() as f64;
        features.volatility_5m = variance.sqrt();

        // Metadata
        features.is_btc = if asset == Asset::BTC { 1.0 } else { 0.0 };
        features.is_15m = if timeframe == Timeframe::Min15 {
            1.0
        } else {
            0.0
        };

        // Calibrador confidence (placeholder)
        features.calibrator_confidence = 0.5;
        features.num_indicators_agreeing = 3.0;

        features
    }

    fn calculate_ema(&self, prices: &[f64], period: usize) -> f64 {
        if prices.len() < period {
            return prices.last().copied().unwrap_or(0.5);
        }

        let k = 2.0 / (period as f64 + 1.0);
        let mut ema = prices[0];

        for price in prices.iter().skip(1) {
            ema = price * k + ema * (1.0 - k);
        }

        ema
    }

    /// Convertir eventos a TradeSamples
    pub async fn convert_events_to_trades(
        &self,
        events: Vec<PolymarketEvent>,
    ) -> Result<Vec<TradeSample>> {
        let mut samples = Vec::new();

        let events_len = events.len();
        for event in events {
            for market in event.markets {
                if !market.closed {
                    continue;
                }

                let (asset, timeframe, direction_str) =
                    self.parse_market_metadata(&market.question);

                // Procesar cada token (Yes/No)
                for token in &market.tokens {
                    if token.token_id.is_empty() {
                        continue;
                    }

                    // Parsear fechas
                    let end_ts = if let Some(end_date) = &market.end_date {
                        DateTime::parse_from_rfc3339(end_date)
                            .ok()
                            .map(|d| d.timestamp())
                            .unwrap_or_else(|| Utc::now().timestamp())
                    } else {
                        Utc::now().timestamp()
                    };

                    let start_ts = end_ts - 86400 * 7; // 7 días antes

                    // Descargar precios históricos
                    let prices = self
                        .fetch_price_history(
                            &token.token_id,
                            start_ts,
                            end_ts,
                            5, // 5 minutos
                        )
                        .await?;

                    if prices.len() < 20 {
                        continue;
                    }

                    // Determinar resultado final
                    let final_price = prices
                        .last()
                        .and_then(|p| p.p.parse::<f64>().ok())
                        .unwrap_or(0.5);

                    let is_win = if direction_str == "UP" {
                        final_price > 0.5
                    } else if direction_str == "DOWN" {
                        final_price < 0.5
                    } else {
                        (final_price - 0.5).abs() > 0.1
                    };

                    // Calcular features
                    let features = self.calculate_features_from_prices(&prices, asset, timeframe);

                    let entry_price = prices
                        .first()
                        .and_then(|p| p.p.parse::<f64>().ok())
                        .unwrap_or(0.5);

                    let sample = TradeSample {
                        trade_id: format!("{}-{}", market.condition_id, token.token_id),
                        entry_ts: prices.first().map(|p| p.t * 1000).unwrap_or(0),
                        exit_ts: prices.last().map(|p| p.t * 1000).unwrap_or(0),
                        asset,
                        timeframe,
                        direction: if direction_str == "UP" {
                            Direction::Up
                        } else {
                            Direction::Down
                        },
                        is_win,
                        entry_features: features,
                        entry_price,
                        exit_price: final_price,
                        pnl: if is_win { 0.8 } else { -1.0 },
                        estimated_edge: (final_price - 0.5).abs(),
                        indicators_triggered: vec!["historical".to_string()],
                    };

                    samples.push(sample);
                }

                // Rate limiting
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }

        info!(
            "Converted {} events into {} trade samples",
            events_len,
            samples.len()
        );
        Ok(samples)
    }

    /// Descargar dataset histórico COMPLETO
    pub async fn download_historical_dataset(
        &self,
        min_samples: usize,
    ) -> Result<Vec<TradeSample>> {
        info!(
            "🚀 Downloading historical dataset (target: {} samples)...",
            min_samples
        );

        let mut all_samples = Vec::new();
        let mut offset = 0;
        let batch_size = 50;
        let max_batches = 20; // Limitar para no saturar la API

        for batch in 0..max_batches {
            if all_samples.len() >= min_samples {
                break;
            }

            info!(
                "Fetching batch {} (have {} samples so far)...",
                batch + 1,
                all_samples.len()
            );

            let events = self.fetch_crypto_markets(true, batch_size).await?;

            if events.is_empty() {
                info!("No more events found");
                break;
            }

            let samples = self.convert_events_to_trades(events).await?;
            all_samples.extend(samples);

            offset += batch_size;

            // Rate limiting entre batches
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        info!(
            "✅ Download complete: {} historical trade samples",
            all_samples.len()
        );

        // Guardar a disco para uso futuro
        self.save_to_cache(&all_samples)?;

        Ok(all_samples)
    }

    /// Guardar datos en caché
    fn save_to_cache(&self, samples: &[TradeSample]) -> Result<()> {
        let cache_path = "data/historical_cache.json";
        let json = serde_json::to_string_pretty(samples)?;
        std::fs::write(cache_path, json)?;
        info!("Saved {} samples to cache", samples.len());
        Ok(())
    }

    /// Cargar datos desde caché
    pub fn load_from_cache(&self) -> Result<Vec<TradeSample>> {
        let cache_path = "data/historical_cache.json";
        if std::path::Path::new(cache_path).exists() {
            let json = std::fs::read_to_string(cache_path)?;
            let samples: Vec<TradeSample> = serde_json::from_str(&json)?;
            info!("Loaded {} samples from cache", samples.len());
            Ok(samples)
        } else {
            Ok(Vec::new())
        }
    }
}

impl Default for PolymarketDataClient {
    fn default() -> Self {
        Self::new()
    }
}
