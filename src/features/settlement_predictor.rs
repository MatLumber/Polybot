//! Settlement Price Predictor for Polymarket
//! 
//! Predicts the Chainlink oracle settlement price for binary prediction markets.
//! This is critical because Polymarket resolves based on the Chainlink price at
//! a specific timestamp, not the current market price.

use std::collections::VecDeque;
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use crate::types::{Asset, Direction, Timeframe};

/// Prediction for settlement price
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementPrediction {
    /// Predicted price at settlement
    pub predicted_price: f64,
    /// Confidence in prediction (0.0 to 1.0)
    pub confidence: f64,
    /// Expected direction based on prediction
    pub direction_bias: Direction,
    /// Expected price movement percentage
    pub expected_movement_pct: f64,
    /// Time to expiry in seconds
    pub time_to_expiry_secs: i64,
    /// Drift per minute
    pub drift_per_min: f64,
}

/// Historical settlement data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementDataPoint {
    pub timestamp: DateTime<Utc>,
    pub price_start: f64,
    pub price_end: f64,
    pub time_to_expiry_secs: i64,
    pub drift_per_min: f64,
    pub volatility: f64,
}

/// Price velocity tracker
#[derive(Debug, Clone, Default)]
pub struct PriceVelocityTracker {
    /// Recent price changes (price, timestamp)
    price_history: VecDeque<(f64, DateTime<Utc>)>,
    /// Window for velocity calculation (seconds)
    window_secs: i64,
}

impl PriceVelocityTracker {
    pub fn new(window_secs: i64) -> Self {
        Self {
            price_history: VecDeque::with_capacity(100),
            window_secs,
        }
    }
    
    pub fn add_price(&mut self, price: f64, timestamp: DateTime<Utc>) {
        self.price_history.push_back((price, timestamp));
        
        // Remove old data outside window
        let cutoff = timestamp - Duration::seconds(self.window_secs);
        while let Some((_, ts)) = self.price_history.front() {
            if *ts < cutoff {
                self.price_history.pop_front();
            } else {
                break;
            }
        }
    }
    
    /// Calculate current velocity (price change per minute)
    pub fn calculate_velocity(&self) -> f64 {
        if self.price_history.len() < 2 {
            return 0.0;
        }
        
        let first = self.price_history.front().unwrap();
        let last = self.price_history.back().unwrap();
        
        let price_change = (last.0 - first.0) / first.0;
        let time_diff_mins = (last.1 - first.1).num_seconds() as f64 / 60.0;
        
        if time_diff_mins > 0.0 {
            price_change / time_diff_mins
        } else {
            0.0
        }
    }
    
