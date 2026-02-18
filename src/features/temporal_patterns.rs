//! Temporal Pattern Analysis for Polymarket
//! 
//! Analyzes performance by hour of day, day of week, and seasonal patterns.
//! This is critical for Polymarket prediction markets as certain time periods
//! show significantly different win rates (e.g., 1am-4am UTC racha).

use std::collections::HashMap;
use chrono::{DateTime, Utc, Weekday, Datelike, Timelike};
use serde::{Deserialize, Serialize};
use crate::types::{Asset, Direction, Timeframe};

/// Statistics for a specific hour of day
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HourlyStats {
    pub hour: u8,
    pub total_signals: usize,
    pub wins: usize,
    pub losses: usize,
    pub win_rate: f64,
    pub avg_confidence: f64,
    pub best_direction: Option<String>,
    pub avg_edge: f64,
}

impl HourlyStats {
    pub fn new(hour: u8) -> Self {
        Self {
            hour,
            total_signals: 0,
            wins: 0,
            losses: 0,
            win_rate: 0.5,
            avg_confidence: 0.65,
            best_direction: None,
            avg_edge: 0.0,
        }
    }
    
    pub fn update(&mut self, win: bool, confidence: f64, direction: Direction, edge: f64) {
        self.total_signals += 1;
        if win {
            self.wins += 1;
        } else {
            self.losses += 1;
        }
        
        // Update running average
        let n = self.total_signals as f64;
        self.avg_confidence = (self.avg_confidence * (n - 1.0) + confidence) / n;
        self.avg_edge = (self.avg_edge * (n - 1.0) + edge) / n;
        self.win_rate = self.wins as f64 / self.total_signals as f64;
        
        // Track best direction
        if self.total_signals > 5 {
            // Simple heuristic: count by direction
            self.best_direction = Some(format!("{:?}", direction));
        }
    }
}

/// Statistics for specific day of week
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStats {
    pub weekday: Weekday,
    pub total_signals: usize,
    pub wins: usize,
    pub losses: usize,
    pub win_rate: f64,
    pub best_hour: Option<u8>,
}

impl DailyStats {
    pub fn new(weekday: Weekday) -> Self {
        Self {
            weekday,
            total_signals: 0,
            wins: 0,
            losses: 0,
            win_rate: 0.5,
            best_hour: None,
        }
    }
}

// Note: Weekday doesn't implement Default, so we provide a custom default value
impl DailyStats {
    pub fn default_with_weekday() -> Self {
        Self::new(Weekday::Mon)
    }
}

/// Temporal pattern analyzer for performance optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalPatternAnalyzer {
    /// Stats per hour (0-23) per asset per timeframe
    hourly_stats: HashMap<(Asset, Timeframe, u8), HourlyStats>,
    /// Stats per day of week per asset per timeframe
    daily_stats: HashMap<(Asset, Timeframe, Weekday), DailyStats>,
    /// Minimum samples before trusting stats
    min_samples: usize,
    /// Cache for recent adjustments
    last_adjustment: Option<(DateTime<Utc>, f64, String)>,
}

impl TemporalPatternAnalyzer {
    pub fn new(min_samples: usize) -> Self {
        let mut analyzer = Self {
            hourly_stats: HashMap::new(),
            daily_stats: HashMap::new(),
            min_samples,
            last_adjustment: None,
        };
        
        // Initialize with default stats
        analyzer.initialize_defaults();
        analyzer
    }
    
    fn initialize_defaults(&mut self) {
        // Initialize all combinations
        for asset in [Asset::BTC, Asset::ETH] {
            for tf in [Timeframe::Min15, Timeframe::Hour1] {
                for hour in 0..24 {
                    self.hourly_stats.insert((asset, tf, hour), HourlyStats::new(hour));
                }
                for weekday in [Weekday::Mon, Weekday::Tue, Weekday::Wed, 
                               Weekday::Thu, Weekday::Fri, Weekday::Sat, Weekday::Sun] {
                    self.daily_stats.insert((asset, tf, weekday), DailyStats::new(weekday));
                }
            }
        }
    }
    
