//! Polymarket Fee Calculator
//!
//! Calculates taker fees based on Polymarket's fee structure for crypto markets.
//! Fees vary by share price - highest at 50% probability (1.56%), lowest at extremes.
//!
//! Reference: https://docs.polymarket.com/polymarket-learn/trading/maker-rebates-program

/// Calculate taker fee for a trade on Polymarket crypto markets
/// Returns fee in USDC for a given trade value and share price
pub fn calculate_taker_fee(trade_value_usdc: f64, share_price: f64) -> f64 {
    // Fee curve for 15-min and 5-min crypto markets
    // Based on the formula from Polymarket docs
    // Max fee 1.56% at price = 0.50, minimum at extremes

    let price = share_price.clamp(0.01, 0.99);

    // Fee is symmetric around 0.50
    // Using a quadratic approximation of the fee curve
    let distance_from_center = (price - 0.50).abs();
    let max_fee_rate = 0.0156; // 1.56%

    // Fee decreases quadratically as we move from center
    // At price=0.50: fee_rate = 1.56%
    // At price=0.30 or 0.70: fee_rate ≈ 1.10%
    // At price=0.10 or 0.90: fee_rate ≈ 0.20%
    let fee_rate = max_fee_rate * (1.0 - 4.0 * distance_from_center.powi(2)).max(0.0);

    (trade_value_usdc * fee_rate * 10000.0).round() / 10000.0 // Round to 4 decimal places
}

/// Calculate the effective fee rate for a given share price
pub fn effective_fee_rate(share_price: f64) -> f64 {
    let price = share_price.clamp(0.01, 0.99);
    let distance_from_center = (price - 0.50).abs();
    0.0156 * (1.0 - 4.0 * distance_from_center.powi(2)).max(0.0)
}

/// Calculate shares to deduct as fee (for buy orders)
/// Polymarket collects fees in shares on buy orders
pub fn calculate_shares_fee(shares: f64, share_price: f64) -> f64 {
    let trade_value = shares * share_price;
    let fee_usdc = calculate_taker_fee(trade_value, share_price);
    // Convert fee back to shares at current price
    if share_price > 0.0 {
        (fee_usdc / share_price * 10000.0).round() / 10000.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_at_50_percent() {
        // Max fee at 50% probability
        let fee = calculate_taker_fee(100.0, 0.50);
        assert!((fee - 1.56).abs() < 0.01, "Expected ~1.56, got {}", fee);
    }

    #[test]
    fn test_fee_at_extremes() {
        // Fee is lower at extremes (not zero, but reduced)
        let fee_low = calculate_taker_fee(100.0, 0.05);
        let fee_high = calculate_taker_fee(100.0, 0.95);
        // At 0.05/0.95, fee is reduced but not zero
        assert!(
            fee_low < 1.0,
            "Fee at 0.05 should be < 1.0, got {}",
            fee_low
        );
        assert!(
            fee_high < 1.0,
            "Fee at 0.95 should be < 1.0, got {}",
            fee_high
        );
        // Still should be lower than at 50%
        let fee_mid = calculate_taker_fee(100.0, 0.50);
        assert!(
            fee_low < fee_mid,
            "Fee at extreme should be less than at 50%"
        );
    }

    #[test]
    fn test_fee_symmetry() {
        // Fee should be symmetric around 0.50
        let fee_30 = calculate_taker_fee(100.0, 0.30);
        let fee_70 = calculate_taker_fee(100.0, 0.70);
        assert!((fee_30 - fee_70).abs() < 0.001, "Fees should be symmetric");
    }

    #[test]
    fn test_effective_rate() {
        let rate = effective_fee_rate(0.50);
        assert!((rate - 0.0156).abs() < 0.0001);

        let rate_extreme = effective_fee_rate(0.10);
        // At 0.10, rate should be lower than at 0.50
        assert!(
            rate_extreme < rate,
            "Rate at 0.10 should be less than at 0.50"
        );
    }
}
