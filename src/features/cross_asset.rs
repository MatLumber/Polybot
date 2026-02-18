//! Cross-Asset Correlation Analysis for Polymarket
//!
//! Analyzes correlations between BTC and ETH, and across timeframes (15m vs 1h).
//! This provides additional signals when assets diverge or when timeframes align.

use crate::types::{Asset, Direction, Timeframe};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Cross-asset signal types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrossAssetSignalType {
    /// BTC and ETH moving together (high correlation)
    CorrelatedMovement,
    /// BTC and ETH diverging (potential mean reversion)
    Divergence,
    /// BTC leading ETH (BTC moves first)
    BTCDominant,
    /// ETH leading BTC (ETH moves first)
    ETHDominant,
    /// 15m and 1h timeframes aligned
    TimeframeConfluence,
    /// 15m and 1h timeframes conflicting
    TimeframeConflict,
}

/// Cross-asset analysis signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossAssetSignal {
    pub signal_type: CrossAssetSignalType,
    pub primary_asset: Asset,
    pub secondary_asset: Asset,
    pub primary_direction: Direction,
    pub secondary_direction: Direction,
    pub correlation: f64,
    pub divergence_magnitude: f64,
    pub confidence: f64,
    pub description: String,
}

/// Price data point for correlation calculation
#[derive(Debug, Clone, Copy)]
struct PricePoint {
    price: f64,
    timestamp: DateTime<Utc>,
    returns: f64,
}

/// Cross-asset analyzer
#[derive(Debug, Clone)]
pub struct CrossAssetAnalyzer {
    /// Price history per asset per timeframe
    btc_15m_history: VecDeque<PricePoint>,
    btc_1h_history: VecDeque<PricePoint>,
    eth_15m_history: VecDeque<PricePoint>,
    eth_1h_history: VecDeque<PricePoint>,
    /// Rolling window size for correlation
    window_size: usize,
    /// Maximum age of data points
    max_age_secs: i64,
    /// Correlation threshold for significance
    correlation_threshold: f64,
}

impl CrossAssetAnalyzer {
    pub fn new(window_size: usize) -> Self {
        Self {
            btc_15m_history: VecDeque::with_capacity(window_size),
            btc_1h_history: VecDeque::with_capacity(window_size),
            eth_15m_history: VecDeque::with_capacity(window_size),
            eth_1h_history: VecDeque::with_capacity(window_size),
            window_size,
            max_age_secs: 3600, // 1 hour
            correlation_threshold: 0.7,
        }
    }

    /// Update price for an asset/timeframe
    pub fn update_price(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        price: f64,
        timestamp: DateTime<Utc>,
    ) {
        let history = match (asset, timeframe) {
            (Asset::BTC, Timeframe::Min15) => &mut self.btc_15m_history,
            (Asset::BTC, Timeframe::Hour1) => &mut self.btc_1h_history,
            (Asset::ETH, Timeframe::Min15) => &mut self.eth_15m_history,
            (Asset::ETH, Timeframe::Hour1) => &mut self.eth_1h_history,
            // For SOL and XRP, we don't track correlation yet - return early
            (Asset::SOL, _) | (Asset::XRP, _) => return,
        };

        // Calculate return if we have previous price
        let returns = if let Some(prev) = history.back() {
            (price - prev.price) / prev.price
        } else {
            0.0
        };

        history.push_back(PricePoint {
            price,
            timestamp,
            returns,
        });

        // Trim old data
        let cutoff = timestamp - Duration::seconds(self.max_age_secs);
        while let Some(point) = history.front() {
            if point.timestamp < cutoff {
                history.pop_front();
            } else {
                break;
            }
        }

        // Keep only window_size most recent
        while history.len() > self.window_size {
            history.pop_front();
        }
    }

