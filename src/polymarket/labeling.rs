use super::market_state::ResolvedOutcome;

#[derive(Debug, Clone, Copy)]
pub struct ResolvedLabel {
    pub y_resolve: bool,
    pub y_trade_return: f64,
}

pub fn label_from_resolution(
    predicted_yes: bool,
    resolved: ResolvedOutcome,
    trade_return: f64,
) -> Option<ResolvedLabel> {
    match resolved {
        ResolvedOutcome::Yes => Some(ResolvedLabel {
            y_resolve: predicted_yes,
            y_trade_return: trade_return,
        }),
        ResolvedOutcome::No => Some(ResolvedLabel {
            y_resolve: !predicted_yes,
            y_trade_return: trade_return,
        }),
        ResolvedOutcome::Invalid => None,
    }
}
