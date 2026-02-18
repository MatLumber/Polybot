#[derive(Debug, Clone, Copy)]
pub struct FeeQuote {
    pub open_fee_usdc: f64,
    pub close_fee_usdc: f64,
    pub total_fee_usdc: f64,
    pub effective_rate_open: f64,
    pub effective_rate_close: f64,
}

pub fn fee_rate_from_price(share_price: f64) -> f64 {
    let p = share_price.clamp(0.01, 0.99);
    (p * (1.0 - p) * 0.0624).max(0.0)
}

pub fn estimate_roundtrip_fees(
    notional_open_usdc: f64,
    entry_share_price: f64,
    notional_close_usdc: f64,
    exit_share_price: f64,
) -> FeeQuote {
    let r_open = fee_rate_from_price(entry_share_price);
    let r_close = fee_rate_from_price(exit_share_price);
    let open_fee_usdc = notional_open_usdc * r_open;
    let close_fee_usdc = notional_close_usdc * r_close;
    FeeQuote {
        open_fee_usdc,
        close_fee_usdc,
        total_fee_usdc: open_fee_usdc + close_fee_usdc,
        effective_rate_open: r_open,
        effective_rate_close: r_close,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fee_rate_higher_near_center() {
        assert!(fee_rate_from_price(0.5) > fee_rate_from_price(0.1));
        assert!(fee_rate_from_price(0.5) > fee_rate_from_price(0.9));
    }
}