    /// Calculate correlation between two return series
    fn calculate_correlation(&self, series1: &[PricePoint], series2: &[PricePoint]) -> f64 {
        if series1.len() < 5 || series2.len() < 5 {
            return 0.0;
        }

        // Use the minimum length
        let len = series1.len().min(series2.len());
        let s1 = &series1[series1.len() - len..];
        let s2 = &series2[series2.len() - len..];

        let mean1: f64 = s1.iter().map(|p| p.returns).sum::<f64>() / len as f64;
        let mean2: f64 = s2.iter().map(|p| p.returns).sum::<f64>() / len as f64;

        let mut numerator = 0.0;
        let mut denom1 = 0.0;
        let mut denom2 = 0.0;

        for i in 0..len {
            let diff1 = s1[i].returns - mean1;
            let diff2 = s2[i].returns - mean2;
            numerator += diff1 * diff2;
            denom1 += diff1 * diff1;
            denom2 += diff2 * diff2;
        }

        let denominator = (denom1 * denom2).sqrt();
        if denominator > 0.0 {
            numerator / denominator
        } else {
            0.0
        }
    }

    /// Detect divergence between BTC and ETH
    pub fn detect_divergence(&self, timeframe: Timeframe) -> Option<CrossAssetSignal> {
        let (btc_hist, eth_hist) = match timeframe {
            Timeframe::Min15 => (&self.btc_15m_history, &self.eth_15m_history),
            Timeframe::Hour1 => (&self.btc_1h_history, &self.eth_1h_history),
        };

        if btc_hist.len() < 5 || eth_hist.len() < 5 {
            return None;
        }

        let correlation = self.calculate_correlation(
            &btc_hist.iter().cloned().collect::<Vec<_>>(),
            &eth_hist.iter().cloned().collect::<Vec<_>>(),
        );

        // Get recent returns
        let btc_return = btc_hist
            .iter()
            .rev()
            .take(3)
            .map(|p| p.returns)
            .sum::<f64>();
        let eth_return = eth_hist
            .iter()
            .rev()
            .take(3)
            .map(|p| p.returns)
            .sum::<f64>();

        let btc_dir = if btc_return > 0.0 {
            Direction::Up
        } else {
            Direction::Down
        };
        let eth_dir = if eth_return > 0.0 {
            Direction::Up
        } else {
            Direction::Down
        };

        // Calculate divergence magnitude
        let divergence = (btc_return - eth_return).abs();

        if correlation.abs() > self.correlation_threshold {
            // High correlation - assets moving together
            if btc_dir == eth_dir {
                Some(CrossAssetSignal {
                    signal_type: CrossAssetSignalType::CorrelatedMovement,
                    primary_asset: Asset::BTC,
                    secondary_asset: Asset::ETH,
                    primary_direction: btc_dir,
                    secondary_direction: eth_dir,
                    correlation,
                    divergence_magnitude: divergence,
                    confidence: correlation.abs(),
                    description: format!("BTC-ETH correlated: {:.1}%", correlation * 100.0),
                })
            } else {
                // High correlation but opposite directions = anomaly
                Some(CrossAssetSignal {
                    signal_type: CrossAssetSignalType::Divergence,
                    primary_asset: Asset::BTC,
                    secondary_asset: Asset::ETH,
                    primary_direction: btc_dir,
                    secondary_direction: eth_dir,
                    correlation,
                    divergence_magnitude: divergence,
                    confidence: 0.8,
                    description: "BTC-ETH divergence anomaly".to_string(),
                })
            }
        } else if divergence > 0.005 {
            // 0.5% divergence
            // Low correlation with significant divergence
            // Determine which asset is leading
            let (signal_type, primary, secondary, primary_dir, secondary_dir) =
                if btc_return.abs() > eth_return.abs() {
                    (
                        CrossAssetSignalType::BTCDominant,
                        Asset::BTC,
                        Asset::ETH,
                        btc_dir,
                        eth_dir,
                    )
                } else {
                    (
                        CrossAssetSignalType::ETHDominant,
                        Asset::ETH,
                        Asset::BTC,
                        eth_dir,
                        btc_dir,
                    )
                };

            Some(CrossAssetSignal {
                signal_type,
                primary_asset: primary,
                secondary_asset: secondary,
                primary_direction: primary_dir,
                secondary_direction: secondary_dir,
                correlation,
                divergence_magnitude: divergence,
                confidence: 0.7,
                description: format!(
                    "{:?} leading divergence: {:.2}%",
                    primary,
                    divergence * 100.0
                ),
            })
        } else {
            None
        }
    }

