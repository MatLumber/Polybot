//! Oracle Aggregator - Combines price data from multiple sources
//!
//! Aggregates prices from Binance, Bybit, Coinbase, and RTDS into a single
//! normalized price with confidence scoring based on source agreement.

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};

use crate::oracle::NormalizedTick;
use crate::types::{Asset, PriceSource as Source};

/// Aggregated price from all sources
#[derive(Debug, Clone)]
pub struct AggregatedPrice {
    pub ts: i64,
    pub asset: Asset,
    pub mid: f64,
    pub confidence: f64,
    pub sources: Vec<Source>,
    pub spread: f64,
    pub source_prices: HashMap<Source, f64>,
}

/// Price aggregator that combines data from multiple sources
pub struct PriceAggregator {
    /// Buffer of recent ticks per asset per source
    buffers: HashMap<Asset, HashMap<Source, VecDeque<NormalizedTick>>>,
    /// Maximum age of ticks to consider (ms)
    max_age_ms: i64,
    /// Minimum number of sources required for high confidence
    min_sources: usize,
    /// Window size for price averaging
    window_size: usize,
}

impl PriceAggregator {
    pub fn new(max_age_ms: i64, min_sources: usize, window_size: usize) -> Self {
        Self {
            buffers: HashMap::new(),
            max_age_ms,
            min_sources,
            window_size,
        }
    }

    /// Add a tick to the buffer
    pub fn add_tick(&mut self, tick: &NormalizedTick) {
        let asset_buffer = self.buffers.entry(tick.asset).or_insert_with(HashMap::new);

        let source_buffer = asset_buffer
            .entry(tick.source)
            .or_insert_with(VecDeque::new);

        source_buffer.push_back(tick.clone());

        // Trim old ticks
        while source_buffer.len() > self.window_size {
            source_buffer.pop_front();
        }
    }

    /// Get aggregated price for an asset
    pub fn aggregate(&mut self, asset: Asset) -> Option<AggregatedPrice> {
        let now = chrono::Utc::now().timestamp_millis();

        let asset_buffer = self.buffers.get_mut(&asset)?;

        // Collect recent prices from each source
        let mut source_prices: HashMap<Source, Vec<f64>> = HashMap::new();

        for (source, buffer) in asset_buffer.iter_mut() {
            // Remove old ticks
            buffer.retain(|t| now - t.ts < self.max_age_ms);

            // Collect prices
            let prices: Vec<f64> = buffer.iter().map(|t| t.mid).collect();
            if !prices.is_empty() {
                source_prices.insert(*source, prices);
            }
        }

        if source_prices.is_empty() {
            return None;
        }

        // Calculate average price per source
        let avg_prices: HashMap<Source, f64> = source_prices
            .iter()
            .map(|(s, prices)| {
                let avg = prices.iter().sum::<f64>() / prices.len() as f64;
                (*s, avg)
            })
            .collect();

        // Calculate overall mid price (weighted average)
        let sources: Vec<Source> = avg_prices.keys().cloned().collect();
        let overall_mid = avg_prices.values().sum::<f64>() / avg_prices.len() as f64;

        // Calculate spread between sources
        let price_values: Vec<f64> = avg_prices.values().cloned().collect();
        let spread = Self::calculate_spread(&price_values);

        // Calculate confidence based on source agreement
        let confidence = Self::calculate_confidence(&price_values, self.min_sources);

        Some(AggregatedPrice {
            ts: now,
            asset,
            mid: overall_mid,
            confidence,
            sources,
            spread,
            source_prices: avg_prices,
        })
    }

    /// Calculate spread between highest and lowest price
    fn calculate_spread(prices: &[f64]) -> f64 {
        if prices.is_empty() {
            return 0.0;
        }
        let min = prices.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        (max - min) / min
    }

    /// Calculate confidence score based on source agreement
    fn calculate_confidence(prices: &[f64], min_sources: usize) -> f64 {
        if prices.is_empty() {
            return 0.0;
        }

        // Base confidence from number of sources
        let source_factor = (prices.len() as f64 / min_sources as f64).min(1.0);

        // Agreement factor based on spread
        let spread = Self::calculate_spread(prices);
        let agreement_factor = 1.0 - (spread * 10.0).min(1.0); // 0.1% spread = 0.0, 0% spread = 1.0

        // Combined confidence (0.5 to 1.0 range)
        let confidence = 0.5 + 0.5 * source_factor * agreement_factor;
        confidence.clamp(0.5, 1.0)
    }

    /// Get all active sources for an asset
    pub fn active_sources(&self, asset: Asset) -> Vec<Source> {
        self.buffers
            .get(&asset)
            .map(|ab| ab.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Check if we have recent data from minimum required sources
    pub fn has_sufficient_sources(&self, asset: Asset) -> bool {
        self.buffers
            .get(&asset)
            .map(|ab| ab.len() >= self.min_sources)
            .unwrap_or(false)
    }
}

impl Default for PriceAggregator {
    fn default() -> Self {
        Self::new(5000, 2, 10) // 5s max age, 2 sources min, 10 tick window
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tick(asset: Asset, source: Source, mid: f64) -> NormalizedTick {
        NormalizedTick {
            ts: chrono::Utc::now().timestamp_millis(),
            asset,
            bid: mid - 0.5,
            ask: mid + 0.5,
            mid,
            source,
            latency_ms: 10,
        }
    }

    #[test]
    fn test_aggregator_single_source() {
        let mut aggregator = PriceAggregator::new(5000, 2, 10);
        let tick = make_tick(Asset::BTC, Source::Binance, 50000.0);
        aggregator.add_tick(&tick);

        let result = aggregator.aggregate(Asset::BTC);
        assert!(result.is_some());
        let price = result.unwrap();
        assert_eq!(price.mid, 50000.0);
        assert_eq!(price.sources.len(), 1);
    }

    #[test]
    fn test_aggregator_multiple_sources() {
        let mut aggregator = PriceAggregator::new(5000, 2, 10);

        aggregator.add_tick(&make_tick(Asset::BTC, Source::Binance, 50000.0));
        aggregator.add_tick(&make_tick(Asset::BTC, Source::Bybit, 50010.0));
        aggregator.add_tick(&make_tick(Asset::BTC, Source::Coinbase, 49990.0));

        let result = aggregator.aggregate(Asset::BTC);
        assert!(result.is_some());
        let price = result.unwrap();
        assert_eq!(price.sources.len(), 3);
        // Mid should be average of all three
        assert!((price.mid - 50000.0).abs() < 10.0);
    }

    #[test]
    fn test_confidence_increases_with_sources() {
        let mut aggregator = PriceAggregator::new(5000, 2, 10);

        // Single source - lower confidence
        aggregator.add_tick(&make_tick(Asset::ETH, Source::Binance, 3000.0));
        let single = aggregator.aggregate(Asset::ETH).unwrap();

        // Multiple sources - higher confidence
        aggregator.add_tick(&make_tick(Asset::ETH, Source::Bybit, 3000.0));
        let multi = aggregator.aggregate(Asset::ETH).unwrap();

        assert!(multi.confidence >= single.confidence);
    }
}
