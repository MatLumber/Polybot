#[derive(Debug, Clone, Copy)]
pub struct ExecutionPlan {
    pub entry_price: f64,
    pub use_taker: bool,
    pub post_only: bool,
}

pub fn plan_buy_execution(
    best_bid: f64,
    best_ask: f64,
    tick_size: f64,
    maker_first: bool,
    post_only: bool,
    seconds_to_expiry: i64,
    fallback_taker_seconds: i64,
    ev_net: f64,
) -> Option<ExecutionPlan> {
    if best_bid <= 0.0 || best_ask <= 0.0 || best_ask < best_bid {
        return None;
    }

    if maker_first && !(seconds_to_expiry <= fallback_taker_seconds && ev_net > 0.0) {
        let tick = tick_size.max(0.0001);
        let maker_price = if post_only {
            (best_bid + tick).min(best_ask - tick).max(0.01).min(0.99)
        } else {
            best_bid.clamp(0.01, 0.99)
        };
        return Some(ExecutionPlan {
            entry_price: maker_price,
            use_taker: false,
            post_only,
        });
    }

    Some(ExecutionPlan {
        entry_price: best_ask.clamp(0.01, 0.99),
        use_taker: true,
        post_only: false,
    })
}