    /// Check timeframe alignment (15m vs 1h) for an asset
    pub fn check_timeframe_confluence(&self, asset: Asset) -> Option<CrossAssetSignal> {
        let (hist_15m, hist_1h) = match asset {
            Asset::BTC => (&self.btc_15m_history, &self.btc_1h_history),
            Asset::ETH => (&self.eth_15m_history, &self.eth_1h_history),
            // SOL and XRP not yet supported for timeframe confluence
            Asset::SOL | Asset::XRP => return None,
        };

        if hist_15m.len() < 5 || hist_1h.len() < 5 {
            return None;
        }

        // Calculate recent returns
        let ret_15m = hist_15m
            .iter()
            .rev()
            .take(3)
            .map(|p| p.returns)
            .sum::<f64>();
        let ret_1h = hist_1h.iter().rev().take(3).map(|p| p.returns).sum::<f64>();

        let dir_15m = if ret_15m > 0.0 {
            Direction::Up
        } else {
            Direction::Down
        };
        let dir_1h = if ret_1h > 0.0 {
            Direction::Up
        } else {
            Direction::Down
        };

        if dir_15m == dir_1h {
            // Alignment - strong signal
            let correlation = self.calculate_correlation(
                &hist_15m.iter().cloned().collect::<Vec<_>>(),
                &hist_1h.iter().cloned().collect::<Vec<_>>(),
            );

            Some(CrossAssetSignal {
                signal_type: CrossAssetSignalType::TimeframeConfluence,
                primary_asset: asset,
                secondary_asset: asset,
                primary_direction: dir_15m,
                secondary_direction: dir_1h,
                correlation,
                divergence_magnitude: (ret_15m - ret_1h).abs(),
                confidence: 0.85,
                description: format!("{:?} timeframe alignment: {:?}", asset, dir_15m),
            })
        } else {
            // Conflict - weak signal or reversal warning
            Some(CrossAssetSignal {
                signal_type: CrossAssetSignalType::TimeframeConflict,
                primary_asset: asset,
                secondary_asset: asset,
                primary_direction: dir_15m,
                secondary_direction: dir_1h,
                correlation: 0.0,
                divergence_magnitude: (ret_15m - ret_1h).abs(),
                confidence: 0.6,
                description: format!(
                    "{:?} timeframe conflict: 15m={:?}, 1h={:?}",
                    asset, dir_15m, dir_1h
                ),
            })
        }
    }

    /// Get all active cross-asset signals
    pub fn get_all_signals(&self) -> Vec<CrossAssetSignal> {
        let mut signals = Vec::new();

        // Check BTC-ETH for both timeframes
        for tf in [Timeframe::Min15, Timeframe::Hour1] {
            if let Some(signal) = self.detect_divergence(tf) {
                signals.push(signal);
            }
        }

        // Check timeframe confluence for both assets
        for asset in [Asset::BTC, Asset::ETH] {
            if let Some(signal) = self.check_timeframe_confluence(asset) {
                signals.push(signal);
            }
        }

        signals
    }

    /// Get correlation matrix for dashboard
    pub fn get_correlation_matrix(&self) -> CorrelationMatrix {
        let btc_eth_15m = self.calculate_correlation(
            &self.btc_15m_history.iter().cloned().collect::<Vec<_>>(),
            &self.eth_15m_history.iter().cloned().collect::<Vec<_>>(),
        );

        let btc_eth_1h = self.calculate_correlation(
            &self.btc_1h_history.iter().cloned().collect::<Vec<_>>(),
            &self.eth_1h_history.iter().cloned().collect::<Vec<_>>(),
        );

        let btc_tf = self.calculate_correlation(
            &self.btc_15m_history.iter().cloned().collect::<Vec<_>>(),
            &self.btc_1h_history.iter().cloned().collect::<Vec<_>>(),
        );

        let eth_tf = self.calculate_correlation(
            &self.eth_15m_history.iter().cloned().collect::<Vec<_>>(),
            &self.eth_1h_history.iter().cloned().collect::<Vec<_>>(),
        );

        CorrelationMatrix {
            btc_eth_15m,
            btc_eth_1h,
            btc_timeframe: btc_tf,
            eth_timeframe: eth_tf,
            timestamp: Utc::now(),
        }
    }

