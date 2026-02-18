#[derive(Debug, Clone, Copy)]
pub struct SlippageQuote {
    pub open_slippage_usdc: f64,
    pub close_slippage_usdc: f64,
    pub total_slippage_usdc: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct ExpectedValueQuote {
    pub p_market: f64,
    pub p_model: f64,
    pub edge_prob: f64,
    pub edge_required: f64,
    pub edge_net: f64,
    pub ev_per_share: f64,
}

pub fn estimate_expected_value(
    p_market: f64,
    p_model: f64,
    share_price: f64,
    fee_rate: f64,
    spread_abs: f64,
    slippage_rate: f64,
) -> ExpectedValueQuote {
    let p_mkt = p_market.clamp(0.01, 0.99);
    let p_mod = p_model.clamp(0.01, 0.99);
    let price = share_price.clamp(0.01, 0.99);

    let edge_prob = p_mod - p_mkt;
    let edge_required = fee_rate + (spread_abs / 2.0) + slippage_rate + 0.005;
    let edge_net = edge_prob - edge_required;

    let gross_win = 1.0 - price;
    let gross_loss = price;
    let ev_per_share =
        (p_mod * gross_win) - ((1.0 - p_mod) * gross_loss) - (fee_rate + slippage_rate);

    ExpectedValueQuote {
        p_market: p_mkt,
        p_model: p_mod,
        edge_prob,
        edge_required,
        edge_net,
        ev_per_share,
    }
}