    /// Calculate acceleration (change in velocity)
    pub fn calculate_acceleration(&self) -> f64 {
        if self.price_history.len() < 10 {
            return 0.0;
        }
        
        let mid = self.price_history.len() / 2;
        let first_half: Vec<_> = self.price_history.iter().take(mid).cloned().collect();
        let second_half: Vec<_> = self.price_history.iter().skip(mid).cloned().collect();
        
        // Simple velocity comparison
        if first_half.len() >= 2 && second_half.len() >= 2 {
            let v1 = (first_half.last().unwrap().0 - first_half.first().unwrap().0) 
                     / first_half.first().unwrap().0;
            let v2 = (second_half.last().unwrap().0 - second_half.first().unwrap().0) 
                     / second_half.first().unwrap().0;
            
            let t1 = (first_half.last().unwrap().1 - first_half.first().unwrap().1).num_seconds() as f64 / 60.0;
            let t2 = (second_half.last().unwrap().1 - second_half.first().unwrap().1).num_seconds() as f64 / 60.0;
            
            if t1 > 0.0 && t2 > 0.0 {
                (v2/t2 - v1/t1)
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
}

/// Settlement predictor for Polymarket
#[derive(Debug, Clone)]
pub struct SettlementPricePredictor {
    /// Historical settlement data for learning
    settlement_history: Vec<SettlementDataPoint>,
    /// Price velocity trackers per asset
    velocity_trackers: std::collections::HashMap<Asset, PriceVelocityTracker>,
    /// Maximum history size
    max_history: usize,
    /// Timeframe-specific parameters
    params: TimeframeParams,
}

#[derive(Debug, Clone)]
struct TimeframeParams {
    /// Minutes to look back for velocity (15m markets)
    velocity_window_15m: i64,
    /// Minutes to look back for velocity (1h markets)
    velocity_window_1h: i64,
    /// Drift decay factor (markets tend to mean-revert near expiry)
    decay_factor: f64,
}

impl Default for TimeframeParams {
    fn default() -> Self {
        Self {
            velocity_window_15m: 10,  // Last 10 mins for 15m markets
            velocity_window_1h: 30,   // Last 30 mins for 1h markets
            decay_factor: 0.8,        // Drift decays 20% near expiry
        }
    }
}

impl SettlementPricePredictor {
    pub fn new(max_history: usize) -> Self {
        let mut velocity_trackers = std::collections::HashMap::new();
        for asset in [Asset::BTC, Asset::ETH] {
            velocity_trackers.insert(asset, PriceVelocityTracker::new(300));  // 5 min default
        }
        
        Self {
            settlement_history: Vec::with_capacity(max_history),
            velocity_trackers,
            max_history,
            params: TimeframeParams::default(),
        }
    }
    
    /// Update with new price tick
    pub fn update_price(&mut self, asset: Asset, price: f64, timestamp: DateTime<Utc>, timeframe: Timeframe) {
        let window = match timeframe {
            Timeframe::Min15 => self.params.velocity_window_15m * 60,
            Timeframe::Hour1 => self.params.velocity_window_1h * 60,
        };
        
        if let Some(tracker) = self.velocity_trackers.get_mut(&asset) {
            tracker.window_secs = window;
            tracker.add_price(price, timestamp);
        }
    }
    
    /// Predict settlement price
    pub fn predict_settlement(
        &self,
        asset: Asset,
        current_price: f64,
        timeframe: Timeframe,
        expiry_timestamp: i64,
        orderbook_pressure: f64,
    ) -> SettlementPrediction {
        let now = Utc::now().timestamp_millis();
        let time_to_expiry_secs = ((expiry_timestamp - now) / 1000).max(0);
        let time_to_expiry_mins = time_to_expiry_secs as f64 / 60.0;
        
        // Get velocity
        let velocity = self.velocity_trackers
            .get(&asset)
            .map(|t| t.calculate_velocity())
            .unwrap_or(0.0);
        
        // Get acceleration
        let acceleration = self.velocity_trackers
            .get(&asset)
            .map(|t| t.calculate_acceleration())
            .unwrap_or(0.0);
        
        // Calculate expected drift
        // Formula: Drift = (velocity * time_remaining) + (0.5 * acceleration * time_remaining^2)
        let base_drift = velocity * time_to_expiry_mins;
        let accel_component = 0.5 * acceleration * time_to_expiry_mins.powi(2);
        
        // Apply decay factor (markets tend to stabilize near expiry)
        let decay = if time_to_expiry_mins < 5.0 {
            self.params.decay_factor * (time_to_expiry_mins / 5.0)
        } else {
            self.params.decay_factor
        };
        
        let total_drift = (base_drift + accel_component) * decay;
        
        // Apply orderbook pressure adjustment
        // Strong buying pressure suggests continuation
        let pressure_adjustment = orderbook_pressure * 0.0005;  // 0.05% per unit of pressure
        
        // Final predicted price
        let predicted_price = current_price * (1.0 + total_drift + pressure_adjustment);
        
        // Calculate confidence based on:
        // 1. Time to expiry (less time = more confident)
        // 2. Velocity stability
        // 3. Historical accuracy
        let time_confidence = if time_to_expiry_mins < 10.0 {
            0.95
        } else if time_to_expiry_mins < 30.0 {
            0.85
        } else {
            0.75 - (time_to_expiry_mins - 30.0) * 0.005  // Decay with more time
        };
        
        let velocity_stability = if velocity.abs() > 0.001 {
            0.8  // High velocity = less certain
        } else {
            0.9  // Low velocity = more certain
        };
        
        let historical_accuracy = self.calculate_historical_accuracy(asset, timeframe);
        
        let confidence = (time_confidence * 0.4 + velocity_stability * 0.3 + historical_accuracy * 0.3)
            .clamp(0.5, 0.98);
        
        // Determine direction bias
        let direction_bias = if total_drift + pressure_adjustment > 0.0 {
            Direction::Up
        } else {
            Direction::Down
        };
        
        SettlementPrediction {
            predicted_price,
            confidence,
            direction_bias,
            expected_movement_pct: total_drift * 100.0,
            time_to_expiry_secs,
            drift_per_min: velocity,
        }
    }
    
    /// Calculate edge vs market-implied probability
    pub fn calculate_settlement_edge(
        &self,
        prediction: &SettlementPrediction,
        market_mid_price: f64,  // In Polymarket, this is the implied probability
        strike_price: f64,      // Price threshold for UP/DOWN
    ) -> SettlementEdge {
        // Determine if predicted price is above or below strike
        let predicted_up = prediction.predicted_price > strike_price;
        
        // Market-implied probability
        let market_prob = market_mid_price;
        
        // Our predicted probability based on confidence and direction
        let our_prob = if predicted_up {
            prediction.confidence
        } else {
            1.0 - prediction.confidence
        };
        
        // Edge = difference in probabilities
        let edge = our_prob - market_prob;
        
        SettlementEdge {
            market_prob,
            predicted_prob: our_prob,
            edge,
            predicted_direction: if predicted_up { Direction::Up } else { Direction::Down },
            confidence: prediction.confidence,
        }
    }
    
    /// Record actual settlement for learning
    pub fn record_settlement(
        &mut self,
        asset: Asset,
        price_start: f64,
        price_end: f64,
        time_to_expiry_secs: i64,
        timestamp: DateTime<Utc>,
    ) {
        let drift_per_min = if time_to_expiry_secs > 0 {
            let price_change = (price_end - price_start) / price_start;
            let mins = time_to_expiry_secs as f64 / 60.0;
            price_change / mins
        } else {
            0.0
        };
        
        let point = SettlementDataPoint {
            timestamp,
            price_start,
            price_end,
            time_to_expiry_secs,
            drift_per_min,
            volatility: (price_end - price_start).abs() / price_start,
        };
        
        self.settlement_history.push(point);
        
        if self.settlement_history.len() > self.max_history {
            self.settlement_history.remove(0);
        }
    }
    
    /// Calculate historical accuracy of predictions
    fn calculate_historical_accuracy(&self, asset: Asset, timeframe: Timeframe) -> f64 {
        // TODO: Filter by asset and timeframe
        if self.settlement_history.len() < 10 {
            return 0.7;  // Default conservative value
        }
        
        // Simple metric: how often did drift direction match outcome
        let correct_predictions = self.settlement_history
            .iter()
            .filter(|p| {
                let drift_direction = p.drift_per_min > 0.0;
                let price_direction = p.price_end > p.price_start;
                drift_direction == price_direction
            })
            .count();
        
        correct_predictions as f64 / self.settlement_history.len() as f64
    }
    
    /// Get prediction quality metrics
    pub fn get_metrics(&self) -> SettlementMetrics {
        let total = self.settlement_history.len();
        let avg_volatility = if total > 0 {
            self.settlement_history.iter().map(|p| p.volatility).sum::<f64>() / total as f64
        } else {
            0.0
        };
        
        let avg_drift = if total > 0 {
            self.settlement_history.iter().map(|p| p.drift_per_min.abs()).sum::<f64>() / total as f64
        } else {
            0.0
        };
        
        SettlementMetrics {
            total_settlements_recorded: total,
            avg_settlement_volatility: avg_volatility,
            avg_price_drift_per_min: avg_drift,
            historical_accuracy: self.calculate_historical_accuracy(Asset::BTC, Timeframe::Min15),
        }
    }
    
    /// Load historical settlement data from API
    pub async fn load_historical_settlements(&mut self, _endpoint: &str) -> anyhow::Result<()> {
        // TODO: Fetch from /prices-history endpoint
        Ok(())
    }
}

/// Settlement edge calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementEdge {
    pub market_prob: f64,
    pub predicted_prob: f64,
    pub edge: f64,
    pub predicted_direction: Direction,
    pub confidence: f64,
}

/// Metrics for dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementMetrics {
    pub total_settlements_recorded: usize,
    pub avg_settlement_volatility: f64,
    pub avg_price_drift_per_min: f64,
    pub historical_accuracy: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_velocity_calculation() {
        let mut tracker = PriceVelocityTracker::new(60);
        let now = Utc::now();
        
        // Add prices increasing by 1% every minute
        for i in 0..5 {
            tracker.add_price(100.0 * (1.0 + i as f64 * 0.01), now + Duration::minutes(i));
        }
        
        let velocity = tracker.calculate_velocity();
        assert!(velocity > 0.0, "Velocity should be positive for rising prices");
    }
    
    #[test]
    fn test_settlement_prediction() {
        let predictor = SettlementPricePredictor::new(1000);
        let expiry = Utc::now().timestamp_millis() + 10 * 60 * 1000;  // 10 mins from now
        
        let prediction = predictor.predict_settlement(
            Asset::BTC,
            50000.0,
            Timeframe::Min15,
            expiry,
            0.2,  // Positive orderbook pressure
        );
        
        assert!(prediction.confidence > 0.5);
        assert!(prediction.confidence <= 1.0);
    }
}