    /// Record trade result for temporal analysis
    pub fn record_trade(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
        timestamp: DateTime<Utc>,
        win: bool,
        confidence: f64,
        direction: Direction,
        edge: f64,
    ) {
        let hour = timestamp.hour() as u8;
        let weekday = timestamp.weekday();
        
        // Update hourly stats
        let key = (asset, timeframe, hour);
        if let Some(stats) = self.hourly_stats.get_mut(&key) {
            stats.update(win, confidence, direction, edge);
        }
        
        // Update daily stats
        let day_key = (asset, timeframe, weekday);
        if let Some(stats) = self.daily_stats.get_mut(&day_key) {
            stats.total_signals += 1;
            if win {
                stats.wins += 1;
            } else {
                stats.losses += 1;
            }
            stats.win_rate = stats.wins as f64 / stats.total_signals as f64;
        }
    }
    
    /// Get confidence adjustment based on temporal patterns
    pub fn get_temporal_adjustment(
        &self,
        asset: Asset,
        timeframe: Timeframe,
        direction: Direction,
        timestamp: DateTime<Utc>,
    ) -> (f64, String) {
        let hour = timestamp.hour() as u8;
        let weekday = timestamp.weekday();
        
        let mut adjustment = 1.0;
        let mut reasons = Vec::new();
        
        // Hour-based adjustment
        let hour_key = (asset, timeframe, hour);
        if let Some(hour_stats) = self.hourly_stats.get(&hour_key) {
            if hour_stats.total_signals >= self.min_samples {
                if hour_stats.win_rate > 0.55 {
                    // This hour is good
                    let boost = ((hour_stats.win_rate - 0.50) * 0.4).min(0.12);
                    
                    // Check if direction matches best direction for this hour
                    if let Some(ref best_dir) = hour_stats.best_direction {
                        if best_dir == &format!("{:?}", direction) {
                            adjustment *= 1.0 + boost;
                            reasons.push(format!("Hour {}: strong WR {:.0}%", 
                                hour, hour_stats.win_rate * 100.0));
                        } else {
                            adjustment *= 1.0 - (boost * 0.5);
                            reasons.push(format!("Hour {}: direction mismatch", hour));
                        }
                    } else {
                        adjustment *= 1.0 + boost;
                        reasons.push(format!("Hour {}: positive WR", hour));
                    }
                } else if hour_stats.win_rate < 0.40 && hour_stats.total_signals > 10 {
                    // This hour is bad - avoid trading
                    adjustment *= 0.3;  // Strong penalty
                    reasons.push(format!("Hour {}: poor WR {:.0}% - avoid", 
                        hour, hour_stats.win_rate * 100.0));
                }
            }
        }
        
        // Day-based adjustment
        let day_key = (asset, timeframe, weekday);
        if let Some(day_stats) = self.daily_stats.get(&day_key) {
            if day_stats.total_signals >= self.min_samples {
                if day_stats.win_rate > 0.55 {
                    let boost = ((day_stats.win_rate - 0.50) * 0.3).min(0.08);
                    adjustment *= 1.0 + boost;
                    reasons.push(format!("{:?}: positive", weekday));
                } else if day_stats.win_rate < 0.42 {
                    adjustment *= 0.85;
                    reasons.push(format!("{:?}: weak", weekday));
                }
            }
        }
        
        // Special patterns for crypto markets
        // 1am-4am UTC (early morning Asia) - historically good for the user
        if (1..=4).contains(&hour) {
            if self.get_hour_sample_count(asset, timeframe, hour) < 5 {
                // Boost initial exploration during known good hours
                adjustment *= 1.08;
                reasons.push("Early AM UTC: exploration boost".to_string());
            }
        }
        
        // 16:00-17:00 UTC (UK tea time) - peak volatility, be careful
        if hour == 16 || hour == 17 {
            adjustment *= 0.92;
            reasons.push("Peak volatility period".to_string());
        }
        
        let reason_str = if reasons.is_empty() {
            "No temporal adjustment".to_string()
        } else {
            reasons.join(", ")
        };
        
        (adjustment, reason_str)
    }
    
    /// Check if we should block trading at this time
    pub fn should_block_trading(
        &self,
        asset: Asset,
        timeframe: Timeframe,
        timestamp: DateTime<Utc>,
    ) -> (bool, String) {
        let hour = timestamp.hour() as u8;
        let key = (asset, timeframe, hour);
        
        if let Some(stats) = self.hourly_stats.get(&key) {
            // Block if win rate is very low and we have enough samples
            if stats.total_signals >= 15 && stats.win_rate < 0.35 {
                return (true, format!("Hour {} has {:.0}% WR - blocking", 
                    hour, stats.win_rate * 100.0));
            }
        }
        
        (false, "".to_string())
    }
    