    /// Get current momentum comparison
    pub fn get_momentum_comparison(&self) -> MomentumComparison {
        let btc_15m_mom = self
            .btc_15m_history
            .iter()
            .rev()
            .take(5)
            .map(|p| p.returns)
            .sum::<f64>();
        let eth_15m_mom = self
            .eth_15m_history
            .iter()
            .rev()
            .take(5)
            .map(|p| p.returns)
            .sum::<f64>();

        let btc_1h_mom = self
            .btc_1h_history
            .iter()
            .rev()
            .take(5)
            .map(|p| p.returns)
            .sum::<f64>();
        let eth_1h_mom = self
            .eth_1h_history
            .iter()
            .rev()
            .take(5)
            .map(|p| p.returns)
            .sum::<f64>();

        MomentumComparison {
            btc_15m_momentum: btc_15m_mom,
            eth_15m_momentum: eth_15m_mom,
            btc_1h_momentum: btc_1h_mom,
            eth_1h_momentum: eth_1h_mom,
            btc_leading_15m: btc_15m_mom > eth_15m_mom,
            btc_leading_1h: btc_1h_mom > eth_1h_mom,
            timestamp: Utc::now(),
        }
    }
}

/// Correlation matrix for dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationMatrix {
    pub btc_eth_15m: f64,
    pub btc_eth_1h: f64,
    pub btc_timeframe: f64,
    pub eth_timeframe: f64,
    pub timestamp: DateTime<Utc>,
}

/// Momentum comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MomentumComparison {
    pub btc_15m_momentum: f64,
    pub eth_15m_momentum: f64,
    pub btc_1h_momentum: f64,
    pub eth_1h_momentum: f64,
    pub btc_leading_15m: bool,
    pub btc_leading_1h: bool,
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correlation_calculation() {
        let mut analyzer = CrossAssetAnalyzer::new(20);
        let now = Utc::now();

        // Add correlated prices (both moving up)
        for i in 0..10 {
            let price = 100.0 + i as f64;
            analyzer.update_price(
                Asset::BTC,
                Timeframe::Min15,
                price,
                now + Duration::minutes(i),
            );
            analyzer.update_price(
                Asset::ETH,
                Timeframe::Min15,
                price * 0.9,
                now + Duration::minutes(i),
            );
        }

        let correlation = analyzer.calculate_correlation(
            &analyzer.btc_15m_history.iter().cloned().collect::<Vec<_>>(),
            &analyzer.eth_15m_history.iter().cloned().collect::<Vec<_>>(),
        );

        assert!(
            correlation > 0.9,
            "Correlation should be high for similar movements"
        );
    }

    #[test]
    fn test_divergence_detection() {
        let mut analyzer = CrossAssetAnalyzer::new(20);
        let now = Utc::now();

        // BTC going up, ETH going down
        for i in 0..10 {
            analyzer.update_price(
                Asset::BTC,
                Timeframe::Min15,
                100.0 + i as f64,
                now + Duration::minutes(i),
            );
            analyzer.update_price(
                Asset::ETH,
                Timeframe::Min15,
                90.0 - i as f64,
                now + Duration::minutes(i),
            );
        }

        let signal = analyzer.detect_divergence(Timeframe::Min15);
        assert!(signal.is_some());

        let sig = signal.unwrap();
        assert_eq!(sig.signal_type, CrossAssetSignalType::Divergence);
    }
}
