use std::collections::HashMap;
use std::sync::RwLock;

use crate::types::{Asset, Timeframe};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedOutcome {
    Yes,
    No,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionLifecycleState {
    Open,
    Closing,
    Closed,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPolicy {
    MakerFirst,
    TakerOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome {
    Yes,
    No,
}

impl Outcome {
    pub fn from_text(raw: &str) -> Option<Self> {
        let text = raw.trim().to_ascii_lowercase();
        if text.contains("yes") || text.contains("up") {
            return Some(Self::Yes);
        }
        if text.contains("no") || text.contains("down") {
            return Some(Self::No);
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MarketWindowKey {
    pub market_id: String,
    pub event_id: String,
    pub asset: Asset,
    pub timeframe: Timeframe,
    pub window_start: i64,
    pub window_end: i64,
}

#[derive(Debug, Clone)]
pub struct OutcomeToken {
    pub token_id: String,
    pub outcome: Outcome,
    pub last: f64,
    pub midpoint: f64,
    pub bid: f64,
    pub ask: f64,
    pub spread: f64,
    pub depth: f64,
}

#[derive(Debug, Clone)]
pub struct TokenRoute {
    pub asset: Asset,
    pub timeframe: Timeframe,
    pub outcome: Outcome,
}

#[derive(Debug, Clone)]
pub struct OrderIntent {
    pub market_key: MarketWindowKey,
    pub token_id: String,
    pub outcome: Outcome,
    pub size_usdc: f64,
    pub policy: ExecutionPolicy,
}

#[derive(Default)]
pub struct MarketStateStore {
    token_routes: RwLock<HashMap<String, TokenRoute>>,
    quotes: RwLock<HashMap<(Asset, Timeframe, Outcome), OutcomeToken>>,
    window_bias: RwLock<HashMap<(Asset, Timeframe, i64), Outcome>>,
}

impl MarketStateStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert_route(&self, token_id: impl Into<String>, route: TokenRoute) {
        if let Ok(mut routes) = self.token_routes.write() {
            routes.insert(token_id.into(), route);
        }
    }

    pub fn get_route(&self, token_id: &str) -> Option<TokenRoute> {
        self.token_routes.read().ok()?.get(token_id).cloned()
    }

    pub fn update_quote(
        &self,
        token_id: &str,
        bid: f64,
        ask: f64,
        last: Option<f64>,
        depth: f64,
    ) -> Option<OutcomeToken> {
        let route = self.get_route(token_id)?;
        let midpoint = if bid > 0.0 && ask > 0.0 {
            (bid + ask) / 2.0
        } else {
            last.unwrap_or(0.0)
        };
        let spread = (ask - bid).max(0.0);

        let quote = OutcomeToken {
            token_id: token_id.to_string(),
            outcome: route.outcome,
            last: last.unwrap_or(midpoint),
            midpoint,
            bid,
            ask,
            spread,
            depth,
        };

        if let Ok(mut quotes) = self.quotes.write() {
            quotes.insert((route.asset, route.timeframe, route.outcome), quote.clone());
        }
        Some(quote)
    }

    pub fn quote_for(
        &self,
        asset: Asset,
        timeframe: Timeframe,
        outcome: Outcome,
    ) -> Option<OutcomeToken> {
        self.quotes
            .read()
            .ok()?
            .get(&(asset, timeframe, outcome))
            .cloned()
    }

    pub fn set_window_bias(
        &self,
        asset: Asset,
        timeframe: Timeframe,
        window_start: i64,
        outcome: Outcome,
    ) {
        if let Ok(mut bias) = self.window_bias.write() {
            bias.insert((asset, timeframe, window_start), outcome);
        }
    }

    pub fn get_window_bias(
        &self,
        asset: Asset,
        timeframe: Timeframe,
        window_start: i64,
    ) -> Option<Outcome> {
        self.window_bias
            .read()
            .ok()?
            .get(&(asset, timeframe, window_start))
            .copied()
    }

    pub fn clear_old_bias_before(&self, cutoff_window_start: i64) {
        if let Ok(mut bias) = self.window_bias.write() {
            bias.retain(|(_, _, start), _| *start >= cutoff_window_start);
        }
    }
}