    /// Get best hours for trading
    pub fn get_best_hours(&self, asset: Asset, timeframe: Timeframe) -> Vec<(u8, f64)> {
        let mut hours: Vec<(u8, f64)> = (0..24)
            .filter_map(|h| {
                let key = (asset, timeframe, h);
                self.hourly_stats.get(&key)
                    .filter(|s| s.total_signals >= self.min_samples)
                    .map(|s| (h, s.win_rate))
            })
            .collect();
        
        hours.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        hours.into_iter().take(5).collect()
    }
    
    /// Get statistics summary for dashboard
    pub fn get_stats_summary(&self, asset: Asset, timeframe: Timeframe) -> TemporalStatsSummary {
        let hourly: Vec<_> = (0..24)
            .map(|h| {
                let key = (asset, timeframe, h);
                self.hourly_stats.get(&key).cloned().unwrap_or_else(|| HourlyStats::new(h))
            })
            .collect();
        
        let best_hours = self.get_best_hours(asset, timeframe);
        
        // Calculate worst hours before moving hourly
        let worst_hour = hourly.iter()
            .filter(|h| h.total_signals >= self.min_samples)
            .min_by(|a, b| a.win_rate.partial_cmp(&b.win_rate).unwrap());
        
        let worst_hours: Vec<_> = worst_hour
            .map(|h| vec![(h.hour, h.win_rate)])
            .unwrap_or_default();
        
        let total_trades: usize = hourly.iter().map(|h| h.total_signals).sum();
        
        TemporalStatsSummary {
            asset,
            timeframe,
            hourly_stats: hourly,
            best_hours,
            worst_hours,
            total_trades_analyzed: total_trades,
        }
    }
    
    fn get_hour_sample_count(&self, asset: Asset, timeframe: Timeframe, hour: u8) -> usize {
        let key = (asset, timeframe, hour);
        self.hourly_stats.get(&key).map(|s| s.total_signals).unwrap_or(0)
    }
    
    /// Load historical data from endpoint
    pub async fn load_historical_patterns(&mut self, endpoint_url: &str) -> anyhow::Result<()> {
        // TODO: Implement fetch from /api/data/temporal-patterns
        // For now, this will be populated from live trading
        Ok(())
    }
    
    /// Export stats for persistence
    pub fn export_stats(&self) -> TemporalExport {
        TemporalExport {
            hourly_stats: self.hourly_stats.clone(),
            daily_stats: self.daily_stats.clone(),
            exported_at: Utc::now(),
        }
    }
}

/// Summary for dashboard display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalStatsSummary {
    pub asset: Asset,
    pub timeframe: Timeframe,
    pub hourly_stats: Vec<HourlyStats>,
    pub best_hours: Vec<(u8, f64)>,
    pub worst_hours: Vec<(u8, f64)>,
    pub total_trades_analyzed: usize,
}

/// Export structure for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalExport {
    pub hourly_stats: HashMap<(Asset, Timeframe, u8), HourlyStats>,
    pub daily_stats: HashMap<(Asset, Timeframe, Weekday), DailyStats>,
    pub exported_at: DateTime<Utc>,
}

impl Default for TemporalPatternAnalyzer {
    fn default() -> Self {
        Self::new(5)  // Min 5 samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hourly_stats_update() {
        let mut stats = HourlyStats::new(2);
        
        // Simulate 5 wins
        for _ in 0..5 {
            stats.update(true, 0.7, Direction::Up, 0.05);
        }
        
        assert_eq!(stats.total_signals, 5);
        assert_eq!(stats.wins, 5);
        assert_eq!(stats.win_rate, 1.0);
        assert!(stats.avg_confidence > 0.65);
    }
    
    #[test]
    fn test_temporal_adjustment() {
        let mut analyzer = TemporalPatternAnalyzer::new(3);
        let now = Utc::now();
        
        // Record some wins at hour 2
        for _ in 0..5 {
            analyzer.record_trade(
                Asset::BTC,
                Timeframe::Min15,
                now.with_hour(2).unwrap(),
                true,
                0.72,
                Direction::Up,
                0.08,
            );
        }
        
        // Check adjustment
        let (adj, reason) = analyzer.get_temporal_adjustment(
            Asset::BTC,
            Timeframe::Min15,
            Direction::Up,
            now.with_hour(2).unwrap(),
        );
        
        assert!(adj > 1.0, "Should boost good hours");
        assert!(!reason.is_empty());
    }
}
